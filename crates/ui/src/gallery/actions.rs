//! Acciones: botones, icon buttons, toggles y teclas.

use super::helpers::{card, note, section};
use super::section_root;
use crate::components::{Button, ButtonVariant, ControlSize, IconButton, Kbd, SwitchField, Tone};
use crate::gpui::{Context, IntoElement, ParentElement, Styled, Window, div, px};
use crate::icons;
use crate::theme;

pub(super) fn render(
    gallery: &mut super::Gallery,
    _window: &mut Window,
    cx: &mut Context<'_, super::Gallery>,
) -> impl IntoElement {
    let state = theme::state(cx);
    let ui = state.theme.ui;
    let metrics = state.metrics;
    let tones = [
        (Tone::Neutral, "Neutral"),
        (Tone::Accent, "Accent"),
        (Tone::Danger, "Danger"),
        (Tone::Warning, "Warning"),
        (Tone::Success, "Success"),
        (Tone::Info, "Info"),
    ];
    let variants = [
        (ButtonVariant::Filled, "Filled"),
        (ButtonVariant::Outline, "Outline"),
        (ButtonVariant::Ghost, "Ghost"),
    ];

    let matrix =
        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .children(variants.into_iter().map(|(variant, variant_label)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.625)
                            .text_color(ui.subtle)
                            .child(variant_label),
                    )
                    .child(div().flex().flex_wrap().gap(metrics.spacing_xs()).children(
                        tones.into_iter().map(move |(tone, tone_label)| {
                            Button::new(format!("gallery-{variant_label}-{tone_label}"), tone_label)
                                .tone(tone)
                                .variant(variant)
                                .icon(icons::CHECK)
                        }),
                    ))
            }));

    let sizes = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(metrics.spacing_sm())
        .child(Button::new("gallery-xs", "Xs").xsmall())
        .child(Button::new("gallery-sm", "Small"))
        .child(Button::new("gallery-md", "Medium").size(ControlSize::Md))
        .child(Button::new("gallery-shortcut", "Con atajo").shortcut("Ctrl+K"))
        .child(
            Button::new("gallery-focused", "Foco")
                .primary()
                .focused(true),
        )
        .child(Button::new("gallery-disabled", "Disabled").disabled(true));

    let icon_buttons = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(metrics.spacing_sm())
        .child(IconButton::new("gallery-ib-sm", icons::SEARCH).tooltip("Buscar"))
        .child(
            IconButton::new("gallery-ib-md", icons::PALETTE)
                .size(ControlSize::Md)
                .tooltip("Tema"),
        )
        .child(
            IconButton::new("gallery-ib-filled", icons::PLUS)
                .secondary()
                .size(ControlSize::Md)
                .aria_label("Añadir"),
        )
        .child(
            IconButton::new("gallery-ib-danger", icons::CLOSE)
                .tone(Tone::Danger)
                .variant(ButtonVariant::Outline)
                .aria_label("Cerrar"),
        )
        .child(
            IconButton::new("gallery-ib-focus", icons::KEYBOARD)
                .focused(true)
                .size(ControlSize::Md)
                .aria_label("Foco de teclado"),
        );

    let toggles = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(metrics.spacing_lg())
        .child(
            SwitchField::new("gallery-toggle-on")
                .checked(true)
                .label("Activado")
                .on_click(cx.listener(|gallery, checked, _, cx| {
                    gallery.toggle = *checked;
                    cx.notify();
                })),
        )
        .child(
            SwitchField::new("gallery-toggle-off")
                .checked(false)
                .label("Desactivado")
                .on_click(cx.listener(|gallery, checked, _, cx| {
                    gallery.toggle = *checked;
                    cx.notify();
                })),
        )
        .child(
            SwitchField::new("gallery-toggle-live")
                .checked(gallery.toggle)
                .label("Estado vivo")
                .on_click(cx.listener(|gallery, checked, _, cx| {
                    gallery.toggle = *checked;
                    cx.notify();
                })),
        );

    let keys = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(metrics.spacing_sm())
        .children(
            ["Ctrl+T", "Ctrl+Shift+P", "Esc", "Tab", "Alt+F4", "F1"]
                .into_iter()
                .map(Kbd::new),
        );

    section_root(cx)
        .child(
            div()
                .text_size(metrics.font_size * 1.25)
                .child("Acciones y controles"),
        )
        .child(section("Matriz de botones", matrix, cx))
        .child(section("Tamaños y estados", sizes, cx))
        .child(section("Icon buttons", icon_buttons, cx))
        .child(
            div()
                .flex()
                .gap(metrics.spacing_lg())
                .items_start()
                .child(div().flex_1().child(section("Switches", toggles, cx)))
                .child(div().w(px(300.0)).child(card("Atajos", keys, cx))),
        )
        .child(note(
            "ControlSize comparte altura, texto, icono y padding con Size de gpui-kit: \
             Xs/XSmall, Sm/Small y Md/Medium.",
            cx,
        ))
}
