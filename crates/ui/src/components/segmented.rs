//! Control segmentado único para sidebar, paleta y ajustes.
//!
//! Usa divs propios para que las opciones compartan altura, texto, hover,
//! pulsado y selección; las opciones son alcanzables con Tab, Enter y Espacio.

use std::rc::Rc;

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, Styled, Window, div,
};
use crate::theme;

use super::{ControlSize, text_tooltip};

/// Opción de un control segmentado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentOption {
    label: SharedString,
    tooltip: Option<SharedString>,
}

impl SegmentOption {
    /// Crea una opción visible.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            tooltip: None,
        }
    }

    /// Añade una descripción emergente.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl From<&str> for SegmentOption {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SegmentOption {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SharedString> for SegmentOption {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

/// Callback con el índice elegido.
pub type SegmentHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

/// Grupo de opciones mutuamente excluyentes.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "el callback de selección no deriva Debug"
)]
pub struct SegmentedControl {
    id: SharedString,
    options: Vec<SegmentOption>,
    selected: usize,
    size: ControlSize,
    full_width: bool,
    on_select: Option<SegmentHandler>,
}

impl SegmentedControl {
    /// Crea un control segmentado.
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            options: Vec::new(),
            selected: 0,
            size: ControlSize::Sm,
            full_width: false,
            on_select: None,
        }
    }

    /// Añade opciones en orden.
    pub fn options(mut self, options: impl IntoIterator<Item = impl Into<SegmentOption>>) -> Self {
        self.options = options.into_iter().map(Into::into).collect();
        self
    }

    /// Índice seleccionado.
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    /// Tamaño del grupo.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Reparte las opciones a lo ancho del contenedor.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Acción al elegir una opción.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ui = theme::state(cx).theme.ui;
        let metrics = theme::metrics(cx);
        let id = self.id;
        let height = self.size.height(metrics);
        let text_size = metrics.font_size * self.size.text_scale();
        let mut row = div()
            .flex()
            .items_center()
            .gap(metrics.hairline())
            .when(self.full_width, |this| this.w_full())
            .when(!self.full_width, |this| this.flex_wrap());
        for (index, option) in self.options.iter().enumerate() {
            let checked = index == self.selected;
            let focus = window
                .use_keyed_state(
                    ElementId::from(format!("{id}-focus-{index}")),
                    cx,
                    |_, cx| cx.focus_handle().tab_stop(true),
                )
                .read(cx)
                .clone();
            let focused = focus.is_focused(window);
            let click = self.on_select.clone();
            let key = self.on_select.clone();
            let mut segment = div()
                .id(ElementId::from(format!("{id}-{index}")))
                .track_focus(&focus)
                .flex()
                .items_center()
                .justify_center()
                .h(height)
                .when(self.full_width, |this| this.flex_1())
                .when(!self.full_width, |this| {
                    this.px(metrics.spacing_sm()).flex_shrink_0()
                })
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(if focused {
                    ui.focus
                } else {
                    crate::gpui::transparent_black()
                })
                .cursor_pointer()
                .text_size(text_size)
                .text_color(if checked { ui.text } else { ui.muted })
                .role(Role::Button)
                .aria_label(option.label.clone())
                .when(checked, |this| this.bg(ui.surface_active))
                .when(!checked, move |this| {
                    this.hover(move |this| this.bg(ui.surface_hover))
                        .active(move |this| this.bg(ui.surface_active))
                })
                .on_click(move |_, window, cx| {
                    if let Some(handler) = &click {
                        handler(&index, window, cx);
                    }
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && let Some(handler) = &key
                    {
                        handler(&index, window, cx);
                        cx.stop_propagation();
                    }
                })
                .child(option.label.clone());
            if let Some(tooltip) = option.tooltip.clone() {
                segment = segment.tooltip(text_tooltip(tooltip));
            }
            row = row.child(segment);
        }
        row
    }
}
