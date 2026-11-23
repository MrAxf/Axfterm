impl Shell {
    /// Arranca una pestaña restaurada, cargando su historial en segundo plano.
    fn ensure_terminal(&mut self, tab: TabId, cx: &mut Context<'_, Self>) {
        let Some(model_tab) = self.workspace.tab(tab) else {
            return;
        };
        let terminal = model_tab.terminal_id;
        let profile = model_tab.profile_id.clone();
        if self.terminals.contains_key(&terminal)
            || self.pending_terminals.contains(&terminal)
            || self.spawn_errors.contains_key(&terminal)
        {
            return;
        }

        // Un perfil ausente tiene su propia pantalla accionable; no se lanza
        // otro shell por sorpresa.
        if self.theme_host.profile(&profile).is_none() {
            return;
        }
        self.pending_terminals.insert(terminal);
        let history = self.history.clone();
        cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move { history.load(terminal) })
                .await;
            let _ = this.update_in(cx, |shell, window, cx| {
                shell.pending_terminals.remove(&terminal);
                if shell.workspace.tab(tab).is_none() || shell.terminals.contains_key(&terminal) {
                    return;
                }
                let (restore, restore_error, truncated) = match loaded {
                    Ok(Some(snapshot)) if !snapshot.is_empty() => {
                        let truncated = snapshot.truncated();
                        (Some(snapshot), None, truncated)
                    }
                    Ok(Some(_)) => {
                        shell.history.remove(terminal);
                        (None, None, false)
                    }
                    Ok(None) => (None, None, false),
                    Err(error) => {
                        if error.discardable() {
                            shell.history.remove(terminal);
                        }
                        (None, Some(error.to_string()), false)
                    }
                };
                let title = shell.tab_title(tab, cx);
                let cwd = shell
                    .workspace
                    .tab(tab)
                    .and_then(|tab| shell.workspace.project(tab.project_id))
                    .map(|project| project.root.clone());
                let focus = shell.workspace.focus().tab == Some(tab);
                shell.spawn_terminal_with_cwd(
                    tab,
                    terminal,
                    cwd,
                    SpawnOptions { restore, focus },
                    window,
                    cx,
                );

                // Si el shell tampoco arrancó, manda su error, no el del historial.
                if !shell.terminals.contains_key(&terminal) {
                    return;
                }
                if let Some(error) = restore_error {
                    shell.set_notice(
                        format!(
                            "No se pudo restaurar el historial de «{title}»: {error}. \
                             Se abrió una terminal nueva con el mismo perfil."
                        ),
                        true,
                        cx,
                    );
                } else if truncated {
                    shell.set_notice(
                        format!(
                            "Historial de «{title}» restaurado parcialmente \
                             (límite de 2 MiB por terminal)."
                        ),
                        false,
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    fn spawn_terminal_with_cwd(
        &mut self,
        tab: TabId,
        terminal: TerminalId,
        cwd: Option<PathBuf>,
        options: SpawnOptions,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let SpawnOptions { restore, focus } = options;
        if self.terminals.contains_key(&terminal) {
            return;
        }
        let Some(cwd) = cwd else {
            self.set_notice("No se pudo resolver la carpeta del proyecto.", true, cx);
            return;
        };
        if !cwd.is_dir() {
            self.set_notice(
                format!("La carpeta del proyecto no existe: {}", cwd.display()),
                true,
                cx,
            );
            return;
        }
        let Some(profile_id) = self.workspace.tab(tab).map(|tab| tab.profile_id.clone()) else {
            self.set_notice("La pestaña ya no existe.", true, cx);
            return;
        };
        let prepared = match self.theme_host.prepare_profile(&profile_id) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.spawn_errors.insert(terminal, error.clone());
                self.set_notice(error, true, cx);
                if focus {
                    self.pending_focus = Some(tab);
                }
                cx.notify();
                return;
            }
        };
        self.spawn_errors.remove(&terminal);
        let style = terminal_style(cx);
        let view =
            cx.new(|cx| TerminalView::new(window, cx, style, cwd, &prepared.spec, restore, focus));
        let terminal_for_meta = terminal;
        let subscription = cx.observe(&view, move |shell, _, cx| {
            shell.sync_terminal_meta(cx);
            shell.drain_terminal_signals(terminal_for_meta, cx);
            cx.notify();
        });
        self.terminals.insert(
            terminal,
            TerminalEntry {
                view: view.clone(),
                _subscription: subscription,
            },
        );
        if let Some(resume) = self
            .workspace
            .take_tab_resume_agent(tab)
            .as_deref()
            .and_then(AgentKind::from_name)
        {
            let command = resume.resume_command();
            cx.spawn(async move |_, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let _ = view.update(cx, |view, _| view.send_command(command));
            })
            .detach();
            self.persist(cx);
        }
        if focus {
            self.pending_focus = Some(tab);
        }
    }

    fn default_profile(&self) -> Option<(ProfileId, String)> {
        self.theme_host
            .default_profile()
            .map(|profile| (profile.id.clone(), profile.name.clone()))
    }

    fn profile_title(&self, id: &ProfileId) -> Option<String> {
        self.theme_host.profile_name(id)
    }

    /// Icono de la pestaña: marca del agente dominante o perfil con fallback.
    fn tab_icon(&mut self, tab: TabId) -> ProfileIconSource {
        if let Some(aggregate) = self
            .workspace
            .tab(tab)
            .and_then(|tab| tab.agent_aggregate())
        {
            return ProfileIconSource::Svg(agent_icon(aggregate.dominant.kind));
        }
        let Some(profile_id) = self.workspace.tab(tab).map(|tab| tab.profile_id.clone()) else {
            return ProfileIconSource::fallback();
        };
        let source = self.theme_host.profile_icon(&profile_id);
        match source {
            ProfileIconSource::Image(path) => ProfileIconSource::Image(path),
            ProfileIconSource::Svg(_) => {
                let stem = self
                    .theme_host
                    .profile(&profile_id)
                    .map(|profile| profile.program_stem())
                    .filter(|stem| !stem.is_empty());
                ProfileIconSource::Svg(terminal_tab_icon(stem.as_deref()))
            }
        }
    }

    fn tab_title(&self, tab: TabId, cx: &mut Context<'_, Self>) -> String {
        let Some(model_tab) = self.workspace.tab(tab) else {
            return String::new();
        };
        let entry = self.terminals.get(&model_tab.terminal_id);
        let program = entry.and_then(|entry| entry.view.read(cx).program().map(str::to_owned));
        let live_title = entry.and_then(|entry| entry.view.read(cx).title().map(str::to_owned));
        let project = self
            .workspace
            .project(model_tab.project_id)
            .map(|project| project.name.as_str());
        resolve_tab_title(
            model_tab,
            live_title.as_deref(),
            program.as_deref(),
            project,
        )
    }

    fn terminal_cwd(&self, tab: TabId, cx: &mut Context<'_, Self>) -> Option<PathBuf> {
        let model_tab = self.workspace.tab(tab)?;
        if let Some(entry) = self.terminals.get(&model_tab.terminal_id)
            && let Some(cwd) = entry.view.read(cx).cwd()
        {
            return Some(cwd.clone());
        }
        self.workspace
            .project(model_tab.project_id)
            .map(|project| project.root.clone())
    }

    fn pane_has_live_processes(&self, pane: PaneId, cx: &mut Context<'_, Self>) -> bool {
        self.workspace
            .pane(pane)
            .map(|pane| {
                pane.tabs.iter().any(|tab| {
                    self.workspace
                        .tab(*tab)
                        .and_then(|tab| self.terminals.get(&tab.terminal_id))
                        .is_some_and(|entry| entry.view.read(cx).is_running())
                })
            })
            .unwrap_or(false)
    }

    fn project_live_terminals(&self, project: ProjectId, cx: &mut Context<'_, Self>) -> Vec<TabId> {
        self.workspace
            .project_tabs(project)
            .into_iter()
            .filter(|tab| {
                self.workspace
                    .tab(*tab)
                    .and_then(|tab| self.terminals.get(&tab.terminal_id))
                    .is_some_and(|entry| entry.view.read(cx).is_running())
            })
            .collect()
    }

    fn focus_tab(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Err(error) = self.workspace.focus_tab(tab) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }

        // Enfocar limpia atención/terminado/error y retira sesiones sin proceso.
        let _ = self
            .workspace
            .acknowledge_tab_agents(tab, SystemTime::now());
        self.reveal_focus();
        self.ensure_terminal(tab, cx);
        if let Some(model_tab) = self.workspace.tab(tab)
            && let Some(entry) = self.terminals.get(&model_tab.terminal_id)
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        } else {
            window.focus(&self.focus, cx);
        }
        self.persist(cx);
        cx.notify();
    }

    fn reveal_focus(&mut self) {
        if let Some(project) = self.workspace.current_project() {
            let _ = self.workspace.set_project_expanded(project, true);
        }
    }

    fn reveal_project(&mut self, project: ProjectId) {
        let _ = self.workspace.set_project_expanded(project, true);
    }

    fn sidebar_cursor_project(&self) -> Option<ProjectId> {
        match self.sidebar_cursor {
            Some(SidebarRow::Project(project)) => Some(project),
            Some(SidebarRow::Tab(tab)) => self.workspace.tab(tab).map(|tab| tab.project_id),
            None => None,
        }
    }

    fn set_notice(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<'_, Self>) {
        self.push_notice(text.into(), error, None, None, cx);
    }

    /// Añade un aviso con deduplicación opcional y cola acotada.
    fn push_notice(
        &mut self,
        text: String,
        error: bool,
        key: Option<String>,
        detail: Option<String>,
        cx: &mut Context<'_, Self>,
    ) {
        let now = Instant::now();
        if let Some(key) = &key {
            self.agents
                .notice_cooldowns
                .retain(|_, at| now.duration_since(*at) < NOTICE_COOLDOWN);
            if self.agents.notice_cooldowns.contains_key(key) {
                return;
            }
            self.agents.notice_cooldowns.insert(key.clone(), now);
            self.notices
                .retain(|notice| notice.key.as_ref() != Some(key));
        }
        self.notice_seq = self.notice_seq.wrapping_add(1);
        let seq = self.notice_seq;
        self.notices.push(Notice {
            text,
            detail,
            error,
            key,
            seq,
        });
        while self.notices.len() > MAX_NOTICES {
            self.notices.remove(0);
        }
        let seconds = if error { 8 } else { 4 };
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(seconds))
                .await;
            this.update(cx, |shell, cx| {
                let before = shell.notices.len();
                shell.notices.retain(|notice| notice.seq != seq);
                if shell.notices.len() != before {
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}
