impl Shell {
    fn palette_next(&mut self, _: &PaletteNext, _: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(palette) = self.palette.as_mut() {
            palette.next();
            cx.notify();
        }
    }

    fn palette_previous(
        &mut self,
        _: &PalettePrevious,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(palette) = self.palette.as_mut() {
            palette.previous();
            cx.notify();
        }
    }

    fn palette_switch_mode(
        &mut self,
        _: &PaletteSwitchMode,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(next) = self.palette.as_ref().map(|palette| match palette.mode {
            PaletteMode::Commands => PaletteMode::Search,
            PaletteMode::Search => PaletteMode::Commands,
        }) else {
            return;
        };
        self.palette_set_mode(next, window, cx);
    }

    /// Cambia de paleta sin cerrar el overlay y sin perder consulta ni filtro.
    fn palette_set_mode(
        &mut self,
        mode: PaletteMode,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((current, query)) = self
            .palette
            .as_ref()
            .map(|palette| (palette.mode, palette.input.read(cx).value().to_string()))
        else {
            return;
        };
        if current != mode {
            let items = self.palette_items(mode, cx);
            let history = self.palette_history.clone();
            if let Some(palette) = self.palette.as_mut() {
                palette.set_mode(mode, items, &query, &history);
            }
        }
        self.palette_focus_input(window, cx);
        cx.notify();
    }

    fn palette_set_filter(
        &mut self,
        filter: Option<PaletteKind>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !self
            .palette
            .as_ref()
            .is_some_and(|palette| palette.mode == PaletteMode::Search)
        {
            return;
        }
        let Some(query) = self
            .palette
            .as_ref()
            .map(|palette| palette.input.read(cx).value().to_string())
        else {
            return;
        };
        let history = self.palette_history.clone();
        if let Some(palette) = self.palette.as_mut() {
            palette.set_filter(filter, &query, &history);
        }
        self.palette_focus_input(window, cx);
        cx.notify();
    }

    fn palette_filter_all(
        &mut self,
        _: &PaletteFilterAll,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(None, window, cx);
    }

    fn palette_filter_tabs(
        &mut self,
        _: &PaletteFilterTabs,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Tab), window, cx);
    }

    fn palette_filter_panes(
        &mut self,
        _: &PaletteFilterPanes,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Pane), window, cx);
    }

    fn palette_filter_projects(
        &mut self,
        _: &PaletteFilterProjects,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Project), window, cx);
    }

    fn palette_filter_profiles(
        &mut self,
        _: &PaletteFilterProfiles,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Profile), window, cx);
    }

    fn palette_filter_agents(
        &mut self,
        _: &PaletteFilterAgents,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Agent), window, cx);
    }

    /// Devuelve el foco al input tras pulsar un segmento de modo o de filtro.
    fn palette_focus_input(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(input) = self.palette.as_ref().map(|palette| palette.input.clone()) else {
            return;
        };
        let handle = input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    fn palette_accept(
        &mut self,
        _: &PaletteAccept,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((id, target)) = self.palette.as_ref().and_then(|palette| {
            let row = palette.current()?;
            let item = palette.item_at(row);
            Some((item.id.clone(), item.target.clone()))
        }) else {
            return;
        };
        self.palette_history.record(&id);
        self.close_palette(window, cx);
        self.run_palette_target(target, window, cx);
    }

    fn palette_cancel(
        &mut self,
        _: &PaletteCancel,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
            return;
        }
        if self.shortcuts_modal {
            self.close_shortcuts(window, cx);
        }
    }

    fn palette_click(&mut self, row: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(palette) = self.palette.as_mut() {
            if row >= palette.rows.len() {
                return;
            }
            palette.selected = row;
        }
        self.palette_accept(&PaletteAccept, window, cx);
    }

    fn run_palette_target(
        &mut self,
        target: PaletteTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match target {
            PaletteTarget::Command(id) => {
                if let Some(action) = crate::actions::action_for(id) {
                    // `App::dispatch_action` reentra en la ventana que ya está
                    // actualizando este shell y el despacho se pierde. Window
                    // encola el despacho para después del frame actual.
                    window.dispatch_action(action, cx);
                }
            }
            PaletteTarget::FocusTab(tab) | PaletteTarget::JumpToAttention(tab) => {
                self.focus_tab(tab, window, cx);
            }
            PaletteTarget::FocusPane(pane) => {
                if self.workspace.focus_pane(pane).is_ok() {
                    if let Some(tab) = self.workspace.focus().tab {
                        self.focus_tab(tab, window, cx);
                    } else {
                        window.focus(&self.focus, cx);
                        cx.notify();
                    }
                }
            }
            PaletteTarget::RevealProject(project) => {
                self.workspace.sidebar_mut().visible = true;
                if self.sidebar_section != SidebarSection::All {
                    self.sidebar_section = SidebarSection::All;
                    self.workspace.sidebar_mut().section = SidebarSection::All;
                }
                let _ = self.workspace.set_project_expanded(project, true);
                self.sidebar_cursor = Some(SidebarRow::Project(project));
                self.persist(cx);
                cx.notify();
            }
            PaletteTarget::NewTabWithProfile(profile) => {
                self.new_tab_for(None, Some(profile), window, cx);
            }
        }
    }

    fn toggle_shortcuts(
        &mut self,
        _: &ShowShortcuts,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.shortcuts_modal {
            self.close_shortcuts(window, cx);
        } else {
            self.close_palette(window, cx);
            self.close_settings(window, cx);
            self.shortcuts_modal = true;
            window.focus(&self.shortcuts_focus, cx);
            cx.notify();
        }
    }

    fn close_shortcuts(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.shortcuts_modal {
            self.shortcuts_modal = false;
            self.restore_workspace_focus(window, cx);
            cx.notify();
        }
    }

    fn agent_next_attention(
        &mut self,
        _: &AgentNextAttention,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let attention = attention_tabs(&self.workspace);
        let Some(next) = next_attention(&attention, self.workspace.focus().tab) else {
            self.set_sidebar_section(SidebarSection::Agents, cx);
            self.set_notice("Ningún agente requiere atención.", false, cx);
            return;
        };
        self.focus_tab(next, window, cx);
    }

    // --- Ajustes -------------------------------------------------------
    /// Abre (o cierra) el panel de ajustes con el foco dentro.
    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.settings.is_some() {
            self.close_settings(window, cx);
            return;
        }
        self.close_palette(window, cx);
        self.shortcuts_modal = false;
        let baseline = self
            .theme_host
            .config_path
            .as_deref()
            .and_then(|path| axf_config::read_config_text(path).ok().flatten());
        self.settings = Some(SettingsState::new(
            self.theme_host.settings.clone(),
            baseline,
        ));
        self.settings_scroll = ScrollHandle::new();
        self.settings_profile_editor = None;
        self.settings_agent_selects = None;
        self.ensure_font_selects(window, cx);
        self.ensure_settings_sliders(cx);
        self.ensure_agent_profile_selects(window, cx);
        self.ensure_shortcut_input(window, cx);
        window.focus(&self.settings_focus, cx);
        cx.notify();
    }

    fn show_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.open_settings(window, cx);
    }

    fn settings_cancel(
        &mut self,
        _: &SettingsCancel,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(settings) = &self.settings
            && settings.capturing.is_some()
        {
            if let Some(settings) = &mut self.settings {
                settings.cancel_capture();
            }
            cx.notify();
            return;
        }
        if self.settings_profile_editor.is_some() {
            self.settings_profile_editor = None;
            window.focus(&self.settings_focus, cx);
            cx.notify();
            return;
        }
        if let Some(settings) = &self.settings
            && settings.pending_delete.is_some()
        {
            if let Some(settings) = &mut self.settings {
                settings.pending_delete = None;
            }
            window.focus(&self.settings_focus, cx);
            cx.notify();
            return;
        }
        if let Some(settings) = &self.settings
            && settings.pending_defaults
        {
            if let Some(settings) = &mut self.settings {
                settings.pending_defaults = false;
            }
            window.focus(&self.settings_focus, cx);
            cx.notify();
            return;
        }
        self.close_settings(window, cx);
    }

    fn settings_save(&mut self, _: &SettingsSave, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.save_settings(cx);
    }

    fn settings_next_section(
        &mut self,
        _: &SettingsNextSection,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(settings) = &mut self.settings {
            settings.move_section(1);
            self.settings_scroll = ScrollHandle::new();
            cx.notify();
        }
    }

    fn settings_previous_section(
        &mut self,
        _: &SettingsPreviousSection,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(settings) = &mut self.settings {
            settings.move_section(-1);
            self.settings_scroll = ScrollHandle::new();
            cx.notify();
        }
    }

    /// Cierra el panel descartando el borrador y restaurando la vista activa.
    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.settings.take().is_none() {
            return;
        }
        self.settings_profile_editor = None;
        self.settings_shortcut_input = None;
        self.settings_shortcut_subscription = None;
        self.font_select_subscriptions.clear();
        self.ui_font_select = None;
        self.terminal_font_select = None;
        self.settings_sliders = None;
        self.settings_agent_selects = None;
        let messages = self.theme_host.clear_preview(cx);
        if !messages.is_empty() {
            tracing::debug!("preview de ajustes descartada: {}", messages.join(" "));
        }
        self.restore_workspace_focus(window, cx);
        cx.notify();
    }

    /// Aplica el borrador al estado visual sin escribir en disco.
    fn preview_settings(&mut self, cx: &mut Context<'_, Self>) {
        let Some(settings) = &self.settings else {
            return;
        };
        let messages = self.theme_host.preview_settings(&settings.draft, cx);
        if !messages.is_empty()
            && let Some(settings) = &mut self.settings
        {
            settings.set_notice(messages.join(" "), false);
        }
        cx.notify();
    }
}
