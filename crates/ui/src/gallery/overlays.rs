//! Overlays: modales, diálogos y tooltips.

use super::helpers::{card, note, section};
use super::section_root;
use crate::components::{Button, Modal, TooltipBubble};
use crate::gpui::{AppContext as _, Context, IntoElement, ParentElement, Styled, Window, div, px};
use crate::icons;
use crate::theme;

pub(super) fn render(
    gallery: &mut super::Gallery,
    _window: &mut Window,
    cx: &mut Context<'_, super::Gallery>,
) -> impl IntoElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);

    let controls = div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .child(
            Button::new("gallery-open-modal", "Abrir modal")
                .primary()
                .icon(icons::MENU)
                .on_click(cx.listener(|gallery, _, _, cx| {
                    gallery.set_modal(true);
                    cx.notify();
                })),
        )
        .child(
            Button::new("gallery-close-modal", "Cerrar")
                .ghost()
                .disabled(!gallery.modal_open())
                .on_click(cx.listener(|gallery, _, _, cx| {
                    gallery.set_modal(false);
                    cx.notify();
                })),
        );

    let modal = gallery.modal_open().then(|| {
        Modal::new("gallery-modal", "Cerrar panel con procesos vivos")
            .description("Hay 2 terminales en ejecución en este panel.")
            .body(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_sm())
                    .child("Cerrar el panel terminará sus procesos y no se podrá deshacer.")
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Los historiales se guardan al cerrar la aplicación."),
                    ),
            )
            .footer(
                div()
                    .flex()
                    .justify_end()
                    .gap(metrics.spacing_sm())
                    .child(
                        Button::new("gallery-modal-cancel", "Cancelar")
                            .ghost()
                            .on_click(cx.listener(|gallery, _, _, cx| {
                                gallery.set_modal(false);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("gallery-modal-confirm", "Cerrar panel")
                            .danger()
                            .on_click(cx.listener(|gallery, _, _, cx| {
                                gallery.set_modal(false);
                                cx.notify();
                            })),
                    ),
            )
            .on_close(cx.listener(|gallery, _, _, cx| {
                gallery.set_modal(false);
                cx.notify();
            }))
    });

    let bubble = cx
        .new(|_| TooltipBubble::new("Ctrl+Shift+P · Abrir paleta"))
        .into_any_element();
    let tooltip_card = card(
        "Tooltip",
        div()
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .child(
                Button::new("gallery-tooltip-target", "Pasa el ratón")
                    .small()
                    .ghost(),
            )
            .child(bubble),
        cx,
    );

    let stage = div()
        .relative()
        .w_full()
        .h(px(360.0))
        .rounded(metrics.radius_md())
        .border_1()
        .border_color(ui.border)
        .bg(ui.surface)
        .overflow_hidden()
        .child(
            div()
                .p(metrics.spacing_md())
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child("Área de overlay"),
        )
        .children(modal);

    section_root(cx)
        .child(div().text_size(metrics.font_size * 1.25).child("Overlays"))
        .child(section("Controles", controls, cx))
        .child(section("Modal dentro de un contenedor", stage, cx))
        .child(tooltip_card)
        .child(note(
            "El velo, la tarjeta y el foco usan el tema activo; Esc cierra el modal.",
            cx,
        ))
}
