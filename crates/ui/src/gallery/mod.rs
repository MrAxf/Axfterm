#![expect(
    clippy::print_stderr,
    reason = "el ejemplo de la galería informa de un fallo al abrir la ventana"
)]

//! Galería del sistema de diseño de Axfterm.
//!
//! Solo se compila con la feature `gallery` (desarrollo). La app la muestra
//! como una superficie de Axfterm Studio; el ejemplo `gallery` la abre en una
//! ventana propia para revisión manual.

mod actions;
mod compositions;
mod feedback;
mod foundations;
mod helpers;
#[path = "icons.rs"]
mod icon_catalog;
mod inputs;
mod navigation;
mod overlays;
mod themes;

use crate::gpui::component::IndexPath;
use crate::gpui::component::input::{InputEvent, InputState};
use crate::gpui::component::scroll::ScrollableElement as _;
use crate::gpui::component::searchable_list::{SearchableListItem, SearchableVec};
use crate::gpui::component::select::SelectState;
use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement, Pixels, Render, StatefulInteractiveElement as _,
    Styled, Subscription, Window, div, px,
};
use crate::icons;
use crate::theme;

const GALLERY_WIDTH: f32 = 1440.0;
const GALLERY_HEIGHT: f32 = 900.0;

/// Sección visible de la galería.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GallerySection {
    /// Tokens, ANSI, tipografía y medidas.
    #[default]
    Foundations,
    /// Botones, iconos y toggles.
    Actions,
    /// Campos de texto y controles de gpui-kit.
    Inputs,
    /// Pestañas, menús y filas de navegación.
    Navigation,
    /// Píldoras, avisos y estados vacíos.
    Feedback,
    /// Modales y tooltips.
    Overlays,
    /// Composiciones de superficies reales.
    Compositions,
    /// Catálogo completo de iconos.
    Icons,
    /// Temas incluidos y sus muestras.
    Themes,
}

impl GallerySection {
    /// Orden del rail y de la navegación con flechas.
    pub const ALL: &'static [Self] = &[
        Self::Foundations,
        Self::Actions,
        Self::Inputs,
        Self::Navigation,
        Self::Feedback,
        Self::Overlays,
        Self::Compositions,
        Self::Icons,
        Self::Themes,
    ];

    /// Identificador estable usado por el protocolo de Studio.
    pub fn id(self) -> &'static str {
        match self {
            Self::Foundations => "foundations",
            Self::Actions => "actions",
            Self::Inputs => "inputs",
            Self::Navigation => "navigation",
            Self::Feedback => "feedback",
            Self::Overlays => "overlays",
            Self::Compositions => "compositions",
            Self::Icons => "icons",
            Self::Themes => "themes",
        }
    }

    /// Etiqueta del rail.
    pub fn label(self) -> &'static str {
        match self {
            Self::Foundations => "Fundaciones",
            Self::Actions => "Acciones",
            Self::Inputs => "Entradas",
            Self::Navigation => "Navegación",
            Self::Feedback => "Feedback",
            Self::Overlays => "Overlays",
            Self::Compositions => "Composiciones",
            Self::Icons => "Iconos",
            Self::Themes => "Temas",
        }
    }

    /// Icono del rail.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Foundations => icons::PALETTE,
            Self::Actions => icons::PLAYER_PLAY,
            Self::Inputs => icons::KEYBOARD,
            Self::Navigation => icons::SIDEBAR,
            Self::Feedback => icons::BELL,
            Self::Overlays => icons::MENU,
            Self::Compositions => icons::TERMINAL,
            Self::Icons => icons::CHECK,
            Self::Themes => icons::SUN,
        }
    }

    /// Resuelve un id del protocolo.
    pub fn from_id(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|section| section.id() == value)
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|item| *item == self).unwrap_or(0)
    }

    fn next(self, delta: isize) -> Self {
        let count = Self::ALL.len() as isize;
        let index = (self.index() as isize + delta).rem_euclid(count) as usize;
        Self::ALL[index]
    }
}

/// Opción de ejemplo para el `Select` de gpui-kit.
#[derive(Clone, Debug)]
pub(crate) struct DemoChoice {
    title: crate::gpui::SharedString,
    value: crate::gpui::SharedString,
}

impl SearchableListItem for DemoChoice {
    type Value = crate::gpui::SharedString;

    fn title(&self) -> crate::gpui::SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

/// Lista del `Select` de la galería.
pub(crate) type DemoList = SearchableVec<DemoChoice>;

/// Estado de la galería. El tema y las métricas son globales.
#[expect(
    missing_debug_implementations,
    reason = "contiene entidades y suscripciones de GPUI que no implementan Debug"
)]
pub struct Gallery {
    focus: FocusHandle,
    section: GallerySection,
    toggle: bool,
    first_input: Entity<InputState>,
    second_input: Entity<InputState>,
    select: Entity<SelectState<DemoList>>,
    modal: bool,
    _subscription: Subscription,
}

impl Gallery {
    /// Crea la galería con foco de teclado.
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let first_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Escribe para probar el input…")
                .default_value("axfterm")
        });
        let second_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Buscar en la aplicación…"));
        let choices = vec![
            DemoChoice {
                title: "Terminal".into(),
                value: "terminal".into(),
            },
            DemoChoice {
                title: "Ajustes".into(),
                value: "settings".into(),
            },
            DemoChoice {
                title: "Atajos".into(),
                value: "shortcuts".into(),
            },
        ];
        let select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::from(choices),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let subscription = cx.subscribe(&first_input, |_, _, _: &InputEvent, cx| cx.notify());
        Self {
            focus,
            section: GallerySection::Foundations,
            toggle: true,
            first_input,
            second_input,
            select,
            modal: false,
            _subscription: subscription,
        }
    }

    /// Sección activa.
    pub fn section(&self) -> GallerySection {
        self.section
    }

    /// Cambia de sección.
    pub fn set_section(&mut self, section: GallerySection) {
        self.section = section;
        self.modal = false;
    }

    /// Alterna el modal de la sección de overlays.
    pub fn set_modal(&mut self, open: bool) {
        self.modal = open;
    }

    /// `true` si el modal está abierto.
    pub fn modal_open(&self) -> bool {
        self.modal
    }

    fn select(&mut self, section: GallerySection, cx: &mut Context<'_, Self>) {
        self.set_section(section);
        cx.notify();
    }

    fn render_nav(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let ui = theme::state(cx).theme.ui;
        let metrics = theme::metrics(cx);
        let mut nav = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_xxs())
            .w(metrics.sidebar_actions_compact_width() * 1.15)
            .flex_shrink_0()
            .p(metrics.spacing_sm())
            .border_r_1()
            .border_color(ui.border)
            .bg(ui.surface);
        for section in GallerySection::ALL {
            let active = *section == self.section;
            let row = div()
                .id(("gallery-nav", section.index()))
                .flex()
                .items_center()
                .gap(metrics.spacing_sm())
                .h(metrics.sidebar_tab_height())
                .px(metrics.spacing_sm())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .text_size(metrics.font_size * 0.8125)
                .text_color(if active { ui.text } else { ui.muted })
                .when(active, |this| {
                    this.bg(ui.surface_raised).text_color(ui.text)
                })
                .when(!active, |this| {
                    this.hover(move |this| this.bg(ui.surface_raised))
                })
                .on_click(cx.listener(move |gallery, _, _, cx| {
                    gallery.select(*section, cx);
                }))
                .child(
                    crate::components::Icon::new(section.icon())
                        .size(crate::components::IconSize::Sm)
                        .color(if active { ui.accent } else { ui.muted }),
                )
                .child(section.label());
            nav = nav.child(row);
        }
        nav.into_any_element()
    }

    fn render_header(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let variant = match state.theme.variant() {
            crate::theme::ThemeVariant::Dark => "oscuro",
            crate::theme::ThemeVariant::Light => "claro",
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(metrics.spacing_lg())
            .px(metrics.spacing_lg())
            .py(metrics.spacing_md())
            .border_b_1()
            .border_color(ui.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 1.1)
                            .child("Sistema de diseño"),
                    )
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child(format!(
                                "{} · {variant} · {:.1} px · escala {:.2} · DPI {:.2}",
                                state.theme.meta.name,
                                metrics.font_size / px(1.0),
                                metrics.scale,
                                metrics.display_scale,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_xs())
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.subtle)
                    .child("1–9 secciones · ←/→ · Esc cierra")
                    .child(crate::components::Kbd::new("Ctrl+Tab")),
            )
            .into_any_element()
    }

    fn render_content(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        match self.section {
            GallerySection::Foundations => foundations::render(window, cx).into_any_element(),
            GallerySection::Actions => actions::render(self, window, cx).into_any_element(),
            GallerySection::Inputs => inputs::render(self, window, cx).into_any_element(),
            GallerySection::Navigation => navigation::render(cx).into_any_element(),
            GallerySection::Feedback => feedback::render(cx).into_any_element(),
            GallerySection::Overlays => overlays::render(self, window, cx).into_any_element(),
            GallerySection::Compositions => compositions::render(cx).into_any_element(),
            GallerySection::Icons => icon_catalog::render(cx).into_any_element(),
            GallerySection::Themes => themes::render(window, cx).into_any_element(),
        }
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let ui = theme::state(cx).theme.ui;
        let nav = self.render_nav(cx);
        let header = self.render_header(cx);
        let content = self.render_content(window, cx);
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(ui.background)
            .text_color(ui.text)
            .key_context("Gallery")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|gallery, event: &KeyDownEvent, _, cx| {
                let key = event.keystroke.key.as_str();
                if key == "escape" {
                    gallery.modal = false;
                    cx.notify();
                    return;
                }
                if let Ok(number) = key.parse::<usize>()
                    && let Some(index) = number.checked_sub(1)
                    && let Some(section) = GallerySection::ALL.get(index)
                {
                    gallery.select(*section, cx);
                    return;
                }
                match key {
                    "right" | "down" | "tab" => gallery.select(gallery.section.next(1), cx),
                    "left" | "up" | "shift-tab" => gallery.select(gallery.section.next(-1), cx),
                    _ => {}
                }
            }))
            .child(header)
            .child(
                div().flex().flex_1().min_h_0().w_full().child(nav).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .child(content),
                ),
            )
    }
}

/// Abre la galería en una ventana independiente (ejemplo y revisión manual).
pub fn run_standalone() {
    crate::gpui::application()
        .with_assets(crate::Assets)
        .run(|cx| {
            crate::init(cx);
            let options = crate::gpui::WindowOptions {
                window_bounds: Some(crate::gpui::WindowBounds::Windowed(
                    crate::gpui::Bounds::centered(
                        None,
                        crate::gpui::size(px(GALLERY_WIDTH), px(GALLERY_HEIGHT)),
                        cx,
                    ),
                )),
                titlebar: Some(crate::gpui::TitlebarOptions {
                    title: Some("Axfterm — sistema de diseño".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(error) = crate::gpui::open_window(options, cx, |window, cx| {
                cx.new(|cx| Gallery::new(window, cx))
            }) {
                eprintln!("gallery: no se pudo abrir la ventana: {error}");
            }
        });
}

/// Utilidades compartidas por las secciones.
fn px_num(value: Pixels) -> f32 {
    value / px(1.0)
}

fn section_root(cx: &App) -> Div {
    let metrics = theme::metrics(cx);
    div()
        .flex()
        .flex_col()
        .gap(metrics.spacing_lg())
        .p(metrics.spacing_lg())
        .w_full()
}
