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
        let opencode = self
            .settings_agent_selects
            .as_ref()
            .map(|selects| selects.opencode.clone());
        let copilot = self
            .settings_agent_selects
            .as_ref()
            .map(|selects| selects.copilot.clone());

        let detection = card("Detección", metrics, ui)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_sm())
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
                                    .child("Detectar agentes"),
                            )
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.6875)
                                    .text_color(ui.muted)
                                    .child("OSC 7501 y árbol de procesos; sin red ni telemetría."),
                            ),
                    )
                    .child(
                        SwitchField::new("settings-agents-enabled")
                            .checked(enabled)
                            .accessibility_label("Detectar agentes")
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
            ));

        let notices = card("Avisos", metrics, ui)
            .child(switch_row(
                cx,
                "settings-agents-notify-finished",
                "Al terminar",
                "Cuando un agente completa su tarea.",
                notify_finished,
                |settings, checked| settings.set_notify_finished(checked),
            ))
            .child(switch_row(
                cx,
                "settings-agents-notify-error",
                "Al fallar",
                "Cuando un agente termina con error.",
                notify_error,
                |settings, checked| settings.set_notify_error(checked),
            ))
            .child(switch_row(
                cx,
                "settings-agents-notify-attention",
                "Al requerir atención",
                "Permisos, preguntas o autenticación pendientes.",
                notify_attention,
                |settings, checked| settings.set_notify_attention(checked),
            ));

        let preferences = card("Perfil preferido", metrics, ui)
            .child(preference_row(
                cx,
                "OpenCode",
                "Con qué perfil se abrirá OpenCode.",
                opencode.as_ref(),
            ))
            .child(preference_row(
                cx,
                "Copilot",
                "Con qué perfil se abrirá Copilot.",
                copilot.as_ref(),
            ))
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        "Se aplica al abrir una terminal de agente desde la app; hasta entonces \
                         solo se guarda en config.toml.",
                    ),
            );

        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .child(detection)
            .child(notices)
            .child(preferences)
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
            .size(metrics.control_height_sm())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .hover(move |this| this.bg(ui.surface_hover))
            .active(move |this| this.bg(ui.surface_active))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.settings_step(field, -1.0, cx);
            }))
            .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    shell.settings_step(field, -1.0, cx);
                    cx.stop_propagation();
                }
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
            .size(metrics.control_height_sm())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .hover(move |this| this.bg(ui.surface_hover))
            .active(move |this| this.bg(ui.surface_active))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.settings_step(field, 1.0, cx);
            }))
            .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    shell.settings_step(field, 1.0, cx);
                    cx.stop_propagation();
                }
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
            .justify_between()
            .flex_wrap()
            .gap(metrics.spacing_sm())
            .min_h(metrics.control_height_md())
            .child(div().text_size(metrics.font_size * 0.75).child(label))
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

/// Tarjeta con título y controles apilados.
fn card(title: &'static str, metrics: UiMetrics, ui: axf_ui::theme::UiTokens) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(metrics.spacing_sm())
        .p(metrics.spacing_md())
        .rounded(metrics.radius_md())
        .border_1()
        .border_color(ui.border)
        .bg(ui.surface)
        .child(
            div()
                .text_size(metrics.font_size * 0.8125)
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
}

fn switch_row(
    cx: &mut Context<'_, Shell>,
    id: &'static str,
    label: &'static str,
    help: &'static str,
    checked: bool,
    set: impl Fn(&mut SettingsState, bool) + Copy + 'static,
) -> AnyElement {
    let metrics = theme::metrics(cx);
    let ui = theme::state(cx).theme.ui;
    div()
        .flex()
        .items_center()
        .gap(metrics.spacing_sm())
        .min_h(metrics.control_height_md())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(metrics.hairline())
                .flex_1()
                .min_w_0()
                .child(div().text_size(metrics.font_size * 0.75).child(label))
                .child(
                    div()
                        .text_size(metrics.font_size * 0.6875)
                        .text_color(ui.muted)
                        .child(help),
                ),
        )
        .child(
            SwitchField::new(id)
                .checked(checked)
                .accessibility_label(label)
                .on_click(cx.listener(move |shell, checked, _, cx| {
                    if let Some(settings) = &mut shell.settings {
                        set(settings, *checked);
                    }
                    cx.notify();
                },
            )),
        )
        .into_any_element()
}

fn preference_row(
    cx: &axf_ui::gpui::App,
    label: &'static str,
    help: &'static str,
    select: Option<&Entity<SelectState<ProfileList>>>,
) -> AnyElement {
    let metrics = theme::metrics(cx);
    let ui = theme::state(cx).theme.ui;
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
                .flex_1()
                .min_w_0()
                .child(div().text_size(metrics.font_size * 0.75).child(label))
                .child(
                    div()
                        .text_size(metrics.font_size * 0.6875)
                        .text_color(ui.muted)
                        .child(help),
                ),
        )
        .when_some(select, |this, select| {
            this.child(
                div()
                    .w(metrics.font_size * 13.0)
                    .min_w_0()
                    .child(
                        SelectField::new(select)
                            .size(ControlSize::Sm)
                            .full_width()
                            .accessibility_label("Perfil preferido"),
                    ),
            )
        })
        .into_any_element()
}
