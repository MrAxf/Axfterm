//! Navegación: pestañas, menús, sidebar y filas de paleta.

use super::helpers::{card, note, section};
use super::section_root;
use crate::components::{Icon, IconSize, Kbd, Menu, MenuItem, Tab, TabStatus};
use crate::gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use crate::icons;
use crate::theme;

pub(super) fn render(cx: &mut Context<'_, super::Gallery>) -> impl IntoElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);

    let tabs = div()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(ui.border)
        .child(
            Tab::new("gallery-tab-terminal", "Terminal")
                .icon(icons::TERMINAL)
                .active(true)
                .focused(true)
                .closable(true),
        )
        .child(
            Tab::new("gallery-tab-opencode", "opencode")
                .icon(icons::BRAND_OPENCODE)
                .status(Some(TabStatus::Working)),
        )
        .child(
            Tab::new("gallery-tab-copilot", "Copilot")
                .icon(icons::BRAND_COPILOT)
                .status(Some(TabStatus::Attention)),
        )
        .child(
            Tab::new("gallery-tab-idle", "pwsh")
                .icon(icons::TERMINAL)
                .status(Some(TabStatus::Idle)),
        )
        .child(
            Tab::new("gallery-tab-error", "Error")
                .icon(icons::TERMINAL)
                .status(Some(TabStatus::Error)),
        );

    let menu = div().w(px(280.0)).child(
        Menu::new()
            .item(
                MenuItem::new("Nueva terminal")
                    .icon(icons::PLUS)
                    .shortcut("Ctrl+T"),
            )
            .item(
                MenuItem::new("Dividir a la derecha")
                    .icon(icons::SPLIT_HORIZONTAL)
                    .shortcut("Ctrl+Shift+D"),
            )
            .item(MenuItem::new("Tema oscuro").icon(icons::MOON).checked(true))
            .separator()
            .item(
                MenuItem::new("Cerrar pestaña")
                    .icon(icons::CLOSE)
                    .shortcut("Ctrl+W"),
            ),
    );

    let project_row = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .h(metrics.sidebar_tab_height())
        .px(metrics.spacing_sm())
        .rounded(metrics.radius_sm())
        .bg(ui.surface_raised)
        .text_size(metrics.font_size * 0.8125)
        .child(
            Icon::new(icons::CHEVRON_DOWN)
                .size(IconSize::Xs)
                .color(ui.muted),
        )
        .child(Icon::new(icons::FOLDER).size(IconSize::Sm).color(ui.muted))
        .child(div().flex_1().min_w_0().child("axfterm"))
        .child(
            div()
                .text_size(metrics.font_size * 0.625)
                .text_color(ui.subtle)
                .child("3"),
        );

    let tab_row = div()
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
        .child(div().flex_1().min_w_0().child("opencode"))
        .child(Icon::new(icons::LOADER).size(IconSize::Xs).color(ui.accent))
        .child(Kbd::new("Ctrl+2"));

    let palette_row = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .h(metrics.control_height_sm())
        .px(metrics.spacing_sm())
        .rounded(metrics.radius_sm())
        .bg(ui.surface)
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(metrics.palette_icon_lane())
                .child(
                    Icon::new(icons::SETTINGS)
                        .size(IconSize::Sm)
                        .color(ui.accent),
                ),
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
                        .child("Abrir ajustes"),
                )
                .child(
                    div()
                        .text_size(metrics.palette_detail_size())
                        .text_color(ui.muted)
                        .child("Aplicación"),
                ),
        )
        .child(Kbd::new("Ctrl+,"));

    let settings_row = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .h(metrics.sidebar_tab_height())
        .px(metrics.spacing_sm())
        .rounded(metrics.radius_sm())
        .bg(ui.surface_raised)
        .child(
            Icon::new(icons::KEYBOARD)
                .size(IconSize::Sm)
                .color(ui.accent),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(div().text_size(metrics.font_size * 0.8125).child("Atajos"))
                .child(
                    div()
                        .text_size(metrics.font_size * 0.625)
                        .text_color(ui.muted)
                        .child("Teclas de la aplicación y conflictos"),
                ),
        )
        .child(
            Icon::new(icons::CHEVRON_RIGHT)
                .size(IconSize::Xs)
                .color(ui.subtle),
        );

    section_root(cx)
        .child(
            div()
                .text_size(metrics.font_size * 1.25)
                .child("Navegación"),
        )
        .child(section("Pestañas", tabs, cx))
        .child(
            div()
                .flex()
                .gap(metrics.spacing_lg())
                .items_start()
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(metrics.spacing_md())
                        .child(card("Menú contextual", menu, cx))
                        .child(card("Fila de paleta", palette_row, cx)),
                )
                .child(
                    div()
                        .w(px(320.0))
                        .flex()
                        .flex_col()
                        .gap(metrics.spacing_md())
                        .child(card(
                            "Sidebar",
                            div()
                                .flex()
                                .flex_col()
                                .gap(metrics.spacing_xxs())
                                .child(project_row)
                                .child(tab_row),
                            cx,
                        ))
                        .child(card("Fila de ajustes", settings_row, cx)),
                ),
        )
        .child(note(
            "Las filas reproducen la composición del shell con los mismos tokens y métricas.",
            cx,
        ))
}
