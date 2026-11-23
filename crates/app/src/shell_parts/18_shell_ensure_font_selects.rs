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
        let (ui_items, ui_selected) = font_list(&fonts, settings.draft.ui.font_family.as_deref());
        let ui_state = cx.new(|cx| SelectState::new(ui_items, ui_selected, window, cx));
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
        let (terminal_items, terminal_selected) =
            font_list(&fonts, settings.draft.terminal.font_family.as_deref());
        let terminal_state =
            cx.new(|cx| SelectState::new(terminal_items, terminal_selected, window, cx));
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
                settings.shortcut_cursor = 0;
            }
            cx.notify();
        });
        self.settings_shortcut_input = Some(input);
        self.settings_shortcut_subscription = Some(subscription);
    }

    /// Abre el editor inline de un perfil ya existente en el borrador.
    fn open_profile_editor(
        &mut self,
        id: ProfileId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(profile) = self
            .settings
            .as_ref()
            .and_then(|settings| settings.profiles().iter().find(|p| p.id == id))
            .cloned()
        else {
            return;
        };
        let name_value = profile.name.clone();
        let command_value = profile.command.clone();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name_value));
        let command = cx.new(|cx| InputState::new(window, cx).default_value(command_value));
        let handle = name.read(cx).focus_handle(cx);
        self.settings_profile_editor = Some(ProfileEditor {
            id,
            name,
            command,
            icon: profile.icon,
            error: None,
        });
        window.focus(&handle, cx);
        cx.notify();
    }

    /// Añade un perfil vacío y abre su editor.
    fn add_profile(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(id) = self
            .settings
            .as_mut()
            .map(|settings| settings.add_profile())
        else {
            return;
        };
        self.open_profile_editor(id, window, cx);
    }

    /// Valida y aplica los campos del editor inline.
    fn apply_profile_editor(&mut self, cx: &mut Context<'_, Self>) {
        let Some(editor) = self.settings_profile_editor.as_ref() else {
            return;
        };
        let name = editor.name.read(cx).value().trim().to_owned();
        let command = editor.command.read(cx).value().trim().to_owned();
        if name.is_empty() {
            if let Some(editor) = &mut self.settings_profile_editor {
                editor.error = Some("El nombre no puede estar vacío.".into());
            }
            cx.notify();
            return;
        }
        if command.is_empty() {
            if let Some(editor) = &mut self.settings_profile_editor {
                editor.error = Some("El comando no puede estar vacío.".into());
            }
            cx.notify();
            return;
        }
        if let Err(error) =
            Profile::new(editor.id.clone(), name.clone(), command.clone()).command_line()
        {
            if let Some(editor) = &mut self.settings_profile_editor {
                editor.error = Some(format!("Comando inválido: {error}"));
            }
            cx.notify();
            return;
        }
        let id = editor.id.clone();
        let icon = editor.icon.clone();
        if let Some(settings) = &mut self.settings
            && let Err(error) = settings.update_profile(&id, name, command, icon)
        {
            settings.set_notice(error, true);
            cx.notify();
            return;
        }
        self.settings_profile_editor = None;
        cx.notify();
    }

    /// Número de pestañas abiertas que usan un perfil.
    fn profile_in_use(&self, id: &ProfileId) -> usize {
        let mut count = 0;
        for project in self.workspace.projects() {
            for tab in self.workspace.project_tabs(project.id) {
                if self
                    .workspace
                    .tab(tab)
                    .is_some_and(|tab| &tab.profile_id == id)
                {
                    count += 1;
                }
            }
        }
        count
    }

    /// Pide borrar el perfil seleccionado, con protección y aviso de uso.
    fn request_delete_profile(&mut self, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let Some(profile) = settings.selected_profile().cloned() else {
            return;
        };
        if settings.profiles().len() <= 1 {
            self.set_notice("Debe quedar al menos un perfil.", true, cx);
            return;
        }
        if settings.draft.terminal.default_profile == profile.id {
            self.set_notice(
                "Elige otro perfil predeterminado antes de borrar este.",
                true,
                cx,
            );
            return;
        }
        let in_use = self.profile_in_use(&profile.id);
        if in_use > 0 {
            if let Some(settings) = &mut self.settings {
                settings.pending_delete = Some(PendingDelete {
                    id: profile.id,
                    name: profile.name,
                    in_use,
                });
            }
            cx.notify();
            return;
        }
        self.delete_profile_now(&profile.id, cx);
    }

    fn delete_profile_now(&mut self, id: &ProfileId, cx: &mut Context<'_, Self>) {
        let result = self
            .settings
            .as_mut()
            .map(|settings| settings.delete_profile(id));
        match result {
            Some(Ok(())) => {
                if self
                    .settings_profile_editor
                    .as_ref()
                    .is_some_and(|editor| &editor.id == id)
                {
                    self.settings_profile_editor = None;
                }
                self.set_notice("Perfil eliminado del borrador.", false, cx);
            }
            Some(Err(error)) => {
                if let Some(settings) = &mut self.settings {
                    settings.pending_delete = None;
                    settings.set_notice(error.clone(), true);
                }
                self.set_notice(error, true, cx);
            }
            None => {}
        }
        cx.notify();
    }

    /// Confirma el borrado de un perfil en uso.
    fn confirm_delete_profile(&mut self, cx: &mut Context<'_, Self>) {
        let Some(pending) = self
            .settings
            .as_mut()
            .and_then(|settings| settings.pending_delete.take())
        else {
            return;
        };
        self.delete_profile_now(&pending.id, cx);
    }

    /// Lanza el descubrimiento local en segundo plano.
    fn start_detect(&mut self, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        if matches!(settings.detect, DetectState::Running) {
            return;
        }
        settings.detect = DetectState::Running;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let discovery = cx
                .background_executor()
                .spawn(async { axf_config::discover() })
                .await;
            let profiles = discovery.profiles_only();
            let result = this.update(cx, |shell, cx| {
                let Some(settings) = &mut shell.settings else {
                    return;
                };
                let mut candidates = profiles;
                candidates.retain(|profile| {
                    !settings.profiles().iter().any(|existing| {
                        existing.id == profile.id
                            || existing
                                .command
                                .trim()
                                .eq_ignore_ascii_case(profile.command.trim())
                    })
                });
                let count = candidates.len();
                settings.detect = DetectState::Ready(candidates);
                settings.set_notice(
                    if count == 0 {
                        "No hay terminales nuevas que añadir.".into()
                    } else {
                        format!(
                            "{count} terminales detectadas. Añádelas al borrador o descártalas."
                        )
                    },
                    false,
                );
                cx.notify();
            });
            result.ok();
        })
        .detach();
    }

    /// Fusiona los perfiles detectados en el borrador.
    fn merge_detected(&mut self, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        let DetectState::Ready(profiles) = std::mem::take(&mut settings.detect) else {
            return;
        };
        let added = settings.merge_detected(profiles);
        settings.detect = DetectState::Idle;
        settings.set_notice(
            if added == 0 {
                "Ninguno de los perfiles detectados era nuevo.".into()
            } else {
                format!("{added} perfiles añadidos al borrador; guarda para aplicarlos.")
            },
            false,
        );
        cx.notify();
    }

    /// Ajusta un valor numérico y refresca la preview visual.
    fn settings_step(&mut self, field: StepField, delta: f32, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        match field {
            StepField::UiFontSize => settings.step_ui_font_size(delta),
            StepField::UiScale => settings.step_ui_scale(delta),
            StepField::TerminalFontSize => settings.step_terminal_font_size(delta),
            StepField::LineHeight => settings.step_line_height(delta),
            StepField::ScanInterval => settings.step_scan_interval(delta),
        }
        if !matches!(field, StepField::ScanInterval) {
            self.preview_settings(cx);
        } else {
            cx.notify();
        }
    }

    /// Captura la tecla pulsada durante un rebind. Devuelve `true` si la consumió.
    fn settings_capture_keystroke(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(action) = self
            .settings
            .as_ref()
            .and_then(|settings| settings.capturing.clone())
        else {
            return false;
        };
        let key = keystroke.key.to_ascii_lowercase();
        if matches!(
            key.as_str(),
            "control"
                | "ctrl"
                | "shift"
                | "alt"
                | "meta"
                | "super"
                | "win"
                | "cmd"
                | "fn"
                | "function"
        ) {
            return false;
        }
        let result = if key == "escape" {
            if let Some(settings) = &mut self.settings {
                settings.cancel_capture();
            }
            Ok(())
        } else if key == "backspace" || key == "delete" {
            self.settings
                .as_mut()
                .map(|settings| settings.unbind_action(&action))
                .unwrap_or(Ok(()))
        } else {
            let chord = Self::chord_from_keystroke(keystroke);
            if chord.is_empty() {
                return false;
            }
            self.settings
                .as_mut()
                .map(|settings| settings.set_binding(&action, &chord))
                .unwrap_or(Ok(()))
        };
        match result {
            Ok(()) => {
                if let Some(settings) = &mut self.settings
                    && settings.capturing.is_none()
                    && settings.notice.is_none()
                {
                    settings.set_notice("Atajo actualizado en el borrador.".into(), false);
                }
            }
            Err(error) => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(error, true);
                }
            }
        }
        cx.notify();
        true
    }
}
