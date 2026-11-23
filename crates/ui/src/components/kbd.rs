//! Chip de combinación de teclas.
//!
//! Se usa dentro de botones y menús para mostrar un atajo sin convertirlo en
//! texto plano. El tamaño deriva de las métricas, igual que el resto de
//! controles.

use crate::gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div};
use crate::theme;

/// Combinación de teclas mostrada como una tecla física.
#[derive(Debug, IntoElement)]
pub struct Kbd {
    label: SharedString,
}

impl Kbd {
    /// Crea el chip con la combinación ya formateada.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;

        div()
            .flex()
            .items_center()
            .justify_center()
            .h(metrics.control_height_xs())
            .flex_shrink_0()
            .px(metrics.spacing_xs())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(ui.border)
            .bg(ui.background)
            .text_color(ui.muted)
            .text_size(metrics.font_size * 0.625)
            .font_family(state.fonts.mono.clone())
            .whitespace_nowrap()
            .child(self.label)
    }
}
