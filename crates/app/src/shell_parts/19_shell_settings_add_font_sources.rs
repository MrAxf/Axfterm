impl Shell {
    /// Pide archivos o carpetas de fuentes y los añade al borrador.
    fn settings_add_font_sources(&mut self, files: bool, cx: &mut Context<'_, Self>) {
        let receiver = cx.prompt_for_paths(axf_ui::gpui::PathPromptOptions {
            files,
            directories: !files,
            multiple: true,
            prompt: Some(if files {
                "Añadir archivos de fuente".into()
            } else {
                "Añadir carpeta de fuentes".into()
            }),
        });
        let config_dir = self.theme_host.config_dir.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                let result = this.update(cx, |shell, cx| {
                    let mut added = 0;
                    let mut duplicate = false;
                    if let Some(settings) = &mut shell.settings {
                        for path in paths {
                            if settings.add_font_source(&path, &config_dir) {
                                added += 1;
                            } else {
                                duplicate = true;
                            }
                        }
                        if added > 0 {
                            settings.set_notice(
                                format!("{added} orígenes de fuentes añadidos al borrador."),
                                false,
                            );
                        } else if duplicate {
                            settings.set_notice(
                                "Esos orígenes ya estaban en la lista o se alcanzó el límite de 16."
                                    .into(),
                                true,
                            );
                        }
                    }
                    if added > 0 {
                        shell.preview_settings(cx);
                    }
                    cx.notify();
                });
                result.ok();
            }
        })
        .detach();
    }

    /// Abre la ruta indicada con la aplicación del sistema.
    fn settings_open_path(&mut self, path: Option<PathBuf>, cx: &mut Context<'_, Self>) {
        match path {
            Some(path) => cx.open_with_system(&path),
            None => self.set_notice("La ruta no está disponible.", true, cx),
        }
    }

    /// Elige el icono del perfil en edición.
    fn settings_choose_icon(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let _ = window;
        let receiver = cx.prompt_for_paths(axf_ui::gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Elegir icono del perfil".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.first().cloned()
            {
                let result = this.update(cx, |shell, cx| {
                    if let Some(editor) = &mut shell.settings_profile_editor {
                        editor.icon = Some(path);
                        editor.error = None;
                    }
                    cx.notify();
                });
                result.ok();
            }
        })
        .detach();
    }

    // --- Panel de ajustes: render --------------------------------------
    /// Barra lateral de secciones del panel.
    fn render_settings_rail(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let section = self
            .settings
            .as_ref()
            .map(|settings| settings.section)
            .unwrap_or(SettingsSection::Appearance);
        let mut rows = div().flex().flex_col().gap(metrics.spacing_xxs()).min_w_0();
        for candidate in SettingsSection::ALL {
            let candidate = *candidate;
            let selected = candidate == section;
            let row = Self::settings_focusable(
                window,
                cx,
                format!("settings-section-{}", candidate.label()),
            )
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .min_w_0()
            .h(metrics.control_height_md())
            .px(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .when(selected, |this| this.bg(ui.selection))
            .when(!selected, |this| {
                this.hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
            })
            .on_click(cx.listener(move |shell, _, _, cx| {
                if let Some(settings) = &mut shell.settings {
                    settings.set_section(candidate);
                }
                shell.settings_scroll = ScrollHandle::new();
                cx.notify();
            }))
            .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if let Some(settings) = &mut shell.settings {
                        settings.set_section(candidate);
                    }
                    shell.settings_scroll = ScrollHandle::new();
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                Icon::new(candidate.icon())
                    .size(IconSize::Sm)
                    .color(if selected { ui.accent } else { ui.muted }),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(if selected { ui.accent } else { ui.text })
                    .child(candidate.label()),
            );
            rows = rows.child(row);
        }
        let rail_width = metrics.font_size * 11.5 + metrics.spacing_sm() * 2.0;
        // Misma compensación de DPI que en `render_settings`: las anchuras de
        // los hijos se escalan y el ancho de la tarjeta no.
        div()
            .w(rail_width / window.scale_factor())
            .max_w(rail_width / window.scale_factor())
            .min_w_0()
            .flex_shrink_0()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .p(metrics.spacing_sm())
                    .border_r_1()
                    .border_color(ui.border)
                    .child(rows),
            )
            .into_any_element()
    }

    /// Cabecera con el título de la sección, la ayuda y los avisos.
    fn render_settings_header(
        &mut self,
        cx: &mut Context<'_, Self>,
        section: SettingsSection,
        notice: Option<(String, bool)>,
        conflict: Option<String>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut header = div()
            .flex()
            .flex_col()
            .w_full()
            .gap(metrics.spacing_xxs())
            .px(metrics.spacing_lg())
            .pt(metrics.spacing_lg())
            .pb(metrics.spacing_sm())
            .child(
                div()
                    .text_size(metrics.font_size * 0.95)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(section.label()),
            )
            .child(
                div()
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.muted)
                    .child(section.description()),
            )
            .when_some(notice, |this, (text, error)| {
                this.child(
                    div()
                        .pt(metrics.spacing_xs())
                        .text_size(metrics.font_size * 0.75)
                        .text_color(if error { ui.danger } else { ui.muted })
                        .child(text),
                )
            });
        if let Some(message) = conflict {
            header = header.child(self.render_settings_conflict(message, cx));
        }
        header.into_any_element()
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
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.danger)
                .child(if errors.len() > 1 {
                    format!("{first} (+{} errores)", errors.len() - 1)
                } else {
                    first.clone()
                })
        } else if dirty {
            div()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child("Borrador sin guardar")
        } else {
            div()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child("Sin cambios")
        };
        div()
            .flex()
            .w_full()
            .flex_wrap()
            .flex_shrink_0()
            .items_center()
            .gap(metrics.spacing_sm())
            .px(metrics.spacing_lg())
            .py(metrics.spacing_sm())
            .border_t_1()
            .border_color(ui.border)
            .child(status)
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

    /// Contenido de la sección activa.
    fn render_settings_body(
        &mut self,
        section: SettingsSection,
        body_width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        match section {
            SettingsSection::Appearance => self.render_settings_appearance(window, cx),
            SettingsSection::Terminal => self.render_settings_terminal(window, cx),
            SettingsSection::Profiles => self.render_settings_profiles(body_width, window, cx),
            SettingsSection::Fonts => self.render_settings_fonts(cx),
            SettingsSection::Shortcuts => self.render_settings_shortcuts(window, cx),
            SettingsSection::Agents => self.render_settings_agents(window, cx),
            SettingsSection::Advanced => self.render_settings_advanced(cx),
        }
    }

    fn render_settings(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let metrics = theme::state(cx).metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let section = settings.section;
        let dirty = settings.is_dirty();
        let notice = settings.notice.clone();
        let conflict = settings.conflict_message().map(str::to_owned);
        let errors = settings.errors.clone();
        let schema = settings.draft.schema_version;
        let rail = self.render_settings_rail(window, cx);
        let header = self.render_settings_header(cx, section, notice, conflict);
        let footer = self.render_settings_footer(cx, dirty, &errors);
        let height = (window.viewport_size().height - metrics.font_size * 28.0)
            .max(metrics.font_size * 16.0);
        let card_width = metrics.font_size * 62.0;
        let scale = window.scale_factor().max(1.0);
        let rail_width = metrics.font_size * 11.5 + metrics.spacing_sm() * 2.0;
        // `Modal::width` fija la tarjeta en píxeles físicos, mientras que las
        // anchuras de los hijos se escalan por DPI. Dividir por `scale_factor`
        // alinea ambos sistemas y evita que las filas desborden la tarjeta.
        let body_width = (card_width - rail_width - metrics.spacing_lg() * 4.0) / scale;
        let body = self.render_settings_body(section, body_width, window, cx);
        let scroll = self.settings_scroll.clone();
        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(header)
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_x_hidden()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .vertical_scrollbar(&scroll)
                    .px(metrics.spacing_lg())
                    .pb(metrics.spacing_sm())
                    .child(div().w(body_width).max_w(body_width).min_w_0().child(body)),
            )
            .child(footer);
        let card_body = div()
            .flex()
            .w_full()
            .min_w_0()
            .h(height)
            .min_h_0()
            .overflow_hidden()
            .child(rail)
            .child(content);
        let modal = Modal::new("settings-modal", "Ajustes")
            .description(format!(
                "config.toml · esquema v{schema} · {}",
                if dirty {
                    "cambios sin guardar"
                } else {
                    "sin cambios"
                }
            ))
            .width(card_width)
            .align_top()
            .body(card_body)
            .on_close(cx.listener(|shell, _, window, cx| {
                shell.close_settings(window, cx);
            }))
            .into_any_element();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context(SETTINGS_CONTEXT)
            .track_focus(&self.settings_focus)
            .tab_group()
            .on_action(cx.listener(Self::settings_cancel))
            .on_action(cx.listener(Self::settings_save))
            .on_action(cx.listener(Self::settings_next_section))
            .on_action(cx.listener(Self::settings_previous_section))
            .child(modal)
            .focus_trap("settings-trap", &self.settings_focus)
            .into_any_element()
    }
}
