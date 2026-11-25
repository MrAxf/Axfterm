impl Workspace {
    /// Workspace vacío con un único panel y sin proyectos.
    ///
    /// Es un estado válido: la UI muestra el onboarding "Abrir carpeta".
    pub fn new() -> Self {
        let mut ids = IdCounters::new();
        let pane = ids.take_pane();
        let mut panes = HashMap::new();
        panes.insert(pane, Pane::empty());
        Self {
            projects: Vec::new(),
            layout: PaneTree::leaf(pane),
            tabs: HashMap::new(),
            panes,
            focus: Focus { pane, tab: None },
            sidebars: SidebarState::default(),
            ids,
        }
    }

    /// Proyectos en orden de sidebar.
    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    /// Busca un proyecto por ID.
    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    /// Busca un proyecto por ruta normalizada.
    pub fn project_by_root(&self, root: &Path) -> Option<&Project> {
        self.projects.iter().find(|project| project.root == root)
    }

    /// Árbol de paneles.
    pub fn layout(&self) -> &PaneTree {
        &self.layout
    }

    /// Panel por ID.
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.get(&id)
    }

    /// Pestaña por ID.
    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.get(&id)
    }

    /// Foco actual.
    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// Sidebar.
    pub fn sidebar(&self) -> &SidebarState {
        &self.sidebars
    }

    /// Sidebar mutable para acciones de presentación (ancho, filtro, foco).
    pub fn sidebar_mut(&mut self) -> &mut SidebarState {
        &mut self.sidebars
    }

    /// Número de terminales/pestañas vivas.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Proyecto actual, derivado de la pestaña enfocada.
    pub fn current_project(&self) -> Option<ProjectId> {
        self.focus
            .tab
            .and_then(|tab| self.tabs.get(&tab))
            .map(|tab| tab.project_id)
    }

    /// Pestaña que usa una terminal concreta.
    pub fn tab_for_terminal(&self, terminal: TerminalId) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|(_, tab)| tab.terminal_id == terminal)
            .map(|(id, _)| *id)
    }

    /// Terminales de todas las pestañas, estén o no arrancadas.
    pub fn terminal_ids(&self) -> impl Iterator<Item = TerminalId> + '_ {
        self.tabs.values().map(|tab| tab.terminal_id)
    }

    /// Panel que contiene una pestaña.
    pub fn pane_of_tab(&self, tab: TabId) -> Option<PaneId> {
        self.panes
            .iter()
            .find(|(_, pane)| pane.tabs.contains(&tab))
            .map(|(id, _)| *id)
    }

    /// Pestañas de un proyecto en orden de layout y tab strip.
    pub fn project_tabs(&self, project: ProjectId) -> Vec<TabId> {
        let mut result = Vec::new();
        for pane in self.layout.leaves() {
            if let Some(pane) = self.panes.get(&pane) {
                result.extend(pane.tabs.iter().copied().filter(|tab| {
                    self.tabs
                        .get(tab)
                        .is_some_and(|tab| tab.project_id == project)
                }));
            }
        }
        result
    }

    /// `true` si el proyecto ya tiene alguna pestaña en el layout.
    pub fn project_has_tabs(&self, project: ProjectId) -> bool {
        self.tabs.values().any(|tab| tab.project_id == project)
    }

    /// Registra una carpeta local, canonicalizando, normalizando y
    /// deduplicando por ruta.
    ///
    /// Devuelve `(id, added)`, donde `added` indica si el proyecto es nuevo.
    pub fn register_project(
        &mut self,
        root: impl AsRef<Path>,
    ) -> Result<(ProjectId, bool), WorkspaceError> {
        self.register_project_with_host(root, ProjectHost::Local)
    }

    /// Registra una carpeta en el host indicado.
    ///
    /// Solo `Local` se resuelve contra el sistema de archivos. Un proyecto WSL
    /// llega ya canonicalizado por la distro y el modelo solo valida la forma
    /// POSIX y deduplica por `(host, root)` (ADR 0011).
    pub fn register_project_with_host(
        &mut self,
        root: impl AsRef<Path>,
        host: ProjectHost,
    ) -> Result<(ProjectId, bool), WorkspaceError> {
        let requested = root.as_ref();
        let canonical = match &host {
            ProjectHost::Local => {
                let canonical = std::fs::canonicalize(requested).map_err(|error| {
                    WorkspaceError::InvalidRoot {
                        path: requested.display().to_string(),
                        message: error.to_string(),
                    }
                })?;
                if !canonical.is_dir() {
                    return Err(WorkspaceError::NotADirectory(canonical));
                }
                // `canonicalize` devuelve `\\?\C:\...` en Windows. Se guarda la
                // forma amigable porque el root también es el CWD del PTY y
                // PowerShell mostraría el proveedor cualificado.
                axf_terminal::paths::normalize(&canonical)
            }
            ProjectHost::Wsl { distro } => {
                validate_wsl_distro(distro)?;
                normalize_posix(requested)?
            }
        };

        if let Some(existing) = self
            .projects
            .iter()
            .find(|project| project.host == host && project.root == canonical)
        {
            let id = existing.id;
            self.sidebars.expanded.insert(id);
            return Ok((id, false));
        }

        let id = self.ids.take_project();
        let name = Project::suggested_name(&host, &canonical);
        self.projects
            .push(Project::with_host(id, canonical, host, name));
        self.sidebars.expanded.insert(id);
        Ok((id, true))
    }

    /// Abre una carpeta local con la semántica de la sidebar.
    pub fn open_project(
        &mut self,
        root: impl AsRef<Path>,
        profile: &ProfileId,
        title: &str,
    ) -> Result<OpenedProject, WorkspaceError> {
        self.open_project_with_host(root, ProjectHost::Local, profile, title)
    }

    /// Abre una carpeta con host explícito.
    ///
    /// Si el proyecto no tiene pestañas, crea la primera en el panel enfocado
    /// y lo expande. Si ya las tiene, solo registra y expande (no roba foco).
    /// En hosts no locales se recuerda el perfil para las terminales nuevas.
    pub fn open_project_with_host(
        &mut self,
        root: impl AsRef<Path>,
        host: ProjectHost,
        profile: &ProfileId,
        title: &str,
    ) -> Result<OpenedProject, WorkspaceError> {
        let (project, _added) = self.register_project_with_host(root, host.clone())?;
        self.sidebars.expanded.insert(project);
        if host.is_remote() {
            let remember = self
                .project(project)
                .is_some_and(|project| project.default_profile.is_none());
            if remember {
                self.set_project_default_profile(project, Some(profile.clone()));
            }
        }
        if self.project_has_tabs(project) {
            return Ok(OpenedProject { project, tab: None });
        }
        let pane = self.focus.pane;
        let tab = self.new_tab(pane, project, profile, title)?;
        Ok(OpenedProject {
            project,
            tab: Some(tab),
        })
    }

    /// Fija el perfil con el que se abren las terminales nuevas del proyecto.
    ///
    /// Devuelve `true` si cambió.
    pub fn set_project_default_profile(
        &mut self,
        project: ProjectId,
        profile: Option<ProfileId>,
    ) -> bool {
        let Some(project) = self.projects.iter_mut().find(|entry| entry.id == project) else {
            return false;
        };
        if project.default_profile == profile {
            return false;
        }
        project.default_profile = profile;
        true
    }

    /// Crea una pestaña en un panel concreto y le da el foco.
    pub fn new_tab(
        &mut self,
        pane: PaneId,
        project: ProjectId,
        profile: &ProfileId,
        title: &str,
    ) -> Result<NewTab, WorkspaceError> {
        if self.project(project).is_none() {
            return Err(WorkspaceError::UnknownProject(project));
        }
        if !self.panes.contains_key(&pane) {
            return Err(WorkspaceError::UnknownPane(pane));
        }
        let tab = self.ids.take_tab();
        let terminal = self.ids.take_terminal();
        self.tabs.insert(
            tab,
            Tab::new(project, terminal, profile.clone(), title.to_owned()),
        );
        let pane_state = self
            .panes
            .get_mut(&pane)
            .ok_or(WorkspaceError::UnknownPane(pane))?;
        pane_state.tabs.push(tab);
        pane_state.active = Some(tab);
        self.focus = Focus {
            pane,
            tab: Some(tab),
        };
        self.sidebars.expanded.insert(project);
        Ok(NewTab {
            tab,
            terminal,
            pane,
        })
    }

    /// Crea una pestaña en el panel enfocado.
    pub fn new_tab_focused(
        &mut self,
        project: ProjectId,
        profile: &ProfileId,
        title: &str,
    ) -> Result<NewTab, WorkspaceError> {
        let pane = self.focus.pane;
        self.new_tab(pane, project, profile, title)
    }

    /// Duplica una pestaña junto a la original, conservando proyecto y título.
    ///
    /// El terminal nuevo lo aporta el modelo; el app es quien arranca su PTY.
    pub fn duplicate_tab(&mut self, tab: TabId) -> Result<NewTab, WorkspaceError> {
        let pane_id = self
            .pane_of_tab(tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?;
        let original = self
            .tabs
            .get(&tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?
            .clone();
        let new_tab = self.ids.take_tab();
        let terminal = self.ids.take_terminal();
        let mut copy = original;
        copy.terminal_id = terminal;
        copy.resume_agent = None;
        copy.agents.clear();
        self.tabs.insert(new_tab, copy);
        let pane = self
            .panes
            .get_mut(&pane_id)
            .ok_or(WorkspaceError::UnknownPane(pane_id))?;
        let index = pane
            .tabs
            .iter()
            .position(|id| *id == tab)
            .map(|index| index + 1)
            .unwrap_or(pane.tabs.len());
        pane.tabs.insert(index, new_tab);
        pane.active = Some(new_tab);
        self.focus = Focus {
            pane: pane_id,
            tab: Some(new_tab),
        };
        Ok(NewTab {
            tab: new_tab,
            terminal,
            pane: pane_id,
        })
    }

    /// Divide un panel en el eje pedido; el panel nuevo nace vacío y enfocado.
    pub fn split_pane(&mut self, pane: PaneId, axis: SplitAxis) -> Result<PaneId, WorkspaceError> {
        if !self.panes.contains_key(&pane) {
            return Err(WorkspaceError::UnknownPane(pane));
        }
        let new_pane = self.ids.take_pane();
        if !self.layout.split(pane, new_pane, axis, 0.5) {
            return Err(WorkspaceError::UnknownPane(pane));
        }
        self.panes.insert(new_pane, Pane::empty());
        self.focus = Focus {
            pane: new_pane,
            tab: None,
        };
        Ok(new_pane)
    }

    /// Divide `pane` en el lado pedido y mueve `tab` al panel nuevo.
    ///
    /// Conserva `project_id`, `terminal_id` y el PTY: solo cambia la ubicación
    /// de la pestaña. Si el panel origen queda vacío, colapsa como cualquier
    /// otro panel no final.
    pub fn split_pane_with_tab(
        &mut self,
        pane: PaneId,
        axis: SplitAxis,
        side: Branch,
        tab: TabId,
    ) -> Result<PaneId, WorkspaceError> {
        if !self.panes.contains_key(&pane) {
            return Err(WorkspaceError::UnknownPane(pane));
        }
        if self.pane_of_tab(tab).is_none() {
            return Err(WorkspaceError::UnknownTab(tab));
        }
        let new_pane = self.ids.take_pane();
        if !self.layout.split_with_side(pane, new_pane, axis, side, 0.5) {
            return Err(WorkspaceError::UnknownPane(pane));
        }
        self.panes.insert(new_pane, Pane::empty());
        self.focus = Focus {
            pane: new_pane,
            tab: None,
        };
        self.move_tab(tab, new_pane, 0)?;
        Ok(new_pane)
    }

    /// Cierra un panel y todas sus pestañas. Devuelve las pestañas destruidas.
    pub fn close_pane(&mut self, pane: PaneId) -> Result<Vec<TabId>, WorkspaceError> {
        let removed = self
            .panes
            .remove(&pane)
            .ok_or(WorkspaceError::UnknownPane(pane))?;
        let tabs = removed.tabs;
        for tab in &tabs {
            self.tabs.remove(tab);
        }

        if self.layout.is_leaf() {
            // El último panel se conserva vacío: el workspace siempre existe.
            self.panes.insert(pane, Pane::empty());
            self.focus = Focus { pane, tab: None };
            return Ok(tabs);
        }

        let sibling = self.layout.sibling_of(pane);
        let collapsed = self.layout.collapse(pane);
        debug_assert!(collapsed, "un panel no raíz debe colapsar");
        let next = sibling
            .or_else(|| self.layout.leaves().first().copied())
            .ok_or(WorkspaceError::NoPane)?;
        self.panes.entry(next).or_default();
        self.focus_on_pane(next);
        Ok(tabs)
    }

    /// Cierra una pestaña y repara el foco.
    pub fn close_tab(&mut self, tab: TabId) -> Result<(), WorkspaceError> {
        let pane_id = self
            .pane_of_tab(tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?;
        let (was_active, index) = {
            let pane = self
                .panes
                .get(&pane_id)
                .ok_or(WorkspaceError::UnknownPane(pane_id))?;
            (
                pane.active == Some(tab),
                pane.tabs.iter().position(|id| *id == tab).unwrap_or(0),
            )
        };

        {
            let pane = self
                .panes
                .get_mut(&pane_id)
                .ok_or(WorkspaceError::UnknownPane(pane_id))?;
            pane.tabs.retain(|id| *id != tab);
            if was_active {
                // Vecina derecha y, si no hay, izquierda.
                pane.active = pane
                    .tabs
                    .get(index)
                    .copied()
                    .or_else(|| pane.tabs.get(index.saturating_sub(1)).copied());
            }
        }
        self.tabs.remove(&tab);

        let empty = self
            .panes
            .get(&pane_id)
            .is_some_and(|pane| pane.tabs.is_empty());
        if empty && !self.layout.is_leaf() {
            let sibling = self.layout.sibling_of(pane_id);
            self.layout.collapse(pane_id);
            self.panes.remove(&pane_id);
            if let Some(next) = sibling.or_else(|| self.layout.leaves().first().copied()) {
                self.panes.entry(next).or_default();
                self.focus_on_pane(next);
            }
        } else if self.focus.tab == Some(tab) || self.focus.pane == pane_id {
            self.focus.pane = pane_id;
            self.focus.tab = self.panes.get(&pane_id).and_then(|pane| pane.active);
        }
        Ok(())
    }
}
