//! Switch con geometría en `rem` y estados de hover/pulsado propios.
//!
//! gpui-kit dibuja su switch con medidas fijas en píxeles. Este componente
//! conserva la interacción sin estilos de `gpui_base`, pero deriva pista,
//! pulgar y recorrido del tamaño base de la fuente.

use std::rc::Rc;

use crate::gpui::base::{Switch as BaseSwitch, SwitchThumb, SwitchTrack};
use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled, Window, div, px, transparent_black,
};
use crate::theme;

/// Callback con el nuevo estado.
pub type SwitchHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// Switch de Axfterm.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "el callback de cambio no deriva Debug"
)]
pub struct SwitchField {
    id: SharedString,
    checked: bool,
    disabled: bool,
    label: Option<SharedString>,
    accessibility_label: Option<SharedString>,
    on_click: Option<SwitchHandler>,
}

impl SwitchField {
    /// Crea un switch controlado.
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            disabled: false,
            label: None,
            accessibility_label: None,
            on_click: None,
        }
    }

    /// Estado actual.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Deshabilita la interacción.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Etiqueta visible a la derecha.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Nombre accesible cuando no hay etiqueta visible.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Acción con el nuevo estado.
    pub fn on_click(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SwitchField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let unit = f32::from(metrics.font_size);
        let track_w = px(unit * 2.0);
        let track_h = px(unit * 1.125);
        let inset = px(unit * 0.125);
        let thumb = track_h - inset * 2.0;
        let travel = track_w - thumb - inset * 2.0;
        let radius = track_h * 0.5;
        let focus = window
            .use_keyed_state(ElementId::from(self.id.clone()), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let disabled = self.disabled;
        let checked = self.checked;
        let rest = if checked { ui.accent } else { ui.switch_off };
        let hover = if checked {
            ui.accent_hover
        } else {
            ui.switch_off.blend(ui.text.opacity(0.10))
        };
        let pressed = if checked {
            ui.accent_active
        } else {
            ui.surface_active
        };
        let thumb_color = if disabled { ui.muted } else { ui.text };
        let label_color = if disabled { ui.muted } else { ui.text };
        let accessibility_label = self
            .accessibility_label
            .clone()
            .or_else(|| self.label.clone());
        let on_click = self.on_click.clone();

        let track = SwitchTrack::new(SharedString::from(format!("{}-track", self.id)))
            .checked(checked)
            .disabled(disabled)
            .w(track_w)
            .h(track_h)
            .flex_shrink_0()
            .rounded(radius)
            .border_1()
            .border_color(if focused {
                ui.focus
            } else {
                transparent_black()
            })
            .p(inset)
            .flex()
            .items_center()
            .bg(if disabled { rest.opacity(0.5) } else { rest })
            .when(!disabled, |this| {
                this.hover(move |this| this.bg(hover))
                    .active(move |this| this.bg(pressed))
            })
            .child(
                SwitchThumb::new(checked)
                    .rounded(radius)
                    .size(thumb)
                    .ml(if checked { travel } else { px(0.0) })
                    .bg(thumb_color),
            );

        let mut root = BaseSwitch::new(ElementId::from(self.id.clone()))
            .checked(checked)
            .disabled(disabled)
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .child(track);
        if !disabled {
            root = root.cursor_pointer();
        }
        if let Some(label) = accessibility_label {
            root = root.accessibility_label(label);
        }
        if let Some(label) = self.label {
            root = root.child(
                div()
                    .min_w_0()
                    .line_height(track_h)
                    .text_size(metrics.font_size * 0.875)
                    .text_color(label_color)
                    .child(label),
            );
        }
        if let Some(handler) = on_click {
            root = root.on_change(move |next, _: &ClickEvent, window, cx| {
                handler(&next, window, cx);
            });
        }
        root
    }
}
