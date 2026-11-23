//! Tooltip nativo de GPUI con el estilo de Axfterm.

use crate::gpui::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement, Render, SharedString,
    Styled, Window, div,
};
use crate::theme;

/// Burbuja de tooltip.
struct TooltipBubble {
    text: SharedString,
}

impl Render for TooltipBubble {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        div()
            .px(metrics.spacing_sm())
            .py(metrics.spacing_xs())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface_raised)
            .text_color(ui.text)
            .text_size(metrics.font_size * 0.75)
            .child(self.text.clone())
    }
}

/// Constructor de tooltip para `Interactivity::tooltip`.
pub fn text_tooltip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_window, cx| {
        let text = text.clone();
        cx.new(|_| TooltipBubble { text }).into()
    }
}
