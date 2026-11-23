impl Shell {
    fn render_settings_agents(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let enabled = settings.draft.agents.enabled;
        let scan = settings.draft.agents.scan_interval_ms;
        let notify_finished = settings.draft.agents.notify_finished;
        let notify_error = settings.draft.agents.notify_error;
        let notify_attention = settings.draft.agents.notify_attention;
        let profiles = settings.profiles().to_vec();
        let preferences: Vec<(String, String)> = SettingsState::AGENT_KEYS
            .iter()
            .map(|agent| {
                let name = settings
                    .preferred_profile(agent)
                    .and_then(|id| profiles.iter().find(|profile| &profile.id == id))
                    .map(|profile| profile.name.clone())
                    .unwrap_or_else(|| "Perfil por defecto".into());
                ((*agent).to_owned(), name)
            })
            .collect();
        let banner = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap(metrics.spacing_sm())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.hairline())
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.8125)
                                    .child("Detección de agentes"),
                            )
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.6875)
                                    .text_color(ui.muted)
                                    .child("OSC 7501 y árbol de procesos; sin red ni telemetría."),
                            ),
                    )
                    .child(
                        Switch::new("settings-agents-enabled")
                            .checked(enabled)
                            .on_click(cx.listener(|shell, checked, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.set_agents_enabled(*checked);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-agents-scan",
                "Intervalo de escaneo",
                format!("{scan} ms"),
                StepField::ScanInterval,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Avisos"),
                    )
                    .child(
                        Switch::new("settings-agents-notify-finished")
                            .checked(notify_finished)
                            .label("Al terminar")
                            .on_click(cx.listener(|shell, checked, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.set_notify_finished(*checked);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Switch::new("settings-agents-notify-error")
                            .checked(notify_error)
                            .label("Al fallar")
                            .on_click(cx.listener(|shell, checked, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.set_notify_error(*checked);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Switch::new("settings-agents-notify-attention")
                            .checked(notify_attention)
                            .label("Al requerir atención")
                            .on_click(cx.listener(|shell, checked, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.set_notify_attention(*checked);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div().flex().flex_col().gap(metrics.spacing_xs()).child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child("Perfil preferido por agente"),
                ),
            );
        let mut banner = banner;
        for (agent, current) in preferences {
            let agent_label = if agent == "opencode" {
                "opencode"
            } else {
                "Copilot"
            };
            let click_agent = agent.clone();
            let row =
                Self::settings_focusable(window, cx, format!("settings-agent-profile-{agent}"))
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.cycle_preferred_profile(&click_agent, 1);
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                        let delta = match event.keystroke.key.as_str() {
                            "left" => -1,
                            "right" => 1,
                            _ => return,
                        };
                        if let Some(settings) = &mut shell.settings {
                            settings.cycle_preferred_profile(&agent, delta);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(div().text_size(metrics.font_size * 0.75).child(agent_label))
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.accent)
                            .child(current),
                    );
            banner = banner.child(row);
        }
        banner
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        "El perfil preferido se usará al abrir una terminal de agente desde la \
                         app; hasta entonces solo se guarda en config.toml.",
                    ),
            )
            .into_any_element()
    }

    fn render_settings_advanced(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let schema = settings.draft.schema_version;
        let config_path = self
            .theme_host
            .config_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "—".into());
        let themes_path = self
            .theme_host
            .themes_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "—".into());
        let pending_defaults = settings.pending_defaults;
        let config_file = self.theme_host.config_path.clone();
        let themes_dir = self.theme_host.themes_dir.clone();
        let mono = state.fonts.mono.clone();

        let path_line = |label: &'static str, path: String| {
            div()
                .flex()
                .flex_col()
                .gap(metrics.hairline())
                .child(
                    div()
                        .text_size(metrics.font_size * 0.6875)
                        .text_color(ui.muted)
                        .child(label),
                )
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(metrics.font_size * 0.75)
                        .child(path),
                )
        };

        let confirm = pending_defaults.then(|| {
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
                .child(
                    "Restaurar apariencia, terminal, fuentes, atajos y agentes a los valores \
                     por defecto. Los perfiles se conservan.",
                )
                .child(
                    KitButton::new("settings-defaults-confirm")
                        .small()
                        .danger()
                        .label("Restaurar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.restore_defaults();
                            }
                            shell.preview_settings(cx);
                        })),
                )
                .child(
                    KitButton::new("settings-defaults-cancel")
                        .small()
                        .ghost()
                        .label("Cancelar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.pending_defaults = false;
                            }
                            cx.notify();
                        })),
                )
                .into_any_element()
        });

        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_lg())
            .child(path_line("Configuración", config_path))
            .child(path_line("Temas", themes_path))
            .child(
                div()
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.muted)
                    .child(format!(
                        "Esquema de configuración v{schema}. La escritura conserva comentarios y \
                         orden, y solo aplica el borrador al pulsar Guardar."
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(metrics.spacing_xs())
                    .child(
                        KitButton::new("settings-open-config")
                            .small()
                            .label("Abrir config.toml")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.settings_open_path(config_file.clone(), cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-open-themes")
                            .small()
                            .ghost()
                            .label("Abrir temas")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.settings_open_path(themes_dir.clone(), cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-restore-defaults")
                            .small()
                            .danger()
                            .label("Restaurar valores por defecto…")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.pending_defaults = true;
                                }
                                cx.notify();
                            })),
                    ),
            )
            .children(confirm)
            .into_any_element()
    }

    /// Fila numérica ajustable con teclado (←/→) y botones de ratón.
    fn render_settings_stepper(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        id: &'static str,
        label: &'static str,
        value: String,
        field: StepField,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let minus = Self::settings_focusable(window, cx, format!("{id}-minus"))
            .flex()
            .items_center()
            .justify_center()
            .size(metrics.control_height_xs())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .hover(move |this| this.bg(ui.surface_raised))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.settings_step(field, -1.0, cx);
            }))
            .child(
                div()
                    .text_size(metrics.font_size * 1.1)
                    .text_color(ui.muted)
                    .child("−"),
            );
        let plus = Self::settings_focusable(window, cx, format!("{id}-plus"))
            .flex()
            .items_center()
            .justify_center()
            .size(metrics.control_height_xs())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .hover(move |this| this.bg(ui.surface_raised))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.settings_step(field, 1.0, cx);
            }))
            .child(
                div()
                    .text_size(metrics.font_size * 1.1)
                    .text_color(ui.muted)
                    .child("+"),
            );
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(metrics.spacing_sm())
            .min_h(metrics.control_height_md())
            .child(div().text_size(metrics.font_size * 0.8125).child(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_xxs())
                    .child(minus)
                    .child(
                        div()
                            .min_w(metrics.font_size * 4.5)
                            .text_center()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.text)
                            .child(value),
                    )
                    .child(plus),
            )
            .into_any_element()
    }

    /// Convierte una pulsación en el chord canónico de `config.toml`.
    fn chord_from_keystroke(keystroke: &Keystroke) -> String {
        let mut parts = Vec::new();
        if keystroke.modifiers.control {
            parts.push("ctrl".to_owned());
        }
        if keystroke.modifiers.alt {
            parts.push("alt".to_owned());
        }
        if keystroke.modifiers.shift {
            parts.push("shift".to_owned());
        }
        if keystroke.modifiers.platform {
            parts.push(if cfg!(target_os = "macos") {
                "cmd".to_owned()
            } else {
                "win".to_owned()
            });
        }
        if keystroke.modifiers.function {
            parts.push("fn".to_owned());
        }
        let key = keystroke.key.to_ascii_lowercase();
        if key.is_empty() {
            return String::new();
        }
        parts.push(key);
        parts.join("-")
    }
}
