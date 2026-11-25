impl Shell {
    /// Crea una vez los selectores de fuente que viven mientras dura el panel.
    fn ensure_font_selects(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.ui_font_select.is_some() && self.terminal_font_select.is_some() {
            return;
        }
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        self.font_select_subscriptions.clear();
        let fonts = self.theme_host.installed_fonts(cx);
        let (ui_items, ui_selected) = font_list(
            &fonts,
            settings.draft.ui.font_family.as_deref(),
            UI_PREFERRED,
        );
        let ui_state = cx.new(|cx| SelectState::new(ui_items, ui_selected, window, cx).searchable(true));
        let subscription =
            cx.subscribe(&ui_state, |shell, _, event: &SelectEvent<FontList>, cx| {
                let SelectEvent::Confirm(value) = event;
                if let Some(value) = value {
                    if let Some(settings) = &mut shell.settings {
                        match value {
                            FontValue::System => settings.set_ui_font(None),
                            FontValue::Family(name) => {
                                settings.set_ui_font(Some(name.to_string()));
                            }
                        }
                    }
                    shell.preview_settings(cx);
                }
            });
        self.font_select_subscriptions.push(subscription);
        self.ui_font_select = Some(ui_state);
        let (terminal_items, terminal_selected) = font_list(
            &fonts,
            settings.draft.terminal.font_family.as_deref(),
            MONO_PREFERRED,
        );
        let terminal_state =
            cx.new(|cx| SelectState::new(terminal_items, terminal_selected, window, cx).searchable(true));
        let subscription = cx.subscribe(
            &terminal_state,
            |shell, _, event: &SelectEvent<FontList>, cx| {
                let SelectEvent::Confirm(value) = event;
                if let Some(value) = value {
                    if let Some(settings) = &mut shell.settings {
                        match value {
                            FontValue::System => settings.set_terminal_font(None),
                            FontValue::Family(name) => {
                                settings.set_terminal_font(Some(name.to_string()));
                            }
                        }
                    }
                    shell.preview_settings(cx);
                }
            },
        );
        self.font_select_subscriptions.push(subscription);
        self.terminal_font_select = Some(terminal_state);
    }

    /// Crea los sliders numéricos del panel y los mantiene enlazados al borrador.
    fn ensure_settings_sliders(&mut self, cx: &mut Context<'_, Self>) {
        if self.settings_sliders.is_some() {
            return;
        }
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let ui_font_size = cx.new(|_| {
            SliderState::new()
                .min(11.0)
                .max(24.0)
                .step(1.0)
                .default_value(settings.draft.ui.font_size.clamp(11.0, 24.0))
        });
        let ui_scale = cx.new(|_| {
            SliderState::new()
                .min(0.8)
                .max(2.0)
                .step(0.05)
                .default_value(settings.draft.ui.scale.clamp(0.8, 2.0))
        });
        let terminal_font_size = cx.new(|_| {
            SliderState::new()
                .min(8.0)
                .max(32.0)
                .step(1.0)
                .default_value(settings.draft.terminal.font_size.clamp(8.0, 32.0))
        });
        let line_height = cx.new(|_| {
            SliderState::new()
                .min(1.0)
                .max(2.5)
                .step(0.05)
                .default_value(settings.draft.terminal.line_height.clamp(1.0, 2.5))
        });
        let scrollback_lines = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(axf_config::MAX_SCROLLBACK_LINES as f32)
                .step(1000.0)
                .default_value(settings.draft.terminal.scrollback_lines as f32)
        });
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe(
            &ui_font_size,
            |shell, _, event: &SliderEvent, cx| {
                let (value, release) = slider_event_value(event);
                if let Some(settings) = &mut shell.settings {
                    settings.set_ui_font_size(value);
                }
                shell.finish_slider_event(release, cx);
            },
        ));
        subscriptions.push(cx.subscribe(&ui_scale, |shell, _, event: &SliderEvent, cx| {
            let (value, release) = slider_event_value(event);
            if let Some(settings) = &mut shell.settings {
                settings.set_ui_scale(value);
            }
            shell.finish_slider_event(release, cx);
        }));
        subscriptions.push(cx.subscribe(
            &terminal_font_size,
            |shell, _, event: &SliderEvent, cx| {
                let (value, release) = slider_event_value(event);
                if let Some(settings) = &mut shell.settings {
                    settings.set_terminal_font_size(value);
                }
                shell.finish_slider_event(release, cx);
            },
        ));
        subscriptions.push(cx.subscribe(
            &line_height,
            |shell, _, event: &SliderEvent, cx| {
                let (value, release) = slider_event_value(event);
                if let Some(settings) = &mut shell.settings {
                    settings.set_line_height(value);
                }
                shell.finish_slider_event(release, cx);
            },
        ));
        subscriptions.push(cx.subscribe(
            &scrollback_lines,
            |shell, _, event: &SliderEvent, cx| {
                let (value, release) = slider_event_value(event);
                if let Some(settings) = &mut shell.settings {
                    settings.set_scrollback_lines(value);
                }
                shell.finish_slider_event(release, cx);
            },
        ));
        self.settings_sliders = Some(SettingsSliders {
            ui_font_size,
            ui_scale,
            terminal_font_size,
            line_height,
            scrollback_lines,
            _subscriptions: subscriptions,
        });
    }

    /// Mantiene los sliders alineados con el borrador (defaults o recarga).
    fn sync_settings_sliders(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(sliders) = self.settings_sliders.as_ref() else {
            return;
        };
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        sync_slider(
            &sliders.ui_font_size,
            settings.draft.ui.font_size,
            window,
            cx,
        );
        sync_slider(&sliders.ui_scale, settings.draft.ui.scale, window, cx);
        sync_slider(
            &sliders.terminal_font_size,
            settings.draft.terminal.font_size,
            window,
            cx,
        );
        sync_slider(
            &sliders.line_height,
            settings.draft.terminal.line_height,
            window,
            cx,
        );
        sync_slider(
            &sliders.scrollback_lines,
            settings.draft.terminal.scrollback_lines as f32,
            window,
            cx,
        );
    }

    /// Crea o reconstruye los selectores de perfil preferido por agente.
    fn ensure_agent_profile_selects(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let profile_signature: Vec<(ProfileId, String)> = settings
            .profiles()
            .iter()
            .map(|profile| (profile.id.clone(), profile.name.clone()))
            .collect();
        if self
            .settings_agent_selects
            .as_ref()
            .is_some_and(|selects| selects.profile_signature == profile_signature)
        {
            return;
        }
        let profiles = settings.profiles().to_vec();
        let opencode_current = settings.preferred_profile("opencode").cloned();
        let copilot_current = settings.preferred_profile("copilot").cloned();
        let (opencode, opencode_subscription) = build_agent_select(
            window,
            cx,
            &profiles,
            "opencode",
            opencode_current.as_ref(),
        );
        let (copilot, copilot_subscription) =
            build_agent_select(window, cx, &profiles, "copilot", copilot_current.as_ref());
        self.settings_agent_selects = Some(AgentProfileSelects {
            profile_signature,
            opencode,
            copilot,
            _subscriptions: vec![opencode_subscription, copilot_subscription],
        });
    }

    /// Campo de búsqueda de atajos.
    fn ensure_shortcut_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.settings_shortcut_input.is_some() {
            return;
        }
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filtrar acciones y atajos…"));
        let subscription = cx.subscribe(&input, |shell, input, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let query = input.read(cx).value().to_string();
            if let Some(settings) = &mut shell.settings {
                settings.shortcut_query = query;
            }
            cx.notify();
        });
        self.settings_shortcut_input = Some(input);
        self.settings_shortcut_subscription = Some(subscription);
    }
}

/// Valor y si el gesto terminó; los sliders previsualizan al soltar.
fn slider_event_value(event: &SliderEvent) -> (f32, bool) {
    match event {
        SliderEvent::Change(value) => (value.end(), false),
        SliderEvent::Release(value) => (value.end(), true),
    }
}

/// Réplica del valor provisional en la etiqueta; el preview completo llega al
/// soltar para no recompilar el tema en cada frame.
impl Shell {
    fn finish_slider_event(&mut self, release: bool, cx: &mut Context<'_, Self>) {
        if release {
            self.preview_settings(cx);
        } else {
            cx.notify();
        }
    }
}

fn build_agent_select(
    window: &mut Window,
    cx: &mut Context<'_, Shell>,
    profiles: &[Profile],
    agent: &'static str,
    current: Option<&ProfileId>,
) -> (Entity<SelectState<ProfileList>>, Subscription) {
    let (items, selected) = profile_list(profiles, current);
    let state = cx.new(|cx| SelectState::new(items, selected, window, cx));
    let subscription = cx.subscribe(&state, move |shell, _, event: &SelectEvent<ProfileList>, cx| {
        let SelectEvent::Confirm(value) = event;
        let Some(value) = value else {
            return;
        };
        let profile = match value {
            ProfileValue::Default => None,
            ProfileValue::Profile(id) => Some(id.clone()),
        };
        if let Some(settings) = &mut shell.settings {
            settings.set_preferred_profile(agent, profile);
        }
        cx.notify();
    });
    (state, subscription)
}

/// Escribe el valor en el slider solo si el borrador lo cambió por otra vía.
fn sync_slider(
    state: &Entity<SliderState>,
    value: f32,
    window: &mut Window,
    cx: &mut Context<'_, Shell>,
) {
    if (state.read(cx).value().end() - value).abs() < 0.001 {
        return;
    }
    state.update(cx, |state, cx| state.set_value(value, window, cx));
}
