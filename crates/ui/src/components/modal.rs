//! Modal con velo, tarjeta y cierre explícito.

use std::rc::Rc;

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    AnyElement, App, ClickEvent, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled, Window, div,
};
use crate::theme;

use super::{ClickHandler, ControlSize, IconButton};

/// Modal centrado sobre el contenido de la ventana.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct Modal {
    id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    body: Option<AnyElement>,
    footer: Option<AnyElement>,
    on_close: Option<ClickHandler>,
}

impl Modal {
    /// Crea un modal.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            body: None,
            footer: None,
            on_close: None,
        }
    }

    /// Texto secundario.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Contenido.
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    /// Acciones al pie.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// Cierre por velo, Esc visual o botón.
    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let close = self.on_close.clone();
        let close_button = {
            let mut button = IconButton::new(
                SharedString::from(format!("{}-close", self.id)),
                crate::icons::CLOSE,
            )
            .size(ControlSize::Sm)
            .tooltip("Cerrar");
            if let Some(handler) = self.on_close.clone() {
                button = button.on_click(move |event, window, cx| handler(event, window, cx));
            }
            button
        };

        let card = div()
            .id(format!("{}-card", self.id))
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .w(metrics.font_size * 34.0)
            .max_w(crate::gpui::relative(0.9))
            .p(metrics.spacing_lg())
            .rounded(metrics.radius_lg())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface_raised)
            .text_color(ui.text)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.spacing_xxs())
                            .child(
                                div()
                                    .text_size(metrics.font_size * 1.05)
                                    .child(self.title.clone()),
                            )
                            .when_some(self.description.clone(), |this, description| {
                                this.child(
                                    div()
                                        .text_size(metrics.font_size * 0.8125)
                                        .text_color(ui.muted)
                                        .child(description),
                                )
                            }),
                    )
                    .child(close_button),
            )
            .when_some(self.body, |this, body| this.child(body))
            .when_some(self.footer, |this, footer| this.child(footer));

        let mut overlay = div()
            .id(self.id.clone())
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(ui.overlay);
        if let Some(handler) = close {
            overlay = overlay.on_click(move |event: &ClickEvent, window, cx| {
                handler(event, window, cx);
            });
        }
        overlay.child(card)
    }
}
