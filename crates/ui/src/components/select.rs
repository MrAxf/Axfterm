//! Selector proyectado de gpui-kit con la escala y la etiqueta de Axfterm.

use crate::gpui::component::Sizable as _;
use crate::gpui::component::searchable_list::SearchableListDelegate;
use crate::gpui::component::select::{Select, SelectState};
use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{App, Entity, IntoElement, RenderOnce, SharedString, Styled, Window};

use super::ControlSize;

/// Selector de una lista con el tamaño canónico de la aplicación.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "la entidad del estado no deriva Debug"
)]
pub struct SelectField<D: SearchableListDelegate + 'static> {
    state: Entity<SelectState<D>>,
    size: ControlSize,
    full_width: bool,
    accessibility_label: Option<SharedString>,
}

impl<D: SearchableListDelegate + 'static> SelectField<D> {
    /// Crea un selector para un `SelectState`.
    pub fn new(state: &Entity<SelectState<D>>) -> Self {
        Self {
            state: state.clone(),
            size: ControlSize::Sm,
            full_width: false,
            accessibility_label: None,
        }
    }

    /// Tamaño del control.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Ocupa todo el ancho disponible.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Nombre accesible del selector.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }
}

impl<D: SearchableListDelegate + 'static> RenderOnce for SelectField<D> {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        Select::new(&self.state)
            .with_size(self.size.kit_size())
            .cursor_pointer()
            .when(self.full_width, |select| select.w_full())
            .when_some(self.accessibility_label, |select, label| {
                select.accessibility_label(label)
            })
    }
}
