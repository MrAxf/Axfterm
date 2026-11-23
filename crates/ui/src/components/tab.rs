//! Pestaña: icono, título, estado y cierre.

use std::rc::Rc;

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div,
};
use crate::theme;

use super::icon::{Icon, IconSize, ProfileIcon, ProfileIconSource};
use super::pill::TabStatus;
use super::{ClickHandler, ControlSize, IconButton};

/// Pestaña presentacional. El drag, el drop y el menú contextual los añade la
/// superficie que la contiene.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct Tab {
    id: SharedString,
    title: SharedString,
    icon: Option<&'static str>,
    image_icon: Option<std::path::PathBuf>,
    active: bool,
    focused: bool,
    status: Option<TabStatus>,
    closable: bool,
    full_width: bool,
    on_close: Option<ClickHandler>,
}

impl Tab {
    /// Crea una pestaña.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            image_icon: None,
            active: false,
            focused: false,
            status: None,
            closable: true,
            full_width: false,
            on_close: None,
        }
    }

    /// Icono de la pestaña.
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Icono raster/SVG de un perfil; sustituye al icono Tabler.
    pub fn image_icon(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.image_icon = Some(path.into());
        self
    }

    /// Icono con origen explícito.
    pub fn icon_source(mut self, source: ProfileIconSource) -> Self {
        match source {
            ProfileIconSource::Svg(path) => {
                self.icon = Some(path);
                self.image_icon = None;
            }
            ProfileIconSource::Image(path) => {
                self.image_icon = Some(path);
            }
        }
        self
    }

    /// Pestaña activa.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Pestaña activa en el panel enfocado.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Estado de proceso/agente.
    pub fn status(mut self, status: Option<TabStatus>) -> Self {
        self.status = status;
        self
    }

    /// Permite cerrar.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Ocupa todo el ancho de su contenedor (tab strip con ancho fijo).
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Acción de cierre.
    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Tab {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let active = self.active;
        let focused = self.focused && active;
        let status = self.status;
        let active_background = if focused {
            ui.surface_raised
        } else {
            ui.surface_raised.opacity(0.55)
        };
        let hover_background = ui.surface_raised.opacity(0.35);
        let icon_color = if focused { ui.accent } else { ui.muted };
        let text_color = if active && focused { ui.text } else { ui.muted };
        let icon_source = self
            .image_icon
            .clone()
            .map(ProfileIconSource::Image)
            .or_else(|| self.icon.map(ProfileIconSource::Svg));

        let ring_color = if active {
            active_background
        } else {
            ui.surface
        };
        let offset = -metrics.agent_badge_offset();
        let ring = metrics.hairline();
        let badge_inner = metrics.agent_badge();
        let badge_outer = badge_inner + ring * 2.0;

        let mut element = div()
            .id(self.id.clone())
            .when(!self.title.is_empty(), |this| {
                this.tooltip(super::tooltip::text_tooltip(self.title.clone()))
            })
            .flex()
            .items_center()
            .gap(metrics.spacing_xs())
            .h(metrics.tab_height())
            .px(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .overflow_hidden()
            .text_color(text_color)
            .when(self.full_width, |this| this.w_full())
            .when(active, |this| this.bg(active_background))
            .when(!active, |this| {
                this.hover(move |this| this.bg(hover_background))
            })
            .when_some(icon_source, |this, source| {
                this.child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(metrics.tab_icon())
                        .flex_shrink_0()
                        .child(
                            ProfileIcon::new(source)
                                .size(IconSize::Custom(metrics.tab_icon()))
                                .color(icon_color),
                        )
                        .when_some(status, |this, status| {
                            this.child(
                                div()
                                    .id(format!("{}-status", self.id))
                                    .absolute()
                                    .right(offset)
                                    .bottom(offset)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(badge_outer)
                                    .p(ring)
                                    .rounded_full()
                                    .bg(ring_color)
                                    .tooltip(super::tooltip::text_tooltip(status.label()))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(badge_inner)
                                            .rounded_full()
                                            .bg(status.tone().color(ui))
                                            .child(
                                                Icon::new(status.icon())
                                                    .size(IconSize::Custom(
                                                        metrics.agent_badge_icon(),
                                                    ))
                                                    .color(status.tone().text(ui))
                                                    .spin(status.is_spinning()),
                                            ),
                                    ),
                            )
                        }),
                )
            });

        element = element.child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(self.title.clone()),
        );

        if self.closable {
            let on_close = self.on_close.clone();
            let mut close = IconButton::new(
                SharedString::from(format!("{}-close", self.id)),
                crate::icons::CLOSE,
            )
            .size(ControlSize::Xs)
            .icon_size(IconSize::Md)
            .tooltip("Cerrar pestaña");
            if let Some(handler) = on_close {
                close = close.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx);
                });
            }
            element = element.child(close);
        }
        element
    }
}
