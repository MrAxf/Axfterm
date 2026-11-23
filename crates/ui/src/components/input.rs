//! Campo de texto: envoltorio de `Input` de gpui-kit con tamaños del sistema.

use crate::gpui::component::Sizable as _;
use crate::gpui::component::input::{Input, InputState};
use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{App, Entity, IntoElement, RenderOnce, Window};
use crate::theme;

use super::ControlSize;
use super::icon::{Icon, IconSize};

/// Campo de texto de una o varias líneas.
#[derive(Debug, IntoElement)]
pub struct TextField {
    state: Entity<InputState>,
    size: ControlSize,
    appearance: bool,
    cleanable: bool,
    prefix_icon: Option<&'static str>,
    aria_label: Option<&'static str>,
}

impl TextField {
    /// Crea un campo para un `InputState`.
    pub fn new(state: Entity<InputState>) -> Self {
        Self {
            state,
            size: ControlSize::Sm,
            appearance: true,
            cleanable: false,
            prefix_icon: None,
            aria_label: None,
        }
    }

    /// Tamaño del control.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Sin borde ni fondo (renombrado inline).
    pub fn borderless(mut self) -> Self {
        self.appearance = false;
        self
    }

    /// Botón para limpiar el contenido.
    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.cleanable = cleanable;
        self
    }

    /// Icono decorativo al inicio del campo.
    pub fn prefix_icon(mut self, path: &'static str) -> Self {
        self.prefix_icon = Some(path);
        self
    }

    /// Nombre accesible del campo.
    pub fn aria_label(mut self, label: &'static str) -> Self {
        self.aria_label = Some(label);
        self
    }
}

impl RenderOnce for TextField {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ui = theme::state(cx).theme.ui;
        let input = Input::new(&self.state)
            .appearance(self.appearance)
            .cleanable(self.cleanable)
            .when_some(self.aria_label, |input, label| input.aria_label(label))
            .when_some(self.prefix_icon, |input, path| {
                input.prefix(Icon::new(path).size(IconSize::Sm).color(ui.muted))
            });
        match self.size {
            ControlSize::Xs | ControlSize::Sm => input.xsmall(),
            ControlSize::Md | ControlSize::Lg => input.small(),
        }
    }
}
