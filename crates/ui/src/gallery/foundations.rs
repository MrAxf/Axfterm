//! Fundaciones: tokens, ANSI, tipografía y medidas.

use super::helpers::{metric_row, note, section, swatch, type_row};
use super::{px_num, section_root};
use crate::gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, Window, div, px};
use crate::theme;
use crate::theme::ThemeVariant;

pub(super) fn render(
    _window: &mut Window,
    cx: &mut Context<'_, super::Gallery>,
) -> impl IntoElement {
    let state = theme::state(cx);
    let theme = &state.theme;
    let ui = theme.ui;
    let metrics = state.metrics;
    let variant = match theme.variant() {
        ThemeVariant::Dark => "Oscuro",
        ThemeVariant::Light => "Claro",
    };

    let tokens = div().flex().flex_wrap().gap(metrics.spacing_md()).children(
        [
            ("background", ui.background),
            ("surface", ui.surface),
            ("surface_raised", ui.surface_raised),
            ("surface_hover", ui.surface_hover),
            ("surface_active", ui.surface_active),
            ("switch_off", ui.switch_off),
            ("border", ui.border),
            ("border_strong", ui.border_strong),
            ("text", ui.text),
            ("muted", ui.muted),
            ("subtle", ui.subtle),
            ("accent", ui.accent),
            ("accent_hover", ui.accent_hover),
            ("accent_active", ui.accent_active),
            ("accent_text", ui.accent_text),
            ("danger", ui.danger),
            ("danger_text", ui.danger_text),
            ("warning", ui.warning),
            ("warning_text", ui.warning_text),
            ("success", ui.success),
            ("success_text", ui.success_text),
            ("info", ui.info),
            ("info_text", ui.info_text),
            ("selection", ui.selection),
            ("focus", ui.focus),
            ("overlay", ui.overlay),
        ]
        .into_iter()
        .map(|(name, color)| swatch(name, color, cx)),
    );

    let ansi_names = [
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "white",
        "br_black",
        "br_red",
        "br_green",
        "br_yellow",
        "br_blue",
        "br_magenta",
        "br_cyan",
        "br_white",
    ];
    let ansi = div().flex().flex_wrap().gap(metrics.spacing_sm()).children(
        ansi_names.into_iter().enumerate().map(|(index, name)| {
            let color = theme.terminal.ansi(index as u8);
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(metrics.spacing_xxs())
                .w(px(52.0))
                .child(
                    div()
                        .size(px(34.0))
                        .rounded(metrics.radius_sm())
                        .border_1()
                        .border_color(ui.border)
                        .bg(color),
                )
                .child(
                    div()
                        .text_size(metrics.font_size * 0.5625)
                        .text_color(ui.muted)
                        .child(format!("{index:02}")),
                )
                .child(
                    div()
                        .text_size(metrics.font_size * 0.5625)
                        .text_color(ui.subtle)
                        .child(name),
                )
        }),
    );

    let typography = div().flex().flex_col().children([
        type_row("Base UI", metrics.font_size, FontWeight::NORMAL, cx),
        type_row("Cuerpo", metrics.font_size * 0.875, FontWeight::NORMAL, cx),
        type_row(
            "Control",
            metrics.font_size * 0.8125,
            FontWeight::NORMAL,
            cx,
        ),
        type_row("Micro", metrics.font_size * 0.6875, FontWeight::NORMAL, cx),
        type_row("Título", metrics.font_size * 1.2, FontWeight::SEMIBOLD, cx),
        type_row(
            "Cabecera",
            metrics.font_size * 1.5,
            FontWeight::SEMIBOLD,
            cx,
        ),
    ]);

    let metrics_grid = div()
        .flex()
        .flex_wrap()
        .gap(metrics.spacing_lg())
        .child(div().flex().flex_col().w(px(260.0)).children([
            metric_row("font_size", format!("{:.2}", px_num(metrics.font_size)), cx),
            metric_row(
                "spacing_xxs",
                format!("{:.2}", px_num(metrics.spacing_xxs())),
                cx,
            ),
            metric_row(
                "spacing_xs",
                format!("{:.2}", px_num(metrics.spacing_xs())),
                cx,
            ),
            metric_row(
                "spacing_sm",
                format!("{:.2}", px_num(metrics.spacing_sm())),
                cx,
            ),
            metric_row(
                "spacing_md",
                format!("{:.2}", px_num(metrics.spacing_md())),
                cx,
            ),
            metric_row(
                "spacing_lg",
                format!("{:.2}", px_num(metrics.spacing_lg())),
                cx,
            ),
            metric_row(
                "spacing_xl",
                format!("{:.2}", px_num(metrics.spacing_xl())),
                cx,
            ),
            metric_row(
                "spacing_xxl",
                format!("{:.2}", px_num(metrics.spacing_xxl())),
                cx,
            ),
        ]))
        .child(div().flex().flex_col().w(px(260.0)).children([
            metric_row(
                "control_height_xs",
                format!("{:.2}", px_num(metrics.control_height_xs())),
                cx,
            ),
            metric_row(
                "control_height_sm",
                format!("{:.2}", px_num(metrics.control_height_sm())),
                cx,
            ),
            metric_row(
                "control_height_md",
                format!("{:.2}", px_num(metrics.control_height_md())),
                cx,
            ),
            metric_row(
                "control_icon_xs",
                format!("{:.2}", px_num(metrics.control_icon_xs())),
                cx,
            ),
            metric_row(
                "control_icon_sm",
                format!("{:.2}", px_num(metrics.control_icon_sm())),
                cx,
            ),
            metric_row(
                "control_icon_md",
                format!("{:.2}", px_num(metrics.control_icon_md())),
                cx,
            ),
            metric_row(
                "radius_sm",
                format!("{:.2}", px_num(metrics.radius_sm())),
                cx,
            ),
            metric_row(
                "radius_md",
                format!("{:.2}", px_num(metrics.radius_md())),
                cx,
            ),
            metric_row(
                "radius_lg",
                format!("{:.2}", px_num(metrics.radius_lg())),
                cx,
            ),
            metric_row(
                "radius_full",
                format!("{:.2}", px_num(metrics.radius_full())),
                cx,
            ),
        ]))
        .child(div().flex().flex_col().w(px(260.0)).children([
            metric_row("icon_xs", format!("{:.2}", px_num(metrics.icon_xs())), cx),
            metric_row("icon_sm", format!("{:.2}", px_num(metrics.icon_sm())), cx),
            metric_row("icon_md", format!("{:.2}", px_num(metrics.icon_md())), cx),
            metric_row("icon_lg", format!("{:.2}", px_num(metrics.icon_lg())), cx),
            metric_row("icon_xl", format!("{:.2}", px_num(metrics.icon_xl())), cx),
            metric_row(
                "tab_height",
                format!("{:.2}", px_num(metrics.tab_height())),
                cx,
            ),
            metric_row(
                "sidebar_tab_height",
                format!("{:.2}", px_num(metrics.sidebar_tab_height())),
                cx,
            ),
            metric_row(
                "titlebar_height",
                format!("{:.2}", px_num(metrics.titlebar_height())),
                cx,
            ),
        ]));

    section_root(cx)
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(metrics.spacing_md())
                .child(
                    div()
                        .text_size(metrics.font_size * 1.35)
                        .child(theme.meta.name.clone()),
                )
                .child(
                    div()
                        .text_size(metrics.font_size * 0.8125)
                        .text_color(ui.muted)
                        .child(format!("{variant} · familia {}", theme.meta.family)),
                ),
        )
        .child(section("Tokens de UI", tokens, cx))
        .child(section("Paleta ANSI del terminal", ansi, cx))
        .child(section("Tipografía", typography, cx))
        .child(section("Medidas derivadas de la fuente", metrics_grid, cx))
        .child(note(
            "Todos los tamaños salen de metrics; la galería no introduce píxeles mágicos.",
            cx,
        ))
}
