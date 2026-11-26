//! Botón e icon-button con variantes, tamaños y foco visible.

use std::rc::Rc;

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, Styled, Window, div, relative,
};
use crate::theme;

use super::icon::{Icon, IconSize};
use super::kbd::Kbd;
use super::{ClickHandler, ControlSize, Tone, tooltip};

/// Variante visual de un botón.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Relleno con el color del tono.
    #[default]
    Filled,
    /// Borde y texto del tono.
    Outline,
    /// Sin fondo hasta el hover.
    Ghost,
}

/// Botón de texto con icono opcional.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct Button {
    id: SharedString,
    label: SharedString,
    icon: Option<&'static str>,
    tone: Tone,
    variant: ButtonVariant,
    size: ControlSize,
    focused: bool,
    shortcut: Option<SharedString>,
    full_width: bool,
    align_start: bool,
    padding_x: Option<crate::gpui::Pixels>,
    disabled: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl Button {
    /// Crea un botón con identificador estable.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            tone: Tone::Neutral,
            variant: ButtonVariant::Filled,
            size: ControlSize::Sm,
            focused: false,
            shortcut: None,
            full_width: false,
            align_start: false,
            padding_x: None,
            disabled: false,
            tooltip: None,
            on_click: None,
        }
    }

    /// Tono semántico.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Variante visual.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Acción principal.
    pub fn primary(self) -> Self {
        self.tone(Tone::Accent).variant(ButtonVariant::Filled)
    }

    /// Acción secundaria.
    pub fn secondary(self) -> Self {
        self.tone(Tone::Neutral).variant(ButtonVariant::Filled)
    }

    /// Acción destructiva.
    pub fn danger(self) -> Self {
        self.tone(Tone::Danger).variant(ButtonVariant::Filled)
    }

    /// Sin fondo.
    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    /// Tamaño.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Tamaño pequeño.
    pub fn small(self) -> Self {
        self.size(ControlSize::Sm)
    }

    /// Tamaño extrapequeño.
    pub fn xsmall(self) -> Self {
        self.size(ControlSize::Xs)
    }

    /// Icono a la izquierda.
    pub fn icon(mut self, path: &'static str) -> Self {
        self.icon = Some(path);
        self
    }

    /// Foco visible.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Atajo mostrado al final del botón.
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Ocupa todo el ancho del contenedor.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Alinea el contenido al inicio; útil en acciones de ancho completo.
    pub fn align_start(mut self) -> Self {
        self.align_start = true;
        self
    }

    /// Sobrescribe el padding horizontal del tamaño.
    pub fn padding_x(mut self, padding: crate::gpui::Pixels) -> Self {
        self.padding_x = Some(padding);
        self
    }

    /// Deshabilita la acción y apaga el contraste.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Tooltip nativo.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
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

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let (mut background, mut foreground, mut border) = match self.variant {
            ButtonVariant::Filled => match self.tone {
                Tone::Neutral => (ui.surface_raised, ui.text, Some(ui.border)),
                tone => (tone.color(ui), tone.text(ui), None),
            },
            ButtonVariant::Outline => match self.tone {
                Tone::Neutral => (crate::gpui::transparent_black(), ui.text, Some(ui.border)),
                tone => (
                    crate::gpui::transparent_black(),
                    tone.color(ui),
                    Some(tone.color(ui)),
                ),
            },
            ButtonVariant::Ghost => (crate::gpui::transparent_black(), ui.text, None),
        };
        if self.disabled {
            background = background.opacity(0.55);
            foreground = ui.muted;
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
        let on_click = self.on_click.clone();
        let icon_color = if self.disabled {
            ui.muted
        } else {
            match self.variant {
                ButtonVariant::Filled => foreground,
                _ => self.tone.color(ui),
            }
        };
        let icon_size = IconSize::Custom(self.size.icon(metrics));
        let label = div()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(self.label.clone());
        let mut leading = div()
            .flex()
            .items_center()
            .min_w_0()
            .gap(self.size.icon_gap(metrics));
        if let Some(icon) = self.icon {
            leading = leading.child(Icon::new(icon).size(icon_size).color(icon_color));
        }
        leading = leading.child(label);

        let border_color = if self.focused {
            ui.focus
        } else {
            border.unwrap_or_else(crate::gpui::transparent_black)
        };
        let mut element = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .justify_center()
            .gap(self.size.icon_gap(metrics))
            .h(self.size.height(metrics))
            .px(self
                .padding_x
                .unwrap_or_else(|| self.size.padding_x(metrics)))
            .rounded(metrics.radius_md())
            .border_1()
            .border_color(border_color)
            .bg(background)
            .text_color(foreground)
            .text_size(metrics.font_size * self.size.text_scale())
            .line_height(relative(1.25))
            .role(Role::Button)
            .aria_label(self.label.clone())
            .when_some(self.shortcut.clone(), |this, shortcut| {
                this.aria_keyshortcuts(shortcut)
            })
            .when_some(self.tooltip.clone(), |this, description| {
                this.aria_description(description)
            })
            .when(self.full_width, |this| this.w_full())
            .when(self.disabled, |this| this.opacity(0.62))
            .when_some(self.tooltip.clone(), |this, text| {
                this.tooltip(tooltip::text_tooltip(text))
            });

        if !self.disabled {
            element = element
                .cursor_pointer()
                .hover(move |this| this.bg(hover))
                .active(move |this| this.bg(pressed));
        }

        if let Some(shortcut) = self.shortcut.clone() {
            element = element
                .justify_between()
                .child(leading.flex_1())
                .child(Kbd::new(shortcut));
        } else if self.align_start {
            element = element.justify_start().child(leading);
        } else {
            element = element.child(leading);
        }

        if !self.disabled
            && let Some(handler) = on_click
        {
            element = element.on_click(move |event, window, cx| handler(event, window, cx));
        }
        element
    }
}
