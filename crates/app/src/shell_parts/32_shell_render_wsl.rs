impl Shell {
    pub(crate) fn render_wsl_open(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let Some(state) = self.wsl_open.as_ref() else {
            return div().into_any_element();
        };
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let label = |text: &'static str, metrics: UiMetrics, ui: axf_ui::theme::UiTokens| {
            div()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child(text)
        };

        let distro_row = {
            let content = if let Some(select) = &state.distro_select {
                SelectField::new(select)
                    .size(ControlSize::Sm)
                    .full_width()
                    .accessibility_label("Distribución")
                    .into_any_element()
            } else if let Some(error) = &state.error {
                div()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(ui.danger)
                    .child(error.clone())
                    .into_any_element()
            } else {
                div()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(ui.muted)
                    .child("Leyendo distribuciones…")
                    .into_any_element()
            };
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xs())
                .child(label("Distribución", metrics, ui))
                .child(content)
        };

        let profile_row = {
            let content = if let Some(error) = &state.profile_error {
                div()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(ui.danger)
                    .child(error.clone())
                    .into_any_element()
            } else if let Some(select) = &state.profile_select {
                SelectField::new(select)
                    .size(ControlSize::Sm)
                    .full_width()
                    .accessibility_label("Terminal")
                    .into_any_element()
            } else {
                div()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(ui.muted)
                    .child("Selecciona una distribución.")
                    .into_any_element()
            };
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xs())
                .child(label("Terminal", metrics, ui))
                .child(content)
        };

        let path_row = {
            let subir = div()
                .rounded(metrics.radius_md())
                .hover(move |this| this.bg(ui.surface_hover))
                .child(with_hand(
                    KitButton::new("wsl-up")
                        .small()
                        .outline()
                        .label("Subir")
                        .disabled(state.distro.is_none())
                        .on_click(cx.listener(|shell, _, window, cx| {
                            let path = shell
                                .wsl_open
                                .as_ref()
                                .map(|state| parent_posix(&state.path))
                                .unwrap_or_else(|| "/".into());
                            shell.navigate_wsl(path, window, cx);
                        })),
                ));
            let inicio = div()
                .rounded(metrics.radius_md())
                .hover(move |this| this.bg(ui.surface_hover))
                .child(with_hand(
                    KitButton::new("wsl-home")
                        .small()
                        .outline()
                        .label("Inicio")
                        .disabled(state.distro.is_none())
                        .on_click(cx.listener(|shell, _, window, cx| {
                            if let Some(home) = shell
                                .wsl_open
                                .as_ref()
                                .and_then(|state| state.home.clone())
                            {
                                shell.navigate_wsl(home, window, cx);
                            }
                        })),
                ));
            let ir = div()
                .rounded(metrics.radius_md())
                .hover(move |this| this.bg(ui.surface_hover))
                .child(with_hand(
                    KitButton::new("wsl-go")
                        .small()
                        .outline()
                        .label("Ir")
                        .disabled(state.distro.is_none())
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.wsl_navigate_from_input(window, cx);
                        })),
                ));
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xs())
                .child(label("Carpeta", metrics, ui))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(metrics.spacing_xs())
                        .child(
                            div().flex_1().min_w_0().child(
                                TextField::new(state.path_input.clone())
                                    .size(ControlSize::Sm)
                                    .full_width()
                                    .aria_label("Carpeta"),
                            ),
                        )
                        .child(subir)
                        .child(inicio)
                        .child(ir),
                )
        };

        let list = div()
            .h(metrics.font_size * 15.0)
            .min_h_0()
            .border_1()
            .border_color(ui.border)
            .rounded(metrics.radius_md())
            .bg(ui.surface)
            .overflow_hidden()
            .child(List::new(&state.list).scrollbar_visible(true));

        let status = state.error.as_ref().map(|error| {
            div()
                .text_size(metrics.font_size * 0.8125)
                .text_color(ui.danger)
                .child(error.clone())
        });

        let body = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .w_full()
            .child(distro_row)
            .child(profile_row)
            .child(path_row)
            .children(status)
            .child(list);

        let path_value = state.path_input.read(cx).value().trim().to_owned();
        let can_open = !state.opening
            && !state.loading_distros
            && state.distro.is_some()
            && state.profile_id.is_some()
            && !path_value.is_empty();
        let footer = div()
            .flex()
            .items_center()
            .justify_end()
            .gap(metrics.spacing_sm())
            .child(with_hand(
                KitButton::new("wsl-cancel")
                    .small()
                    .ghost()
                    .label("Cancelar")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.close_wsl_modal(window, cx);
                    })),
            ))
            .child(with_hand(
                KitButton::new("wsl-open")
                    .small()
                    .primary()
                    .label("Abrir")
                    .disabled(!can_open)
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.open_wsl_current(window, cx);
                    })),
            ));

        let modal = Modal::new("wsl-open-modal", "Abrir proyecto en WSL")
            .width(metrics.font_size * 52.0)
            .description("Elige la distribución, su terminal y la carpeta del proyecto.")
            .body(body)
            .footer(footer)
            .on_close(cx.listener(|shell, _, window, cx| {
                shell.close_wsl_modal(window, cx);
            }));

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context(crate::WSL_CONTEXT)
            .track_focus(&state.focus)
            .tab_group()
            .on_key_down(cx.listener(|shell, event: &KeyDownEvent, window, cx| {
                shell.wsl_key(event, window, cx);
            }))
            .child(modal)
            .focus_trap("wsl-open-trap", &state.focus)
            .into_any_element()
    }
}
