impl Shell {
    fn watch_config(&mut self, cx: &mut Context<'_, Self>) {
        let Some(watcher) = self.theme_host.watcher.as_ref() else {
            return;
        };
        let events = watcher.events.clone();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let applied = this.update(cx, |shell, cx| {
                    let panel_config = event.config && shell.settings.is_some();
                    let report = if panel_config {
                        let source = shell
                            .theme_host
                            .config_path
                            .as_deref()
                            .and_then(|path| axf_config::read_config_text(path).ok().flatten());
                        shell.settings_external_change(source, cx);
                        if event.themes {
                            shell.theme_host.reload(
                                axf_config::WatchEvent {
                                    config: false,
                                    themes: true,
                                },
                                cx,
                            )
                        } else {
                            ApplyReport::default()
                        }
                    } else {
                        shell.theme_host.reload(event, cx)
                    };
                    if report.applied {
                        cx.notify();
                    }
                    let mut keymap_notes = Vec::new();
                    if report.applied {
                        let keymap = Keymap::resolve(&shell.theme_host.settings.keymap);
                        keymap_notes.extend(keymap.errors.iter().cloned());
                        keymap_notes.extend(keymap.warnings.iter().cloned());
                        keymap_notes.extend(keymap.install(cx));
                        shell.keymap = keymap;
                        cx.notify();
                    }
                    let had = !report.errors.is_empty() || !report.messages.is_empty();
                    let has_error = !report.errors.is_empty();
                    let mut parts = report.errors;
                    parts.extend(report.messages);
                    if !parts.is_empty() {
                        shell.set_notice(parts.join(" "), has_error, cx);
                    }
                    if !keymap_notes.is_empty() {
                        shell.set_notice(keymap_notes.join(" "), true, cx);
                    }
                    if !had && keymap_notes.is_empty() && !panel_config {
                        let message = match (event.config, event.themes) {
                            (true, true) => "Configuración y temas recargados.",
                            (true, false) => "Configuración recargada.",
                            (false, true) => "Temas recargados.",
                            (false, false) => "",
                        };
                        if !message.is_empty() {
                            shell.set_notice(message, false, cx);
                        }
                    }
                });
                if applied.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    // --- Sesión ---------------------------------------------------------
    fn sync_terminal_meta(&mut self, cx: &mut Context<'_, Self>) {
        let mut updates = Vec::new();
        for (terminal, entry) in &self.terminals {
            let program = entry.view.read(cx).program().map(str::to_owned);
            if let Some(title) = entry.view.read(cx).title()
                && !title_is_shell_path(title, program.as_deref())
                && let Some(tab) = self.workspace.tab_for_terminal(*terminal)
            {
                updates.push((tab, title.to_owned()));
            }
        }
        for (tab, title) in updates {
            self.workspace.set_tab_title(tab, title);
        }
    }

    fn persist(&mut self, cx: &mut Context<'_, Self>) {
        if self.quitting {
            return;
        }
        self.sync_terminal_meta(cx);
        let Some(path) = self.session_path.clone() else {
            return;
        };
        let document = SessionFile::current(self.workspace.clone());
        self.save_generation = self.save_generation.wrapping_add(1);
        let generation = self.save_generation;
        cx.spawn(async move |this, cx| {
            let timer = cx.background_executor().timer(Duration::from_millis(250));
            timer.await;
            let latest = this
                .update(cx, |shell, _| shell.save_generation == generation)
                .unwrap_or(false);
            if latest {
                cx.background_executor()
                    .spawn(async move {
                        if let Err(error) = axf_workspace::save_session(&path, &document) {
                            tracing::warn!("no se pudo guardar la sesión: {error}");
                        }
                    })
                    .detach();
            }
        })
        .detach();
    }

    fn request_quit(&mut self, cx: &mut Context<'_, Self>) {
        if self.quitting {
            return;
        }
        self.quitting = true;
        self.sync_terminal_meta(cx);
        let path = self.session_path.clone();
        let document = SessionFile::current(self.workspace.clone());
        let history = self.history.clone();
        let keep: HashSet<TerminalId> = self.workspace.terminal_ids().collect();
        let requests: Vec<(TerminalId, axf_terminal::SnapshotRequest)> = self
            .terminals
            .iter()
            .filter_map(|(terminal, entry)| {
                entry
                    .view
                    .read(cx)
                    .request_snapshot()
                    .map(|request| (*terminal, request))
            })
            .collect();
        cx.spawn(async move |_, cx| {
            let writer = cx.background_executor().spawn(async move {
                for (terminal, request) in requests {
                    if let Some(snapshot) = request.wait().await
                        && let Err(error) = history.save(terminal, &snapshot)
                    {
                        tracing::warn!(
                            "no se pudo guardar el historial de la terminal {}: {error}",
                            terminal.get()
                        );
                    }
                }
                history.prune(&keep);
            });
            let _ = writer.await;
            if let Some(path) = path {
                let _ = cx
                    .background_executor()
                    .spawn(async move { axf_workspace::save_session(&path, &document) })
                    .await;
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }

    // --- Acciones -------------------------------------------------------
    fn quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.request_quit(cx);
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<'_, Self>) {
        let visible = self.workspace.sidebar().visible;
        self.set_sidebar_visible(!visible, cx);
    }

    /// Cambia la visibilidad de la sidebar y la persiste.
    fn set_sidebar_visible(&mut self, visible: bool, cx: &mut Context<'_, Self>) {
        if self.workspace.sidebar().visible == visible {
            return;
        }
        self.workspace.sidebar_mut().visible = visible;
        self.persist(cx);
        cx.notify();
    }

    fn open_project(&mut self, _: &OpenProject, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.prompt_open_project(cx);
    }

    fn focus_sidebar(&mut self, _: &FocusSidebar, window: &mut Window, cx: &mut Context<'_, Self>) {
        if !self.workspace.sidebar().visible {
            self.workspace.sidebar_mut().visible = true;
            self.persist(cx);
        }
        let rows = self.sidebar_rows();
        let current = self
            .sidebar_cursor
            .filter(|cursor| rows.contains(cursor))
            .or_else(|| {
                self.workspace
                    .current_project()
                    .map(SidebarRow::Project)
                    .filter(|row| rows.contains(row))
            });
        self.sidebar_cursor = current.or_else(|| rows.first().copied());
        window.focus(&self.sidebar_focus, cx);
        cx.notify();
    }

    fn toggle_sidebar_agents(
        &mut self,
        _: &ToggleSidebarAgents,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let section = self.sidebar_section.toggled();
        self.set_sidebar_section(section, cx);
    }

    fn set_sidebar_section(&mut self, section: SidebarSection, cx: &mut Context<'_, Self>) {
        if !self.workspace.sidebar().visible {
            self.workspace.sidebar_mut().visible = true;
        }
        self.workspace.sidebar_mut().section = section;
        self.sidebar_section = section;

        // El cursor anterior puede pertenecer a la otra sección.
        self.sidebar_cursor = self.sidebar_rows().first().copied();
        self.persist(cx);
        cx.notify();
    }

    fn open_command_palette(
        &mut self,
        _: &OpenCommandPalette,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_palette(PaletteMode::Commands, window, cx);
    }

    fn open_search_palette(
        &mut self,
        _: &OpenSearchPalette,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_palette(PaletteMode::Search, window, cx);
    }

    /// Abre una de las dos paletas; si la otra estaba abierta, la reemplaza.
    fn open_palette(&mut self, mode: PaletteMode, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self
            .palette
            .as_ref()
            .is_some_and(|palette| palette.mode == mode)
        {
            return;
        }
        self.palette = None;
        self.palette_subscription = None;
        self.shortcuts_modal = false;
        let items = self.palette_items(mode, cx);
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(mode.placeholder()));
        let subscription = cx.subscribe(&input, |shell, input, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let query = input.read(cx).value().to_string();
            let history = shell.palette_history.clone();
            if let Some(palette) = shell.palette.as_mut() {
                palette.refilter(&query, &history);
            }
            cx.notify();
        });
        let palette = PaletteState::new(mode, input.clone(), items, &self.palette_history);
        self.palette = Some(palette);
        self.palette_subscription = Some(subscription);
        let handle = input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    /// Recolecta las entradas de cada paleta. El orden de inserción es el
    /// desempate estable.
    fn palette_items(&mut self, mode: PaletteMode, cx: &mut Context<'_, Self>) -> Vec<PaletteItem> {
        match mode {
            PaletteMode::Commands => self.command_items(),
            PaletteMode::Search => self.search_items(cx),
        }
    }

    /// Acciones del registro, sin las que ya cubren la búsqueda o el modal de tema.
    fn command_items(&self) -> Vec<PaletteItem> {
        crate::actions::palette_actions()
            .map(|action| PaletteItem {
                id: format!("cmd:{}", action.id),
                label: action.label.to_owned(),
                detail: Some(action.category.label().to_owned()),
                search: format!("{} {}", action.id, action.keywords.join(" ")),
                kind: PaletteKind::Command,
                icon: PaletteIcon::Svg(crate::actions::category_icon(action.category)),
                shortcut: self.keymap.chords_for(action.id),
                target: PaletteTarget::Command(action.id),
            })
            .collect()
    }

    /// Navegación: proyectos, paneles, pestañas, agentes y perfiles.
    fn search_items(&mut self, cx: &mut Context<'_, Self>) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        let projects: Vec<(ProjectId, String, std::path::PathBuf)> = self
            .workspace
            .projects()
            .iter()
            .map(|project| (project.id, project.name.clone(), project.root.clone()))
            .collect();
        for (project, name, root) in projects {
            let count = self.workspace.project_tabs(project).len();
            items.push(PaletteItem {
                id: format!("project:{}", project.get()),
                label: name,
                detail: Some(format!(
                    "{count} terminal{} · {}",
                    if count == 1 { "" } else { "es" },
                    root.display()
                )),
                search: root.display().to_string(),
                kind: PaletteKind::Project,
                icon: PaletteIcon::Svg(axf_ui::icons::FOLDER),
                shortcut: None,
                target: PaletteTarget::RevealProject(project),
            });
        }
        for pane in self.workspace.layout().leaves() {
            let count = self
                .workspace
                .pane(pane)
                .map(|pane| pane.tabs.len())
                .unwrap_or(0);
            items.push(PaletteItem {
                id: format!("pane:{}", pane.get()),
                label: format!("Panel {}", pane.get()),
                detail: Some(format!(
                    "{count} pestaña{}",
                    if count == 1 { "" } else { "s" }
                )),
                search: format!("pane panel {}", pane.get()),
                kind: PaletteKind::Pane,
                icon: PaletteIcon::Svg(axf_ui::icons::SPLIT_HORIZONTAL),
                shortcut: None,
                target: PaletteTarget::FocusPane(pane),
            });
        }
        let tabs: Vec<(TabId, ProjectId)> = self
            .workspace
            .projects()
            .iter()
            .flat_map(|project| {
                self.workspace
                    .project_tabs(project.id)
                    .into_iter()
                    .map(move |tab| (tab, project.id))
            })
            .collect();
        for (tab, project_id) in &tabs {
            let title = self.tab_title(*tab, cx);
            let project = self
                .workspace
                .project(*project_id)
                .map(|project| project.name.clone())
                .unwrap_or_default();
            let attention = self.workspace.tab_agents(*tab).is_some_and(|sessions| {
                sessions
                    .iter()
                    .any(|session| session.state == AgentState::NeedsAttention)
            });
            items.push(PaletteItem {
                id: format!("tab:{}", tab.get()),
                label: format!("{}{title}", if attention { "! " } else { "" }),
                detail: Some(project),
                search: format!("terminal tab pestaña {}", tab.get()),
                kind: PaletteKind::Tab,
                icon: PaletteIcon::Profile(self.tab_icon(*tab)),
                shortcut: None,
                target: PaletteTarget::FocusTab(*tab),
            });
        }
        for (tab, project_id) in &tabs {
            let Some(sessions) = self.workspace.tab_agents(*tab) else {
                continue;
            };
            for session in sessions {
                if session.state != AgentState::NeedsAttention {
                    continue;
                }
                let title = self.tab_title(*tab, cx);
                let project = self
                    .workspace
                    .project(*project_id)
                    .map(|project| project.name.clone())
                    .unwrap_or_default();
                items.push(PaletteItem {
                    id: format!("agent:{}:{}", tab.get(), session.id),
                    label: format!("{} requiere atención · {title}", session.kind.as_str()),
                    detail: Some(project),
                    search: format!("agente agent atencion attention {}", session.kind.as_str()),
                    kind: PaletteKind::Agent,
                    icon: PaletteIcon::Svg(agent_icon(session.kind)),
                    shortcut: None,
                    target: PaletteTarget::JumpToAttention(*tab),
                });
            }
        }
        let profiles: Vec<(ProfileId, String)> = self
            .theme_host
            .profiles()
            .iter()
            .map(|profile| (profile.id.clone(), profile.name.clone()))
            .collect();
        for (id, name) in profiles {
            let icon = self.theme_host.profile_icon(&id);
            items.push(PaletteItem {
                id: format!("profile:{}", id.as_str()),
                label: format!("Nueva terminal · {name}"),
                detail: Some(format!("perfil {}", id.as_str())),
                search: format!("perfil profile terminal nueva {}", id.as_str()),
                kind: PaletteKind::Profile,
                icon: PaletteIcon::Profile(icon),
                shortcut: None,
                target: PaletteTarget::NewTabWithProfile(id),
            });
        }
        items
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.palette.take().is_none() {
            return;
        }
        self.palette_subscription = None;
        self.restore_workspace_focus(window, cx);
        cx.notify();
    }
}
