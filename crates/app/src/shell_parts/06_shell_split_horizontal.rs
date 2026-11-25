impl Shell {
    fn split_horizontal(
        &mut self,
        _: &SplitHorizontal,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.split(SplitAxis::Horizontal, window, cx);
    }

    fn split_vertical(
        &mut self,
        _: &SplitVertical,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.split(SplitAxis::Vertical, window, cx);
    }

    fn split(&mut self, axis: SplitAxis, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let project = self
            .workspace
            .current_project()
            .or_else(|| self.workspace.projects().first().map(|project| project.id));
        let Some(project) = project else {
            self.prompt_open_project(cx);
            return;
        };
        let pane = self.workspace.focus().pane;
        let (profile, title) = match self.resolve_project_profile(project, None) {
            Ok(profile) => profile,
            Err(error) => {
                self.set_notice(error, true, cx);
                return;
            }
        };
        let new_pane = match self.workspace.split_pane(pane, axis) {
            Ok(pane) => pane,
            Err(error) => {
                self.set_notice(error.to_string(), true, cx);
                return;
            }
        };
        match self.workspace.new_tab(new_pane, project, &profile, &title) {
            Ok(new_tab) => {
                self.reveal_project(project);
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn close_pane(&mut self, _: &ClosePane, window: &mut Window, cx: &mut Context<'_, Self>) {
        let pane = self.workspace.focus().pane;
        if self.pane_has_live_processes(pane, cx) {
            self.confirm(
                window,
                "Cerrar panel",
                "Hay procesos vivos en este panel. Cerrarlo terminará sus terminales.",
                PendingConfirm::ClosePane(pane),
                cx,
            );
        } else {
            self.close_pane_impl(pane, cx);
        }
    }

    fn close_pane_impl(&mut self, pane: PaneId, cx: &mut Context<'_, Self>) {
        let terminals: Vec<TerminalId> = self
            .workspace
            .pane(pane)
            .map(|pane| {
                pane.tabs
                    .iter()
                    .filter_map(|tab| self.workspace.tab(*tab).map(|tab| tab.terminal_id))
                    .collect()
            })
            .unwrap_or_default();
        match self.workspace.close_pane(pane) {
            Ok(_tabs) => {
                for terminal in terminals {
                    self.terminals.remove(&terminal);
                    self.pending_terminals.remove(&terminal);
                    self.spawn_errors.remove(&terminal);
                    self.history.remove(terminal);
                }
                if self.zoomed == Some(pane) {
                    self.zoomed = None;
                }
                if let Some(tab) = self.workspace.focus().tab {
                    self.pending_focus = Some(tab);
                    self.ensure_terminal(tab, cx);
                }
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn focus_next_pane(
        &mut self,
        _: &FocusNextPane,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.cycle_pane(1, window, cx);
    }

    fn focus_previous_pane(
        &mut self,
        _: &FocusPreviousPane,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.cycle_pane(-1, window, cx);
    }

    fn cycle_pane(&mut self, delta: isize, window: &mut Window, cx: &mut Context<'_, Self>) {
        let leaves = self.workspace.layout().leaves();
        if leaves.is_empty() {
            return;
        }
        let current = leaves
            .iter()
            .position(|id| *id == self.workspace.focus().pane)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(leaves.len() as isize) as usize;
        let pane = leaves[next];
        if let Err(error) = self.workspace.focus_pane(pane) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        if let Some(tab) = self.workspace.focus().tab {
            self.focus_tab(tab, window, cx);
        } else {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    fn rename_tab(&mut self, _: &RenameTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab {
            self.begin_rename(tab, window, cx);
        }
    }

    fn begin_rename(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let original = self.tab_title(tab, cx);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(original.clone()));
        let subscription =
            cx.subscribe(&input, |shell, input, event: &InputEvent, cx| match event {
                InputEvent::PressEnter { .. } => shell.finish_rename(input.clone(), true, cx),
                // Perder el foco equivale a confirmar: el clic que lo provoca
                // decide dónde queda el foco, así que aquí no se fuerza.
                InputEvent::Blur => shell.finish_rename(input.clone(), false, cx),
                InputEvent::Change | InputEvent::Focus => {}
            });
        self.editing = Some(TabEdit {
            tab,
            input,
            original,
            focused: false,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Cierra el editor inline y, si el texto cambió, renombra la pestaña.
    ///
    /// `refocus` solo se activa al confirmar con Enter: en un blur manda el
    /// destino del clic y robarle el foco sería un error.
    fn finish_rename(
        &mut self,
        input: Entity<InputState>,
        refocus: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        let value = input.read(cx).value();
        let title = value.as_ref().trim();

        // Un doble clic seguido de clic fuera no debe congelar el título
        // automático ni marcar la pestaña como renombrada.
        if title != edit.original.trim() {
            match self.workspace.rename_tab(edit.tab, title) {
                Ok(()) => self.persist(cx),
                Err(error) => self.set_notice(error.to_string(), true, cx),
            }
        }
        if refocus {
            self.pending_focus = self.workspace.focus().tab;
        }
        cx.notify();
    }

    fn cancel_rename(&mut self, cx: &mut Context<'_, Self>) {
        if self.editing.take().is_some() {
            self.pending_focus = self.workspace.focus().tab;
            cx.notify();
        }
    }

    fn duplicate_tab(&mut self, _: &DuplicateTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab {
            self.duplicate_tab_with_window(tab, window, cx);
        }
    }

    fn toggle_zoom(&mut self, _: &ToggleZoom, _: &mut Window, cx: &mut Context<'_, Self>) {
        let pane = self.workspace.focus().pane;
        self.zoomed = if self.zoomed == Some(pane) {
            None
        } else {
            Some(pane)
        };
        cx.notify();
    }

    fn restart_tab(&mut self, _: &RestartTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab {
            self.restart_tab_id(tab, window, cx);
        }
    }

    fn restart_tab_id(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(model_tab) = self.workspace.tab(tab) else {
            return;
        };
        let terminal = model_tab.terminal_id;
        if self
            .terminals
            .get(&terminal)
            .is_some_and(|entry| entry.view.read(cx).is_running())
        {
            self.set_notice(
                "La terminal sigue viva; ciérrala o espera a que termine.",
                false,
                cx,
            );
            return;
        }
        self.terminals.remove(&terminal);
        self.pending_terminals.remove(&terminal);
        self.spawn_errors.remove(&terminal);
        self.spawn_terminal(tab, terminal, window, cx);
        self.persist(cx);
        cx.notify();
    }

    /// Cambia una pestaña sin perfil al predeterminado tras una elección
    /// explícita del usuario.
    fn use_default_profile(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(project) = self.workspace.tab(tab).map(|tab| tab.project_id) else {
            self.set_notice("La pestaña ya no existe.", true, cx);
            return;
        };
        let (profile, title) = match self.resolve_project_profile(project, None) {
            Ok(profile) => profile,
            Err(error) => {
                self.set_notice(error, true, cx);
                return;
            }
        };
        if let Err(error) = self.workspace.set_tab_profile(tab, profile, &title) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        let Some(terminal) = self.workspace.tab(tab).map(|tab| tab.terminal_id) else {
            return;
        };
        self.terminals.remove(&terminal);
        self.spawn_errors.remove(&terminal);
        self.spawn_terminal(tab, terminal, window, cx);
        self.persist(cx);
        cx.notify();
    }

    fn confirm(
        &mut self,
        window: &mut Window,
        title: &'static str,
        description: &'static str,
        action: PendingConfirm,
        cx: &mut Context<'_, Self>,
    ) {
        let this = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let this = this.clone();
            alert
                .title(title)
                .description(description)
                .confirm()
                .button_props(
                    axf_ui::gpui::component::dialog::DialogButtonProps::default()
                        .ok_text("Cerrar")
                        .cancel_text("Cancelar"),
                )
                .on_ok(move |_, _, cx| {
                    if let Some(this) = this.upgrade() {
                        let _ = this.update(cx, |shell, cx| {
                            shell.apply_confirm(action, cx);
                        });
                    }
                    true
                })
        });
    }

    fn apply_confirm(&mut self, action: PendingConfirm, cx: &mut Context<'_, Self>) {
        match action {
            PendingConfirm::ClosePane(pane) => self.close_pane_impl(pane, cx),
            PendingConfirm::RemoveProject(project) => {
                self.remove_project_impl(project, cx);
            }
        }
    }

    fn remove_project_checked(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let live_tabs = self.project_live_terminals(project, cx);
        if !live_tabs.is_empty() {
            self.confirm(
                window,
                "Quitar proyecto",
                "Hay procesos vivos en este proyecto. Quitarlo cerrará sus terminales.",
                PendingConfirm::RemoveProject(project),
                cx,
            );
            return;
        }
        self.remove_project_impl(project, cx);
    }

    fn remove_project_impl(&mut self, project: ProjectId, cx: &mut Context<'_, Self>) {
        let terminals: Vec<TerminalId> = self
            .workspace
            .project_tabs(project)
            .into_iter()
            .filter_map(|tab| self.workspace.tab(tab).map(|tab| tab.terminal_id))
            .collect();
        match self.workspace.remove_project(project) {
            Ok(_tabs) => {
                self.project_health.remove(&project);
                for terminal in terminals {
                    self.terminals.remove(&terminal);
                    self.pending_terminals.remove(&terminal);
                    self.spawn_errors.remove(&terminal);
                    self.history.remove(terminal);
                }
                if let Some(tab) = self.workspace.focus().tab {
                    self.pending_focus = Some(tab);
                    self.ensure_terminal(tab, cx);
                }
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    // --- Helpers de modelo/terminal -------------------------------------
    fn refresh_terminal_styles(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.terminals.is_empty() {
            return;
        }
        let style = terminal_style(cx);
        let views: Vec<Entity<TerminalView>> = self
            .terminals
            .values()
            .map(|entry| entry.view.clone())
            .collect();
        for view in views {
            view.update(cx, |view, cx| view.update_style(window, cx, style.clone()));
        }
    }

    fn spawn_terminal(
        &mut self,
        tab: TabId,
        terminal: TerminalId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let target = self
            .workspace
            .tab(tab)
            .and_then(|tab| self.project_target(tab.project_id));
        self.spawn_terminal_with_target(
            tab,
            terminal,
            target,
            SpawnOptions {
                restore: None,
                focus: true,
            },
            window,
            cx,
        );
    }

    /// Restaura las terminales de las pestañas activas de cada panel visible.
    ///
    /// Las pestañas ocultas no arrancan hasta que se activan; así reabrir un
    /// workspace con muchas terminales no lanza todos los PTYs de golpe.
    fn restore_visible_terminals(&mut self, cx: &mut Context<'_, Self>) {
        let tabs: Vec<TabId> = self
            .workspace
            .layout()
            .leaves()
            .into_iter()
            .filter_map(|pane| self.workspace.pane(pane).and_then(|pane| pane.active))
            .collect();
        for tab in tabs {
            self.ensure_terminal(tab, cx);
        }
    }
}
