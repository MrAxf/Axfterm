impl Shell {
    /// Abre el modal de edición de un perfil ya existente en el borrador.
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
            mode: ProfileEditorMode::Edit(id),
            name,
            command,
            icon: profile.icon,
            error: None,
        });
        window.focus(&handle, cx);
        cx.notify();
    }

    /// Abre el modal de alta; el perfil no entra en el borrador hasta Aplicar.
    fn open_new_profile(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(profile) = self.settings.as_ref().map(SettingsState::new_profile) else {
            return;
        };
        let name_value = profile.name.clone();
        let command_value = profile.command.clone();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name_value));
        let command = cx.new(|cx| InputState::new(window, cx).default_value(command_value));
        let handle = name.read(cx).focus_handle(cx);
        self.settings_profile_editor = Some(ProfileEditor {
            mode: ProfileEditorMode::New(profile.id),
            name,
            command,
            icon: None,
            error: None,
        });
        window.focus(&handle, cx);
        cx.notify();
    }

    /// Cierra el modal de perfil sin tocar el borrador.
    fn close_profile_editor(&mut self, cx: &mut Context<'_, Self>) {
        self.settings_profile_editor = None;
        cx.notify();
    }

    /// Valida y aplica los campos del modal de perfil.
    fn apply_profile_editor(&mut self, cx: &mut Context<'_, Self>) {
        let Some(editor) = self.settings_profile_editor.as_ref() else {
            return;
        };
        let mode = editor.mode.clone();
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
        if let Err(error) = Profile::new(mode.id().clone(), name.clone(), command.clone())
            .command_line()
        {
            if let Some(editor) = &mut self.settings_profile_editor {
                editor.error = Some(format!("Comando inválido: {error}"));
            }
            cx.notify();
            return;
        }
        let icon = editor.icon.clone();
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        let result = match &mode {
            ProfileEditorMode::New(id) => {
                let mut profile = Profile::new(id.clone(), name, command);
                profile.icon = icon;
                settings.insert_profile(profile)
            }
            ProfileEditorMode::Edit(id) => settings.update_profile(id, name, command, icon),
        };
        if let Err(error) = result {
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

    /// Pide borrar un perfil, con protección y aviso de uso.
    fn request_delete_profile(&mut self, id: &ProfileId, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let Some(profile) = settings
            .profiles()
            .iter()
            .find(|profile| &profile.id == id)
            .cloned()
        else {
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
                    .is_some_and(|editor| editor.mode.id() == id)
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
}
