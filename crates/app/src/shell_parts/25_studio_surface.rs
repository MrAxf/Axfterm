// Puente entre Axfterm Studio y las superficies privadas del shell.

use crate::studio::Surface;
use axf_ui::gallery::GallerySection;

fn studio_spawn_spec(
    mut spec: axf_terminal::SpawnSpec,
    isolate_user_profile: bool,
) -> axf_terminal::SpawnSpec {
    if !isolate_user_profile {
        return spec;
    }

    let filename = spec.program.rsplit(['\\', '/']).next().unwrap_or(&spec.program);
    let stem = filename
        .rsplit_once('.')
        .map_or(filename, |(stem, _)| stem);
    if !stem.eq_ignore_ascii_case("pwsh")
        && !stem.eq_ignore_ascii_case("powershell")
    {
        return spec;
    }
    if !spec
        .args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("-NoProfile"))
    {
        spec.args.push("-NoProfile".into());
    }
    spec
}

impl Shell {
    /// Cambia la superficie de la galería y le pasa el foco.
    pub(crate) fn studio_set_gallery_section(
        &mut self,
        section: GallerySection,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(studio) = self.studio.as_mut() else {
            return;
        };
        studio.surface = Surface::Gallery(section);
        studio.fixture = None;
        let gallery = studio.gallery.clone();
        gallery.update(cx, |gallery, cx| {
            gallery.set_section(section);
            cx.notify();
        });
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Abre una superficie de la app.
    pub(crate) fn studio_open_app_surface(
        &mut self,
        surface: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        let Some(studio) = self.studio.as_mut() else {
            return Err("Studio no está activo".into());
        };
        studio.surface = Surface::App;
        studio.app_surface = surface.to_owned();
        self.close_palette(window, cx);
        self.close_settings(window, cx);
        self.close_shortcuts(window, cx);

        match surface {
            "app.onboarding" => {
                self.studio_workspace("empty", window, cx)?;
                self.studio_set_app_surface("app.onboarding");
            }
            "app.workspace" => {
                self.studio_set_app_surface("app.workspace");
                self.restore_workspace_focus(window, cx);
            }
            "app.shortcuts" => {
                self.toggle_shortcuts(&ShowShortcuts, window, cx);
                self.studio_set_app_surface("app.shortcuts");
            }
            "app.sidebar.terminals" => {
                self.set_sidebar_section(SidebarSection::All, cx);
                self.studio_set_app_surface("app.sidebar.terminals");
                window.focus(&self.sidebar_focus, cx);
            }
            "app.sidebar.agents" => {
                self.set_sidebar_section(SidebarSection::Agents, cx);
                self.studio_set_app_surface("app.sidebar.agents");
                window.focus(&self.sidebar_focus, cx);
            }
            "app.wsl" => {
                self.studio_wsl_fixture(window, cx)?;
                self.studio_set_app_surface("app.wsl");
            }
            "app.wsl.live" => {
                self.open_wsl_modal(window, cx);
                self.studio_set_app_surface("app.wsl.live");
            }
            "app.wsl.project" => {
                self.studio_open_wsl_project(window, cx)?;
                self.studio_set_app_surface("app.wsl.project");
            }
            value if value.starts_with("app.settings.") => {
                let (section, editor, capture) = match value.trim_start_matches("app.settings.") {
                    "appearance" => (SettingsSection::Appearance, false, false),
                    "profiles" => (SettingsSection::Profiles, false, false),
                    "profiles.editor" => (SettingsSection::Profiles, true, false),
                    "shortcuts" => (SettingsSection::Shortcuts, false, false),
                    "shortcuts.capture" => (SettingsSection::Shortcuts, false, true),
                    "agents" => (SettingsSection::Agents, false, false),
                    other => return Err(format!("sección de ajustes desconocida: {other}")),
                };
                self.open_settings(window, cx);
                if let Some(settings) = &mut self.settings {
                    settings.set_section(section);
                    if capture {
                        settings.begin_capture("app.quit");
                    }
                }
                if editor {
                    let first = self
                        .settings
                        .as_ref()
                        .and_then(|settings| settings.profiles().first())
                        .map(|profile| profile.id.clone());
                    if let Some(id) = first {
                        self.open_profile_editor(id, window, cx);
                    }
                }
                self.settings_scroll = ScrollHandle::new();
                self.studio_set_app_surface(value);
                cx.notify();
            }
            value if value.starts_with("app.palette.") => {
                let (kind, query) = value
                    .trim_start_matches("app.palette.")
                    .split_once('?')
                    .map(|(kind, rest)| {
                        (
                            kind,
                            rest.strip_prefix("q=").unwrap_or(rest).to_owned(),
                        )
                    })
                    .unwrap_or((value.trim_start_matches("app.palette."), String::new()));
                let mode = match kind {
                    "commands" => PaletteMode::Commands,
                    "search" => PaletteMode::Search,
                    other => return Err(format!("paleta desconocida: {other}")),
                };
                self.open_palette(mode, window, cx);
                if !query.is_empty()
                    && let Some(palette) = &self.palette
                {
                    let input = palette.input.clone();
                    let history = self.palette_history.clone();
                    input.update(cx, |input, cx| input.set_value(query.clone(), window, cx));
                    if let Some(palette) = &mut self.palette {
                        palette.refilter(&query, &history);
                    }
                }
                self.studio_set_app_surface(value);
                cx.notify();
            }
            other => return Err(format!("superficie desconocida: {other}")),
        }
        Ok(())
    }

    fn studio_set_app_surface(&mut self, surface: &str) {
        if let Some(studio) = self.studio.as_mut() {
            studio.surface = Surface::App;
            studio.app_surface = surface.to_owned();
        }
    }

    /// Ejecuta una acción segura del registro.
    pub(crate) fn studio_dispatch_action(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        use crate::{
            AgentNextAttention, ClosePane, CloseTab, DuplicateTab, FocusNextPane, FocusPreviousPane,
            MoveTabNextPane, NewTab, NextTheme, NextTab, OpenCommandPalette, OpenSearchPalette,
            PreviousTab, PreviousTheme, ReloadThemes, RenameTab, RestartTab, ShowShortcuts,
            SplitHorizontal, SplitVertical, ToggleSidebar, ToggleSidebarAgents, ToggleThemeVariant,
            ToggleZoom,
        };
        let action: Box<dyn axf_ui::gpui::Action> = match id {
            "sidebar.toggle" => Box::new(ToggleSidebar),
            "sidebar.focus" => Box::new(crate::FocusSidebar),
            "sidebar.toggle_agents" => Box::new(ToggleSidebarAgents),
            "tab.new" => Box::new(NewTab),
            "tab.close" => Box::new(CloseTab),
            "tab.next" => Box::new(NextTab),
            "tab.previous" => Box::new(PreviousTab),
            "tab.rename" => Box::new(RenameTab),
            "tab.duplicate" => Box::new(DuplicateTab),
            "tab.restart" => Box::new(RestartTab),
            "tab.move_next_pane" => Box::new(MoveTabNextPane),
            "pane.split_horizontal" => Box::new(SplitHorizontal),
            "pane.split_vertical" => Box::new(SplitVertical),
            "pane.close" => Box::new(ClosePane),
            "pane.focus_next" => Box::new(FocusNextPane),
            "pane.focus_previous" => Box::new(FocusPreviousPane),
            "pane.toggle_zoom" => Box::new(ToggleZoom),
            "palette.open" => Box::new(OpenCommandPalette),
            "palette.search" => Box::new(OpenSearchPalette),
            "settings.open" | "theme.info" => Box::new(crate::OpenSettings),
            "theme.toggle_variant" => Box::new(ToggleThemeVariant),
            "theme.next" => Box::new(NextTheme),
            "theme.previous" => Box::new(PreviousTheme),
            "theme.reload" => Box::new(ReloadThemes),
            "agent.next_attention" => Box::new(AgentNextAttention),
            "help.shortcuts" => Box::new(ShowShortcuts),
            "app.quit" | "project.open" | "theme.open_folder" | "theme.import"
            | "theme.export" | "terminal.copy" | "terminal.paste"
            | "agent.open_opencode" | "agent.open_copilot" => {
                return Err(format!("acción no permitida en Studio: {id}"));
            }
            other => return Err(format!("acción desconocida: {other}")),
        };
        window.focus(&self.focus, cx);
        window.dispatch_action(action, cx);
        cx.notify();
        Ok(())
    }

    /// Cambia el tema activo.
    pub(crate) fn studio_theme(
        &mut self,
        name: &str,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        if !self.theme_host.select_theme(name, cx) {
            return Err(format!("tema desconocido: {name}"));
        }
        cx.notify();
        Ok(())
    }

    /// Fuerza la variante clara u oscura.
    pub(crate) fn studio_variant(
        &mut self,
        variant: &str,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        let variant = match variant {
            "dark" => axf_ui::theme::ThemeVariant::Dark,
            "light" => axf_ui::theme::ThemeVariant::Light,
            other => return Err(format!("variante desconocida: {other}")),
        };
        self.theme_host.set_preview_variant(Some(variant), cx);
        cx.notify();
        Ok(())
    }

    /// Cambia la escala global de la UI.
    pub(crate) fn studio_scale(
        &mut self,
        value: f32,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        if !(0.8..=2.0).contains(&value) {
            return Err("la escala debe estar entre 0.8 y 2.0".into());
        }
        self.theme_host.settings.ui.scale = (value * 1000.0).round() / 1000.0;
        self.theme_host.apply_current(cx);
        cx.notify();
        Ok(())
    }

    /// Enfoca una zona concreta.
    pub(crate) fn studio_focus(
        &mut self,
        target: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        match target {
            "shell" => window.focus(&self.focus, cx),
            "sidebar" => {
                if !self.workspace.sidebar().visible {
                    self.workspace.sidebar_mut().visible = true;
                }
                window.focus(&self.sidebar_focus, cx);
            }
            "settings" => {
                if self.settings.is_none() {
                    self.open_settings(window, cx);
                }
                window.focus(&self.settings_focus, cx);
            }
            "palette" => {
                if self.palette.is_none() {
                    self.open_palette(PaletteMode::Commands, window, cx);
                }
                if let Some(palette) = &self.palette {
                    let input = palette.input.clone();
                    let handle = input.read(cx).focus_handle(cx);
                    window.focus(&handle, cx);
                }
            }
            "terminal" => {
                self.restore_workspace_focus(window, cx);
            }
            other => return Err(format!("foco desconocido: {other}")),
        }
        cx.notify();
        Ok(())
    }

    /// Reemplaza el workspace por el fixture pedido.
    pub(crate) fn studio_workspace(
        &mut self,
        fixture: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        self.terminals.clear();
        self.pending_terminals.clear();
        self.spawn_errors.clear();
        self.pane_scroll.clear();
        self.revealed.clear();
        self.zoomed = None;
        self.workspace = Workspace::new();
        self.sidebar_cursor = None;
        self.editing = None;
        match fixture {
            "empty" => {
                if let Some(studio) = self.studio.as_mut() {
                    studio.fixture = None;
                    studio.agents_frozen = false;
                }
            }
            "demo" => self.studio_seed_demo(window, cx)?,
            other => return Err(format!("fixture desconocida: {other}")),
        }
        self.restore_visible_terminals(cx);
        self.reveal_focus();
        cx.notify();
        Ok(())
    }

    fn studio_seed_demo(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        let root = self
            .studio
            .as_ref()
            .map(|studio| studio.shots_dir.parent().unwrap_or(&studio.shots_dir).join("projects"))
            .unwrap_or_else(|| PathBuf::from("target/studio/projects"));
        let profile = self.theme_host.settings.terminal.default_profile.clone();
        let Some(title) = self.profile_title(&profile) else {
            return Err("no hay un perfil por defecto válido para el fixture".into());
        };
        let now = SystemTime::now();
        for (index, name) in ["atlas", "orbit", "lumen"].into_iter().enumerate() {
            let directory = root.join(name);
            std::fs::create_dir_all(&directory)
                .map_err(|error| format!("no se pudo crear {}: {error}", directory.display()))?;
            let readme = directory.join("README.md");
            if !readme.exists() {
                let _ = std::fs::write(
                    &readme,
                    format!("# {name}\n\nProyecto de demostración para Axfterm Studio.\n"),
                );
            }
            let opened = self
                .workspace
                .open_project(&directory, &profile, &title)
                .map_err(|error| error.to_string())?;
            let pane = opened
                .tab
                .map(|tab| tab.pane)
                .or_else(|| self.workspace.focus().tab.and_then(|tab| self.workspace.pane_of_tab(tab)))
                .ok_or_else(|| "el fixture no pudo resolver un panel".to_owned())?;
            let opened_tab = opened.tab.map(|tab| tab.tab);
            let mut split_tab = None;
            if index == 0 {
                let split = self
                    .workspace
                    .split_pane(pane, SplitAxis::Vertical)
                    .map_err(|error| error.to_string())?;
                let new_tab = self
                    .workspace
                    .new_tab(split, opened.project, &profile, &title)
                    .map_err(|error| error.to_string())?;
                split_tab = Some(new_tab.tab);
            }
            if let Some(tab) = opened_tab {
                let _ = self
                    .workspace
                    .set_tab_title(tab, format!("{name} · terminal"));
            }
            if let Some(tab) = split_tab {
                let _ = self
                    .workspace
                    .set_tab_title(tab, format!("{name} · tests"));
            }
            if let Some(tab) = opened_tab {
                let id = index as u64;
                let mut session = AgentSession::new(
                    id + 1,
                    if index == 1 {
                        AgentKind::Copilot
                    } else {
                        AgentKind::OpenCode
                    },
                    0,
                    now,
                );
                session.state = match index {
                    0 => AgentState::Working,
                    1 => AgentState::NeedsAttention,
                    _ => AgentState::Idle,
                };
                session.message = Some(match index {
                    0 => "Editando el parser".into(),
                    1 => "Necesita confirmación".into(),
                    _ => "En espera".into(),
                });
                self.workspace.set_tab_agents(tab, vec![session]);
            }
        }
        if let Some(studio) = self.studio.as_mut() {
            studio.fixture = Some("demo".into());
            studio.agents_frozen = true;
        }
        self.set_sidebar_section(SidebarSection::All, cx);
        cx.notify();
        Ok(())
    }

    /// Número de proyectos, pestañas y paneles.
    pub(crate) fn studio_workspace_counts(&self) -> (usize, usize, usize) {
        (
            self.workspace.projects().len(),
            self.workspace.tab_count(),
            self.workspace.layout().leaves().len(),
        )
    }

    /// Cierra la app con el guardado normal.
    pub(crate) fn studio_quit(&mut self, cx: &mut Context<'_, Self>) {
        self.request_quit(cx);
    }
}
