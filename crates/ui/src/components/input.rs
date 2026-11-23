//! Campo de texto: envoltorio de `Input` de gpui-kit con tamaños del sistema.

use crate::gpui::component::Sizable as _;
use crate::gpui::component::input::{Input, InputState};
use crate::gpui::{App, Entity, IntoElement, RenderOnce, Window};

use super::ControlSize;

/// Campo de texto de una o varias líneas.
#[derive(Debug, IntoElement)]
pub struct TextField {
    state: Entity<InputState>,
    size: ControlSize,
    appearance: bool,
    cleanable: bool,
}

impl TextField {
    /// Crea un campo para un `InputState`.
    pub fn new(state: Entity<InputState>) -> Self {
        Self {
            state,
            size: ControlSize::Sm,
            appearance: true,
            cleanable: false,
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
}

impl RenderOnce for TextField {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let input = Input::new(&self.state)
            .appearance(self.appearance)
            .cleanable(self.cleanable);
        match self.size {
            ControlSize::Xs | ControlSize::Sm => input.xsmall(),
            ControlSize::Md | ControlSize::Lg => input.small(),
        }
    }
}
