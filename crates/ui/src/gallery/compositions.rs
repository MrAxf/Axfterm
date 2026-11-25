//! Composiciones: piezas reales del shell montadas con primitivas.

use super::helpers::{card, note, section};
use super::section_root;
use crate::components::{Icon, IconButton, IconSize, Kbd, StatusPill, Tab, TabStatus};
use crate::gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use crate::icons;
use crate::theme;

pub(super) fn render(cx: &mut Context<'_, super::Gallery>) -> impl IntoElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);

    let tab_strip = div()
        .flex()
        .items_center()
        .w_full()
        .border_b_1()
        .border_color(ui.border)
        .bg(ui.surface)
        .child(
            Tab::new("composition-tab-1", "pwsh")
                .icon(icons::TERMINAL)
                .active(true)
                .focused(true),
        )
        .child(
            Tab::new("composition-tab-2", "opencode")
                .icon(icons::BRAND_OPENCODE)
                .status(Some(TabStatus::Working)),
        )
        .child(
            Tab::new("composition-tab-3", "Copilot")
                .icon(icons::BRAND_COPILOT)
                .status(Some(TabStatus::Attention)),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(metrics.spacing_xxs())
                .pr(metrics.spacing_sm())
                .child(
                    IconButton::new("composition-split", icons::SPLIT_HORIZONTAL)
                        .aria_label("Dividir panel"),
                )
                .child(
                    IconButton::new("composition-zoom", icons::ZOOM)
                        .aria_label("Alternar zoom del panel"),
                )
                .child(
                    IconButton::new("composition-new", icons::PLUS).aria_label("Nueva terminal"),
                ),
        );

    let sidebar_project = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .h(metrics.sidebar_tab_height())
        .px(metrics.spacing_sm())
        .child(
            Icon::new(icons::CHEVRON_DOWN)
                .size(IconSize::Xs)
                .color(ui.muted),
        )
        .child(Icon::new(icons::FOLDER).size(IconSize::Sm).color(ui.muted))
        .child(div().flex_1().child("atlas"))
        .child(
            div()
                .text_size(metrics.font_size * 0.625)
                .text_color(ui.subtle)
                .child("4"),
        );

    let sidebar_tab = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .h(metrics.sidebar_tab_height())
        .pl(metrics.sidebar_icon_lane())
        .pr(metrics.spacing_sm())
        .rounded(metrics.radius_sm())
        .bg(ui.surface_raised)
        .child(
            Icon::new(icons::BRAND_OPENCODE)
                .size(IconSize::Sm)
                .color(ui.text),
        )
        .child(div().flex_1().child("Refactor del parser"))
        .child(StatusPill::new(TabStatus::Attention));

    let agents = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_xs())
        .child(StatusPill::new(TabStatus::Working))
        .child(StatusPill::new(TabStatus::Attention))
        .child(StatusPill::new(TabStatus::Finished));

    let titlebar = div()
        .flex()
        .items_center()
        .justify_between()
        .h(metrics.titlebar_height())
        .px(metrics.titlebar_padding())
        .bg(ui.surface)
        .child(
            div()
                .flex()
                .items_center()
                .gap(metrics.titlebar_gap())
                .child(
                    Icon::new(icons::TERMINAL)
                        .size(IconSize::Sm)
                        .color(ui.accent),
                )
                .child(
                    div()
                        .text_size(metrics.titlebar_font_size())
                        .child("Axfterm"),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(metrics.spacing_xs())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(metrics.titlebar_button())
                        .rounded(metrics.radius_sm())
                        .child(Icon::new(icons::SIDEBAR).size(IconSize::Sm).color(ui.muted)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(metrics.titlebar_button())
                        .rounded(metrics.radius_sm())
                        .child(
                            Icon::new(icons::WINDOW_MINIMIZE)
                                .size(IconSize::Xs)
                                .color(ui.muted),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(metrics.titlebar_button())
                        .rounded(metrics.radius_sm())
                        .child(
                            Icon::new(icons::WINDOW_MAXIMIZE)
                                .size(IconSize::Xs)
                                .color(ui.muted),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(metrics.titlebar_button())
                        .rounded(metrics.radius_sm())
                        .child(Icon::new(icons::CLOSE).size(IconSize::Xs).color(ui.muted)),
                ),
        );

    let settings_header = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(metrics.control_height_md())
                .rounded(metrics.radius_sm())
                .bg(ui.surface_raised)
                .child(
                    Icon::new(icons::TERMINAL)
                        .size(IconSize::Md)
                        .color(ui.accent),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().text_size(metrics.font_size * 0.875).child("Terminal"))
                .child(
                    div()
                        .text_size(metrics.font_size * 0.6875)
                        .text_color(ui.muted)
                        .child("Fuente monoespaciada y métricas de la terminal."),
                ),
        )
        .child(div().flex_1())
        .child(Kbd::new("Ctrl+Tab"));

    let palette_row = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .h(metrics.control_height_sm())
        .px(metrics.spacing_sm())
        .rounded(metrics.radius_sm())
        .bg(ui.surface_raised)
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(metrics.palette_icon_lane())
                .child(Icon::new(icons::SUN).size(IconSize::Sm).color(ui.accent)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(metrics.palette_label_size())
                        .child("Cambiar a tema claro"),
                )
                .child(
                    div()
                        .text_size(metrics.palette_detail_size())
                        .text_color(ui.muted)
                        .child("Tema y fuentes"),
                ),
        )
        .child(Kbd::new("Ctrl+Shift+L"));

    section_root(cx)
        .child(
            div()
                .text_size(metrics.font_size * 1.25)
                .child("Composiciones del shell"),
        )
        .child(section("Tab strip", tab_strip, cx))
        .child(
            div()
                .flex()
                .gap(metrics.spacing_lg())
                .items_start()
                .child(
                    div()
                        .flex_1()
                        .child(
                            card(
                                "Sidebar con agente",
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(metrics.spacing_xxs())
                                    .child(sidebar_project)
                                    .child(sidebar_tab),
                                cx,
                            ),
                        ),
                )
                .child(div().w(px(380.0)).child(card("Chips de agentes", agents, cx))),
        )
        .child(section("Título de ventana", titlebar, cx))
        .child(section("Cabecera de ajustes", settings_header, cx))
        .child(section("Fila de paleta", palette_row, cx))
        .child(note(
            "Estas composiciones son la referencia para nuevas superficies; no duplican estilos ad hoc.",
            cx,
        ))
}
