//! Toggle (switch) temado.

use std::rc::Rc;

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div,
};
use crate::theme;

use super::ToggleHandler;

/// Interruptor booleano con etiqueta opcional.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct Toggle {
    id: SharedString,
    checked: bool,
    label: Option<SharedString>,
    on_click: Option<ToggleHandler>,
}

impl Toggle {
    /// Crea un toggle.
    pub fn new(id: impl Into<SharedString>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            label: None,
            on_click: None,
        }
    }

    /// Etiqueta a la izquierda.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Acción con el nuevo valor.
    pub fn on_click(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let checked = self.checked;
        let next = !checked;
        let on_click = self.on_click.clone();
        let track_height = metrics.control_height_xs();
        let track_width = track_height * 1.85;
        let thumb = track_height * 0.72;
        let track = div()
            .flex()
            .items_center()
            .w(track_width)
            .h(track_height)
            .px(metrics.phys(2.0))
            .rounded(metrics.radius_full())
            .border_1()
            .border_color(if checked { ui.accent } else { ui.border })
            .bg(if checked {
                ui.accent
            } else {
                ui.surface_raised
            })
            .map(|this| {
                if checked {
                    this.justify_end()
                } else {
                    this.justify_start()
                }
            })
            .child(
                div()
                    .size(thumb)
                    .rounded(metrics.radius_full())
                    .bg(if checked { ui.accent_text } else { ui.muted }),
            );
        let mut element = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .cursor_pointer()
            .text_size(metrics.font_size * 0.8125)
            .text_color(ui.text)
            .child(track);
        if let Some(label) = self.label.clone() {
            element = element.child(label);
        }
        if let Some(handler) = on_click {
            element = element.on_click(move |_event: &ClickEvent, window, cx| {
                handler(&next, window, cx);
            });
        }
        element
    }
}
