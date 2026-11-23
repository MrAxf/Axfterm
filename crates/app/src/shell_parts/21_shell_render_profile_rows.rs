impl Shell {
    /// Filas seleccionables de perfiles dentro de los ajustes.
    fn render_profile_rows(
        &mut self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let profiles = self
            .settings
            .as_ref()
            .map(|settings| settings.profiles().to_vec())
            .unwrap_or_default();
        let cursor = self
            .settings
            .as_ref()
            .map(|settings| settings.profile_cursor)
            .unwrap_or(0);
        let default_profile = self
            .settings
            .as_ref()
            .map(|settings| settings.draft.terminal.default_profile.clone());
        let icons: Vec<ProfileIconSource> = profiles
            .iter()
            .map(|profile| self.theme_host.profile_icon(&profile.id))
            .collect();
        let mut list = div()
            .flex()
            .flex_col()
            .w(width)
            .max_w(width)
            .min_w_0()
            .gap(metrics.hairline());
        let profile_count = profiles.len();
        for (index, profile) in profiles.iter().enumerate() {
            let selected = index == cursor;
            let is_default = default_profile.as_ref() == Some(&profile.id);
            let row =
                Self::settings_focusable(window, cx, format!("settings-profile-{}", profile.id))
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .w(width)
                    .max_w(width)
                    .min_w_0()
                    .when(selected, |this| this.bg(ui.selection))
                    .when(!selected, |this| {
                        this.hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                    })
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.profile_cursor = index;
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "up" => {
                                if let Some(settings) = &mut shell.settings {
                                    settings.profile_cursor =
                                        settings.profile_cursor.saturating_sub(1);
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }
                            "down" => {
                                if let Some(settings) = &mut shell.settings {
                                    settings.profile_cursor = (settings.profile_cursor + 1)
                                        .min(profile_count.saturating_sub(1));
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }
                            "enter" | "space" => {
                                if let Some(settings) = &mut shell.settings {
                                    settings.profile_cursor = index;
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }
                            _ => {}
                        }
                    }))
                    .child(
                        ProfileIcon::new(icons[index].clone())
                            .size(IconSize::Sm)
                            .color(ui.text),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.hairline())
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.75)
                                    .child(profile.name.clone()),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(metrics.font_size * 0.6875)
                                    .text_color(ui.muted)
                                    .child(profile.command.clone()),
                            ),
                    )
                    .when(is_default, |this| {
                        this.child(
                            div()
                                .text_size(metrics.font_size * 0.625)
                                .text_color(ui.accent)
                                .child("Predeterminado"),
                        )
                    });
            list = list.child(row);
        }
        list.into_any_element()
    }

    /// Botones para añadir, editar, duplicar, mover, marcar o borrar perfiles.
    fn render_profile_toolbar(&mut self, width: Pixels, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let metrics = state.metrics;
        let selected_id = self
            .settings
            .as_ref()
            .and_then(|settings| settings.profiles().get(settings.profile_cursor))
            .map(|profile| profile.id.clone());
        let running = matches!(
            self.settings.as_ref().map(|settings| &settings.detect),
            Some(DetectState::Running)
        );
        let mut toolbar = div()
            .flex()
            .flex_wrap()
            .w(width)
            .max_w(width)
            .min_w_0()
            .gap(metrics.spacing_xs());
        toolbar = toolbar
            .child(
                KitButton::new("settings-profile-add")
                    .small()
                    .label("Añadir")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.add_profile(window, cx);
                    })),
            )
            .child(
                KitButton::new("settings-profile-detect")
                    .small()
                    .ghost()
                    .label("Detectar terminales")
                    .disabled(running)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.start_detect(cx);
                    })),
            )
            .when_some(selected_id, |this, id| {
                let edit_id = id.clone();
                let duplicate_id = id.clone();
                let up_id = id.clone();
                let down_id = id.clone();
                this.child(
                    KitButton::new("settings-profile-edit")
                        .small()
                        .label("Editar")
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.open_profile_editor(edit_id.clone(), window, cx);
                        })),
                )
                .child(
                    KitButton::new("settings-profile-duplicate")
                        .small()
                        .ghost()
                        .label("Duplicar")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            let result = shell
                                .settings
                                .as_mut()
                                .map(|settings| settings.duplicate_profile(&duplicate_id));
                            if let Some(Err(error)) = result {
                                shell.set_notice(error, true, cx);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-profile-up")
                        .small()
                        .ghost()
                        .label("Subir")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.move_profile(&up_id, -1);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-profile-down")
                        .small()
                        .ghost()
                        .label("Bajar")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.move_profile(&down_id, 1);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-profile-default")
                        .small()
                        .ghost()
                        .label("Predeterminado")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                if let Err(error) = settings.set_default_profile(&id) {
                                    settings.set_notice(error, true);
                                }
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-profile-delete")
                        .small()
                        .danger()
                        .label("Eliminar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.request_delete_profile(cx);
                        })),
                )
            });
        toolbar.into_any_element()
    }

    /// Resultado del descubrimiento local de terminales.
    fn render_profile_detect(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut detect = div().flex().flex_col().gap(metrics.spacing_xs());
        if let Some(settings) = self.settings.as_ref() {
            match &settings.detect {
                DetectState::Running => {
                    detect = detect.child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Buscando terminales en PATH…"),
                    );
                }
                DetectState::Ready(found) => {
                    let mut found_list = div().flex().flex_col().gap(metrics.hairline());
                    for profile in found {
                        found_list = found_list.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(metrics.spacing_sm())
                                .text_size(metrics.font_size * 0.75)
                                .child(
                                    Icon::new(icons::TERMINAL)
                                        .size(IconSize::Xs)
                                        .color(ui.muted),
                                )
                                .child(profile.name.clone())
                                .child(
                                    div()
                                        .text_size(metrics.font_size * 0.6875)
                                        .text_color(ui.muted)
                                        .child(profile.command.clone()),
                                ),
                        );
                    }
                    detect = detect.child(found_list).child(
                        div()
                            .flex()
                            .gap(metrics.spacing_xs())
                            .child(
                                KitButton::new("settings-profile-detect-add")
                                    .small()
                                    .primary()
                                    .label(format!("Añadir {}", found.len()))
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        shell.merge_detected(cx);
                                    })),
                            )
                            .child(
                                KitButton::new("settings-profile-detect-discard")
                                    .small()
                                    .ghost()
                                    .label("Descartar")
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        if let Some(settings) = &mut shell.settings {
                                            settings.detect = DetectState::Idle;
                                        }
                                        cx.notify();
                                    })),
                            ),
                    );
                }
                DetectState::Idle => {}
            }
        }
        detect.into_any_element()
    }

    /// Confirmación al borrar un perfil en uso.
    fn render_profile_delete_warning(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let pending = self
            .settings
            .as_ref()
            .and_then(|settings| settings.pending_delete.clone())?;
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        Some(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(metrics.spacing_sm())
                .p(metrics.spacing_sm())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.warning)
                .text_size(metrics.font_size * 0.75)
                .child(format!(
                    "«{}» se usa en {} pestaña(s) abiertas. Al cerrarlas no podrán arrancar.",
                    pending.name, pending.in_use
                ))
                .child(
                    KitButton::new("settings-profile-delete-confirm")
                        .small()
                        .danger()
                        .label("Eliminar perfil")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.confirm_delete_profile(cx);
                        })),
                )
                .child(
                    KitButton::new("settings-profile-delete-cancel")
                        .small()
                        .ghost()
                        .label("Cancelar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.pending_delete = None;
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}
