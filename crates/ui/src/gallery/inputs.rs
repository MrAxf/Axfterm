//! Entradas: campos propios y controles proyectados de gpui-kit.

use super::helpers::{card, note, section};
use super::section_root;
use crate::components::{
    ControlSize, SegmentedControl, SelectField, SwitchField, TextField, with_hand,
};
use crate::gpui::component::Disableable as _;
use crate::gpui::component::button::{Button as KitButton, ButtonVariants as _};
use crate::gpui::{Context, IntoElement, ParentElement, Styled, Window, div, px};
use crate::icons;
use crate::theme;

pub(super) fn render(
    gallery: &mut super::Gallery,
    _window: &mut Window,
    cx: &mut Context<'_, super::Gallery>,
) -> impl IntoElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);

    let fields = div()
        .flex()
        .flex_col()
        .gap(metrics.spacing_md())
        .child(
            div()
                .flex()
                .gap(metrics.spacing_md())
                .child(
                    div()
                        .flex_1()
                        .child(TextField::new(gallery.first_input.clone()).size(ControlSize::Md)),
                )
                .child(
                    div().flex_1().child(
                        TextField::new(gallery.second_input.clone())
                            .size(ControlSize::Md)
                            .prefix_icon(icons::SEARCH),
                    ),
                ),
        )
        .child(
            div()
                .flex()
                .gap(metrics.spacing_md())
                .child(
                    div().flex_1().child(
                        TextField::new(gallery.first_input.clone())
                            .size(ControlSize::Sm)
                            .cleanable(true),
                    ),
                )
                .child(
                    div().flex_1().child(
                        TextField::new(gallery.second_input.clone())
                            .size(ControlSize::Sm)
                            .borderless(),
                    ),
                ),
        );

    let kit = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(metrics.spacing_md())
        .child(with_hand(
            KitButton::new("gallery-kit-primary")
                .primary()
                .label("Primary"),
        ))
        .child(with_hand(
            KitButton::new("gallery-kit-secondary")
                .secondary()
                .label("Secondary"),
        ))
        .child(with_hand(
            KitButton::new("gallery-kit-ghost").ghost().label("Ghost"),
        ))
        .child(with_hand(
            KitButton::new("gallery-kit-danger")
                .danger()
                .label("Danger"),
        ))
        .child(with_hand(
            KitButton::new("gallery-kit-disabled")
                .disabled(true)
                .label("Disabled"),
        ));

    let switches = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(metrics.spacing_lg())
        .child(
            SwitchField::new("gallery-switch-on")
                .checked(true)
                .label("Encendido"),
        )
        .child(
            SwitchField::new("gallery-switch-off")
                .checked(false)
                .label("Apagado"),
        )
        .child(
            SwitchField::new("gallery-switch-disabled")
                .disabled(true)
                .label("Deshabilitado"),
        );

    let selectors = div()
        .flex()
        .gap(metrics.spacing_lg())
        .items_start()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xs())
                .w(px(300.0))
                .child(
                    div()
                        .text_size(metrics.font_size * 0.625)
                        .text_color(ui.subtle)
                        .child("Select"),
                )
                .child(
                    SelectField::new(&gallery.select)
                        .size(ControlSize::Sm)
                        .accessibility_label("Select"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xs())
                .child(
                    div()
                        .text_size(metrics.font_size * 0.625)
                        .text_color(ui.subtle)
                        .child("Segmentado"),
                )
                .child(
                    SegmentedControl::new("gallery-segmented")
                        .options(["Oscuro", "Claro", "Sistema"])
                        .selected(0),
                ),
        );

    let focus_note = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .text_size(metrics.font_size * 0.75)
        .text_color(ui.muted)
        .child(
            div()
                .size(px(14.0))
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.focus),
        )
        .child("El anillo de foco usa ui.focus; nunca cambia el tamaño del control.");

    section_root(cx)
        .child(
            div()
                .text_size(metrics.font_size * 1.25)
                .child("Entradas de texto"),
        )
        .child(section("TextField", fields, cx))
        .child(section("Botones de gpui-kit", kit, cx))
        .child(section("Switches", switches, cx))
        .child(section("Select y segmentado", selectors, cx))
        .child(card("Foco", focus_note, cx))
        .child(note(
            "Input, Select y TextField comparten el tema proyectado desde axf_ui.",
            cx,
        ))
}
