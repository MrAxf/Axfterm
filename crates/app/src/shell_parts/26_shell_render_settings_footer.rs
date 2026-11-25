impl Shell {
    /// Menú del pie con lo que antes vivía en Avanzado.
    fn render_settings_more_menu(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let config_path = self.theme_host.config_path.clone();
        let themes_dir = self.theme_host.themes_dir.clone();
        let this = cx.entity().downgrade();
        KitButton::new("settings-more")
            .small()
            .ghost()
            .label("Más")
            .tooltip("Abrir archivos de configuración y restaurar valores")
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                let config_click = config_path.clone();
                let this_config = this.clone();
                let menu = menu.item(
                    menu_path_item("Abrir config.toml", config_path.clone()).on_click(
                        move |_, _, cx| {
                            if let Some(this) = this_config.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    shell.settings_open_path(config_click.clone(), cx);
                                });
                            }
                        },
                    ),
                );
                let themes_click = themes_dir.clone();
                let this_themes = this.clone();
                let menu = menu.item(
                    menu_path_item("Abrir carpeta de temas", themes_dir.clone()).on_click(
                        move |_, _, cx| {
                            if let Some(this) = this_themes.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    shell.settings_open_path(themes_click.clone(), cx);
                                });
                            }
                        },
                    ),
                );
                let this_defaults = this.clone();
                menu.separator().item(
                    PopupMenuItem::new("Restaurar valores por defecto…").on_click(
                        move |_, window, cx| {
                            if let Some(this) = this_defaults.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    if let Some(settings) = &mut shell.settings {
                                        settings.pending_defaults = true;
                                    }
                                    window.focus(&shell.settings_profile_focus, cx);
                                    cx.notify();
                                });
                            }
                        },
                    ),
                )
            })
            .into_any_element()
    }

    /// Pie con el estado del borrador y las acciones de descartar/guardar.
    fn render_settings_footer(
        &mut self,
        cx: &mut Context<'_, Self>,
        dirty: bool,
        errors: &[String],
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let status = if let Some(first) = errors.first() {
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.danger)
                .child(if errors.len() > 1 {
                    format!("{first} (+{} errores)", errors.len() - 1)
                } else {
                    first.clone()
                })
        } else if dirty {
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child("Borrador sin guardar")
        } else {
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child("Sin cambios")
        };
        let more = self.render_settings_more_menu(cx);
        div()
            .flex()
            .w_full()
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .gap(metrics.spacing_sm())
            .px(metrics.spacing_lg())
            .py(metrics.spacing_sm())
            .border_t_1()
            .border_color(ui.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .min_w_0()
                    .child(more)
                    .child(status),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .child(
                        KitButton::new("settings-discard")
                            .small()
                            .ghost()
                            .label("Descartar")
                            .on_click(cx.listener(|shell, _, window, cx| {
                                shell.close_settings(window, cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-save")
                            .small()
                            .primary()
                            .label("Guardar")
                            .disabled(!dirty)
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.save_settings(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    /// Confirmación de restauración de valores por defecto.
    fn render_settings_defaults_confirm(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        if !self.settings.as_ref()?.pending_defaults {
            return None;
        }
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let body = div()
            .text_size(metrics.font_size * 0.8125)
            .text_color(ui.text)
            .child(
                "Se restauran apariencia, tipografía, atajos y agentes. Los perfiles y las \
                 fuentes adicionales se conservan. Guarda después para aplicar el cambio.",
            );
        let footer = div()
            .flex()
            .justify_end()
            .gap(metrics.spacing_sm())
            .child(
                KitButton::new("settings-defaults-cancel")
                    .small()
                    .ghost()
                    .label("Cancelar")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.pending_defaults = false;
                        }
                        window.focus(&shell.settings_focus, cx);
                        cx.notify();
                    })),
            )
            .child(
                KitButton::new("settings-defaults-confirm")
                    .small()
                    .danger()
                    .label("Restaurar")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.restore_defaults();
                        }
                        window.focus(&shell.settings_focus, cx);
                        shell.preview_settings(cx);
                    })),
            );
        Some(
            Modal::new("settings-defaults-modal", "Restaurar valores por defecto")
                .description("Esta acción afecta al borrador; no se escribe hasta Guardar.")
                .width(metrics.font_size * 34.0)
                .body(body)
                .footer(footer)
                .on_close(cx.listener(|shell, _, window, cx| {
                    if let Some(settings) = &mut shell.settings {
                        settings.pending_defaults = false;
                    }
                    window.focus(&shell.settings_focus, cx);
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    /// Confirmación de borrado de un perfil en uso.
    fn render_settings_delete_confirm(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let pending = self.settings.as_ref()?.pending_delete.clone()?;
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let body = div()
            .text_size(metrics.font_size * 0.8125)
            .text_color(ui.text)
            .child(format!(
                "«{}» se usa en {} pestaña(s) abiertas. Al cerrarlas no podrán arrancar.",
                pending.name, pending.in_use
            ));
        let footer = div()
            .flex()
            .justify_end()
            .gap(metrics.spacing_sm())
            .child(
                KitButton::new("settings-profile-delete-cancel")
                    .small()
                    .ghost()
                    .label("Cancelar")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.pending_delete = None;
                        }
                        window.focus(&shell.settings_focus, cx);
                        cx.notify();
                    })),
            )
            .child(
                KitButton::new("settings-profile-delete-confirm")
                    .small()
                    .danger()
                    .label("Eliminar perfil")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.confirm_delete_profile(cx);
                        window.focus(&shell.settings_focus, cx);
                    })),
            );
        Some(
            Modal::new("settings-delete-modal", "Eliminar perfil")
                .width(metrics.font_size * 32.0)
                .body(body)
                .footer(footer)
                .on_close(cx.listener(|shell, _, window, cx| {
                    if let Some(settings) = &mut shell.settings {
                        settings.pending_delete = None;
                    }
                    window.focus(&shell.settings_focus, cx);
                    cx.notify();
                }))
                .into_any_element(),
        )
    }
}

/// Elemento de menú con título y ruta secundaria.
fn menu_path_item(title: &'static str, path: Option<PathBuf>) -> PopupMenuItem {
    PopupMenuItem::element(move |_, cx| {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let path = path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "—".into());
        div()
            .flex()
            .flex_col()
            .gap(metrics.hairline())
            .child(div().text_size(metrics.font_size * 0.8125).child(title))
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(path),
            )
            .into_any_element()
    })
}
