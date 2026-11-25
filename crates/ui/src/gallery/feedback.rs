//! Feedback: estados, avisos y estados vacíos.

use super::helpers::{card, note, section};
use super::section_root;
use crate::components::{Button, EmptyState, StatusPill, TabStatus, Toast};
use crate::gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use crate::icons;
use crate::theme;

pub(super) fn render(cx: &mut Context<'_, super::Gallery>) -> impl IntoElement {
    let ui = theme::state(cx).theme.ui;
    let metrics = theme::metrics(cx);

    let pills = div().flex().flex_wrap().gap(metrics.spacing_sm()).children(
        [
            TabStatus::Running,
            TabStatus::Working,
            TabStatus::Attention,
            TabStatus::Finished,
            TabStatus::Error,
            TabStatus::Idle,
            TabStatus::Stopped,
        ]
        .into_iter()
        .map(StatusPill::new),
    );

    let toasts = div()
        .flex()
        .flex_wrap()
        .items_start()
        .gap(metrics.spacing_md())
        .child(
            Toast::new("La configuración se recargó.").detail("Tema Monokai aplicado en caliente."),
        )
        .child(
            Toast::error("No se pudo iniciar el perfil.")
                .detail("El ejecutable `pwsh` no está en PATH."),
        );

    let empty = div()
        .flex()
        .gap(metrics.spacing_md())
        .items_start()
        .child(
            div().flex_1().child(
                EmptyState::new(
                    icons::FOLDER_OPEN,
                    "Abre una carpeta",
                    "Registra un proyecto para reunir sus terminales en la sidebar.",
                )
                .action(
                    Button::new("gallery-empty-action", "Abrir carpeta")
                        .primary()
                        .icon(icons::FOLDER_OPEN),
                ),
            ),
        )
        .child(
            div()
                .w(px(320.0))
                .p(metrics.spacing_md())
                .rounded(metrics.radius_md())
                .border_1()
                .border_color(ui.border)
                .child(
                    EmptyState::new(
                        icons::SEARCH,
                        "Sin resultados",
                        "Prueba con otro término o cambia el filtro.",
                    )
                    .compact(),
                ),
        );

    section_root(cx)
        .child(div().text_size(metrics.font_size * 1.25).child("Feedback"))
        .child(section("Estados de terminal y agente", pills, cx))
        .child(section("Avisos", toasts, cx))
        .child(section("Estados vacíos", empty, cx))
        .child(card(
            "Regla",
            note(
                "El color nunca es la única señal: cada estado lleva icono y etiqueta.",
                cx,
            ),
            cx,
        ))
}
