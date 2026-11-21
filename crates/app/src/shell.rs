//! Shell placeholder de M0: sidebar, área central y barra de estado.
//!
//! Es deliberadamente mínimo: no monta terminales ni el modelo de workspace.
//! Sirve para validar ventana, tema, assets y dispatch de acciones.

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, rems, svg,
};

use crate::icons;
use crate::{Quit, ToggleSidebar, WORKSPACE_CONTEXT};

/// Vista raíz de la ventana.
pub(crate) struct Shell {
    sidebar_visible: bool,
    focus: FocusHandle,
}

impl Shell {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        Self {
            sidebar_visible: true,
            focus,
        }
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    fn quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<'_, Self>) {
        cx.quit();
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // Los tokens son `Copy`; se extraen antes de pedir listeners (que
        // necesitan `&mut cx`) para no mantener el préstamo del tema.
        let (background, foreground, muted_foreground, border, sidebar_background) = {
            let theme = cx.theme();
            (
                theme.background,
                theme.foreground,
                theme.muted_foreground,
                theme.border,
                theme.sidebar,
            )
        };

        let sidebar = self.sidebar_visible.then(|| {
            div()
                .h_full()
                .w(rems(15.0))
                .flex_shrink_0()
                .border_r_1()
                .border_color(border)
                .bg(sidebar_background)
        });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .key_context(WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::quit))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .children(sidebar)
                    .child(div().flex_1().min_w_0()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(rems(1.75))
                    .px_3()
                    .border_t_1()
                    .border_color(border)
                    .text_color(muted_foreground)
                    .child(svg().path(icons::TERMINAL).size(rems(0.875)))
                    .child("Axfterm"),
            )
    }
}
