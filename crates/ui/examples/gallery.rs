//! Galería visual del sistema de M3.
//!
//! ```powershell
//! $env:AXFTERM_GALLERY_FONT = "12"; cargo run -p axf_ui --example gallery
//! $env:AXFTERM_GALLERY_FONT = "16"; cargo run -p axf_ui --example gallery
//! ```
//!
//! Teclas: `1`..`6` cambian de tema, `+`/`-` ajustan la escala global y `Esc`
//! cierra el modal. Sirve para revisar contraste, foco y densidad a dos tamaños.

#![expect(
    clippy::print_stderr,
    reason = "ejemplo visual: informa de un fallo al abrir la ventana"
)]

use axf_ui::components::{
    Button, ControlSize, EmptyState, Icon, IconButton, IconSize, Menu, MenuItem, Modal, StatusPill,
    Tab, TabStatus, TextField, Toast, Toggle,
};
use axf_ui::fonts::FontSet;
use axf_ui::gpui::component::input::{InputEvent, InputState};
use axf_ui::gpui::component::scroll::ScrollableElement as _;
use axf_ui::gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement, Render, Styled, Subscription, Window, div, px, relative, size,
};
use axf_ui::metrics::UiMetrics;
use axf_ui::theme::{self, ThemeState, builtin};
use axf_ui::{Assets, brand, icons};

struct Gallery {
    focus: FocusHandle,
    theme: String,
    base_font: f32,
    scale: f32,
    toggle: bool,
    editing: Entity<InputState>,
    _subscription: Subscription,
    modal: bool,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let base_font = std::env::var("AXFTERM_GALLERY_FONT")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(14.0)
            .clamp(11.0, 24.0);
        let scale = std::env::var("AXFTERM_GALLERY_SCALE")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(1.0)
            .clamp(0.8, 2.0);
        let editing = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Escribe para probar el input…")
                .default_value("axfterm")
        });
        let subscription = cx.subscribe(&editing, |_, _, _: &InputEvent, cx| cx.notify());
        let gallery = Self {
            focus,
            theme: "One Dark".into(),
            base_font,
            scale,
            toggle: true,
            editing,
            _subscription: subscription,
            modal: false,
        };
        Self::install(&gallery, cx);
        gallery
    }

    fn install(gallery: &Self, cx: &mut App) {
        let theme = builtin::builtin(&gallery.theme).unwrap_or_else(theme::Theme::fallback);
        let metrics = UiMetrics::scaled(gallery.base_font, gallery.scale, 1.0);
        theme::install(
            cx,
            ThemeState {
                theme,
                metrics,
                fonts: FontSet::system(),
                ui_font_size: metrics.font_size,
                mono_font_size: px(gallery.base_font - 1.0),
                terminal_line_height: px(0.0),
            },
        );
    }

    fn cycle_theme(&mut self, delta: isize, cx: &mut Context<'_, Self>) {
        let themes = builtin::BUILTIN_THEMES;
        let current = themes
            .iter()
            .position(|builtin| builtin.name == self.theme)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(themes.len() as isize) as usize;
        self.theme = themes[next].name.to_string();
        Self::install(self, cx);
        cx.notify();
    }

    fn set_scale(&mut self, delta: f32, cx: &mut Context<'_, Self>) {
        self.scale = (self.scale + delta).clamp(0.8, 2.0);
        Self::install(self, cx);
        cx.notify();
    }
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let theme_name = theme::state(cx).theme.meta.name.clone();

        let buttons = div()
            .flex()
            .flex_wrap()
            .gap(metrics.spacing_sm())
            .child(
                Button::new("gallery-primary", "Primario")
                    .primary()
                    .icon(icons::CHECK)
                    .on_click(|_, _, _| {}),
            )
            .child(
                Button::new("gallery-secondary", "Secundario")
                    .secondary()
                    .on_click(|_, _, _| {}),
            )
            .child(
                Button::new("gallery-ghost", "Ghost")
                    .ghost()
                    .icon(icons::PLUS)
                    .on_click(|_, _, _| {}),
            )
            .child(
                Button::new("gallery-danger", "Peligro")
                    .danger()
                    .on_click(|_, _, _| {}),
            )
            .child(
                Button::new("gallery-modal", "Abrir modal")
                    .ghost()
                    .on_click(cx.listener(|gallery, _, _, cx| {
                        gallery.modal = true;
                        cx.notify();
                    })),
            );

        let icon_buttons = div()
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .children([
                IconButton::new("gallery-icon-1", icons::SEARCH)
                    .tooltip("Buscar")
                    .on_click(|_, _, _| {}),
                IconButton::new("gallery-icon-2", icons::PALETTE)
                    .size(ControlSize::Md)
                    .tooltip("Tema")
                    .on_click(|_, _, _| {}),
                IconButton::new("gallery-icon-3", icons::SETTINGS)
                    .size(ControlSize::Lg)
                    .tooltip("Ajustes")
                    .on_click(|_, _, _| {}),
            ]);

        let tabs = div()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(ui.border)
            .child(
                Tab::new("gallery-tab-1", "Terminal")
                    .icon(icons::TERMINAL)
                    .active(true)
                    .focused(true)
                    .closable(false),
            )
            .child(
                Tab::new("gallery-tab-2", "opencode")
                    .icon(icons::BRAND_OPENCODE)
                    .status(Some(TabStatus::Working)),
            )
            .child(
                Tab::new("gallery-tab-3", "Copilot")
                    .icon(icons::BRAND_COPILOT)
                    .status(Some(TabStatus::Attention)),
            );

        let pills = div()
            .flex()
            .flex_wrap()
            .gap(metrics.spacing_sm())
            .children([
                StatusPill::new(TabStatus::Running),
                StatusPill::new(TabStatus::Working),
                StatusPill::new(TabStatus::Attention),
                StatusPill::new(TabStatus::Idle),
                StatusPill::new(TabStatus::Stopped),
            ]);

        let icons_grid = div().flex().flex_wrap().gap(metrics.spacing_md()).children(
            [
                icons::TERMINAL,
                icons::FOLDER,
                icons::SEARCH,
                icons::PALETTE,
                icons::SUN,
                icons::MOON,
                icons::ALERT_TRIANGLE,
                icons::INFO_CIRCLE,
                icons::CHECK,
                icons::PLAYER_PLAY,
                icons::LOADER,
                icons::KEYBOARD,
            ]
            .into_iter()
            .map(|path| {
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(metrics.icon_xl() * 1.8)
                    .rounded(metrics.radius_md())
                    .border_1()
                    .border_color(ui.border)
                    .bg(ui.surface)
                    .child(Icon::new(path).size(IconSize::Lg).color(ui.accent))
            }),
        );

        let menu = Menu::new()
            .item(
                MenuItem::new("Uno")
                    .icon(icons::PALETTE)
                    .shortcut("Ctrl+1")
                    .checked(true),
            )
            .item(MenuItem::new("Dos").icon(icons::SUN).shortcut("Ctrl+2"))
            .separator()
            .item(MenuItem::new("Tres").icon(icons::MOON).shortcut("Ctrl+3"));

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_size(metrics.font_size * 1.2).child("Galería M3"))
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child(format!(
                                "{theme_name} · {} px · escala {:.2} · DPI {:.2}",
                                metrics.font_size, metrics.scale, metrics.display_scale
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .child(
                        Button::new("gallery-prev", "Tema anterior")
                            .small()
                            .icon(icons::CHEVRON_LEFT)
                            .on_click(cx.listener(|gallery, _, _, cx| gallery.cycle_theme(-1, cx))),
                    )
                    .child(
                        Button::new("gallery-next", "Tema siguiente")
                            .small()
                            .icon(icons::CHEVRON_RIGHT)
                            .on_click(cx.listener(|gallery, _, _, cx| gallery.cycle_theme(1, cx))),
                    )
                    .child(
                        Button::new("gallery-minus", "−")
                            .small()
                            .ghost()
                            .on_click(cx.listener(|gallery, _, _, cx| gallery.set_scale(-0.1, cx))),
                    )
                    .child(
                        Button::new("gallery-plus", "+")
                            .small()
                            .ghost()
                            .on_click(cx.listener(|gallery, _, _, cx| gallery.set_scale(0.1, cx))),
                    ),
            );

        let content = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_lg())
            .p(metrics.spacing_xl())
            .child(header)
            .child(section("Botones", buttons, metrics.spacing_md(), ui.border))
            .child(section(
                "Icon buttons",
                icon_buttons,
                metrics.spacing_md(),
                ui.border,
            ))
            .child(tabs)
            .child(section("Estados", pills, metrics.spacing_md(), ui.border))
            .child(section(
                "Iconos",
                icons_grid,
                metrics.spacing_md(),
                ui.border,
            ))
            .child(
                div()
                    .flex()
                    .gap(metrics.spacing_xl())
                    .items_start()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.spacing_sm())
                            .w(relative(0.5))
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.75)
                                    .text_color(ui.muted)
                                    .child("Input"),
                            )
                            .child(TextField::new(self.editing.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.spacing_sm())
                            .w(relative(0.5))
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.75)
                                    .text_color(ui.muted)
                                    .child("Toggle y menú"),
                            )
                            .child(
                                Toggle::new("gallery-toggle", self.toggle)
                                    .label("Casilla temada")
                                    .on_click(cx.listener(|gallery, checked, _, cx| {
                                        gallery.toggle = *checked;
                                        cx.notify();
                                    })),
                            )
                            .child(menu),
                    ),
            )
            .child(
                EmptyState::new(
                    icons::FOLDER_OPEN,
                    "Empty state",
                    "Un estado vacío siempre ofrece la siguiente acción.",
                )
                .action(
                    Button::new("gallery-empty", "Acción")
                        .small()
                        .primary()
                        .on_click(|_, _, _| {}),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_md())
                    .child(
                        Icon::new(icons::RUST)
                            .size(IconSize::Lg)
                            .color(brand::hsla(brand::RUST)),
                    )
                    .child("Marca de Rust a color; las marcas monocromas usan el token de texto.")
                    .child(
                        Icon::new(icons::BRAND_OPENCODE)
                            .size(IconSize::Lg)
                            .color(ui.text),
                    ),
            );

        let toast = div()
            .absolute()
            .bottom(metrics.spacing_xl())
            .right(metrics.spacing_xl())
            .child(Toast::new("El tema se aplicó en caliente."));

        let modal = self.modal.then(|| {
            Modal::new("gallery-modal", "Modal de prueba")
                .description("Velo, tarjeta, cierre por fondo y foco visible.")
                .body(div().child("El modal usa el mismo tema, métricas y tokens que el resto."))
                .footer(
                    div().flex().gap(metrics.spacing_sm()).child(
                        Button::new("gallery-modal-close", "Cerrar")
                            .small()
                            .primary()
                            .on_click(cx.listener(|gallery, _, _, cx| {
                                gallery.modal = false;
                                cx.notify();
                            })),
                    ),
                )
                .on_close(cx.listener(|gallery, _, _, cx| {
                    gallery.modal = false;
                    cx.notify();
                }))
        });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(ui.background)
            .text_color(ui.text)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|gallery, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "1" | "2" | "3" | "4" | "5" | "6" => {
                        let index = event
                            .keystroke
                            .key
                            .parse::<usize>()
                            .ok()
                            .and_then(|value| value.checked_sub(1));
                        if let Some(index) = index
                            && let Some(theme) = builtin::BUILTIN_THEMES.get(index)
                        {
                            gallery.theme = theme.name.to_string();
                            Gallery::install(gallery, cx);
                            cx.notify();
                        }
                    }
                    "=" | "+" => gallery.set_scale(0.1, cx),
                    "-" => gallery.set_scale(-0.1, cx),
                    "escape" => {
                        gallery.modal = false;
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(content),
            )
            .child(toast)
            .children(modal)
    }
}

fn section(
    title: &'static str,
    body: impl IntoElement,
    gap: axf_ui::gpui::Pixels,
    border: axf_ui::gpui::Hsla,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(gap)
        .pt(gap)
        .border_t_1()
        .border_color(border)
        .child(div().text_size(gap).child(title))
        .child(body)
}

fn main() {
    axf_ui::gpui::application().with_assets(Assets).run(|cx| {
        axf_ui::init(cx);
        let options = axf_ui::gpui::WindowOptions {
            window_bounds: Some(axf_ui::gpui::WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(980.0), px(760.0)),
                cx,
            ))),
            titlebar: Some(axf_ui::gpui::TitlebarOptions {
                title: Some("Axfterm — galería visual".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        if let Err(error) = axf_ui::gpui::open_window(options, cx, |window, cx| {
            cx.new(|cx| Gallery::new(window, cx))
        }) {
            eprintln!("gallery: no se pudo abrir la ventana: {error}");
        }
    });
}
