//! Botón e icon-button con variantes, tamaños y foco visible.

use std::rc::Rc;

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, Styled, Window, div, transparent_black,
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
            size: ControlSize::Md,
            focused: false,
            shortcut: None,
            full_width: false,
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
                Tone::Neutral => (ui.surface_raised, ui.text, None),
                tone => (tone.color(ui), tone.text(ui), None),
            },
            ButtonVariant::Outline => (
                crate::gpui::transparent_black(),
                self.tone.color(ui),
                Some(self.tone.color(ui)),
            ),
            ButtonVariant::Ghost => (crate::gpui::transparent_black(), ui.text, None),
        };
        if self.disabled {
            background = background.opacity(0.55);
            foreground = ui.muted;
            border = border.map(|border| border.opacity(0.55));
        }
        let hover = match self.variant {
            ButtonVariant::Filled => background.opacity(0.88),
            ButtonVariant::Outline => self.tone.color(ui).opacity(0.12),
            ButtonVariant::Ghost => ui.surface_raised,
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
        let icon_size = match self.size {
            ControlSize::Xs => IconSize::Xs,
            ControlSize::Sm => IconSize::Sm,
            ControlSize::Md => IconSize::Md,
            ControlSize::Lg => IconSize::Lg,
        };
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
            .gap(metrics.spacing_xxs());
        if let Some(icon) = self.icon {
            leading = leading.child(Icon::new(icon).size(icon_size).color(icon_color));
        }
        leading = leading.child(label);

        let mut element = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .justify_center()
            .gap(metrics.spacing_xxs())
            .h(self.size.height(metrics))
            .px(self.size.padding_x(metrics))
            .rounded(metrics.radius_md())
            .bg(background)
            .text_color(foreground)
            .text_size(metrics.font_size * self.size.text_scale())
            .role(Role::Button)
            .aria_label(self.label.clone())
            .when_some(self.shortcut.clone(), |this, shortcut| {
                this.aria_keyshortcuts(shortcut)
            })
            .when_some(self.tooltip.clone(), |this, description| {
                this.aria_description(description)
            })
            .when(self.full_width, |this| this.w_full())
            .when_some(border, |this, border| this.border_1().border_color(border))
            .when(self.focused, |this| this.border_1().border_color(ui.focus))
            .when(self.disabled, |this| this.opacity(0.62))
            .when_some(self.tooltip.clone(), |this, text| {
                this.tooltip(tooltip::text_tooltip(text))
            });

        if !self.disabled {
            element = element.cursor_pointer().hover(move |this| this.bg(hover));
        }

        if let Some(shortcut) = self.shortcut.clone() {
            element = element
                .justify_between()
                .child(leading.flex_1())
                .child(Kbd::new(shortcut));
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
        let color = match self.tone {
            Tone::Neutral => ui.text,
            tone => tone.color(ui),
        };
        let (background, border) = match self.variant {
            ButtonVariant::Filled => (ui.surface_raised, None),
            ButtonVariant::Outline => (transparent_black(), Some(self.tone.color(ui))),
            ButtonVariant::Ghost => (transparent_black(), None),
        };
        let hover = match self.variant {
            ButtonVariant::Filled => background.opacity(0.88),
            ButtonVariant::Outline => self.tone.color(ui).opacity(0.12),
            ButtonVariant::Ghost => ui.surface_raised,
        };
        let radius = match self.variant {
            ButtonVariant::Ghost => metrics.radius_sm(),
            ButtonVariant::Filled | ButtonVariant::Outline => metrics.radius_md(),
        };
        let on_click = self.on_click.clone();
        let mut element = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .justify_center()
            .size(self.size_px.unwrap_or_else(|| self.size.height(metrics)))
            .rounded(radius)
            .bg(background)
            .when_some(border, |this, border| this.border_1().border_color(border))
            .role(Role::Button)
            .when_some(accessibility_label, |this, label| this.aria_label(label))
            .cursor_pointer()
            .hover(move |this| this.bg(hover))
            .when(self.focused, |this| this.border_1().border_color(ui.focus))
            .child(
                Icon::new(self.icon)
                    .size(self.icon_size.unwrap_or_else(|| self.size.icon_size()))
                    .color(color),
            );
        if let Some(tooltip) = self.tooltip.clone() {
            element = element.tooltip(tooltip::text_tooltip(tooltip));
        }
        if let Some(handler) = on_click {
            element = element.on_click(move |event, window, cx| handler(event, window, cx));
        }
        element
    }
}
