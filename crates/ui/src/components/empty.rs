//! Empty state accionable.

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div,
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
    compact: bool,
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
            compact: false,
        }
    }

    /// Variante compacta para regiones pequeñas: sin fondo de página ni alto completo.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
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
        let mut root = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(if self.compact {
                metrics.spacing_sm()
            } else {
                metrics.spacing_md()
            })
            .text_color(ui.muted);
        if !self.compact {
            root = root.size_full().bg(ui.background);
        }

        let title = div()
            .text_size(if self.compact {
                metrics.font_size * 0.9375
            } else {
                metrics.font_size * 1.05
            })
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(ui.text)
            .child(self.title.clone());

        let root = if self.compact {
            // Fila de una sola línea: el icono no compite con el texto y el
            // estado vacío cabe incluso a 200 % de escala.
            root.child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_xs())
                    .child(Icon::new(self.icon).size(IconSize::Sm).color(ui.muted))
                    .child(title),
            )
        } else {
            root.child(
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
            .child(title)
        };

        root.child(
            div()
                .max_w(metrics.font_size * 26.0)
                .text_center()
                .text_size(metrics.font_size * 0.8125)
                .child(self.description.clone()),
        )
        .when_some(self.action, |this, action| this.child(action))
    }
}
