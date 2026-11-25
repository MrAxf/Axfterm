//! Temas incluidos y muestras de sus tokens.

use super::helpers::{note, section, swatch};
use super::section_root;
use crate::gpui::{Context, IntoElement, ParentElement, Styled, Window, div, px};
use crate::theme;
use crate::theme::{ThemeVariant, builtin};

pub(super) fn render(
    _window: &mut Window,
    cx: &mut Context<'_, super::Gallery>,
) -> impl IntoElement {
    let state = theme::state(cx);
    let ui = state.theme.ui;
    let metrics = state.metrics;

    let cards = div().flex().flex_wrap().gap(metrics.spacing_md()).children(
        builtin::BUILTIN_THEMES.iter().map(|entry| {
            let theme = builtin::builtin(entry.name).unwrap_or_else(theme::Theme::fallback);
            let variant = match theme.variant() {
                ThemeVariant::Dark => "Oscuro",
                ThemeVariant::Light => "Claro",
            };
            let active = theme.meta.name == state.theme.meta.name;
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_sm())
                .w(px(250.0))
                .p(metrics.spacing_md())
                .rounded(metrics.radius_md())
                .border_1()
                .border_color(if active { ui.accent } else { ui.border })
                .bg(if active {
                    ui.surface_raised
                } else {
                    ui.surface
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(metrics.font_size * 0.875)
                                .child(theme.meta.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(metrics.font_size * 0.625)
                                .text_color(ui.muted)
                                .child(variant),
                        ),
                )
                .child(
                    div().flex().gap(metrics.spacing_xxs()).children(
                        [
                            theme.ui.background,
                            theme.ui.surface,
                            theme.ui.accent,
                            theme.ui.danger,
                            theme.ui.success,
                            theme.ui.info,
                        ]
                        .into_iter()
                        .map(|color| {
                            div()
                                .flex_1()
                                .h(px(24.0))
                                .rounded(metrics.radius_sm())
                                .bg(color)
                        }),
                    ),
                )
                .child(swatch("texto", theme.ui.text, cx))
                .child(swatch("acento", theme.ui.accent, cx))
        }),
    );

    section_root(cx)
        .child(div().text_size(metrics.font_size * 1.25).child("Temas"))
        .child(section("Built-in", cards, cx))
        .child(note(
            "Los temas de usuario siguen el mismo esquema TOML y aparecen en ajustes.",
            cx,
        ))
}
