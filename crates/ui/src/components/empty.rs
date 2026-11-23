//! Empty state accionable.

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
};
use crate::theme;

use super::icon::{Icon, IconSize};

/// Estado vacío con icono, mensaje y una acción primaria.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct EmptyState {
    icon: &'static str,
    title: SharedString,
    description: SharedString,
    action: Option<AnyElement>,
}

impl EmptyState {
    /// Crea un empty state.
    pub fn new(
        icon: &'static str,
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
    ) -> Self {
        Self {
            icon,
            title: title.into(),
            description: description.into(),
            action: None,
        }
    }

    /// Acción principal.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(metrics.spacing_md())
            .size_full()
            .bg(ui.background)
            .text_color(ui.muted)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(metrics.icon_xl() * 2.0)
                    .rounded(metrics.radius_lg())
                    .border_1()
                    .border_color(ui.border)
                    .bg(ui.surface)
                    .child(Icon::new(self.icon).size(IconSize::Xl).color(ui.accent)),
            )
            .child(
                div()
                    .text_size(metrics.font_size * 1.05)
                    .text_color(ui.text)
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .max_w(metrics.font_size * 26.0)
                    .text_center()
                    .text_size(metrics.font_size * 0.8125)
                    .child(self.description.clone()),
            )
            .when_some(self.action, |this, action| this.child(action))
    }
}
