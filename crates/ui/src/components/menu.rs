//! Menú presentacional (listas de acciones y selectores).

use std::rc::Rc;

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div,
};
use crate::theme;

use super::ClickHandler;
use super::icon::{Icon, IconSize};

/// Acción de un menú.
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct MenuItem {
    label: SharedString,
    icon: Option<&'static str>,
    shortcut: Option<SharedString>,
    checked: bool,
    on_click: Option<ClickHandler>,
}

impl MenuItem {
    /// Crea una entrada.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            checked: false,
            on_click: None,
        }
    }

    /// Icono a la izquierda.
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Atajo visible a la derecha.
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Marca la entrada como activa.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Acción.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

enum Entry {
    Item(MenuItem),
    Separator,
}

/// Lista vertical de acciones.
#[derive(Default, IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct Menu {
    entries: Vec<Entry>,
}

impl Menu {
    /// Menú vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade una entrada.
    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }

    /// Añade un separador.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
}

impl RenderOnce for Menu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut column = div().flex().flex_col().w_full().gap(metrics.spacing_xxs());
        for (index, entry) in self.entries.into_iter().enumerate() {
            match entry {
                Entry::Separator => {
                    column = column.child(
                        div()
                            .h(metrics.hairline())
                            .my(metrics.spacing_xxs())
                            .bg(ui.border),
                    );
                }
                Entry::Item(item) => {
                    let checked = item.checked;
                    let handler = item.on_click.clone();
                    let mut row = div()
                        .id(("menu-item", index))
                        .flex()
                        .items_center()
                        .gap(metrics.spacing_sm())
                        .h(metrics.control_height_sm())
                        .px(metrics.spacing_sm())
                        .rounded(metrics.radius_sm())
                        .cursor_pointer()
                        .text_size(metrics.font_size * 0.8125)
                        .text_color(if checked { ui.accent } else { ui.text })
                        .when(checked, |this| this.bg(ui.surface))
                        .hover(|this| this.bg(ui.surface));
                    if let Some(icon) = item.icon {
                        row = row.child(Icon::new(icon).size(IconSize::Sm).color(if checked {
                            ui.accent
                        } else {
                            ui.muted
                        }));
                    }
                    row = row.child(div().flex_1().child(item.label.clone()));
                    if let Some(shortcut) = item.shortcut {
                        row = row.child(
                            div()
                                .text_size(metrics.font_size * 0.6875)
                                .text_color(ui.subtle)
                                .child(shortcut),
                        );
                    }
                    if checked {
                        row = row.child(
                            Icon::new(crate::icons::CHECK)
                                .size(IconSize::Sm)
                                .color(ui.accent),
                        );
                    }
                    if let Some(handler) = handler {
                        row = row.on_click(move |event, window, cx| handler(event, window, cx));
                    }
                    column = column.child(row);
                }
            }
        }
        column
    }
}
