impl Shell {
    fn render_settings_profiles(
        &mut self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let metrics = state.metrics;
        let rows = self.render_profile_rows(width, window, cx);
        let toolbar = self.render_profile_toolbar(width, cx);
        let detect = self.render_profile_detect(cx);
        div()
            .flex()
            .flex_col()
            .w(width)
            .max_w(width)
            .min_w_0()
            .gap(metrics.spacing_md())
            .child(toolbar)
            .child(rows)
            .child(detect)
            .into_any_element()
    }

    /// Modal de alta o edición de un perfil.
    fn render_settings_profile_modal(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let editor = self.settings_profile_editor.as_ref()?;
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let name_input = editor.name.clone();
        let command_input = editor.command.clone();
        let icon = editor.icon.clone();
        let error = editor.error.clone();
        let title = match &editor.mode {
            ProfileEditorMode::New(_) => "Nuevo perfil".to_owned(),
            ProfileEditorMode::Edit(id) => {
                let name = self
                    .settings
                    .as_ref()
                    .and_then(|settings| {
                        settings
                            .profiles()
                            .iter()
                            .find(|profile| &profile.id == id)
                    })
                    .map(|profile| profile.name.clone())
                    .unwrap_or_else(|| id.to_string());
                format!("Editar «{name}»")
            }
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(metrics.control_height_lg())
                            .rounded(metrics.radius_md())
                            .bg(ui.surface_raised)
                            .child(Icon::new(icons::USER).size(IconSize::Md).color(ui.muted)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child(
                                icon.as_ref()
                                    .map(|icon| icon.display().to_string())
                                    .unwrap_or_else(|| {
                                        "Sin icono propio: se usa el del ejecutable.".into()
                                    }),
                            ),
                    )
                    .child(
                        KitButton::new("settings-profile-icon")
                            .small()
                            .ghost()
                            .label("Elegir icono…")
                            .on_click(cx.listener(|shell, _, window, cx| {
                                shell.settings_choose_icon(window, cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-profile-icon-clear")
                            .small()
                            .ghost()
                            .label("Quitar")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                if let Some(editor) = &mut shell.settings_profile_editor {
                                    editor.icon = None;
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child("Nombre visible"),
                    )
                    .child(
                        TextField::new(name_input)
                            .size(ControlSize::Md)
                            .aria_label("Nombre"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child("Comando (ejecutable y argumentos)"),
                    )
                    .child(
                        TextField::new(command_input)
                            .size(ControlSize::Md)
                            .aria_label("Comando"),
                    ),
            )
            .when_some(error, |this, error| {
                this.child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.danger)
                        .child(error),
                )
            });
        let footer = div()
            .flex()
            .justify_end()
            .gap(metrics.spacing_sm())
            .child(
                KitButton::new("settings-profile-cancel")
                    .small()
                    .ghost()
                    .label("Cancelar")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.close_profile_editor(cx);
                        window.focus(&shell.settings_focus, cx);
                    })),
            )
            .child(
                KitButton::new("settings-profile-apply")
                    .small()
                    .primary()
                    .label("Aplicar")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.apply_profile_editor(cx);
                    })),
            );
        Some(
            Modal::new("settings-profile-modal", title)
                .description("Los cambios se aplican al seleccionar Aplicar.")
                .width(metrics.font_size * 36.0)
                .body(body)
                .footer(footer)
                .on_close(cx.listener(|shell, _, window, cx| {
                    shell.close_profile_editor(cx);
                    window.focus(&shell.settings_focus, cx);
                }))
                .into_any_element(),
        )
    }

    fn render_settings_shortcuts(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let _ = window;
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let actions = settings.shortcut_actions();
        let keymap = Keymap::resolve(&settings.draft.keymap);
        let capturing = settings.capturing.clone();
        let input = self.settings_shortcut_input.clone();

        let mut list = div().flex().flex_col().gap(metrics.spacing_xxs());
        for category in crate::actions::ActionCategory::ALL {
            let group: Vec<_> = actions
                .iter()
                .copied()
                .filter(|action| action.category == *category)
                .collect();
            if group.is_empty() {
                continue;
            }
            list = list.child(
                div()
                    .pt(metrics.spacing_sm())
                    .pb(metrics.spacing_xxs())
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(category.label()),
            );
            for action in group {
                let action_id = action.id;
                if capturing.as_deref() == Some(action_id) {
                    list = list.child(self.render_shortcut_capture(action_id, cx));
                    continue;
                }                let chords = keymap.chords_for(action_id);
                let row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .p(metrics.spacing_sm())
                    .rounded(metrics.radius_sm())
                    .border_1()
                    .border_color(ui.border)
                    .bg(ui.surface)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.hairline())
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.75)
                                    .child(action.label),
                            )
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.625)
                                    .text_color(ui.muted)
                                    .child(action.id),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_shrink_0()
                            .gap(metrics.spacing_sm())
                            .child(match chords {
                                Some(chords) => Kbd::new(chords).into_any_element(),
                                None => div()
                                    .text_size(metrics.font_size * 0.6875)
                                    .text_color(ui.muted)
                                    .child("Sin atajo")
                                    .into_any_element(),
                            })
                            .child(
                                KitButton::new((
                                    ElementId::from("settings-shortcut-edit"),
                                    action_id,
                                ))
                                .small()
                                .ghost()
                                .label("Editar")
                                    .on_click(cx.listener(move |shell, _, _, cx| {
                                        if let Some(settings) = &mut shell.settings {
                                            settings.begin_capture(action_id);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    );
                list = list.child(row);
            }
        }

        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .when_some(input, |this, input| {
                this.child(
                    TextField::new(input)
                        .size(ControlSize::Md)
                        .prefix_icon(icons::SEARCH)
                        .cleanable(true)
                        .aria_label("Filtrar atajos"),
                )
            })
            .child(
                div()
                    .id("settings-shortcut-list")
                    .max_h(metrics.font_size * 26.0)
                    .overflow_y_scroll()
                    .child(list),
            )
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        "Editar captura la siguiente combinación. Supr la quita y Esc cancela. \
                         Los atajos de app requieren Ctrl, Alt o Win; Ctrl+C/D/Z, Tab, Esc y las \
                         flechas quedan para la terminal.",
                    ),
            )
            .into_any_element()
    }

    /// Fila en estado de captura para una acción.
    fn render_shortcut_capture(
        &mut self,
        action_id: &str,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let label = crate::actions::by_id(action_id)
            .map(|action| action.label)
            .unwrap_or(action_id);
        let unbind_action = action_id.to_owned();
        let reset_action = action_id.to_owned();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(metrics.spacing_sm())
            .p(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(ui.accent)
            .bg(ui.surface_raised)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(metrics.font_size * 0.75)
                    .child(format!(
                        "Pulsa la combinación para «{label}»; Esc cancela y Supr la quita."
                    )),
            )
            .child(
                KitButton::new("settings-shortcut-unbind")
                    .small()
                    .danger()
                    .label("Quitar atajo")
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings
                            && let Err(error) = settings.unbind_action(&unbind_action)
                        {
                            settings.set_notice(error, true);
                        }
                        cx.notify();
                    })),
            )
            .child(
                KitButton::new("settings-shortcut-reset")
                    .small()
                    .ghost()
                    .label("Restaurar")
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings
                            && let Err(error) = settings.reset_action(&reset_action)
                        {
                            settings.set_notice(error, true);
                        }
                        cx.notify();
                    })),
            )
            .child(
                KitButton::new("settings-shortcut-cancel")
                    .small()
                    .ghost()
                    .label("Cancelar")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.cancel_capture();
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
