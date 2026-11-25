//! Catálogo de iconos vendorizados.

use super::helpers::{note, section};
use super::section_root;
use crate::components::{Icon, IconSize};
use crate::gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use crate::theme;
use crate::{brand, icons};

pub(super) fn render(cx: &mut Context<'_, super::Gallery>) -> impl IntoElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);

    let grid = div()
        .flex()
        .flex_wrap()
        .gap(metrics.spacing_sm())
        .children(icons::ALL.iter().map(|path| {
            let name = path
                .rsplit('/')
                .next()
                .unwrap_or(path)
                .trim_end_matches(".svg");
            let color = if *path == icons::RUST {
                brand::hsla(brand::RUST)
            } else if path.starts_with("icons/simple/") {
                ui.text
            } else {
                ui.muted
            };
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(metrics.spacing_xxs())
                .w(px(104.0))
                .p(metrics.spacing_xs())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.border)
                .bg(ui.surface)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(metrics.icon_xl() * 1.7)
                        .child(Icon::new(path).size(IconSize::Lg).color(color)),
                )
                .child(
                    div()
                        .text_size(metrics.font_size * 0.5625)
                        .text_color(ui.muted)
                        .child(name.to_owned()),
                )
        }));

    let sizes = div().flex().items_end().gap(metrics.spacing_lg()).children(
        [
            ("Xs", IconSize::Xs),
            ("Sm", IconSize::Sm),
            ("Md", IconSize::Md),
            ("Lg", IconSize::Lg),
            ("Xl", IconSize::Xl),
        ]
        .into_iter()
        .map(|(label, size)| {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(metrics.spacing_xs())
                .child(Icon::new(icons::TERMINAL).size(size).color(ui.accent))
                .child(
                    div()
                        .text_size(metrics.font_size * 0.625)
                        .text_color(ui.muted)
                        .child(label),
                )
        }),
    );

    section_root(cx)
        .child(
            div()
                .text_size(metrics.font_size * 1.25)
                .child(format!("Iconos ({} vendorizados)", icons::ALL.len())),
        )
        .child(section("Tamaños", sizes, cx))
        .child(section("Tabler y marcas", grid, cx))
        .child(note(
            "Tabler se tiñe con el token de texto/acento; las marcas usan su color oficial.",
            cx,
        ))
}
