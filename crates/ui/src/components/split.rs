//! Botón partido: acción + menú en una sola silueta.
//!
//! Usa dos botones de gpui-kit para conservar su tipografía y sus estados,
//! pero controla el cursor, las esquinas y la divisoria desde Axfterm. Las
//! dos mitades se leen como un solo control y comparten el hover en la
//! variante ghost.

use crate::gpui::component::button::{
    Button as KitButton, ButtonVariant as KitVariant, ButtonVariants as _,
};
use crate::gpui::component::menu::{DropdownMenu as _, PopupMenu};
use crate::gpui::component::{Disableable as _, Sizable as _};
use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{
    Anchor, App, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, px,
};
use crate::theme;

use super::{ControlSize, with_hand};

/// Variante visual del grupo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplitButtonVariant {
    /// Acción principal.
    Primary,
    /// Acción secundaria.
    #[default]
    Secondary,
    /// Sin fondo hasta el hover.
    Ghost,
}

type MenuBuilder =
    Box<dyn Fn(PopupMenu, &mut Window, &mut Context<'_, PopupMenu>) -> PopupMenu + 'static>;

/// Botón con una acción principal y un desplegable pegado.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "el menú es un callback y no deriva Debug"
)]
pub struct SplitButton {
    id: SharedString,
    button: Option<KitButton>,
    menu: Option<MenuBuilder>,
    variant: SplitButtonVariant,
    size: ControlSize,
    anchor: Anchor,
    menu_accessibility_label: Option<SharedString>,
    disabled: bool,
}

impl SplitButton {
    /// Crea un botón partido con identificador estable.
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            button: None,
            menu: None,
            variant: SplitButtonVariant::Secondary,
            size: ControlSize::Sm,
            anchor: Anchor::BottomLeft,
            menu_accessibility_label: None,
            disabled: false,
        }
    }

    /// Mitad de acción.
    pub fn button(mut self, button: KitButton) -> Self {
        self.button = Some(button);
        self
    }

    /// Menú de la mitad derecha.
    pub fn dropdown_menu(
        mut self,
        menu: impl Fn(PopupMenu, &mut Window, &mut Context<'_, PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        self.menu = Some(Box::new(menu));
        self
    }

    /// Menú anclado a una esquina concreta.
    pub fn dropdown_menu_with_anchor(
        mut self,
        anchor: Anchor,
        menu: impl Fn(PopupMenu, &mut Window, &mut Context<'_, PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        self.anchor = anchor;
        self.dropdown_menu(menu)
    }

    /// Variante visual.
    pub fn variant(mut self, variant: SplitButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Acción principal.
    pub fn primary(self) -> Self {
        self.variant(SplitButtonVariant::Primary)
    }

    /// Acción secundaria.
    pub fn secondary(self) -> Self {
        self.variant(SplitButtonVariant::Secondary)
    }

    /// Sin fondo hasta el hover.
    pub fn ghost(self) -> Self {
        self.variant(SplitButtonVariant::Ghost)
    }

    /// Tamaño del grupo.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Nombre accesible del disparador del menú.
    pub fn menu_accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.menu_accessibility_label = Some(label.into());
        self
    }

    /// Deshabilita el grupo completo.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for SplitButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let height = self.size.height(metrics);
        let radius = metrics.radius_md();
        let Some(button) = self.button else {
            return div().into_any_element();
        };
        let Some(menu) = self.menu else {
            return with_hand(button).into_any_element();
        };

        let id = self.id;
        let group = format!("{id}-split");
        let menu_open = window.use_keyed_state(ElementId::from(id.clone()), cx, |_, _| false);
        let is_open = *menu_open.read(cx);
        let is_ghost = self.variant == SplitButtonVariant::Ghost;
        let surfaced = !is_ghost || is_open;
        let kit_variant = match self.variant {
            SplitButtonVariant::Primary => KitVariant::Primary,
            SplitButtonVariant::Secondary => KitVariant::Secondary,
            SplitButtonVariant::Ghost => KitVariant::Ghost,
        };
        let disabled = self.disabled;
        let hover_bg = ui.surface_hover;

        let action = with_hand(button)
            .with_size(self.size.kit_size())
            .with_variant(kit_variant)
            .when(self.variant == SplitButtonVariant::Secondary, |this| {
                this.border_1().border_color(ui.border)
            })
            .disabled(disabled)
            .rounded_tl(radius)
            .rounded_tr(px(0.0))
            .rounded_bl(radius)
            .rounded_br(px(0.0));

        let caret_id = SharedString::from(format!("{id}-caret"));
        let accessibility_label = self
            .menu_accessibility_label
            .unwrap_or_else(|| SharedString::from("Más opciones"));
        let open_state = menu_open.clone();
        let caret = with_hand(
            KitButton::new(caret_id)
                .dropdown_caret(true)
                .accessibility_label(accessibility_label)
                .with_size(self.size.kit_size())
                .with_variant(kit_variant)
                .when(self.variant == SplitButtonVariant::Secondary, |this| {
                    this.border_1().border_color(ui.border)
                })
                .disabled(self.disabled)
                .rounded_tl(px(0.0))
                .rounded_tr(radius)
                .rounded_bl(px(0.0))
                .rounded_br(radius),
        )
        .dropdown_menu_with_anchor(self.anchor, menu)
        .on_open_change(move |open, _, cx| {
            open_state.update(cx, |state, cx| {
                *state = *open;
                cx.notify();
            });
        });

        let action_wrap = div()
            .flex()
            .items_center()
            .when(is_ghost && surfaced, |this| this.bg(hover_bg))
            .when(is_ghost && !surfaced, |this| {
                this.group_hover(group.clone(), move |this| this.bg(hover_bg))
            })
            .child(action);
        let caret_wrap = div()
            .flex()
            .items_center()
            .when(is_ghost && surfaced, |this| this.bg(hover_bg))
            .when(is_ghost && !surfaced, |this| {
                this.group_hover(group.clone(), move |this| this.bg(hover_bg))
            })
            .child(caret);

        let divider = div()
            .absolute()
            .top(metrics.spacing_xs())
            .bottom(metrics.spacing_xs())
            .right(height)
            .w(metrics.hairline())
            .bg(if self.variant == SplitButtonVariant::Primary {
                ui.background.opacity(0.45)
            } else {
                ui.border
            })
            .when(is_ghost && !is_open, |this| {
                this.opacity(0.0)
                    .group_hover(group.clone(), |this| this.opacity(1.0))
            });

        div()
            .relative()
            .group(group)
            .h(height)
            .flex()
            .items_center()
            .child(action_wrap)
            .child(caret_wrap)
            .child(divider)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_variants_are_stable() {
        assert_eq!(
            SplitButton::new("split").variant,
            SplitButtonVariant::Secondary
        );
        assert_eq!(SplitButton::new("split").size, ControlSize::Sm);
        assert_eq!(
            SplitButton::new("split").primary().variant,
            SplitButtonVariant::Primary
        );
        assert_eq!(
            SplitButton::new("split").ghost().variant,
            SplitButtonVariant::Ghost
        );
    }
}
