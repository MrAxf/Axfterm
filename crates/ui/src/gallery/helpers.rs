//! Piezas comunes de la galería: tarjetas, filas y muestras de color.

use crate::gpui::{AnyElement, App, Hsla, IntoElement, ParentElement, Pixels, Styled, div, px};
use crate::theme;

/// Tarjeta con título y cuerpo.
pub(super) fn card(title: impl Into<&'static str>, body: impl IntoElement, cx: &App) -> AnyElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);
    div()
        .flex()
        .flex_col()
        .gap(metrics.spacing_sm())
        .p(metrics.spacing_md())
        .rounded(metrics.radius_md())
        .border_1()
        .border_color(ui.border)
        .bg(ui.surface)
        .child(
            div()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child(title.into()),
        )
        .child(body)
        .into_any_element()
}

/// Bloque con título de sección y contenido.
pub(super) fn section(
    title: impl Into<&'static str>,
    body: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);
    div()
        .flex()
        .flex_col()
        .gap(metrics.spacing_sm())
        .child(
            div()
                .text_size(metrics.font_size * 0.8125)
                .text_color(ui.muted)
                .child(title.into()),
        )
        .child(body)
        .into_any_element()
}

/// Fila simple etiqueta/valor.
pub(super) fn metric_row(
    label: impl Into<String>,
    value: impl Into<String>,
    cx: &App,
) -> AnyElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(metrics.spacing_md())
        .py(metrics.spacing_xxs())
        .text_size(metrics.font_size * 0.75)
        .child(div().text_color(ui.muted).child(label.into()))
        .child(div().text_color(ui.text).child(value.into()))
        .into_any_element()
}

/// Muestra de color con nombre y hex.
pub(super) fn swatch(name: &'static str, color: Hsla, cx: &App) -> AnyElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);
    let hex = theme::hsla_hex(color);
    div()
        .flex()
        .items_center()
        .gap(metrics.spacing_xs())
        .min_w(px(120.0))
        .child(
            div()
                .size(px(18.0))
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.border)
                .bg(color),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(
                    div()
                        .text_size(metrics.font_size * 0.6875)
                        .text_color(ui.text)
                        .child(name),
                )
                .child(
                    div()
                        .text_size(metrics.font_size * 0.625)
                        .text_color(ui.subtle)
                        .child(hex),
                ),
        )
        .into_any_element()
}

/// Texto atenuado pequeño.
pub(super) fn note(text: impl Into<String>, cx: &App) -> AnyElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);
    div()
        .text_size(metrics.font_size * 0.6875)
        .text_color(ui.muted)
        .child(text.into())
        .into_any_element()
}

/// Caja de una muestra de tipografía.
pub(super) fn type_row(
    label: &'static str,
    size: Pixels,
    weight: crate::gpui::FontWeight,
    cx: &App,
) -> AnyElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);
    div()
        .flex()
        .items_baseline()
        .gap(metrics.spacing_md())
        .py(metrics.spacing_xxs())
        .child(
            div()
                .w(px(160.0))
                .text_size(metrics.font_size * 0.625)
                .text_color(ui.subtle)
                .child(label),
        )
        .child(
            div()
                .text_size(size)
                .font_weight(weight)
                .text_color(ui.text)
                .child("Axfterm — terminal y agentes"),
        )
        .into_any_element()
}
