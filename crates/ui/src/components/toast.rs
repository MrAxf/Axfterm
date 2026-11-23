//! Aviso flotante.

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div};
use crate::theme;

use super::Tone;
use super::icon::{Icon, IconSize};

/// Aviso flotante de ancho fijo con texto que envuelve y detalle opcional.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct Toast {
    text: SharedString,
    detail: Option<SharedString>,
    tone: Tone,
    icon: &'static str,
}

impl Toast {
    /// Crea un toast informativo.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            detail: None,
            tone: Tone::Info,
            icon: crate::icons::INFO_CIRCLE,
        }
    }

    /// Toast de error.
    pub fn error(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            detail: None,
            tone: Tone::Danger,
            icon: crate::icons::ALERT_TRIANGLE,
        }
    }

    /// Detalle secundario.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let tone = self.tone.color(ui);
        div()
            .flex()
            .items_start()
            .gap(metrics.spacing_sm())
            // Un ancho explícito deja al texto un espacio definido donde
            // envolver; con solo `max_w` el texto se mide sin límite y se sale
            // por el borde derecho del aviso.
            .w(metrics.toast_width())
            .p(metrics.spacing_md())
            .rounded(metrics.radius_md())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface_raised)
            .text_color(ui.text)
            .text_size(metrics.font_size * 0.8125)
            .child(Icon::new(self.icon).size(IconSize::Md).color(tone))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(metrics.spacing_xxs())
                    .child(self.text.clone())
                    .when_some(self.detail, |this, detail| {
                        this.child(
                            div()
                                .text_size(metrics.font_size * 0.75)
                                .text_color(ui.muted)
                                .child(detail),
                        )
                    }),
            )
    }
}
