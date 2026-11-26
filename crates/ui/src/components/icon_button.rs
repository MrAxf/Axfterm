//! Botón cuadrado solo con icono.

use std::rc::Rc;

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, Styled, Window, div, transparent_black,
};
use crate::theme;

use super::button::ButtonVariant;
use super::icon::{Icon, IconSize};
use super::{ClickHandler, ControlSize, Tone, tooltip};

/// Botón cuadrado solo con icono.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct IconButton {
    id: SharedString,
    icon: &'static str,
    tooltip: Option<SharedString>,
    tone: Tone,
    variant: ButtonVariant,
    size: ControlSize,
    size_px: Option<crate::gpui::Pixels>,
    icon_size: Option<IconSize>,
    focused: bool,
    disabled: bool,
    aria_label: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    /// Crea un icon-button.
    pub fn new(id: impl Into<SharedString>, icon: &'static str) -> Self {
        Self {
            id: id.into(),
            icon,
            tooltip: None,
            tone: Tone::Neutral,
            variant: ButtonVariant::Ghost,
            size: ControlSize::Sm,
            size_px: None,
            icon_size: None,
            focused: false,
            disabled: false,
            aria_label: None,
            on_click: None,
        }
    }

    /// Tono del icono.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Variante visual.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Acción secundaria, con el mismo cuerpo que un botón relleno.
    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Filled)
    }

    /// Tamaño.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Lado fijo en píxeles, independiente del tamaño de la fuente.
    ///
    /// Para chrome de ventana que debe seguir las medidas del SO.
    pub fn size_px(mut self, size: crate::gpui::Pixels) -> Self {
        self.size_px = Some(size);
        self
    }

    /// Tamaño del icono, independiente del área pulsable.
    pub fn icon_size(mut self, size: IconSize) -> Self {
        self.icon_size = Some(size);
        self
    }

    /// Tooltip nativo.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Foco visible.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Deshabilita la acción y apaga el contraste.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Nombre accesible; si falta, se usa el texto del tooltip.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Acción al pulsar.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let accessibility_label = self.aria_label.clone().or_else(|| self.tooltip.clone());
        let color = if self.disabled {
            ui.muted
        } else {
            match self.tone {
                Tone::Neutral => ui.text,
                tone => tone.color(ui),
            }
        };
        let (background, mut border) = match self.variant {
            ButtonVariant::Filled => (ui.surface_raised, Some(ui.border)),
            ButtonVariant::Outline => match self.tone {
                Tone::Neutral => (transparent_black(), Some(ui.border)),
                tone => (transparent_black(), Some(tone.color(ui))),
            },
            ButtonVariant::Ghost => (transparent_black(), None),
        };
        if self.disabled {
            border = border.map(|border| border.opacity(0.55));
        }
        let hover = match self.variant {
            ButtonVariant::Filled => self.tone.hover(ui),
            ButtonVariant::Outline => match self.tone {
                Tone::Neutral => ui.surface_hover,
                tone => tone.color(ui).opacity(0.14),
            },
            ButtonVariant::Ghost => ui.surface_hover,
        };
        let pressed = match self.variant {
            ButtonVariant::Filled => self.tone.active(ui),
            ButtonVariant::Outline => match self.tone {
                Tone::Neutral => ui.surface_active,
                tone => tone.color(ui).opacity(0.20),
            },
            ButtonVariant::Ghost => ui.surface_active,
        };
        let border_color = if self.focused {
            ui.focus
        } else {
            border.unwrap_or_else(transparent_black)
        };
        let on_click = self.on_click.clone();
        let mut element = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .justify_center()
            .size(self.size_px.unwrap_or_else(|| self.size.height(metrics)))
            .rounded(metrics.radius_md())
            .border_1()
            .border_color(border_color)
            .bg(background)
            .text_color(color)
            .role(Role::Button)
            .when_some(accessibility_label, |this, label| this.aria_label(label))
            .when(self.disabled, |this| this.opacity(0.62))
            .child(
                Icon::new(self.icon)
                    .size(
                        self.icon_size
                            .unwrap_or_else(|| IconSize::Custom(self.size.icon(metrics))),
                    )
                    .color(color),
            );
        if !self.disabled {
            element = element
                .cursor_pointer()
                .hover(move |this| this.bg(hover))
                .active(move |this| this.bg(pressed));
        }
        if let Some(tooltip) = self.tooltip.clone() {
            element = element.tooltip(tooltip::text_tooltip(tooltip));
        }
        if !self.disabled
            && let Some(handler) = on_click
        {
            element = element.on_click(move |event, window, cx| handler(event, window, cx));
        }
        element
    }
}
