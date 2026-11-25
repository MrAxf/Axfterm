impl Shell {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        startup: Vec<PathBuf>,
        theme_host: ThemeHost,
        keymap: Keymap,
        keymap_notes: Vec<String>,
        #[cfg(feature = "studio")] studio_launch: Option<crate::studio::Launch>,
    ) -> Self {
        let focus = cx.focus_handle();
        let sidebar_focus = cx.focus_handle();
        let settings_focus = cx.focus_handle();
        let settings_profile_focus = cx.focus_handle();
        let shortcuts_focus = cx.focus_handle();
        window.focus(&focus, cx);
        let session_path = match axf_config::session_path() {
            Ok(path) => Some(path),
            Err(error) => {
                // Seguimos sin sesión: el workspace funciona en memoria.
                tracing::warn!("no se pudo resolver session.json: {error}");
                let _ = axf_config::ensure_config_dir();
                None
            }
        };
        let load = session_path
            .as_deref()
            .map(|path| {
                axf_workspace::load_or_recover(path, &theme_host.settings.terminal.default_profile)
            })
            .unwrap_or_default();
        let startup_warning = load.warning.or_else(|| {
            session_path.is_none().then(|| {
                "No se pudo resolver el directorio de configuración; la sesión no se guardará."
                    .to_owned()
            })
        });
        let fresh_session = load.session.is_none();
        let workspace = load
            .session
            .map(|session| session.workspace)
            .unwrap_or_default();
        let history = HistoryStore::from_config(fresh_session);
        let theme_notes = theme_host.notes.clone();
        let sidebar_section = workspace.sidebar().section;
        let mut shell = Self {
            workspace,
            session_path,
            focus,
            sidebar_focus,
            settings_focus,
            terminals: HashMap::new(),
            pending_terminals: HashSet::new(),
            spawn_errors: HashMap::new(),
            history,
            pane_scroll: HashMap::new(),
            body_bounds: Rc::new(RefCell::new(HashMap::new())),
            split_bounds: Rc::new(RefCell::new(HashMap::new())),
            revealed: HashMap::new(),
            last_focused_pane: None,
            tab_drop: None,
            tab_insert: None,
            file_drag_hover: FileDragHoverState::default(),
            sidebar_list_bounds: Rc::new(RefCell::new(None)),
            zoomed: None,
            sidebar_cursor: None,
            sidebar_section,
            editing: None,
            pending_focus: None,
            notices: Vec::new(),
            notice_seq: 0,
            agents: AgentUiState::default(),
            palette: None,
            palette_subscription: None,
            palette_history: PaletteHistory::default(),
            wsl_open: None,
            project_health: HashMap::new(),
            ui_font_select: None,
            terminal_font_select: None,
            font_select_subscriptions: Vec::new(),
            shortcuts_modal: false,
            shortcuts_focus,
            keymap,
            theme_host,
            settings: None,
            settings_scroll: ScrollHandle::new(),
            settings_profile_editor: None,
            settings_profile_focus,
            settings_sliders: None,
            settings_agent_selects: None,
            settings_shortcut_input: None,
            settings_shortcut_subscription: None,
            keystroke_interceptor: None,
            style_revision: 0,
            appearance_subscription: None,
            window_title: "Axfterm".into(),
            startup: crate::process_start(),
            startup_logged: false,
            save_generation: 0,
            quitting: false,
            close_hook_registered: false,
            #[cfg(feature = "studio")]
            studio: None,
        };
        #[cfg(feature = "studio")]
        if let Some(launch) = &studio_launch {
            if launch.gallery_only {
                shell.workspace = Workspace::new();
                shell.session_path = None;
                shell.sidebar_cursor = None;
            }
            match crate::studio::attach(launch, window, cx) {
                Ok(state) => shell.studio = Some(state),
                Err(error) => shell.set_notice(error, true, cx),
            }
            if let Some(fixture) = launch.fixture.as_deref()
                && shell.studio.is_some()
                && let Err(error) = shell.studio_workspace(fixture, window, cx)
            {
                shell.set_notice(error, true, cx);
            }
        }
        for path in startup {
            shell.open_project_path(path, window, cx);
        }
        // Otras instancias reenvían sus carpetas por el pipe local (M9).
        if let Some(receiver) = crate::single_instance::take_receiver() {
            cx.spawn(async move |this, cx| {
                while let Ok(paths) = receiver.recv().await {
                    let updated = this.update_in(cx, |shell, window, cx| {
                        for path in paths {
                            shell.open_project_path(path, window, cx);
                        }
                        window.activate_window();
                        cx.notify();
                    });
                    if updated.is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        if let Some(tab) = shell.workspace.focus().tab {
            shell.pending_focus = Some(tab);
        }
        shell.restore_visible_terminals(cx);
        shell.spawn_project_health_checks(cx);
        shell.reveal_focus();
        shell.watch_config(cx);
        shell.spawn_wsl_discovery(cx);
        shell.spawn_agent_scanner(cx);
        let this = cx.entity().downgrade();
        shell.keystroke_interceptor = Some(cx.intercept_keystrokes(move |event, _window, cx| {
            let handled = this
                .update(cx, |shell, cx| {
                    shell.settings_capture_keystroke(&event.keystroke, cx)
                })
                .unwrap_or(false);
            if handled {
                cx.stop_propagation();
            }
        }));
        let this = cx.entity().downgrade();
        shell.appearance_subscription = Some(window.observe_window_appearance(move |_, cx| {
            if let Some(this) = this.upgrade() {
                let _ = this.update(cx, |shell, cx| {
                    if shell.theme_host.settings.appearance == axf_config::Appearance::System {
                        let messages = shell.theme_host.apply_current(cx);
                        for message in messages {
                            shell.set_notice(message, false, cx);
                        }
                        cx.notify();
                    }
                });
            }
        }));
        let mut warnings = theme_notes;
        warnings.extend(keymap_notes);
        if let Some(warning) = startup_warning {
            warnings.push(warning);
        }
        if !warnings.is_empty() {
            shell.set_notice(warnings.join(" "), true, cx);
        }
        shell
    }

    /// Detecta WSL en segundo plano solo cuando los perfiles vienen del
    /// descubrimiento inicial; una config con perfiles propios no se toca.
    fn spawn_wsl_discovery(&mut self, cx: &mut Context<'_, Self>) {
        if !self.theme_host.profiles_detected {
            return;
        }
        self.theme_host.profiles_detected = false;
        cx.spawn(async move |this, cx| {
            let discovered = cx
                .background_executor()
                .spawn(async { axf_config::discover_wsl() })
                .await;
            if discovered.is_empty() {
                return;
            }
            this.update(cx, |shell, cx| {
                match shell.theme_host.merge_wsl_profiles(discovered, cx) {
                    Ok(added) if !added.is_empty() => {
                        let names: Vec<String> = added
                            .iter()
                            .filter_map(|id| shell.theme_host.profile_name(id))
                            .collect();
                        shell.set_notice(
                            format!("Terminales WSL añadidas: {}.", names.join(", ")),
                            false,
                            cx,
                        );
                        shell.persist(cx);
                        cx.notify();
                    }
                    Ok(_) => {}
                    Err(error) => shell.set_notice(error, true, cx),
                }
            })
            .ok();
        })
        .detach();
    }

    // --- Agentes --------------------------------------------------------
    /// Escaneo de procesos en segundo plano; nunca bloquea el hilo de UI.
    fn spawn_agent_scanner(&mut self, cx: &mut Context<'_, Self>) {
        let scanner = Arc::new(Mutex::new(ProcessScanner::new()));
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(settings) =
                    this.update(cx, |shell, _| shell.theme_host.settings.agents.clone())
                else {
                    break;
                };
                if !settings.enabled {
                    let _ = this.update(cx, |shell, cx| {
                        if shell.workspace.clear_agents() {
                            cx.notify();
                        }
                    });
                    cx.background_executor()
                        .timer(Duration::from_millis(1_000))
                        .await;
                    continue;
                }
                let roots = this
                    .update(cx, |shell, cx| shell.terminal_roots(cx))
                    .unwrap_or_default();
                if !roots.is_empty() {
                    let scanner = Arc::clone(&scanner);
                    let snapshot = cx
                        .background_executor()
                        .spawn(async move {
                            match scanner.lock() {
                                Ok(mut scanner) => scanner.scan(),
                                Err(poisoned) => poisoned.into_inner().scan(),
                            }
                        })
                        .await;
                    let alive = this
                        .update(cx, |shell, cx| shell.apply_agent_snapshot(&snapshot, cx))
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_millis(settings.scan_interval_ms))
                    .await;
            }
        })
        .detach();
    }

    /// PID raíz de cada terminal viva.
    fn terminal_roots(&self, cx: &mut Context<'_, Self>) -> Vec<(TerminalId, u32)> {
        self.terminals
            .iter()
            .filter_map(|(terminal, entry)| entry.view.read(cx).pid().map(|pid| (*terminal, pid)))
            .collect()
    }

    /// Reconcilia las sesiones de cada pestaña con un snapshot de procesos.
    fn apply_agent_snapshot(&mut self, snapshot: &ProcessSnapshot, cx: &mut Context<'_, Self>) {
        #[cfg(feature = "studio")]
        if self
            .studio
            .as_ref()
            .is_some_and(|studio| studio.agents_frozen)
        {
            return;
        }
        let now = SystemTime::now();
        let mut next_id = self.agents.seq;
        let mut updates: Vec<AgentTabUpdate> = Vec::new();
        for (terminal, entry) in &self.terminals {
            let Some(tab) = self.workspace.tab_for_terminal(*terminal) else {
                continue;
            };
            let view = entry.view.read(cx);
            let Some(pid) = view.pid() else {
                continue;
            };
            let running = view.is_running();
            let exit = view.exit_info();
            let recent_output = view
                .last_output_at()
                .is_some_and(|at| at.elapsed() < OUTPUT_QUIET);
            let detected = snapshot.agents_for(pid);
            let mut scan_next_id = next_id;
            let outcome = self.workspace.update_tab_agents(tab, |sessions| {
                reconcile_agents(
                    sessions,
                    &detected,
                    running,
                    exit,
                    recent_output,
                    now,
                    &mut scan_next_id,
                )
            });
            next_id = scan_next_id;
            if let Some((changed, transitions)) = outcome {
                updates.push(AgentTabUpdate {
                    tab,
                    changed,
                    transitions,
                });
            }
        }
        self.agents.seq = next_id;
        let mut changed = false;
        let mut persist = false;
        for update in updates {
            changed |= update.changed;
            let resume = self.tab_resume(update.tab, cx);
            if self.workspace.set_tab_resume(update.tab, resume) {
                persist = true;
            }
            for (session, transition) in update.transitions {
                self.notify_agent_transition(update.tab, session, transition, cx);
            }
        }
        if persist {
            self.persist(cx);
        }
        if changed {
            cx.notify();
        }
    }

    /// Procesa las señales OSC de una terminal para la máquina de agentes.
    fn drain_terminal_signals(&mut self, terminal: TerminalId, cx: &mut Context<'_, Self>) {
        let Some(entry) = self.terminals.get(&terminal) else {
            return;
        };
        let signals = entry.view.update(cx, |view, _| view.take_signals());
        if signals.is_empty() {
            return;
        }
        let Some(tab) = self.workspace.tab_for_terminal(terminal) else {
            return;
        };
        let now = SystemTime::now();
        let mut next_id = self.agents.seq;
        let mut transitions: AgentTransitions = Vec::new();
        let mut changed = false;
        for signal in signals {
            match signal {
                TerminalSignal::ProgramStatus(status) => {
                    let report = agent_report_from_status(&status);
                    let outcome = self.workspace.update_tab_agents(tab, |sessions| {
                        let hint = report.app.as_deref().and_then(AgentKind::from_app);
                        sessions.truncate(1);
                        let id = match sessions.first_mut() {
                            Some(session) => {
                                if let Some(kind) = hint
                                    && session.kind != kind
                                {
                                    // El id anterior pertenece a otro programa.
                                    session.external_id = None;
                                    session.kind = kind;
                                }
                                session.id
                            }
                            None => {
                                let kind = hint?;
                                let id = next_id;
                                next_id = next_id.wrapping_add(1);
                                sessions.push(AgentSession::new(id, kind, 0, now));
                                id
                            }
                        };
                        let session = sessions.first_mut()?;
                        Some((id, session.apply_report(&report, now)))
                    });
                    if let Some(outcome) = outcome.flatten() {
                        transitions.push(outcome);
                    }
                }
                TerminalSignal::Attention(message) => {
                    let message = message.or_else(|| Some("Señal de atención".into()));
                    let outcome = self.workspace.update_tab_agents(tab, |sessions| {
                        let session = sessions
                            .iter_mut()
                            .max_by_key(|session| (session.last_activity, session.id))?;
                        let id = session.id;
                        Some((id, session.apply_attention(None, message.clone(), now)))
                    });
                    if let Some(outcome) = outcome.flatten() {
                        transitions.push(outcome);
                    }
                }
                TerminalSignal::Exited(_) => {}
            }
        }
        self.agents.seq = next_id;
        let resume = self.tab_resume(tab, cx);
        let persist = self.workspace.set_tab_resume(tab, resume);
        for (session, transition) in transitions {
            changed = true;
            self.notify_agent_transition(tab, session, transition, cx);
        }
        if persist {
            self.persist(cx);
        }
        if changed {
            cx.notify();
        }
    }

    /// Aviso in-app y marca de título para un cambio de estado de agente.
    fn notify_agent_transition(
        &mut self,
        tab: TabId,
        session_id: AgentSessionId,
        transition: AgentTransition,
        cx: &mut Context<'_, Self>,
    ) {
        if !transition.notify {
            return;
        }
        let settings = &self.theme_host.settings.agents;
        let enabled = match transition.to {
            AgentState::Finished => settings.notify_finished,
            AgentState::Error => settings.notify_error,
            AgentState::NeedsAttention => settings.notify_attention,
            _ => false,
        };
        if !enabled || self.workspace.focus().tab == Some(tab) {
            return;
        }
        let Some(session) = self
            .workspace
            .tab_agents(tab)
            .and_then(|sessions| sessions.iter().find(|session| session.id == session_id))
            .cloned()
        else {
            return;
        };
        let title = self.tab_title(tab, cx);
        let (text, error) = match transition.to {
            AgentState::Finished => (format!("«{title}» terminó"), false),
            AgentState::Error => (format!("«{title}» falló"), true),
            AgentState::NeedsAttention => (format!("«{title}» requiere atención"), false),
            _ => return,
        };
        let detail = session.message.clone().or_else(|| {
            session
                .attention
                .map(|kind| format!("Motivo: {}", kind.label()))
        });
        let key = format!(
            "agent:{}:{}:{}",
            tab.get(),
            session.kind.as_str(),
            transition.to.label()
        );
        self.push_notice(text, error, Some(key), detail, cx);
    }
}
