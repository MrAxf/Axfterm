impl Shell {
    /// Guarda el borrador validado y con la política de conflicto de ADR 0007.
    fn save_settings(&mut self, cx: &mut Context<'_, Self>) {
        if self.settings_profile_editor.is_some() {
            if let Some(settings) = &mut self.settings {
                settings.set_notice(
                    "Aplica o cancela la edición del perfil antes de guardar.".into(),
                    true,
                );
            }
            cx.notify();
            return;
        }
        if self
            .settings
            .as_ref()
            .is_some_and(|settings| settings.capturing.is_some())
        {
            if let Some(settings) = &mut self.settings {
                settings.set_notice(
                    "Termina o cancela la captura del atajo antes de guardar.".into(),
                    true,
                );
            }
            cx.notify();
            return;
        }
        let Some(path) = self.theme_host.config_path.clone() else {
            if let Some(settings) = &mut self.settings {
                settings.set_notice("No se pudo resolver config.toml.".into(), true);
            }
            cx.notify();
            return;
        };
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let errors = settings.validate();
        if !errors.is_empty() {
            if let Some(settings) = &mut self.settings {
                settings.set_errors(errors.clone());
                settings.set_notice(errors[0].clone(), true);
            }
            cx.notify();
            return;
        }
        let current = axf_config::read_config_text(&path).ok().flatten();
        if current != settings.baseline && !settings.overwrite {
            if let Some(settings) = &mut self.settings {
                settings.conflict = Some(crate::settings::Conflict {
                    source: current.unwrap_or_default(),
                    summary:
                        "config.toml cambió fuera del panel. Guardar sobrescribirá ese cambio; \
                              recarga para descartar el borrador."
                            .into(),
                });
            }
            cx.notify();
            return;
        }
        let base = settings.base.clone();
        let draft = settings.draft.clone();
        match axf_config::write_config(&path, &base, &draft) {
            Ok(written) => {
                let messages = self.theme_host.commit_settings(draft.clone(), cx);
                if let Some(settings) = &mut self.settings {
                    settings.commit_saved(written);
                    if !messages.is_empty() {
                        settings.set_notice(messages.join(" "), false);
                    }
                }
                let keymap = Keymap::resolve(&draft.keymap);
                let mut keymap_errors = keymap.errors.clone();
                keymap_errors.extend(keymap.install(cx));
                self.keymap = keymap;
                if !keymap_errors.is_empty() {
                    self.set_notice(keymap_errors.join(" "), true, cx);
                }
                cx.notify();
            }
            Err(error) => {
                if let Some(settings) = &mut self.settings {
                    settings.set_errors(vec![error.to_string()]);
                    settings.set_notice(error.to_string(), true);
                }
                cx.notify();
            }
        }
    }

    /// Resultado de un cambio externo mientras el panel está abierto.
    fn settings_external_change(&mut self, source: Option<String>, cx: &mut Context<'_, Self>) {
        let Some(source) = source else {
            return;
        };
        let parsed = axf_config::parse_str(&source, "config.toml");
        let Some(outcome) = self
            .settings
            .as_mut()
            .map(|settings| settings.external_change(source, parsed))
        else {
            return;
        };
        match outcome {
            ExternalOutcome::Reloaded => {
                let Some(draft) = self
                    .settings
                    .as_ref()
                    .map(|settings| settings.draft.clone())
                else {
                    return;
                };

                // El archivo ya cambió: la config activa pasa a ser la nueva y
                // el panel deja de previsualizar. Cancelar no debe revertirla.
                let messages = self.theme_host.commit_settings(draft.clone(), cx);
                let keymap = Keymap::resolve(&draft.keymap);
                let mut notes = keymap.errors.clone();
                notes.extend(keymap.install(cx));
                self.keymap = keymap;
                if let Some(settings) = &mut self.settings {
                    let mut parts = messages;
                    parts.extend(notes);
                    settings.set_notice(
                        if parts.is_empty() {
                            "config.toml cambió fuera del panel; borrador recargado.".into()
                        } else {
                            parts.join(" ")
                        },
                        false,
                    );
                }
            }
            ExternalOutcome::Conflict => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(
                        "Hay cambios externos sin guardar. Recarga o sigue editando; \
                         guardar pedirá confirmación."
                            .into(),
                        true,
                    );
                }
            }
            ExternalOutcome::Invalid(message) => {
                tracing::debug!("{message}");
            }
            ExternalOutcome::Ignored => {}
        }
        cx.notify();
    }

    /// Recarga el archivo externo descartando el borrador.
    fn reload_settings_from_disk(&mut self, cx: &mut Context<'_, Self>) {
        let Some(path) = self.theme_host.config_path.clone() else {
            return;
        };
        let source = axf_config::read_config_text(&path).ok().flatten();
        let parsed = source
            .as_deref()
            .map(|source| axf_config::parse_str(source, "config.toml"));
        match (source, parsed) {
            (Some(source), Some(Ok(config))) => {
                // El usuario descarta el borrador: la config del disco pasa a
                // ser la activa, no una previsualización.
                let messages = self.theme_host.commit_settings(config.clone(), cx);
                let keymap = Keymap::resolve(&config.keymap);
                let mut notes = keymap.errors.clone();
                notes.extend(keymap.install(cx));
                self.keymap = keymap;
                if let Some(settings) = &mut self.settings {
                    settings.reload_external(config, source);
                    let mut parts = messages;
                    parts.extend(notes);
                    if !parts.is_empty() {
                        settings.set_notice(parts.join(" "), false);
                    }
                }
                cx.notify();
            }
            (_, Some(Err(error))) => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(format!("No se pudo recargar: {error}"), true);
                }
                cx.notify();
            }
            _ => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(
                        "config.toml no existe; se conserva el borrador.".into(),
                        true,
                    );
                }
                cx.notify();
            }
        }
    }

    fn toggle_theme_variant(
        &mut self,
        _: &ToggleThemeVariant,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.theme_host.toggle_preview_variant(cx);
        let variant = theme::state(cx).theme.variant().label().to_lowercase();
        self.set_notice(format!("Previsualizando variante {variant}."), false, cx);
    }

    fn next_theme(&mut self, _: &NextTheme, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.cycle_theme(1, cx);
    }

    fn previous_theme(&mut self, _: &PreviousTheme, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.cycle_theme(-1, cx);
    }

    fn cycle_theme(&mut self, delta: isize, cx: &mut Context<'_, Self>) {
        let names = self.theme_host.theme_names();
        if names.is_empty() {
            return;
        }
        let current = names
            .iter()
            .position(|(name, _)| name == &self.theme_host.selected)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(names.len() as isize) as usize;
        let name = names[next].0.clone();
        if self.theme_host.select_theme(&name, cx) {
            self.set_notice(format!("Tema: {name}"), false, cx);
        }
    }

    fn reload_themes(&mut self, _: &ReloadThemes, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.reload_themes_impl(cx);
    }

    fn open_themes_folder(
        &mut self,
        _: &OpenThemesFolder,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_themes_folder_impl(cx);
    }

    fn import_theme(&mut self, _: &ImportTheme, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.import_theme_impl(cx);
    }

    fn export_theme(&mut self, _: &ExportTheme, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.export_theme_impl(cx);
    }

    fn restore_workspace_focus(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab
            && let Some(entry) = self
                .workspace
                .tab(tab)
                .and_then(|tab| self.terminals.get(&tab.terminal_id))
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        } else {
            window.focus(&self.focus, cx);
        }
    }

    fn prompt_open_project(&mut self, cx: &mut Context<'_, Self>) {
        let receiver = cx.prompt_for_paths(axf_ui::gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Abrir carpeta".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                this.update_in(cx, |shell, window, cx| {
                    for path in paths {
                        shell.open_project_path(path, window, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn open_project_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((profile, title)) = self.default_profile() else {
            self.set_notice(
                "No hay un perfil por defecto válido en config.toml.",
                true,
                cx,
            );
            return;
        };
        match self.workspace.open_project(&path, &profile, &title) {
            Ok(opened) => {
                if let Some(new_tab) = opened.tab {
                    self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                }
                self.reveal_project(opened.project);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.new_tab_for(None, None, window, cx);
    }

    fn new_tab_for(
        &mut self,
        project: Option<ProjectId>,
        profile: Option<ProfileId>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let project = project
            .or_else(|| self.workspace.current_project())
            .or_else(|| self.sidebar_cursor_project())
            .or_else(|| self.workspace.projects().first().map(|project| project.id));
        let Some(project) = project else {
            self.prompt_open_project(cx);
            return;
        };
        let requested =
            profile.unwrap_or_else(|| self.theme_host.settings.terminal.default_profile.clone());
        let Some(title) = self.profile_title(&requested) else {
            self.set_notice(
                format!("El perfil `{requested}` no existe en config.toml."),
                true,
                cx,
            );
            return;
        };
        let pane = self.workspace.focus().pane;
        match self.workspace.new_tab(pane, project, &requested, &title) {
            Ok(new_tab) => {
                self.reveal_project(project);
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab {
            self.close_tab_id(tab, cx);
        }
    }

    fn close_tab_id(&mut self, tab: TabId, cx: &mut Context<'_, Self>) {
        let terminal = self.workspace.tab(tab).map(|tab| tab.terminal_id);
        if let Err(error) = self.workspace.close_tab(tab) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        if let Some(terminal) = terminal {
            self.terminals.remove(&terminal);
            self.pending_terminals.remove(&terminal);
            self.spawn_errors.remove(&terminal);
            self.history.remove(terminal);
        }
        if self.editing.as_ref().is_some_and(|edit| edit.tab == tab) {
            self.editing = None;
        }
        if let Some(focused) = self.workspace.focus().tab {
            self.pending_focus = Some(focused);
            self.ensure_terminal(focused, cx);
        }
        self.persist(cx);
        cx.notify();
    }

    fn next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.cycle_tab(1, window, cx);
    }

    fn previous_tab(&mut self, _: &PreviousTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.cycle_tab(-1, window, cx);
    }

    fn cycle_tab(&mut self, delta: isize, window: &mut Window, cx: &mut Context<'_, Self>) {
        let pane_id = self.workspace.focus().pane;
        let Some(pane) = self.workspace.pane(pane_id) else {
            return;
        };
        if pane.tabs.is_empty() {
            return;
        }
        let current = pane
            .active
            .and_then(|active| pane.tabs.iter().position(|tab| *tab == active))
            .unwrap_or(0) as isize;
        let len = pane.tabs.len() as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        let tab = pane.tabs[next];
        self.focus_tab(tab, window, cx);
    }

    fn move_tab_next_pane(
        &mut self,
        _: &MoveTabNextPane,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(tab) = self.workspace.focus().tab else {
            return;
        };
        let leaves = self.workspace.layout().leaves();
        if leaves.len() < 2 {
            return;
        }
        let current = self
            .workspace
            .pane_of_tab(tab)
            .and_then(|pane| leaves.iter().position(|id| *id == pane))
            .unwrap_or(0);
        let target = leaves[(current + 1) % leaves.len()];
        if let Err(error) = self.workspace.move_tab(tab, target, usize::MAX) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        self.focus_tab(tab, window, cx);
    }
}
