//! `TerminalView`: entidad GPUI que pinta el grid y traduce la entrada al PTY.
//!
//! El estado de filas vive en un `Rc<RefCell<_>>` compartido con el elemento de
//! render, igual que en el spike S4. El hilo parser es el único que interpreta
//! VT; aquí solo se aplican frames y se shapea lo dañado.

use std::cell::RefCell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use axf_ui::gpui::component::menu::ContextMenuExt as _;
use axf_ui::gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Font, FontFallbacks, FontId, GlobalElementId, Hsla,
    InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, Keystroke, LayoutId,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, Render, ScrollDelta, ScrollEvent, ShapedLine, SharedString, Size, StrikethroughStyle,
    Style, Styled, Subscription, Task, TextAlign, TextRun, TextSystem, UTF16Selection,
    UnderlineStyle, Window, div, fill, font, point, px, size,
};
use axf_ui::theme::TerminalPalette;

use crate::engine::{
    CellColor, CursorShape, CursorSnapshot, HyperlinkSpan, ModeFlags, MouseMode, ParserHandle,
    ProgramStatus, ResizeRequest, ScreenFrame, SelectionPoint, SelectionSpan, SnapshotCell,
    SnapshotRequest, TerminalEvent,
};
use crate::ime::ImeState;
use crate::input;
use crate::mouse::{self, MouseAction, MouseButton as TerminalMouseButton, MouseModifiers};
use crate::paste::normalize_paste;
use crate::session::{ExitInfo, PtyWriter, TerminalSession};
use crate::{GridSize, TerminalError};

// El derive de `actions!` expande a `gpui::Action`; el alias mantiene la
// resolución a través de la fachada de `axf_ui` (ADR 0002).
use axf_ui::gpui;

axf_ui::gpui::actions!(
    axfterm_terminal,
    [CopySelection, PasteSelection, SendTab, SendBacktab]
);

/// Contexto de teclado de la terminal.
pub const TERMINAL_CONTEXT: &str = "Terminal";

/// Tamaño máximo de pegado que se acepta en v0.1 (ADR 0005).
const MAX_PASTE_BYTES: usize = 1024 * 1024;

/// Tokens que la app pasa al terminal.
#[derive(Clone, Debug)]
pub struct TerminalStyle {
    /// Familia monoespaciada.
    pub font_family: SharedString,
    /// Cadena de fallback (Nerd Font, emoji, CJK).
    pub fallbacks: Vec<SharedString>,
    /// Tamaño de fuente.
    pub font_size: Pixels,
    /// Altura de línea explícita; `None` usa la natural del glifo.
    pub line_height: Option<Pixels>,
    /// Paleta ANSI y colores base.
    pub palette: TerminalPalette,
    /// Fondo del área de terminal.
    pub background: Hsla,
    /// Borde del área de terminal; `None` lo oculta.
    pub border: Option<Hsla>,
    /// Padding interior derivado del tamaño de fuente.
    pub padding: Pixels,
}

/// Señal que la vista entrega a la app para la máquina de agentes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalSignal {
    /// Reporte OSC 7501.
    ProgramStatus(ProgramStatus),
    /// Notificación OSC 9/777/1337 o campana.
    Attention(Option<String>),
    /// El proceso terminó.
    Exited(Option<ExitInfo>),
}

/// Interruptores de diagnóstico del render. Todos son opt-in por entorno y no
/// afectan al uso normal.
#[derive(Clone, Copy, Debug, Default)]
struct DebugRender {
    trace_cells: bool,
    no_underline: bool,
    no_bg: bool,
    bg_overlap: bool,
    row_lines: bool,
}

impl DebugRender {
    fn from_env() -> Self {
        Self {
            trace_cells: env_flag("AXFTERM_TRACE_CELLS"),
            no_underline: env_flag("AXFTERM_DEBUG_NO_UNDERLINE"),
            no_bg: env_flag("AXFTERM_DEBUG_NO_BG"),
            bg_overlap: env_flag("AXFTERM_DEBUG_BG_OVERLAP"),
            row_lines: env_flag("AXFTERM_DEBUG_ROW_LINES"),
        }
    }
}

fn env_flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value != "0")
}

/// Comprueba que la fuente tenga el mismo avance para `M` e `i`.
fn is_monospaced(text_system: &TextSystem, font_id: FontId, font_size: Pixels) -> bool {
    let wide = text_system
        .advance(font_id, font_size, 'M')
        .map(|size| size.width);
    let narrow = text_system
        .advance(font_id, font_size, 'i')
        .map(|size| size.width);
    match (wide, narrow) {
        (Ok(wide), Ok(narrow)) => (f32::from(wide) - f32::from(narrow)).abs() < 0.5,
        _ => false,
    }
}

#[derive(Clone)]
struct Metrics {
    font: Font,
    font_size: Pixels,
    cell_width: Pixels,
    line_height: Pixels,
    palette: TerminalPalette,
}

/// Posición del grid en la ventana, para traducir el ratón a celdas.
#[derive(Clone, Copy, Debug)]
struct GridGeometry {
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
}

impl Metrics {
    fn new(window: &mut Window, style: &TerminalStyle) -> Self {
        let text_system = window.text_system();
        let mut font = font(&style.font_family);
        if !style.fallbacks.is_empty() {
            font.fallbacks = Some(FontFallbacks::from_fonts(
                style
                    .fallbacks
                    .iter()
                    .map(|family| family.to_string())
                    .collect(),
            ));
        }
        let mut font_id = text_system.resolve_font(&font);
        let mut cell_width = text_system
            .advance(font_id, style.font_size, 'M')
            .map(|advance| advance.width)
            .unwrap_or_else(|_| style.font_size * 0.6);

        // La terminal exige una familia monoespaciada. Si el tema entrega una
        // proporcional (o una que no resuelve), se cae a una instalada.
        if !is_monospaced(text_system, font_id, style.font_size) {
            let installed = text_system.all_font_names();
            for candidate in ["Cascadia Mono", "Consolas", "Courier New"] {
                if !installed.iter().any(|name| name == candidate) {
                    continue;
                }
                let candidate_font = axf_ui::gpui::font(candidate);
                let candidate_id = text_system.resolve_font(&candidate_font);
                if !is_monospaced(text_system, candidate_id, style.font_size) {
                    continue;
                }
                if let Ok(advance) = text_system.advance(candidate_id, style.font_size, 'M') {
                    font = candidate_font;
                    font_id = candidate_id;
                    cell_width = advance.width;
                    break;
                }
            }
        }

        // La altura natural (ascent + descent) es la que hace que los glifos de
        // bloque y las cajas de una TUI queden contiguos. Multiplicar el tamaño
        // de fuente dejaba una línea de 1 px entre filas.
        let ascent = text_system.ascent(font_id, style.font_size);
        let descent = text_system.descent(font_id, style.font_size);
        // GPUI entrega el descent con signo negativo en Windows. Restarlo da
        // exactamente la altura del glifo de bloque (█), así que las cajas y el
        // logo de una TUI quedan contiguos sin recortar el texto.
        let natural = if descent < px(0.0) {
            ascent - descent
        } else {
            ascent + descent
        };
        let natural = if natural > px(1.0) {
            natural
        } else {
            style.font_size * 1.2
        };
        let line_height = style
            .line_height
            .filter(|height| *height > px(0.0))
            .map(|height| height.max(natural))
            .unwrap_or(natural);

        if let Some(path) = std::env::var_os("AXFTERM_TRACE_FONT")
            && let Ok(mut file) = std::fs::File::create(path)
        {
            use std::io::Write as _;
            let block = text_system
                .typographic_bounds(font_id, style.font_size, '█')
                .map(|bounds| bounds.size);
            let _ = writeln!(
                file,
                "family={:?} size={:?} ascent={:?} descent={:?} line={:?} block={:?} cell={:?}",
                style.font_family, style.font_size, ascent, descent, line_height, block, cell_width
            );
        }

        Self {
            font,
            font_size: style.font_size,
            cell_width,
            line_height,
            palette: style.palette,
        }
    }
}

struct RenderState {
    cols: usize,
    rows: usize,
    cells: Vec<Vec<SnapshotCell>>,
    combining: Vec<Vec<(usize, char)>>,
    links: Vec<Vec<HyperlinkSpan>>,
    cache: Vec<Option<ShapedLine>>,
    cursor: Option<CursorSnapshot>,
    mode: ModeFlags,
    display_offset: usize,
    history_size: usize,
    selection: Option<SelectionSpan>,
    ended: bool,
    exited: Option<ExitInfo>,
    bell_flash: bool,
    last_output: Option<Instant>,
    scratch_text: String,
    scratch_runs: Vec<TextRun>,
}

impl RenderState {
    fn new() -> Self {
        let cols = 80;
        let rows = 24;
        Self {
            cols,
            rows,
            cells: vec![vec![SnapshotCell::default(); cols]; rows],
            combining: vec![Vec::new(); rows],
            links: vec![Vec::new(); rows],
            cache: vec![None; rows],
            cursor: None,
            mode: ModeFlags::default(),
            display_offset: 0,
            history_size: 0,
            selection: None,
            ended: false,
            exited: None,
            bell_flash: false,
            last_output: None,
            scratch_text: String::new(),
            scratch_runs: Vec::new(),
        }
    }

    fn invalidate_caches(&mut self) {
        self.cache = vec![None; self.rows];
        self.scratch_text.clear();
        self.scratch_runs.clear();
    }

    fn apply(&mut self, frame: ScreenFrame) {
        let resized = frame.cols != self.cols || frame.rows != self.rows;
        let scrolled = frame.display_offset != self.display_offset;
        if frame.full || resized || scrolled {
            self.cols = frame.cols.max(1);
            self.rows = frame.rows.max(1);
            self.cells = vec![vec![SnapshotCell::default(); self.cols]; self.rows];
            self.combining = vec![Vec::new(); self.rows];
            self.links = vec![Vec::new(); self.rows];
            self.cache = vec![None; self.rows];
            self.scratch_text.clear();
            self.scratch_runs.clear();
        }

        for update in frame.updates {
            if update.line >= self.rows {
                continue;
            }
            let mut cells = update.cells;
            cells.resize(self.cols, SnapshotCell::default());
            cells.truncate(self.cols);
            self.cells[update.line] = cells;
            self.combining[update.line] = update.combining;
            self.links[update.line] = update.links;
            self.cache[update.line] = None;
        }

        self.cursor = frame.cursor;
        self.mode = frame.mode;
        self.display_offset = frame.display_offset;
        self.history_size = frame.history_size;
        self.selection = frame.selection;
    }
}

/// Vista de una terminal viva.
pub struct TerminalView {
    focus: FocusHandle,
    session: Option<TerminalSession>,
    parser: Option<ParserHandle>,
    state: Rc<RefCell<RenderState>>,
    metrics: Metrics,
    style: TerminalStyle,
    geometry: Option<GridGeometry>,
    requested_grid: Option<(usize, usize)>,
    error: Option<String>,
    notice: Option<String>,
    pending_url: Option<String>,
    title: Option<String>,
    cwd: Option<PathBuf>,
    pid: Option<u32>,
    signals: Vec<TerminalSignal>,
    debug: DebugRender,
    ime: ImeState,
    selection_anchor: Option<SelectionPoint>,
    selection_dragging: bool,
    selection_moved: bool,
    _focus_in: Option<Subscription>,
    _focus_out: Option<Subscription>,
    _pump: Option<Task<()>>,
}

impl TerminalView {
    /// Arranca el PTY y el parser. Si falla, la vista muestra el error en vez
    /// de tumbar la ventana.
    pub fn new(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        style: TerminalStyle,
        cwd: PathBuf,
        spec: &crate::SpawnSpec,
        restore: Option<crate::TerminalSnapshot>,
        focus: bool,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        if focus {
            window.focus(&focus_handle, cx);
        }
        let metrics = Metrics::new(window, &style);
        let state = Rc::new(RefCell::new(RenderState::new()));
        let debug = DebugRender::from_env();

        match Self::start(&metrics, &state, cwd, spec, restore, cx) {
            Ok((session, parser, pump)) => {
                let writer = session.writer();
                let pid = session.pid();
                let (focus_in, focus_out) =
                    register_focus_listeners(window, cx, &focus_handle, &state, writer);
                Self {
                    focus: focus_handle,
                    session: Some(session),
                    parser: Some(parser),
                    state,
                    metrics,
                    style,
                    geometry: None,
                    requested_grid: None,
                    error: None,
                    notice: None,
                    pending_url: None,
                    title: None,
                    cwd: None,
                    pid,
                    signals: Vec::new(),
                    debug,
                    ime: ImeState::default(),
                    selection_anchor: None,
                    selection_dragging: false,
                    selection_moved: false,
                    _focus_in: focus_in,
                    _focus_out: focus_out,
                    _pump: Some(pump),
                }
            }
            Err(error) => Self {
                focus: focus_handle,
                session: None,
                parser: None,
                state,
                metrics,
                style,
                geometry: None,
                requested_grid: None,
                error: Some(error.to_string()),
                notice: None,
                pending_url: None,
                title: None,
                cwd: None,
                pid: None,
                signals: Vec::new(),
                debug,
                ime: ImeState::default(),
                selection_anchor: None,
                selection_dragging: false,
                selection_moved: false,
                _focus_in: None,
                _focus_out: None,
                _pump: None,
            },
        }
    }

    /// Título anunciado por la shell (`OSC 0/2`), si lo hay.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Error de arranque o del parser, para que la app ofrezca acciones.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Programa que arrancó el PTY (`pwsh`, `cmd`, etc.), si hay sesión.
    pub fn program(&self) -> Option<&str> {
        self.session.as_ref().map(TerminalSession::program)
    }

    /// Aplica un tema, fuente o escala nuevos sin recrear el PTY.
    pub fn update_style(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        style: TerminalStyle,
    ) {
        let same = self.style.font_family == style.font_family
            && self.style.font_size == style.font_size
            && self.style.fallbacks == style.fallbacks
            && self.style.line_height == style.line_height
            && self.style.palette == style.palette
            && self.style.background == style.background
            && self.style.border == style.border
            && self.style.padding == style.padding;
        if same {
            return;
        }
        self.metrics = Metrics::new(window, &style);
        self.style = style;
        self.state.borrow_mut().invalidate_caches();
        self.geometry = None;
        self.requested_grid = None;
        cx.notify();
    }

    /// Último cwd anunciado por la shell (`OSC 7`).
    pub fn cwd(&self) -> Option<&PathBuf> {
        self.cwd.as_ref()
    }

    /// `true` mientras el proceso del PTY sigue vivo.
    pub fn is_running(&self) -> bool {
        self.session.is_some() && !self.state.borrow().ended
    }

    /// PID raíz del PTY, si el sistema lo expuso.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Instante del último bloque de salida recibido.
    pub fn last_output_at(&self) -> Option<Instant> {
        self.state.borrow().last_output
    }

    /// Estado de salida, si el proceso ya terminó.
    pub fn exit_info(&self) -> Option<ExitInfo> {
        self.state.borrow().exited
    }

    /// Retira y devuelve las señales pendientes para los agentes.
    pub fn take_signals(&mut self) -> Vec<TerminalSignal> {
        std::mem::take(&mut self.signals)
    }

    /// Código de salida, si el proceso ya terminó.
    pub fn exit_code(&self) -> Option<u32> {
        self.state.borrow().exited.and_then(|info| info.code)
    }

    /// Envía una línea de comando al PTY terminado en retorno de carro.
    pub fn send_command(&self, command: &str) {
        let command = command.trim();
        if command.is_empty() || !self.is_running() {
            return;
        }
        let mut bytes = Vec::with_capacity(command.len() + 1);
        bytes.extend_from_slice(command.as_bytes());
        bytes.push(b'\r');
        self.write(&bytes);
    }

    /// Lleva el foco de teclado a la terminal.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.focus.focus(window, cx);
    }

    /// Pide una instantánea del buffer para guardarla al cerrar.
    pub fn request_snapshot(&self) -> Option<SnapshotRequest> {
        self.parser.as_ref().and_then(ParserHandle::snapshot)
    }

    fn start(
        metrics: &Metrics,
        state: &Rc<RefCell<RenderState>>,
        cwd: PathBuf,
        spec: &crate::SpawnSpec,
        restore: Option<crate::TerminalSnapshot>,
        cx: &mut Context<'_, Self>,
    ) -> Result<(TerminalSession, ParserHandle, Task<()>), TerminalError> {
        let (commands, inbox) = async_channel::bounded(64);
        let initial = GridSize::new(80, 24);
        let cell_width = pixels_to_u16(metrics.cell_width);
        let cell_height = pixels_to_u16(metrics.line_height);

        let session = TerminalSession::spawn(
            &cwd,
            crate::PtySizeSpec::new(80, 24, cell_width, cell_height),
            commands.clone(),
            spec,
        )?;
        let writer = session
            .writer()
            .ok_or_else(|| TerminalError::Pty("el PTY no expuso un escritor".into()))?;
        let (frames, frame_inbox) = async_channel::bounded(2);
        let parser = crate::engine::spawn_parser(
            commands,
            inbox,
            frames,
            crate::engine::ParserSetup {
                writer,
                palette: metrics.palette,
                initial,
                cell_width,
                cell_height,
                restore,
            },
        )?;

        let state = Rc::clone(state);
        let pump = cx.spawn(async move |this, cx| {
            while let Ok(mut frame) = frame_inbox.recv().await {
                let events = std::mem::take(&mut frame.events);
                let bytes = frame.bytes;
                {
                    let mut state = state.borrow_mut();
                    if bytes > 0 {
                        state.last_output = Some(Instant::now());
                    }
                    state.apply(frame);
                }
                let updated = this.update_in(cx, |view, window, cx| {
                    view.handle_events(events, window, cx);
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        });

        Ok((session, parser, pump))
    }

    fn write(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(writer) = self.session.as_ref().and_then(TerminalSession::writer) {
            let _ = writer.write(bytes);
        }
    }

    fn scroll_to_bottom(&self) {
        if let Some(parser) = &self.parser {
            parser.scroll_to_bottom();
        }
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_dragging = false;
        self.selection_moved = false;
        if self.state.borrow().selection.is_some()
            && let Some(parser) = &self.parser
        {
            parser.clear_selection();
        }
    }

    fn handle_events(
        &mut self,
        events: Vec<TerminalEvent>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        for event in events {
            match event {
                TerminalEvent::Title(title) => {
                    // El título es propiedad de la pestaña; la app decide el
                    // título de la ventana a partir de la pestaña enfocada.
                    self.title = title.filter(|title| !title.is_empty());
                }
                TerminalEvent::Cwd(path) => self.cwd = Some(path),
                TerminalEvent::Bell => {
                    self.signals.push(TerminalSignal::Attention(None));
                    self.state.borrow_mut().bell_flash = true;
                    self.schedule_bell_clear(cx);
                }
                TerminalEvent::Attention(message) => {
                    self.signals.push(TerminalSignal::Attention(message));
                }
                TerminalEvent::ProgramStatus(status) => {
                    self.signals.push(TerminalSignal::ProgramStatus(status));
                }
                TerminalEvent::MouseCursor(shape) => {
                    window.set_window_cursor_style(cursor_style_from_osc(&shape));
                }
                TerminalEvent::Clipboard(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
                TerminalEvent::Exited(info) => {
                    self.signals.push(TerminalSignal::Exited(info));
                    let mut state = self.state.borrow_mut();
                    state.ended = true;
                    // EOF puede llegar después del código real; no borrarlo.
                    if info.is_some() || state.exited.is_none() {
                        state.exited = info;
                    }
                }
                TerminalEvent::Error(message) => self.error = Some(message),
            }
        }
    }

    fn schedule_bell_clear(&self, cx: &mut Context<'_, Self>) {
        let timer = cx.background_executor().timer(Duration::from_millis(140));
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |view, cx| {
                view.state.borrow_mut().bell_flash = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if event.keystroke.modifiers.platform {
            return;
        }

        // Diagnóstico: F12 vuelca el grid cuando `AXFTERM_TRACE_CELLS` está
        // activo. No se reenvía al PTY para no alterar la TUI observada.
        if self.debug.trace_cells
            && event.keystroke.key == "f12"
            && !event.keystroke.modifiers.modified()
        {
            if let Some(parser) = &self.parser {
                parser.dump();
            }
            return;
        }

        // Fallback si el host no registró los keybindings de copiar/pegar.
        // En la app, la acción se resuelve antes y este bloque no se ejecuta.
        if event.keystroke.modifiers.control && event.keystroke.modifiers.shift {
            match event.keystroke.key.as_str() {
                "v" => {
                    self.paste_from_clipboard(cx);
                    return;
                }
                "c" => {
                    if let Some(parser) = &self.parser {
                        parser.copy_selection();
                    }
                    return;
                }
                _ => {}
            }
        }

        // Los caracteres imprimibles llegan por el manejador de texto (IME):
        // si además los escribiéramos aquí, cada tecla se duplicaría. Las
        // teclas de control y las secuencias van por `input::encode`.
        if is_text_input(&event.keystroke) {
            return;
        }

        let mode = self.state.borrow().mode;
        let bytes = input::encode(&event.keystroke, event.prefer_character_input, mode);
        if bytes.is_empty() {
            return;
        }
        cx.stop_propagation();
        self.scroll_to_bottom();
        self.clear_selection();
        self.write(&bytes);
        cx.notify();
    }

    fn copy_action(&mut self, _: &CopySelection, _: &mut Window, _: &mut Context<'_, Self>) {
        if let Some(parser) = &self.parser {
            parser.copy_selection();
        }
    }

    fn paste_action(&mut self, _: &PasteSelection, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.paste_from_clipboard(cx);
    }

    fn tab_action(&mut self, _: &SendTab, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.scroll_to_bottom();
        self.clear_selection();
        self.write(b"\t");
        cx.notify();
    }

    fn backtab_action(&mut self, _: &SendBacktab, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.scroll_to_bottom();
        self.clear_selection();
        self.write(b"\x1b[Z");
        cx.notify();
    }

    fn paste_from_clipboard(&mut self, cx: &mut Context<'_, Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let Some(text) = item.text() else {
            return;
        };
        self.paste_text(&text, cx);
    }

    fn paste_text(&mut self, text: &str, cx: &mut Context<'_, Self>) {
        if text.len() > MAX_PASTE_BYTES {
            self.notice = Some(format!(
                "Pegado rechazado: {} bytes (máximo 1 MiB). Usa un archivo.",
                text.len()
            ));
            cx.notify();
            return;
        }
        let bracketed = self.state.borrow().mode.bracketed_paste;
        let bytes = normalize_paste(text, bracketed);
        self.scroll_to_bottom();
        self.clear_selection();
        self.write(&bytes);
        cx.notify();
    }

    /// Traduce una posición de ventana a una celda 0-based del grid.
    fn mouse_cell(&self, position: Point<Pixels>) -> Option<(usize, usize)> {
        let geometry = self.geometry?;
        let state = self.state.borrow();
        if state.cols == 0 || state.rows == 0 {
            return None;
        }

        let x = f32::from(position.x - geometry.origin.x) / f32::from(geometry.cell_width);
        let y = f32::from(position.y - geometry.origin.y) / f32::from(geometry.line_height);
        if x < 0.0 || y < 0.0 {
            return None;
        }

        let column = (x.floor() as usize).min(state.cols - 1);
        let row = (y.floor() as usize).min(state.rows - 1);
        Some((column, row))
    }

    fn write_mouse(&self, action: MouseAction, position: Point<Pixels>, modifiers: Modifiers) {
        let mode = self.state.borrow().mode;
        if mode.mouse_mode == MouseMode::Off {
            return;
        }
        let Some((column, row)) = self.mouse_cell(position) else {
            return;
        };
        let bytes = mouse::encode(
            action,
            column + 1,
            row + 1,
            MouseModifiers {
                shift: modifiers.shift,
                alt: modifiers.alt,
                control: modifiers.control,
            },
            mode.sgr_mouse,
            mode.utf8_mouse,
        );
        self.write(&bytes);
    }

    fn set_selection(&self, start: SelectionPoint, end: SelectionPoint, block: bool) {
        if let Some(parser) = &self.parser {
            parser.select(start, end, block);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if event.button == MouseButton::Left {
            window.focus(&self.focus, cx);
        }

        let mode = self.state.borrow().mode;
        if event.button == MouseButton::Left && mode.mouse_mode == MouseMode::Off {
            if event.modifiers.control
                && let Some((col, row)) = self.mouse_cell(event.position)
                && let Some(url) = self.url_at(row, col)
            {
                self.pending_url = Some(url);
                cx.notify();
                return;
            }

            if let Some((col, row)) = self.mouse_cell(event.position) {
                let point = SelectionPoint { row, col };
                if event.modifiers.shift {
                    let anchor = self.selection_anchor.unwrap_or(point);
                    self.selection_anchor = Some(anchor);
                    self.selection_dragging = false;
                    self.selection_moved = true;
                    self.set_selection(anchor, point, event.modifiers.alt);
                } else {
                    self.clear_selection();
                    self.selection_anchor = Some(point);
                    self.selection_dragging = true;
                    self.selection_moved = false;
                }
                cx.notify();
            }
            return;
        }

        if let Some(button) = terminal_mouse_button(event.button) {
            self.write_mouse(MouseAction::Press(button), event.position, event.modifiers);
        }
    }

    fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) {
        let mode = self.state.borrow().mode;
        if event.button == MouseButton::Left && mode.mouse_mode == MouseMode::Off {
            if self.selection_dragging {
                self.selection_dragging = false;
                if self.selection_moved {
                    if let Some((col, row)) = self.mouse_cell(event.position) {
                        let end = SelectionPoint { row, col };
                        let start = self.selection_anchor.unwrap_or(end);
                        self.set_selection(start, end, event.modifiers.alt);
                    }
                } else {
                    // Un clic sin arrastre limpia la selección y no deja una
                    // celda marcada.
                    self.selection_anchor = None;
                    self.selection_moved = false;
                }
            }
            return;
        }

        if let Some(button) = terminal_mouse_button(event.button) {
            self.write_mouse(
                MouseAction::Release(button),
                event.position,
                event.modifiers,
            );
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) {
        let mode = self.state.borrow().mode;
        if mode.mouse_mode != MouseMode::Off {
            let should_send = mode.mouse_mode == MouseMode::Motion
                || (mode.mouse_mode == MouseMode::Drag && event.pressed_button.is_some());
            if should_send {
                let button = event.pressed_button.and_then(terminal_mouse_button);
                self.write_mouse(MouseAction::Motion(button), event.position, event.modifiers);
            }
            return;
        }

        if self.selection_dragging
            && event.pressed_button == Some(MouseButton::Left)
            && let Some((col, row)) = self.mouse_cell(event.position)
        {
            let end = SelectionPoint { row, col };
            let start = self.selection_anchor.unwrap_or(end);
            self.selection_moved = true;
            self.set_selection(start, end, event.modifiers.alt);
        }
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollEvent,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) {
        let mode = self.state.borrow().mode;
        let (delta_x, delta_y) = match event.delta {
            ScrollDelta::Pixels(delta) => (f32::from(delta.x), f32::from(delta.y)),
            ScrollDelta::Lines(delta) => (delta.x, delta.y),
        };
        if delta_x.abs() < f32::EPSILON && delta_y.abs() < f32::EPSILON {
            return;
        }

        let steps = match event.delta {
            ScrollDelta::Pixels(_) => (delta_y.abs() / f32::from(self.metrics.line_height))
                .ceil()
                .max(1.0) as usize,
            ScrollDelta::Lines(_) => delta_y.abs().ceil().max(1.0) as usize,
        }
        .min(10);

        // Shift+rueda es scroll local aunque la aplicación tenga tracking.
        if mode.mouse_mode != MouseMode::Off && !event.modifiers.shift {
            let action = wheel_action(delta_x, delta_y);
            for _ in 0..steps {
                self.write_mouse(action, event.position, event.modifiers);
            }
            return;
        }

        // Sin tracking, la rueda horizontal no tiene destino local.
        if delta_y.abs() < f32::EPSILON {
            return;
        }

        if mode.alternate_scroll && mode.alt_screen {
            let sequence: &[u8] = if delta_y < 0.0 {
                if mode.app_cursor {
                    b"\x1bOB"
                } else {
                    b"\x1b[B"
                }
            } else if mode.app_cursor {
                b"\x1bOA"
            } else {
                b"\x1b[A"
            };
            for _ in 0..steps {
                self.write(sequence);
            }
            return;
        }

        let delta = if delta_y > 0.0 {
            steps as i32
        } else {
            -(steps as i32)
        };
        if let Some(parser) = &self.parser {
            let _ = parser.scroll(delta);
        }
    }

    /// Aplica el tamaño que ofrece el elemento de render. Solo el parser y el
    /// PTY cambian de tamaño; el estado sigue pintando el frame anterior hasta
    /// que llega el damage completo.
    fn observe_bounds(&mut self, origin: Point<Pixels>, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        self.geometry = Some(GridGeometry {
            origin,
            cell_width: self.metrics.cell_width,
            line_height: self.metrics.line_height,
        });
        if self.requested_grid == Some((cols, rows)) {
            return;
        }
        let Some(parser) = &self.parser else {
            return;
        };

        let request = ResizeRequest {
            cols,
            rows,
            cell_width: pixels_to_u16(self.metrics.cell_width),
            cell_height: pixels_to_u16(self.metrics.line_height),
        };
        if !parser.resize(request) {
            // Cola llena: el siguiente frame volverá a intentarlo.
            return;
        }
        self.requested_grid = Some((cols, rows));

        if let Some(session) = &self.session {
            session.resize(crate::PtySizeSpec::new(
                cols,
                rows,
                request.cell_width,
                request.cell_height,
            ));
        }
    }

    fn link_at(&self, row: usize, column: usize) -> Option<String> {
        let state = self.state.borrow();
        state
            .links
            .get(row)?
            .iter()
            .find(|link| column >= link.start && column < link.end)
            .map(|link| link.uri.clone())
    }

    fn url_at(&self, row: usize, column: usize) -> Option<String> {
        if let Some(uri) = self.link_at(row, column)
            && is_web_url(&uri)
        {
            return Some(uri);
        }

        let state = self.state.borrow();
        let cells = state.cells.get(row)?;
        let mut text = String::with_capacity(cells.len());
        let mut columns = Vec::with_capacity(cells.len());
        for (index, cell) in cells.iter().enumerate() {
            if cell.attrs.wide_spacer {
                continue;
            }
            let ch = if cell.attrs.hidden { ' ' } else { cell.ch };
            text.push(ch);
            columns.push(index);
            for (combining_column, combining) in &state.combining[row] {
                if *combining_column == index {
                    text.push(*combining);
                    columns.push(index);
                }
            }
        }

        for candidate in find_urls(&text) {
            let start = columns.get(candidate.start)?;
            let end = columns.get(candidate.end.saturating_sub(1))?;
            if column >= *start && column <= *end {
                return Some(candidate.text);
            }
        }
        None
    }

    fn confirm_url(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<'_, Self>) {
        cx.stop_propagation();
        if let Some(url) = self.pending_url.take() {
            cx.open_url(&url);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn cancel_url(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<'_, Self>) {
        cx.stop_propagation();
        self.pending_url = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn dismiss_notice(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        cx.stop_propagation();
        self.notice = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }
}

impl std::fmt::Debug for TerminalView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalView")
            .field("error", &self.error)
            .field("requested_grid", &self.requested_grid)
            .field("cwd", &self.cwd)
            .finish_non_exhaustive()
    }
}

impl Drop for TerminalView {
    fn drop(&mut self) {
        if let Some(mut parser) = self.parser.take() {
            parser.close();
        }
        if let Some(mut session) = self.session.take() {
            session.close();
        }
    }
}

fn register_focus_listeners(
    window: &mut Window,
    cx: &mut Context<'_, TerminalView>,
    focus: &FocusHandle,
    state: &Rc<RefCell<RenderState>>,
    writer: Option<PtyWriter>,
) -> (Option<Subscription>, Option<Subscription>) {
    let Some(writer) = writer else {
        return (None, None);
    };

    let focus_in_state = Rc::clone(state);
    let focus_in_writer = writer.clone();
    let focus_in = window.on_focus_in(focus, cx, move |_, _| {
        if focus_in_state.borrow().mode.focus_in_out {
            let _ = focus_in_writer.write(b"\x1b[I");
        }
    });

    let focus_out_state = Rc::clone(state);
    let focus_out_writer = writer;
    let focus_out = window.on_focus_out(focus, cx, move |_, _, _| {
        if focus_out_state.borrow().mode.focus_in_out {
            let _ = focus_out_writer.write(b"\x1b[O");
        }
    });

    (Some(focus_in), Some(focus_out))
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = self.metrics.palette;
        let background = self.style.background;
        let border = self.style.border;
        let padding = self.style.padding;
        let has_session = self.session.is_some();
        let error = self.error.clone();
        let notice = self.notice.clone();
        let pending_url = self.pending_url.clone();
        let view = cx.entity();
        let preedit = self.ime.preedit().to_owned();
        let (ended, exit_info, bell_flash) = {
            let state = self.state.borrow();
            (state.ended, state.exited, state.bell_flash)
        };

        let body = match error {
            Some(error) => div()
                .flex_1()
                .p_4()
                .text_color(palette.foreground)
                .child(error)
                .into_any_element(),
            None => div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(TerminalElement {
                    view: view.clone(),
                    state: Rc::clone(&self.state),
                    metrics: self.metrics.clone(),
                    debug: self.debug,
                    preedit,
                })
                .into_any_element(),
        };

        let border_color = if bell_flash {
            palette.cursor
        } else {
            border.unwrap_or(background)
        };

        let exit_overlay = ended.then(|| {
            let accent = match exit_info {
                Some(info) if info.success => palette.ansi(2),
                Some(_) => palette.ansi(1),
                None => palette.ansi(8),
            };
            let label = exit_info
                .map(ExitInfo::label)
                .unwrap_or_else(|| "proceso terminado".into());
            div()
                .absolute()
                .bottom_2()
                .left_2()
                .right_2()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .border_1()
                .border_color(accent)
                .bg(background.opacity(0.94))
                .text_color(palette.foreground)
                .child(label)
        });

        let url_overlay = pending_url.map(|url| {
            div()
                .absolute()
                .bottom_2()
                .left_2()
                .right_2()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .border_1()
                .border_color(palette.cursor)
                .bg(background.opacity(0.96))
                .text_color(palette.foreground)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().flex_1().overflow_hidden().child(url))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .bg(palette.ansi(2))
                        .text_color(background)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::confirm_url))
                        .child("Abrir"),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .bg(palette.ansi(8))
                        .text_color(palette.foreground)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::cancel_url))
                        .child("Cancelar"),
                )
        });

        let notice_overlay = notice.map(|notice| {
            div()
                .absolute()
                .top_2()
                .left_2()
                .right_2()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .border_1()
                .border_color(palette.ansi(3))
                .bg(background.opacity(0.96))
                .text_color(palette.foreground)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().flex_1().overflow_hidden().child(notice))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .bg(palette.ansi(8))
                        .text_color(palette.foreground)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::dismiss_notice))
                        .child("Cerrar"),
                )
        });

        div()
            .flex()
            .flex_col()
            .relative()
            .size_full()
            .bg(background)
            .text_color(palette.foreground)
            .p(padding)
            .border_1()
            .border_color(border_color)
            .key_context(TERMINAL_CONTEXT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(Self::copy_action))
            .on_action(cx.listener(Self::paste_action))
            .on_action(cx.listener(Self::tab_action))
            .on_action(cx.listener(Self::backtab_action))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll(cx.listener(Self::on_scroll_wheel))
            .children(has_session.then(|| ImeAnchor {
                focus: self.focus.clone(),
                view,
            }))
            .child(body)
            .children(exit_overlay)
            .children(url_overlay)
            .children(notice_overlay)
            .context_menu(|menu, _, _| {
                menu.menu("Copiar", Box::new(CopySelection))
                    .separator()
                    .menu("Pegar", Box::new(PasteSelection))
            })
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<String> {
        self.ime.text_for_range(range)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<UTF16Selection> {
        self.ime.marked_range().map(|range| UTF16Selection {
            range,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Range<usize>> {
        self.ime.marked_range()
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        self.ime.clear();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let bytes = self.ime.commit(text);
        if !bytes.is_empty() {
            self.scroll_to_bottom();
            self.clear_selection();
            self.write(&bytes);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.ime.set_preedit(new_text);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Bounds<Pixels>> {
        let Some(geometry) = self.geometry else {
            return Some(element_bounds);
        };
        let state = self.state.borrow();
        let cursor = state.cursor.or_else(|| {
            state.rows.checked_sub(1).map(|line| CursorSnapshot {
                line,
                column: 0,
                shape: CursorShape::Block,
            })
        })?;
        let origin = point(
            geometry.origin.x + geometry.cell_width * cursor.column,
            geometry.origin.y + geometry.line_height * cursor.line,
        );
        Some(Bounds::new(
            origin,
            size(geometry.cell_width, geometry.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        None
    }

    fn paste(&mut self, item: ClipboardItem, _window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(text) = item.text() else {
            return;
        };
        self.paste_text(&text, cx);
    }
}

/// Ancla de tamaño cero que registra el `EntityInputHandler` en la fase de paint.
struct ImeAnchor {
    focus: FocusHandle,
    view: Entity<TerminalView>,
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
        let layout = window.request_layout(Style::default(), std::iter::empty(), cx);
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

struct TerminalElement {
    view: Entity<TerminalView>,
    state: Rc<RefCell<RenderState>>,
    metrics: Metrics,
    debug: DebugRender,
    preedit: String,
}

struct PrepaintState {
    lines: Vec<(usize, ShapedLine)>,
    preedit: Option<(ShapedLine, Point<Pixels>)>,
    rows: usize,
    cols: usize,
    origin: Point<Pixels>,
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

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
                size: Size::full(),
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
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let cols = (f32::from(bounds.size.width) / f32::from(self.metrics.cell_width))
            .floor()
            .max(1.0) as usize;
        let rows = (f32::from(bounds.size.height) / f32::from(self.metrics.line_height))
            .floor()
            .max(1.0) as usize;

        self.view
            .update(cx, |view, _| view.observe_bounds(bounds.origin, cols, rows));

        let mut state = self.state.borrow_mut();
        let visible = rows.min(state.rows);
        let cursor = state.cursor;
        let mut lines = Vec::with_capacity(visible);
        for row in 0..visible {
            if let Some(cached) = state.cache[row].as_ref() {
                lines.push((row, cached.clone()));
                continue;
            }
            let RenderState {
                cells,
                combining,
                cache,
                scratch_text,
                scratch_runs,
                ..
            } = &mut *state;
            row_text_runs(
                &cells[row],
                &combining[row],
                &self.metrics,
                self.debug,
                scratch_text,
                scratch_runs,
            );
            let line = window.text_system().shape_line(
                scratch_text.clone().into(),
                self.metrics.font_size,
                scratch_runs,
                None,
            );
            cache[row] = Some(line.clone());
            lines.push((row, line));
        }

        let preedit = if self.preedit.is_empty() {
            None
        } else {
            let cursor = cursor.unwrap_or(CursorSnapshot {
                line: 0,
                column: 0,
                shape: CursorShape::Block,
            });
            let run = TextRun {
                len: self.preedit.len(),
                font: self.metrics.font.clone(),
                color: self.metrics.palette.foreground,
                background_color: None,
                underline: Some(UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(self.metrics.palette.cursor),
                    wavy: false,
                }),
                strikethrough: None,
            };
            let line = window.text_system().shape_line(
                self.preedit.clone().into(),
                self.metrics.font_size,
                &[run],
                None,
            );
            let origin = point(
                bounds.origin.x + self.metrics.cell_width * cursor.column,
                bounds.origin.y + self.metrics.line_height * cursor.line,
            );
            Some((line, origin))
        };

        PrepaintState {
            lines,
            preedit,
            rows,
            cols,
            origin: bounds.origin,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = self.state.borrow();
        let metrics = &self.metrics;
        let origin = prepaint.origin;
        let cell_width = metrics.cell_width;
        let line_height = metrics.line_height;
        let rows = prepaint.rows.min(state.rows);
        let cols = prepaint.cols.min(state.cols);

        window.paint_layer(bounds, |window| {
            // Fondo del grid: las celdas con color por defecto deben usar la
            // paleta del terminal aunque el contenedor tenga otro token.
            window.paint_quad(fill(bounds, metrics.palette.background));

            for row in 0..rows {
                paint_row_background(
                    &state.cells[row],
                    cols,
                    row,
                    origin,
                    cell_width,
                    line_height,
                    metrics,
                    window,
                    self.debug,
                );
            }

            if let Some(selection) = state.selection {
                for row in 0..rows {
                    paint_selection_row(
                        selection,
                        row,
                        cols,
                        origin,
                        cell_width,
                        line_height,
                        metrics,
                        window,
                    );
                }
            }

            for (row, line) in &prepaint.lines {
                if *row >= rows {
                    continue;
                }
                let line_origin = point(origin.x, origin.y + line_height * *row);
                let _ = line.paint(line_origin, line_height, TextAlign::Left, None, window, cx);
            }

            for row in 0..rows {
                for link in &state.links[row] {
                    paint_hyperlink(
                        link,
                        row,
                        cols,
                        origin,
                        cell_width,
                        line_height,
                        metrics,
                        window,
                    );
                }
            }

            if let Some(cursor) = state.cursor
                && cursor.line < rows
                && cursor.column < cols
            {
                paint_cursor(cursor, origin, cell_width, line_height, metrics, window);
            }

            if let Some((line, origin)) = &prepaint.preedit {
                window.paint_quad(fill(
                    Bounds::new(
                        point(origin.x, origin.y),
                        size(line.width().max(cell_width), line_height),
                    ),
                    metrics.palette.background.opacity(0.96),
                ));
                let _ = line.paint(*origin, line_height, TextAlign::Left, None, window, cx);
            }

            paint_scrollbar(
                bounds,
                rows,
                state.display_offset,
                state.history_size,
                line_height,
                metrics,
                window,
            );

            if state.bell_flash {
                window.paint_quad(fill(bounds, metrics.palette.cursor.opacity(0.06)));
            }

            if self.debug.row_lines {
                let color = metrics.palette.cursor.opacity(0.5);
                for row in 0..=rows {
                    let y = origin.y + line_height * row;
                    window.paint_quad(fill(
                        Bounds::new(point(origin.x, y), size(cell_width * cols, px(1.0))),
                        color,
                    ));
                }
            }
        });
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn row_text_runs(
    cells: &[SnapshotCell],
    combining: &[(usize, char)],
    metrics: &Metrics,
    debug: DebugRender,
    text: &mut String,
    runs: &mut Vec<TextRun>,
) {
    text.clear();
    runs.clear();
    let mut current: Option<(RunKey, usize)> = None;
    let mut combining_index = 0;

    for (column, cell) in cells.iter().enumerate() {
        if cell.attrs.wide_spacer {
            continue;
        }

        let ch = if cell.attrs.hidden { ' ' } else { cell.ch };
        let (foreground, _) = resolve_cell_colors(cell, &metrics.palette);
        let key = RunKey {
            color: foreground,
            bold: cell.attrs.bold,
            italic: cell.attrs.italic,
            underline: cell.attrs.underline && !debug.no_underline,
            strikethrough: cell.attrs.strikethrough && !debug.no_underline,
        };
        let mut len = ch.len_utf8();
        text.push(ch);

        while combining_index < combining.len() && combining[combining_index].0 < column {
            combining_index += 1;
        }
        while combining_index < combining.len() && combining[combining_index].0 == column {
            let extra = combining[combining_index].1;
            text.push(extra);
            len += extra.len_utf8();
            combining_index += 1;
        }

        match current {
            Some((previous, previous_len)) if previous == key => {
                current = Some((previous, previous_len + len));
            }
            Some((previous, previous_len)) => {
                runs.push(make_run(previous, previous_len, metrics));
                current = Some((key, len));
            }
            None => current = Some((key, len)),
        }
    }

    if let Some((key, len)) = current {
        runs.push(make_run(key, len, metrics));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RunKey {
    color: Hsla,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

fn make_run(key: RunKey, len: usize, metrics: &Metrics) -> TextRun {
    let mut run_font = metrics.font.clone();
    if key.bold {
        run_font = run_font.bold();
    }
    if key.italic {
        run_font = run_font.italic();
    }

    TextRun {
        len,
        font: run_font,
        color: key.color,
        background_color: None,
        underline: key.underline.then_some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(key.color),
            wavy: false,
        }),
        strikethrough: key.strikethrough.then_some(StrikethroughStyle {
            thickness: px(1.0),
            color: Some(key.color),
        }),
    }
}

fn resolve_cell_colors(cell: &SnapshotCell, palette: &TerminalPalette) -> (Hsla, Hsla) {
    let mut foreground = resolve_color(cell.fg, true, palette);
    let background = resolve_color(cell.bg, false, palette);
    let background = if cell.attrs.reverse {
        std::mem::replace(&mut foreground, background)
    } else {
        background
    };

    if cell.attrs.dim {
        foreground = if matches!(cell.fg, CellColor::Default) {
            palette.dim_foreground()
        } else {
            foreground.opacity(0.75)
        };
    }

    (foreground, background)
}

fn resolve_color(color: CellColor, foreground: bool, palette: &TerminalPalette) -> Hsla {
    match color {
        CellColor::Default => {
            if foreground {
                palette.foreground
            } else {
                palette.background
            }
        }
        CellColor::Indexed(index) => palette.ansi(index),
        CellColor::Rgb(rgb) => axf_ui::gpui::rgb(rgb).into(),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "el pintado por fila necesita origen, métricas y ventana"
)]
fn paint_row_background(
    cells: &[SnapshotCell],
    cols: usize,
    row: usize,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
    debug: DebugRender,
) {
    if debug.no_bg {
        return;
    }

    let overlap = if debug.bg_overlap { px(1.0) } else { px(0.0) };
    let mut run: Option<(usize, usize, Hsla)> = None;

    for (column, cell) in cells.iter().enumerate().take(cols.min(cells.len())) {
        let cell = *cell;
        if cell.attrs.wide_spacer {
            continue;
        }
        let width = if cell.attrs.wide { 2 } else { 1 };
        let (_, background) = resolve_cell_colors(&cell, &metrics.palette);
        let paint = cell.attrs.reverse || cell.bg != CellColor::Default;

        if !paint {
            if let Some((start, end, color)) = run.take() {
                paint_background_quad(
                    start,
                    end,
                    row,
                    origin,
                    cell_width,
                    line_height,
                    overlap,
                    color,
                    window,
                );
            }
            continue;
        }

        match run {
            Some((start, end, color)) if color == background && end == column => {
                run = Some((start, (column + width).min(cols), color));
            }
            Some((start, end, color)) => {
                paint_background_quad(
                    start,
                    end,
                    row,
                    origin,
                    cell_width,
                    line_height,
                    overlap,
                    color,
                    window,
                );
                run = Some((column, (column + width).min(cols), background));
            }
            None => run = Some((column, (column + width).min(cols), background)),
        }
    }

    if let Some((start, end, color)) = run {
        paint_background_quad(
            start,
            end,
            row,
            origin,
            cell_width,
            line_height,
            overlap,
            color,
            window,
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "coordenadas del quad calculadas en el helper de runs"
)]
fn paint_background_quad(
    start: usize,
    end: usize,
    row: usize,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    overlap: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    if end <= start {
        return;
    }
    let x = origin.x + cell_width * start;
    let y = origin.y + line_height * row;
    window.paint_quad(fill(
        Bounds::new(
            point(x, y),
            size(cell_width * (end - start), line_height + overlap),
        ),
        color,
    ));
}

#[expect(
    clippy::too_many_arguments,
    reason = "la selección se pinta por fila con la geometría del grid"
)]
fn paint_selection_row(
    selection: SelectionSpan,
    row: usize,
    cols: usize,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
) {
    if row < selection.start_line || row > selection.end_line || cols == 0 {
        return;
    }

    let (start, end) = if selection.block || selection.start_line == selection.end_line {
        (selection.start_col, selection.end_col)
    } else if row == selection.start_line {
        (selection.start_col, cols - 1)
    } else if row == selection.end_line {
        (0, selection.end_col)
    } else {
        (0, cols - 1)
    };
    if end < start {
        return;
    }

    let x = origin.x + cell_width * start;
    let y = origin.y + line_height * row;
    window.paint_quad(fill(
        Bounds::new(
            point(x, y),
            size(cell_width * (end - start + 1), line_height),
        ),
        metrics.palette.selection.opacity(0.45),
    ));
}

#[expect(
    clippy::too_many_arguments,
    reason = "el subrayado de un enlace necesita los mismos datos que la fila"
)]
fn paint_hyperlink(
    link: &HyperlinkSpan,
    row: usize,
    cols: usize,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
) {
    let start = link.start.min(cols);
    let end = link.end.min(cols);
    if end <= start {
        return;
    }
    let x = origin.x + cell_width * start;
    let y = origin.y + line_height * (row + 1) - px(1.0);
    window.paint_quad(fill(
        Bounds::new(point(x, y), size(cell_width * (end - start), px(1.0))),
        metrics.palette.ansi(12).opacity(0.85),
    ));
}

fn paint_scrollbar(
    bounds: Bounds<Pixels>,
    rows: usize,
    display_offset: usize,
    history_size: usize,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
) {
    if history_size == 0 || rows == 0 {
        return;
    }

    let width = (metrics.cell_width * 0.25).max(px(2.0));
    let track_height = line_height * rows;
    let total = rows + history_size;
    let thumb_height =
        (f32::from(track_height) * (rows as f32 / total as f32)).max(f32::from(line_height) * 0.5);
    let max_top = (f32::from(track_height) - thumb_height).max(0.0);
    let offset = display_offset.min(history_size) as f32 / history_size as f32;
    let top = max_top * (1.0 - offset);

    let x = bounds.origin.x + bounds.size.width - width;
    let y = bounds.origin.y;
    window.paint_quad(fill(
        Bounds::new(point(x, y), size(width, track_height)),
        metrics.palette.foreground.opacity(0.06),
    ));
    window.paint_quad(fill(
        Bounds::new(point(x, y + px(top)), size(width, px(thumb_height))),
        metrics.palette.cursor.opacity(0.45),
    ));
}

fn paint_cursor(
    cursor: CursorSnapshot,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
) {
    let x = origin.x + cell_width * cursor.column;
    let y = origin.y + line_height * cursor.line;
    let color = metrics.palette.cursor;

    let bounds = match cursor.shape {
        CursorShape::Beam => Bounds::new(point(x, y), size(px(2.0), line_height)),
        CursorShape::Underline => Bounds::new(
            point(x, y + line_height - px(2.0)),
            size(cell_width, px(2.0)),
        ),
        CursorShape::HollowBlock => Bounds::new(point(x, y), size(cell_width, line_height)),
        CursorShape::Block => Bounds::new(point(x, y), size(cell_width, line_height)),
    };

    let alpha = if matches!(cursor.shape, CursorShape::HollowBlock) {
        0.25
    } else {
        0.55
    };
    window.paint_quad(fill(bounds, color.opacity(alpha)));
}

/// Teclas cuyo texto entrega el manejador de texto de GPUI (IME/WM_CHAR).
///
/// En Windows un carácter imprimible llega dos veces: como `KeyDownEvent` con
/// `key_char` y como commit del `EntityInputHandler`. Escribir en ambos sitios
/// duplica cada tecla; la terminal deja el texto al IME y reserva
/// `input::encode` para teclas de control y secuencias.
fn is_text_input(keystroke: &Keystroke) -> bool {
    let modifiers = keystroke.modifiers;
    if modifiers.control || modifiers.alt {
        // AltGr en Windows es ctrl+alt; el carácter lo entrega el IME.
        return modifiers.control && modifiers.alt && keystroke.key_char.is_some();
    }
    keystroke.key_char.is_some() && !is_named_key(&keystroke.key)
}

/// Solo abrimos enlaces web; el resto de esquemas se muestran pero no se abren.
fn is_web_url(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Traduce los nombres estándar de OSC 22 al cursor de GPUI.
fn cursor_style_from_osc(shape: &str) -> CursorStyle {
    match shape.trim().to_ascii_lowercase().as_str() {
        "text" | "ibeam" | "i-beam" => CursorStyle::IBeam,
        "pointer" | "hand" | "pointinghand" => CursorStyle::PointingHand,
        "crosshair" | "cross" => CursorStyle::Crosshair,
        "grab" | "openhand" | "move" => CursorStyle::OpenHand,
        "grabbing" | "closedhand" => CursorStyle::ClosedHand,
        "ew-resize" | "col-resize" | "e-resize" | "w-resize" => CursorStyle::ResizeLeftRight,
        "ns-resize" | "row-resize" | "n-resize" | "s-resize" => CursorStyle::ResizeUpDown,
        "nesw-resize" => CursorStyle::ResizeUpLeftDownRight,
        "nwse-resize" => CursorStyle::ResizeUpRightDownLeft,
        _ => CursorStyle::Arrow,
    }
}

/// GPUI entrega el delta con el signo opuesto a la dirección física de la
/// rueda: negativo al bajar. Xterm espera 64 al subir y 65 al bajar.
fn wheel_action(delta_x: f32, delta_y: f32) -> MouseAction {
    if delta_y.abs() >= delta_x.abs() {
        if delta_y < 0.0 {
            MouseAction::WheelDown
        } else {
            MouseAction::WheelUp
        }
    } else if delta_x < 0.0 {
        MouseAction::WheelRight
    } else {
        MouseAction::WheelLeft
    }
}

fn terminal_mouse_button(button: MouseButton) -> Option<TerminalMouseButton> {
    match button {
        MouseButton::Left => Some(TerminalMouseButton::Left),
        MouseButton::Middle => Some(TerminalMouseButton::Middle),
        MouseButton::Right => Some(TerminalMouseButton::Right),
        MouseButton::Navigate(_) => None,
    }
}

fn is_named_key(key: &str) -> bool {
    matches!(
        key,
        "enter"
            | "return"
            | "tab"
            | "backtab"
            | "backspace"
            | "escape"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "insert"
            | "delete"
            | "pageup"
            | "pagedown"
            | "f1"
            | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
    )
}

fn pixels_to_u16(pixels: Pixels) -> u16 {
    let value = f32::from(pixels).round();
    if value < 1.0 {
        1
    } else if value > f32::from(u16::MAX) {
        u16::MAX
    } else {
        value as u16
    }
}

/// Candidato de URL encontrado en una fila.
#[derive(Clone, Debug, PartialEq, Eq)]
struct UrlCandidate {
    start: usize,
    end: usize,
    text: String,
}

/// Busca URLs `http`/`https` con índices de carácter (no de byte).
fn find_urls(text: &str) -> Vec<UrlCandidate> {
    let chars: Vec<char> = text.chars().collect();
    let mut candidates = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let remaining: String = chars[index..].iter().collect();
        let lower = remaining.to_ascii_lowercase();
        let prefix = if lower.starts_with("https://") {
            "https://"
        } else if lower.starts_with("http://") {
            "http://"
        } else {
            index += 1;
            continue;
        };

        let start = index;
        let mut end = index + prefix.len();
        while end < chars.len() {
            let ch = chars[end];
            if ch.is_whitespace()
                || ch.is_control()
                || matches!(
                    ch,
                    '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            {
                break;
            }
            end += 1;
        }
        while end > start + prefix.len()
            && matches!(chars[end - 1], '.' | ',' | ';' | ':' | '!' | '?' | '…')
        {
            end -= 1;
        }
        if end > start + prefix.len() {
            candidates.push(UrlCandidate {
                start,
                end,
                text: chars[start..end].iter().collect(),
            });
        }
        index = end.max(index + 1);
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_direction_matches_the_physical_wheel() {
        assert_eq!(wheel_action(0.0, -1.0), MouseAction::WheelDown);
        assert_eq!(wheel_action(0.0, 1.0), MouseAction::WheelUp);
        assert_eq!(wheel_action(-1.0, 0.0), MouseAction::WheelRight);
        assert_eq!(wheel_action(1.0, 0.0), MouseAction::WheelLeft);
    }

    #[test]
    fn only_web_urls_are_openable() {
        assert!(is_web_url("https://example.com"));
        assert!(is_web_url("http://example.com"));
        assert!(is_web_url("HTTPS://EXAMPLE.COM"));
        assert!(!is_web_url("file:///C:/Windows"));
        assert!(!is_web_url("javascript:alert(1)"));
    }

    #[test]
    fn osc22_maps_standard_cursor_names() {
        assert_eq!(cursor_style_from_osc("pointer"), CursorStyle::PointingHand);
        assert_eq!(cursor_style_from_osc("text"), CursorStyle::IBeam);
        assert_eq!(
            cursor_style_from_osc("ew-resize"),
            CursorStyle::ResizeLeftRight
        );
        assert_eq!(cursor_style_from_osc("default"), CursorStyle::Arrow);
        assert_eq!(cursor_style_from_osc("desconocido"), CursorStyle::Arrow);
    }

    #[test]
    fn detects_plain_urls() {
        let urls = find_urls("ver https://example.com/x y http://a.test");
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].text, "https://example.com/x");
        assert_eq!(urls[1].text, "http://a.test");
    }

    #[test]
    fn trims_url_punctuation() {
        let urls = find_urls("(https://example.com/x), fin.");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].text, "https://example.com/x");
    }

    #[test]
    fn ignores_incomplete_urls() {
        assert!(find_urls("http://").is_empty());
        assert!(find_urls("https:// ejemplo").is_empty());
    }
}
