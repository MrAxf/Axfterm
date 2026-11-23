//! Spike S5 — teclado e IME.
//!
//! Ventana que registra cada `KeyDownEvent` y el texto que llega por IME
//! (`EntityInputHandler`). Sirve para validar, con un teclado real:
//!
//! - `Ctrl+C`, flechas, `Esc`, `Tab` y `Backspace` (los bytes que irán al PTY).
//! - AltGr en layout ES (`@`, `#`, `|`) sin disparar un binding.
//! - Dead keys (`´` + vocal) e IME preedit/commit (ES y CJK).
//! - Portapapeles: `Ctrl+Shift+V` normaliza el pegado con `axf_terminal::paste`.
//!
//! ```powershell
//! cargo run -p axfterm --example keylog
//! ```
//!
//! No hay verificación automática posible: el entregable es la tabla que imprime
//! y la observación manual. Anotar el resultado en `progress.md`.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "spike de teclado: su salida es el entregable"
)]

use std::ops::Range;

use axf_terminal::paste::normalize_paste;
use axf_ui::gpui::component::ActiveTheme;
use axf_ui::gpui::component::theme::{Theme, ThemeMode};
use axf_ui::gpui::{
    App, AppContext, Bounds, ClipboardItem, Context, Element, ElementId, ElementInputHandler,
    EntityInputHandler, FocusHandle, GlobalElementId, InspectorElementId, InteractiveElement,
    IntoElement, KeyDownEvent, LayoutId, ParentElement, Pixels, Point, Render, Size, Style, Styled,
    UTF16Selection, Window, div, px, rems, size,
};

const CONTEXT: &str = "Terminal";

struct KeyLog {
    focus: FocusHandle,
    log: Vec<String>,
    /// Texto "confirmado" que entregaría el IME.
    text: String,
    /// Preedit actual del IME.
    marked: Option<String>,
}

impl KeyLog {
    fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            focus,
            log: Vec::new(),
            text: String::new(),
            marked: None,
        }
    }

    fn push(&mut self, line: String, cx: &mut Context<'_, Self>) {
        println!("{line}");
        self.log.push(line);
        if self.log.len() > 40 {
            self.log.remove(0);
        }
        cx.notify();
    }

    fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let keystroke = &event.keystroke;
        let predicted = predicted_bytes(keystroke, event.prefer_character_input);
        self.push(
            format!(
                "key={:<10} char={:<8} mods=[{}] held={} prefer_char={} -> {:<14} {}",
                keystroke.key,
                keystroke.key_char.as_deref().unwrap_or("-"),
                modifiers(keystroke),
                event.is_held,
                event.prefer_character_input,
                escape_bytes(&predicted),
                if self.marked.is_some() { "[ime]" } else { "" }
            ),
            cx,
        );

        // Clipboard (S6): atajo explícito para no robar Ctrl+C/V a la terminal.
        if keystroke.modifiers.control && keystroke.modifiers.shift {
            match keystroke.key.as_str() {
                "v" => {
                    let Some(item) = cx.read_from_clipboard() else {
                        self.push("clipboard: vacío".into(), cx);
                        return;
                    };
                    let Some(text) = item.text() else {
                        self.push("clipboard: sin texto".into(), cx);
                        return;
                    };
                    let bytes = normalize_paste(&text, false);
                    self.push(
                        format!(
                            "paste: {} bytes desde {} chars -> {}",
                            bytes.len(),
                            text.chars().count(),
                            escape_bytes(&bytes)
                        ),
                        cx,
                    );
                }
                "c" => {
                    cx.write_to_clipboard(ClipboardItem::new_string(self.text.clone()));
                    self.push(
                        format!("copy: {} chars al portapapeles", self.text.chars().count()),
                        cx,
                    );
                }
                _ => {}
            }
        }
    }
}

fn modifiers(keystroke: &axf_ui::gpui::Keystroke) -> String {
    let mut parts = Vec::new();
    if keystroke.modifiers.control {
        parts.push("ctrl");
    }
    if keystroke.modifiers.alt {
        parts.push("alt");
    }
    if keystroke.modifiers.shift {
        parts.push("shift");
    }
    if keystroke.modifiers.platform {
        parts.push("platform");
    }
    parts.join("+")
}

/// Bytes que M1 enviará al PTY para esta tecla. Es la tabla que valida el spike.
fn predicted_bytes(keystroke: &axf_ui::gpui::Keystroke, prefer_character_input: bool) -> Vec<u8> {
    // AltGr llega como ctrl+alt en Windows; si el layout produjo un carácter,
    // ese carácter manda.
    if prefer_character_input && let Some(text) = &keystroke.key_char {
        return text.as_bytes().to_vec();
    }

    if keystroke.modifiers.control
        && keystroke.modifiers.alt
        && let Some(text) = &keystroke.key_char
    {
        return text.as_bytes().to_vec();
    }

    if keystroke.modifiers.control
        && !keystroke.modifiers.alt
        && keystroke.key.len() == 1
        && let Some(byte) = keystroke.key.bytes().next()
    {
        return vec![byte.to_ascii_uppercase() & 0x1f];
    }

    match keystroke.key.as_str() {
        "enter" => vec![b'\r'],
        "tab" => vec![b'\t'],
        "backspace" => vec![0x7f],
        "escape" => vec![0x1b],
        "up" => b"\x1b[A".to_vec(),
        "down" => b"\x1b[B".to_vec(),
        "right" => b"\x1b[C".to_vec(),
        "left" => b"\x1b[D".to_vec(),
        "home" => b"\x1b[H".to_vec(),
        "end" => b"\x1b[F".to_vec(),
        "insert" => b"\x1b[2~".to_vec(),
        "delete" => b"\x1b[3~".to_vec(),
        "pageup" => b"\x1b[5~".to_vec(),
        "pagedown" => b"\x1b[6~".to_vec(),
        _ => keystroke
            .key_char
            .as_ref()
            .map(|text| text.as_bytes().to_vec())
            .unwrap_or_default(),
    }
}

fn escape_bytes(bytes: &[u8]) -> String {
    let mut out = String::from("\"");
    for &byte in bytes {
        match byte {
            0x1b => out.push_str("\\e"),
            b'\r' => out.push_str("\\r"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            other => out.push_str(&format!("\\x{other:02x}")),
        }
    }
    out.push('"');
    out
}

impl EntityInputHandler for KeyLog {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<String> {
        let start = utf16_to_byte(&self.text, range.start);
        let end = utf16_to_byte(&self.text, range.end);
        *adjusted_range = None;
        self.text.get(start..end).map(ToOwned::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<UTF16Selection> {
        let end = self.text.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|text| 0..text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(marked) = self.marked.take() {
            self.push(format!("ime: unmark preedit {marked:?}"), cx);
        }
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.text.push_str(text);
        self.marked = None;
        self.push(format!("ime: commit {text:?}"), cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.marked = Some(new_text.to_owned());
        self.push(format!("ime: preedit {new_text:?}"), cx);
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        Some(0)
    }

    fn paste(&mut self, item: ClipboardItem, _window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(text) = item.text() else {
            return;
        };
        let bytes = normalize_paste(&text, false);
        self.push(
            format!(
                "ime: paste {} bytes -> {}",
                bytes.len(),
                escape_bytes(&bytes)
            ),
            cx,
        );
        self.text.push_str(&text);
    }
}

fn utf16_to_byte(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        if units >= offset {
            return index;
        }
        units += ch.len_utf16();
    }
    text.len()
}

/// Ancla de tamaño cero que registra el `EntityInputHandler` en la fase de paint.
struct ImeAnchor {
    focus: FocusHandle,
    view: axf_ui::gpui::Entity<KeyLog>,
}

impl Element for ImeAnchor {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout = window.request_layout(
            Style {
                size: Size::default(),
                ..Default::default()
            },
            std::iter::empty(),
            cx,
        );
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.handle_input(
            &self.focus,
            ElementInputHandler::new(window.bounds(), self.view.clone()),
            cx,
        );
    }
}

impl IntoElement for ImeAnchor {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Render for KeyLog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let (background, foreground, muted, border) = {
            let theme = cx.theme();
            (
                theme.background,
                theme.foreground,
                theme.muted_foreground,
                theme.border,
            )
        };

        let rows = self
            .log
            .iter()
            .map(|line| {
                div()
                    .text_size(rems(0.8))
                    .child(line.clone())
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(ImeAnchor {
                focus: self.focus.clone(),
                view: cx.entity(),
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_1()
                    .p_4()
                    .children(rows),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(border)
                    .text_color(muted)
                    .text_size(rems(0.75))
                    .child("keylog: Ctrl+Shift+V pega, Ctrl+Shift+C copia el texto IME")
                    .child(format!("ime={:?} preedit={:?}", self.text, self.marked)),
            )
    }
}

fn main() {
    axf_ui::gpui::application()
        .with_assets(axf_ui::Assets)
        .run(|cx| {
            axf_ui::gpui::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            let options = axf_ui::gpui::WindowOptions {
                window_bounds: Some(axf_ui::gpui::WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(900.0), px(600.0)),
                    cx,
                ))),
                titlebar: Some(axf_ui::gpui::TitlebarOptions {
                    title: Some("Axfterm — spike teclado/IME".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let result = axf_ui::gpui::open_window(options, cx, |window, cx| {
                cx.new(|cx| KeyLog::new(window, cx))
            });
            if let Err(error) = result {
                eprintln!("keylog: no se pudo abrir la ventana: {error}");
            }
        });
}
