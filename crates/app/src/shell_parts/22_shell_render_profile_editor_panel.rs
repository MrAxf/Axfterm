impl Shell {
    /// Editor inline del perfil seleccionado.
    fn render_profile_editor_panel(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let editor = self.settings_profile_editor.as_ref()?;
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let name = editor.name.clone();
        let command = editor.command.clone();
        let icon = editor.icon.clone();
        let error = editor.error.clone();
        let id = editor.id.clone();
        Some(
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_sm())
                .p(metrics.spacing_sm())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.border)
                .child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("Editar «{id}»")),
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
                            TextField::new(name)
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
                            TextField::new(command)
                                .size(ControlSize::Md)
                                .aria_label("Comando"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(metrics.spacing_xs())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(metrics.font_size * 0.6875)
                                .text_color(ui.muted)
                                .child(icon.map(|icon| icon.display().to_string()).unwrap_or_else(
                                    || "Sin icono propio: se usa el del ejecutable.".into(),
                                )),
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
                                .label("Quitar icono")
                                .on_click(cx.listener(|shell, _, _, cx| {
                                    if let Some(editor) = &mut shell.settings_profile_editor {
                                        editor.icon = None;
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .when_some(error, |this, error| {
                    this.child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.danger)
                            .child(error),
                    )
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(metrics.spacing_xs())
                        .child(
                            KitButton::new("settings-profile-cancel")
                                .small()
                                .ghost()
                                .label("Cancelar")
                                .on_click(cx.listener(|shell, _, _, cx| {
                                    shell.settings_profile_editor = None;
                                    cx.notify();
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
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_settings_profiles(
        &mut self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let rows = self.render_profile_rows(width, window, cx);
        let toolbar = self.render_profile_toolbar(width, cx);
        let detect = self.render_profile_detect(cx);
        let delete_warning = self.render_profile_delete_warning(cx);
        let editor = self.render_profile_editor_panel(cx);
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
            .children(delete_warning)
            .children(editor)
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        "El id del perfil es estable y no cambia al renombrarlo. Un perfil \
                         borrado en uso deja sus pestañas sin arranque hasta reiniciarlas.",
                    ),
            )
            .into_any_element()
    }

    fn render_settings_fonts(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let sources = settings.draft.fonts.sources.clone();
        let config_dir = self.theme_host.config_dir.clone();

        let mut list = div().flex().flex_col().gap(metrics.hairline());
        if sources.is_empty() {
            list = list.child(
                div()
                    .py(metrics.spacing_sm())
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.muted)
                    .child("No hay orígenes adicionales; se usan las fuentes del sistema."),
            );
        }
        for (index, source) in sources.iter().enumerate() {
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .rounded(metrics.radius_sm())
                    .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                    .child(
                        Icon::new(icons::TYPOGRAPHY)
                            .size(IconSize::Xs)
                            .color(ui.muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.75)
                            .child(source.display().to_string()),
                    )
                    .child(
                        KitButton::new(("settings-font-remove", index))
                            .small()
                            .ghost()
                            .label("Quitar")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.remove_font_source(index);
                                }
                                shell.preview_settings(cx);
                            })),
                    ),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(metrics.spacing_xs())
                    .child(
                        KitButton::new("settings-font-file")
                            .small()
                            .label("Añadir archivo…")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.settings_add_font_sources(true, cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-font-folder")
                            .small()
                            .ghost()
                            .label("Añadir carpeta…")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.settings_add_font_sources(false, cx);
                            })),
                    ),
            )
            .child(list)
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(format!(
                        "Acepta .ttf, .otf y .ttc. Las rutas dentro de {} se guardan relativas. \
                         Máximo 16 orígenes.",
                        config_dir.display()
                    )),
            )
            .into_any_element()
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

        let capture_label = capturing
            .as_deref()
            .and_then(crate::actions::by_id)
            .map(|action| action.label.to_owned());

        let banner = capturing.as_ref().map(|action| {
            let action = action.clone();
            let unbind_action = action.clone();
            let reset_action = action.clone();
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
                            "Pulsa la combinación para «{}»; Supr la quita y Esc cancela.",
                            capture_label.as_deref().unwrap_or(&action)
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
        });

        let mut list = div().flex().flex_col().gap(metrics.hairline());
        for action in actions {
            let chords = keymap.chords_for(action.id);
            let action_id = action.id;
            let row =
                Self::settings_focusable(window, cx, format!("settings-shortcut-{action_id}"))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.begin_capture(action_id);
                        }
                        cx.notify();
                    }))
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
                    .child(match chords {
                        Some(chords) => Kbd::new(chords).into_any_element(),
                        None => div()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child("Sin atajo")
                            .into_any_element(),
                    });
            list = list.child(row);
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
            .children(banner)
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
                        "Enter sobre una acción captura la combinación. Los atajos de app \
                         requieren Ctrl, Alt o Win; Ctrl+C/D/Z, Tab, Esc y las flechas quedan \
                         para la terminal.",
                    ),
            )
            .into_any_element()
    }
}
