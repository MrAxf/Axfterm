impl Workspace {
    /// Mueve una pestaña a otro panel o a otra posición del mismo panel.
    ///
    /// No toca `project_id`, `terminal_id` ni el PTY.
    pub fn move_tab(
        &mut self,
        tab: TabId,
        target: PaneId,
        index: usize,
    ) -> Result<(), WorkspaceError> {
        let source = self
            .pane_of_tab(tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?;
        if !self.panes.contains_key(&target) {
            return Err(WorkspaceError::UnknownPane(target));
        }
        if source == target {
            let pane = self
                .panes
                .get_mut(&target)
                .ok_or(WorkspaceError::UnknownPane(target))?;
            let from = pane
                .tabs
                .iter()
                .position(|id| *id == tab)
                .ok_or(WorkspaceError::UnknownTab(tab))?;
            pane.tabs.remove(from);
            let to = index.min(pane.tabs.len());
            pane.tabs.insert(to, tab);
            pane.active = Some(tab);
            self.focus = Focus {
                pane: target,
                tab: Some(tab),
            };
            return Ok(());
        }

        let source_pane = self
            .panes
            .get_mut(&source)
            .ok_or(WorkspaceError::UnknownPane(source))?;
        source_pane.tabs.retain(|id| *id != tab);
        if source_pane.active == Some(tab) {
            source_pane.active = source_pane
                .tabs
                .first()
                .copied()
                .or_else(|| source_pane.tabs.last().copied());
        }

        let target_pane = self
            .panes
            .get_mut(&target)
            .ok_or(WorkspaceError::UnknownPane(target))?;
        let to = index.min(target_pane.tabs.len());
        target_pane.tabs.insert(to, tab);
        target_pane.active = Some(tab);
        self.focus = Focus {
            pane: target,
            tab: Some(tab),
        };

        let source_empty = self
            .panes
            .get(&source)
            .is_some_and(|pane| pane.tabs.is_empty());
        if source_empty && !self.layout.is_leaf() {
            self.layout.collapse(source);
            self.panes.remove(&source);
        }
        Ok(())
    }

    /// Reordena una pestaña dentro de su panel.
    pub fn reorder_tab(&mut self, tab: TabId, index: usize) -> Result<(), WorkspaceError> {
        let pane = self
            .pane_of_tab(tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?;
        self.move_tab(tab, pane, index)
    }

    /// Enfoca una pestaña viva.
    pub fn focus_tab(&mut self, tab: TabId) -> Result<(), WorkspaceError> {
        let pane = self
            .pane_of_tab(tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?;
        let pane_state = self
            .panes
            .get_mut(&pane)
            .ok_or(WorkspaceError::UnknownPane(pane))?;
        pane_state.active = Some(tab);
        self.focus = Focus {
            pane,
            tab: Some(tab),
        };
        Ok(())
    }

    /// Enfoca un panel y su pestaña activa.
    pub fn focus_pane(&mut self, pane: PaneId) -> Result<(), WorkspaceError> {
        if !self.panes.contains_key(&pane) {
            return Err(WorkspaceError::UnknownPane(pane));
        }
        self.focus_on_pane(pane);
        Ok(())
    }

    fn focus_on_pane(&mut self, pane: PaneId) {
        let active = self.panes.get(&pane).and_then(|pane| pane.active);
        self.focus = Focus { pane, tab: active };
    }

    /// Ajusta el ratio del split en `path` (clamp a `MIN_RATIO`).
    pub fn resize_split(&mut self, path: &[Branch], ratio: f32) -> bool {
        self.layout.set_ratio_at(path, ratio)
    }

    /// Título propuesto por el shell (OSC), si el usuario no lo ha renombrado.
    pub fn set_tab_title(&mut self, tab: TabId, title: impl Into<String>) -> bool {
        let Some(tab) = self.tabs.get_mut(&tab) else {
            return false;
        };
        if tab.custom_title {
            return false;
        }
        let title = title.into();
        if title.is_empty() || title == tab.title {
            return false;
        }
        tab.title = title;
        true
    }

    /// Renombra una pestaña; a partir de aquí el OSC no la pisa.
    pub fn rename_tab(&mut self, tab: TabId, title: &str) -> Result<(), WorkspaceError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(WorkspaceError::EmptyTitle);
        }
        let tab = self
            .tabs
            .get_mut(&tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?;
        tab.title = title.to_owned();
        tab.custom_title = true;
        Ok(())
    }

    /// Devuelve una pestaña a su título automático (nombre del perfil).
    pub fn reset_tab_title(&mut self, tab: TabId) -> Result<(), WorkspaceError> {
        let tab = self
            .tabs
            .get_mut(&tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?;
        tab.title = tab.auto_title.clone();
        tab.custom_title = false;
        Ok(())
    }

    /// Cambia el perfil de una pestaña terminada y su título automático.
    ///
    /// El app solo lo usa tras una elección explícita (perfil borrado o
    /// reinicio); nunca cambia el perfil de un PTY vivo por sorpresa.
    pub fn set_tab_profile(
        &mut self,
        tab: TabId,
        profile_id: ProfileId,
        title: &str,
    ) -> Result<(), WorkspaceError> {
        let tab = self
            .tabs
            .get_mut(&tab)
            .ok_or(WorkspaceError::UnknownTab(tab))?;
        tab.profile_id = profile_id;
        tab.auto_title = title.to_owned();
        if !tab.custom_title {
            tab.title = title.to_owned();
        }
        Ok(())
    }

    /// `true` si alguna sesión requiere atención (marca el título).
    pub fn has_pending_attention(&self) -> bool {
        self.tabs
            .values()
            .any(|tab| tab.agents.iter().any(AgentSession::needs_attention))
    }

    /// Sesiones de agente de una pestaña.
    pub fn tab_agents(&self, tab: TabId) -> Option<&[AgentSession]> {
        self.tabs.get(&tab).map(|tab| tab.agents.as_slice())
    }

    /// Reemplaza las sesiones de una pestaña. Devuelve `true` si cambió.
    pub fn set_tab_agents(&mut self, tab: TabId, agents: Vec<AgentSession>) -> bool {
        let Some(model_tab) = self.tabs.get_mut(&tab) else {
            return false;
        };
        if model_tab.agents == agents {
            return false;
        }
        model_tab.agents = agents;
        true
    }

    /// Muta las sesiones de una pestaña y devuelve el resultado del cierre.
    pub fn update_tab_agents<R>(
        &mut self,
        tab: TabId,
        update: impl FnOnce(&mut Vec<AgentSession>) -> R,
    ) -> Option<R> {
        self.tabs.get_mut(&tab).map(|tab| update(&mut tab.agents))
    }

    /// Enfocar una pestaña limpia atención, terminado y error, y retira las
    /// sesiones cuyo proceso ya no existe (ADR 0009).
    pub fn acknowledge_tab_agents(&mut self, tab: TabId, now: SystemTime) -> bool {
        let Some(model_tab) = self.tabs.get_mut(&tab) else {
            return false;
        };
        let before = model_tab.agents.len();
        let mut changed = false;
        for session in &mut model_tab.agents {
            changed |= session.acknowledge(now);
        }
        model_tab
            .agents
            .retain(|session| !session.is_stale_on_focus());
        changed || model_tab.agents.len() != before
    }

    /// Retira todas las sesiones de agente; se usa al desactivar la detección.
    pub fn clear_agents(&mut self) -> bool {
        let mut changed = false;
        for tab in self.tabs.values_mut() {
            if !tab.agents.is_empty() {
                tab.agents.clear();
                changed = true;
            }
            if tab.resume_agent.take().is_some() {
                changed = true;
            }
        }
        changed
    }

    /// Define el agente a reanudar al reabrir una pestaña. Devuelve `true` si cambió.
    pub fn set_tab_resume_agent(&mut self, tab: TabId, agent: Option<String>) -> bool {
        let Some(model_tab) = self.tabs.get_mut(&tab) else {
            return false;
        };
        if model_tab.resume_agent == agent {
            return false;
        }
        model_tab.resume_agent = agent;
        true
    }

    /// Toma y limpia el agente pendiente de reanudar de una pestaña.
    pub fn take_tab_resume_agent(&mut self, tab: TabId) -> Option<String> {
        self.tabs
            .get_mut(&tab)
            .and_then(|model_tab| model_tab.resume_agent.take())
    }

    /// Elimina un proyecto y todas sus pestañas.
    ///
    /// Devuelve las pestañas destruidas para que el app cierre sus PTYs.
    pub fn remove_project(&mut self, project: ProjectId) -> Result<Vec<TabId>, WorkspaceError> {
        if self.project(project).is_none() {
            return Err(WorkspaceError::UnknownProject(project));
        }
        let tabs = self.project_tabs(project);
        for tab in &tabs {
            self.close_tab(*tab)?;
        }
        self.projects.retain(|entry| entry.id != project);
        self.sidebars.expanded.remove(&project);
        Ok(tabs)
    }

    /// Cambia el orden de un proyecto en la sidebar.
    pub fn move_project(&mut self, project: ProjectId, index: usize) -> Result<(), WorkspaceError> {
        let from = self
            .projects
            .iter()
            .position(|entry| entry.id == project)
            .ok_or(WorkspaceError::UnknownProject(project))?;
        let project = self.projects.remove(from);
        let to = index.min(self.projects.len());
        self.projects.insert(to, project);
        Ok(())
    }

    /// Despliega o pliega un proyecto en la sidebar.
    pub fn set_project_expanded(
        &mut self,
        project: ProjectId,
        expanded: bool,
    ) -> Result<(), WorkspaceError> {
        if self.project(project).is_none() {
            return Err(WorkspaceError::UnknownProject(project));
        }
        if expanded {
            self.sidebars.expanded.insert(project);
        } else {
            self.sidebars.expanded.remove(&project);
        }
        Ok(())
    }

    /// Sanea el workspace cargado de una sesión.
    ///
    /// Es idempotente y nunca deja referencias a IDs desaparecidos.
    pub fn repair(&mut self) {
        self.repair_projects();
        self.repair_layout();
        self.repair_panes_and_tabs();
        self.repair_focus();

        let project_ids: Vec<_> = self.projects.iter().map(|project| project.id).collect();
        let pane_ids: Vec<_> = self.layout.leaves();
        let tab_ids: Vec<_> = self.tabs.keys().copied().collect();
        let terminal_ids: Vec<_> = self.tabs.values().map(|tab| tab.terminal_id).collect();
        self.ids
            .reconcile(project_ids, pane_ids, tab_ids, terminal_ids);

        let existing: HashSet<_> = self.projects.iter().map(|project| project.id).collect();
        self.sidebars.expanded.retain(|id| existing.contains(id));
        if !self.sidebars.width.is_finite() {
            self.sidebars.width = 240.0;
        }
        self.sidebars.width = self.sidebars.width.clamp(160.0, 480.0);
    }

    fn repair_projects(&mut self) {
        let mut seen_ids = HashSet::new();
        let mut seen_roots = HashSet::new();
        let mut replacements: HashMap<ProjectId, ProjectId> = HashMap::new();
        let mut kept = Vec::with_capacity(self.projects.len());

        for mut project in self.projects.drain(..) {
            // Las sesiones anteriores guardaban la forma verbatim; se sanea
            // antes de deduplicar para que `\\?\C:\x` y `C:\x` sean el mismo.
            project.root = axf_terminal::paths::normalize(&project.root);
            if seen_ids.contains(&project.id) {
                continue;
            }
            if seen_roots.contains(&project.root) {
                // Mismo root canónico: se conserva el primero y se reasignan
                // sus pestañas en `repair_panes_and_tabs` vía `replacements`.
                if let Some(owner) = kept
                    .iter()
                    .find(|kept: &&Project| kept.root == project.root)
                {
                    replacements.insert(project.id, owner.id);
                }
                continue;
            }
            seen_ids.insert(project.id);
            seen_roots.insert(project.root.clone());
            kept.push(project);
        }
        self.projects = kept;
        if !replacements.is_empty() {
            for tab in self.tabs.values_mut() {
                if let Some(replacement) = replacements.get(&tab.project_id) {
                    tab.project_id = *replacement;
                }
            }
        }
    }

    fn repair_layout(&mut self) {
        if !self.layout.repair() {
            // Hojas duplicadas: conservamos la primera y reiniciamos el árbol.
            let first = self.layout.leaves().first().copied();
            let pane = first.unwrap_or_else(|| self.ids.take_pane());
            self.layout = PaneTree::leaf(pane);
        }
    }

    fn repair_panes_and_tabs(&mut self) {
        let leaves = self.layout.leaves();
        let project_ids: HashSet<_> = self.projects.iter().map(|project| project.id).collect();
        let mut placed_tabs = HashSet::new();
        let mut placed_terminals = HashSet::new();

        for pane_id in &leaves {
            let pane = self.panes.entry(*pane_id).or_default();
            pane.tabs.retain(|tab_id| {
                let Some(tab) = self.tabs.get(tab_id) else {
                    return false;
                };
                if !project_ids.contains(&tab.project_id)
                    || placed_tabs.contains(tab_id)
                    || placed_terminals.contains(&tab.terminal_id)
                {
                    return false;
                }
                placed_tabs.insert(*tab_id);
                placed_terminals.insert(tab.terminal_id);
                true
            });
            pane.repair_active();
        }

        let valid_panes: HashSet<_> = leaves.into_iter().collect();
        self.panes.retain(|id, _| valid_panes.contains(id));
        for tab_id in &placed_tabs {
            if let Some(tab) = self.tabs.get_mut(tab_id)
                && tab.auto_title.is_empty()
            {
                tab.auto_title = tab.title.clone();
            }
        }
        self.tabs.retain(|id, _| placed_tabs.contains(id));
    }

    fn repair_focus(&mut self) {
        let leaves = self.layout.leaves();
        if leaves.is_empty() {
            // No ocurre con un `PaneTree` bien formado, pero deja el modelo
            // en un estado utilizable ante una sesión manipulada.
            let pane = self.ids.take_pane();
            self.layout = PaneTree::leaf(pane);
            self.panes.insert(pane, Pane::empty());
            self.focus = Focus { pane, tab: None };
            return;
        }
        if !leaves.contains(&self.focus.pane) {
            let pane = leaves[0];
            self.focus = Focus { pane, tab: None };
        }
        let active = self
            .panes
            .get(&self.focus.pane)
            .and_then(|pane| pane.active);
        self.focus.tab = active;
    }

    /// Proyectos desplegados en la sidebar.
    pub fn expanded_projects(&self) -> &BTreeSet<ProjectId> {
        &self.sidebars.expanded
    }
}
