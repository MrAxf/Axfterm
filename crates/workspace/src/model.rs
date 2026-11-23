//! Modelo puro del workspace.
//!
//! Todas las operaciones conservan los invariantes de ADR 0003: una pestaña
//! pertenece a un único panel, el foco apunta a una pestaña viva y siempre hay
//! al menos un panel. No hay estado duplicado del proyecto actual: se deriva
//! del foco.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use axf_agents::AgentSession;
use axf_config::ProfileId;
use serde::{Deserialize, Serialize};

use crate::ids::IdCounters;
use crate::layout::{Branch, PaneTree, SplitAxis};
use crate::project::{Project, SidebarState};
use crate::tab::{Focus, Pane, Tab};
use crate::{PaneId, ProjectId, TabId, TerminalId};

/// Error de una operación del workspace.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceError {
    /// Proyecto inexistente en el modelo.
    #[error("proyecto desconocido: {0}")]
    UnknownProject(ProjectId),
    /// Panel inexistente en el modelo.
    #[error("panel desconocido: {0}")]
    UnknownPane(PaneId),
    /// Pestaña inexistente en el modelo.
    #[error("pestaña desconocida: {0}")]
    UnknownTab(TabId),
    /// La ruta no se pudo resolver.
    #[error("no se pudo abrir la carpeta {path}: {message}")]
    InvalidRoot {
        /// Ruta pedida.
        path: String,
        /// Causa del sistema de archivos.
        message: String,
    },
    /// La ruta existe pero no es una carpeta.
    #[error("la ruta no es una carpeta: {0}")]
    NotADirectory(PathBuf),
    /// Varias pestañas intentarían usar la misma terminal.
    #[error("la terminal {0} ya pertenece a otra pestaña")]
    DuplicateTerminal(TerminalId),
    /// Un título vacío no es un nombre válido.
    #[error("el título de la pestaña no puede estar vacío")]
    EmptyTitle,
    /// No hay un panel donde crear la pestaña.
    #[error("el workspace no tiene paneles")]
    NoPane,
}

/// Pestaña recién creada: el app debe arrancar su terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewTab {
    /// Pestaña creada.
    pub tab: TabId,
    /// Terminal asociada (nueva, sin PTY todavía).
    pub terminal: TerminalId,
    /// Panel que la contiene.
    pub pane: PaneId,
}

/// Resultado de abrir/registrar una carpeta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenedProject {
    /// Proyecto registrado o existente.
    pub project: ProjectId,
    /// Pestaña nueva, solo cuando el proyecto no tenía ninguna.
    pub tab: Option<NewTab>,
}

/// Workspace completo de la ventana.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    /// Proyectos en orden de sidebar.
    projects: Vec<Project>,
    /// Árbol de paneles.
    layout: PaneTree,
    /// Pestañas por ID.
    tabs: HashMap<TabId, Tab>,
    /// Paneles por ID.
    panes: HashMap<PaneId, Pane>,
    /// Foco actual.
    focus: Focus,
    /// Estado de la sidebar.
    sidebars: SidebarState,
    /// Contadores persistidos.
    ids: IdCounters,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

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

    /// Registra una carpeta, canonicalizando, normalizando y deduplicando por
    /// ruta.
    ///
    /// Devuelve `(id, added)`, donde `added` indica si el proyecto es nuevo.
    pub fn register_project(
        &mut self,
        root: impl AsRef<Path>,
    ) -> Result<(ProjectId, bool), WorkspaceError> {
        let requested = root.as_ref();
        let canonical =
            std::fs::canonicalize(requested).map_err(|error| WorkspaceError::InvalidRoot {
                path: requested.display().to_string(),
                message: error.to_string(),
            })?;
        if !canonical.is_dir() {
            return Err(WorkspaceError::NotADirectory(canonical));
        }
        // `canonicalize` devuelve `\\?\C:\...` en Windows. Se guarda la forma
        // amigable porque el root también es el CWD del PTY y PowerShell
        // mostraría `Microsoft.PowerShell.Core\FileSystem::\\?\...`.
        let canonical = axf_terminal::paths::normalize(&canonical);

        if let Some(existing) = self
            .projects
            .iter()
            .find(|project| project.root == canonical)
        {
            let id = existing.id;
            self.sidebars.expanded.insert(id);
            return Ok((id, false));
        }

        let id = self.ids.take_project();
        let name = Project::default_name(&canonical);
        self.projects.push(Project::new(id, canonical, name));
        self.sidebars.expanded.insert(id);
        Ok((id, true))
    }

    /// Abre una carpeta con la semántica de la sidebar.
    ///
    /// Si el proyecto no tiene pestañas, crea la primera en el panel enfocado
    /// y lo expande. Si ya las tiene, solo registra y expande (no roba foco).
    pub fn open_project(
        &mut self,
        root: impl AsRef<Path>,
        profile: &ProfileId,
        title: &str,
    ) -> Result<OpenedProject, WorkspaceError> {
        let (project, _added) = self.register_project(root)?;
        self.sidebars.expanded.insert(project);
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

    /// IDs actuales, para tests y persistencia.
    pub fn counters(&self) -> &IdCounters {
        &self.ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Branch, SplitAxis};

    fn temp_project(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("axfterm-workspace-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("carpeta temporal");
        dir
    }

    fn profile() -> ProfileId {
        ProfileId::new("shell")
    }

    const PROFILE_TITLE: &str = "Shell";

    fn workspace_with_project(name: &str) -> (Workspace, ProjectId, NewTab) {
        let mut workspace = Workspace::new();
        let root = temp_project(name);
        let opened = workspace
            .open_project(&root, &profile(), PROFILE_TITLE)
            .expect("abre proyecto");
        let tab = opened.tab.expect("pestaña inicial");
        (workspace, opened.project, tab)
    }

    #[test]
    fn empty_workspace_is_valid() {
        let workspace = Workspace::new();
        assert_eq!(workspace.layout().leaves().len(), 1);
        assert_eq!(workspace.tab_count(), 0);
        assert_eq!(workspace.focus().tab, None);
        assert_eq!(workspace.current_project(), None);
    }

    #[test]
    fn open_project_creates_tab_only_once() {
        let mut workspace = Workspace::new();
        let root = temp_project("open-once");
        let first = workspace
            .open_project(&root, &profile(), PROFILE_TITLE)
            .expect("primer open");
        assert!(first.tab.is_some());
        let second = workspace
            .open_project(&root, &profile(), PROFILE_TITLE)
            .expect("segundo open");
        assert_eq!(second.project, first.project);
        assert!(second.tab.is_none());
        assert_eq!(workspace.tab_count(), 1);
    }

    #[test]
    fn registering_the_same_root_is_deduplicated() {
        let root = temp_project("dedupe");
        let mut workspace = Workspace::new();
        let (first, added) = workspace.register_project(&root).expect("registra");
        assert!(added);
        let canonical = std::fs::canonicalize(&root).expect("canonicaliza");
        let (second, added) = workspace
            .register_project(canonical.join("."))
            .expect("registra otra forma");
        assert!(!added);
        assert_eq!(first, second);
        assert_eq!(workspace.projects().len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn registered_root_is_not_verbatim() {
        let root = temp_project("friendly-root");
        let mut workspace = Workspace::new();
        let (project, _) = workspace.register_project(&root).expect("registra");
        let stored = workspace.project(project).expect("proyecto").root.clone();
        let expected =
            axf_terminal::paths::normalize(&std::fs::canonicalize(&root).expect("canonicaliza"));
        assert_eq!(stored, expected);
        assert!(!stored.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn split_and_close_collapse_the_tree() {
        let (mut workspace, _project, tab) = workspace_with_project("split");
        let pane = workspace.pane_of_tab(tab.tab).expect("panel original");
        let new_pane = workspace
            .split_pane(pane, SplitAxis::Horizontal)
            .expect("divide");
        assert_eq!(workspace.layout().leaves().len(), 2);
        assert_eq!(workspace.focus().pane, new_pane);

        let closed = workspace.close_pane(new_pane).expect("cierra panel");
        assert!(closed.is_empty());
        assert_eq!(workspace.layout().leaves(), vec![pane]);
        assert_eq!(workspace.focus().pane, pane);
    }

    #[test]
    fn closing_last_pane_keeps_an_empty_pane() {
        let (mut workspace, _project, tab) = workspace_with_project("last-pane");
        let pane = workspace.pane_of_tab(tab.tab).expect("panel");
        let closed = workspace.close_pane(pane).expect("cierra");
        assert_eq!(closed, vec![tab.tab]);
        assert_eq!(workspace.layout().leaves(), vec![pane]);
        assert_eq!(workspace.tab_count(), 0);
        assert_eq!(workspace.focus().tab, None);
    }

    #[test]
    fn moving_a_tab_keeps_project_and_terminal() {
        let (mut workspace, project, first) = workspace_with_project("move-tab");
        let original = workspace.tab(first.tab).expect("tab").clone();
        let pane = workspace.pane_of_tab(first.tab).expect("panel");
        let second_pane = workspace
            .split_pane(pane, SplitAxis::Vertical)
            .expect("split");
        let second = workspace
            .new_tab(second_pane, project, &profile(), PROFILE_TITLE)
            .expect("nueva tab");

        workspace
            .move_tab(first.tab, second_pane, 0)
            .expect("mueve");
        let moved = workspace.tab(first.tab).expect("sigue viva");
        assert_eq!(moved.project_id, original.project_id);
        assert_eq!(moved.terminal_id, original.terminal_id);
        assert_eq!(moved.profile_id, original.profile_id);
        assert_eq!(workspace.focus().tab, Some(first.tab));
        assert_eq!(workspace.focus().pane, second_pane);
        // El panel origen queda vacío y se colapsa.
        assert_eq!(workspace.layout().leaves(), vec![second_pane]);
        assert_eq!(
            workspace.pane(second_pane).expect("panel").tabs,
            vec![first.tab, second.tab]
        );
    }

    #[test]
    fn split_pane_with_tab_creates_the_pane_on_the_requested_side() {
        let (mut workspace, project, first) = workspace_with_project("split-with-tab");
        let pane = workspace.pane_of_tab(first.tab).expect("panel");
        let second = workspace
            .new_tab(pane, project, &profile(), PROFILE_TITLE)
            .expect("segunda");
        let terminal = workspace.tab(second.tab).expect("tab").terminal_id;

        let new_pane = workspace
            .split_pane_with_tab(pane, SplitAxis::Horizontal, Branch::First, second.tab)
            .expect("divide con pestaña");

        assert_eq!(workspace.layout().leaves(), vec![new_pane, pane]);
        assert_eq!(workspace.focus().pane, new_pane);
        assert_eq!(workspace.focus().tab, Some(second.tab));
        assert_eq!(
            workspace.tab(second.tab).expect("tab").terminal_id,
            terminal
        );
        assert_eq!(
            workspace.pane(pane).expect("panel original").tabs,
            vec![first.tab]
        );
    }

    #[test]
    fn split_pane_with_the_only_tab_collapses_the_empty_source() {
        let (mut workspace, _project, first) = workspace_with_project("split-only-tab");
        let pane = workspace.pane_of_tab(first.tab).expect("panel");

        let new_pane = workspace
            .split_pane_with_tab(pane, SplitAxis::Vertical, Branch::Second, first.tab)
            .expect("divide con pestaña");

        assert_eq!(workspace.layout().leaves(), vec![new_pane]);
        assert_eq!(workspace.focus().pane, new_pane);
        assert_eq!(workspace.focus().tab, Some(first.tab));
        assert_eq!(
            workspace.pane(new_pane).expect("panel nuevo").tabs,
            vec![first.tab]
        );
    }

    #[test]
    fn duplicate_preserves_profile_and_project() {
        let (mut workspace, project, first) = workspace_with_project("duplicate-profile");
        let original = workspace.tab(first.tab).expect("tab").clone();
        let copy = workspace.duplicate_tab(first.tab).expect("duplica");
        let duplicate = workspace.tab(copy.tab).expect("copia");
        assert_eq!(duplicate.project_id, project);
        assert_eq!(duplicate.profile_id, original.profile_id);
        assert_eq!(duplicate.auto_title, original.auto_title);
        assert_ne!(duplicate.terminal_id, original.terminal_id);
    }

    #[test]
    fn closing_active_tab_focuses_right_neighbor_then_left() {
        let (mut workspace, project, first) = workspace_with_project("neighbors");
        let pane = workspace.pane_of_tab(first.tab).expect("panel");
        let second = workspace
            .new_tab(pane, project, &profile(), PROFILE_TITLE)
            .expect("segunda");
        let third = workspace
            .new_tab(pane, project, &profile(), PROFILE_TITLE)
            .expect("tercera");
        workspace.focus_tab(second.tab).expect("foco");

        workspace.close_tab(second.tab).expect("cierra");
        assert_eq!(workspace.focus().tab, Some(third.tab));

        workspace.close_tab(third.tab).expect("cierra");
        assert_eq!(workspace.focus().tab, Some(first.tab));
    }

    #[test]
    fn removing_project_closes_all_its_tabs_and_keeps_others() {
        let (mut workspace, project_a, _) = workspace_with_project("remove-a");
        let root_b = temp_project("remove-b");
        let opened_b = workspace
            .open_project(&root_b, &profile(), PROFILE_TITLE)
            .expect("proyecto b");
        let project_b = opened_b.project;
        let tab_b = opened_b.tab.expect("tab b").tab;

        let closed = workspace.remove_project(project_a).expect("quita a");
        assert!(!closed.is_empty());
        assert_eq!(workspace.tabs_for_test(project_a), 0);
        assert_eq!(workspace.current_project(), Some(project_b));
        assert_eq!(workspace.focus().tab, Some(tab_b));
        assert!(workspace.project(project_a).is_none());
    }

    #[test]
    fn current_project_is_derived_from_focus_after_move() {
        let (mut workspace, project_a, tab_a) = workspace_with_project("focus-a");
        let root_b = temp_project("focus-b");
        let opened_b = workspace
            .open_project(&root_b, &profile(), PROFILE_TITLE)
            .expect("proyecto b");
        let project_b = opened_b.project;
        let tab_b = opened_b.tab.expect("tab b").tab;

        assert_eq!(workspace.current_project(), Some(project_b));
        workspace.focus_tab(tab_a.tab).expect("enfoca a");
        assert_eq!(workspace.current_project(), Some(project_a));

        let pane = workspace.pane_of_tab(tab_b).expect("panel b");
        workspace
            .move_tab(tab_b, pane, usize::MAX)
            .expect("reordena");
        assert_eq!(workspace.current_project(), Some(project_b));
    }

    #[test]
    fn rename_blocks_osc_and_reset_restores_the_auto_title() {
        let (mut workspace, _project, tab) = workspace_with_project("rename");
        assert!(workspace.set_tab_title(tab.tab, "desde OSC"));
        assert_eq!(workspace.tab(tab.tab).expect("tab").title, "desde OSC");

        workspace.rename_tab(tab.tab, "Mi shell").expect("renombra");
        assert!(!workspace.set_tab_title(tab.tab, "otra vez OSC"));
        assert_eq!(workspace.tab(tab.tab).expect("tab").title, "Mi shell");

        workspace.reset_tab_title(tab.tab).expect("reset");
        let title = &workspace.tab(tab.tab).expect("tab").title;
        assert_eq!(*title, PROFILE_TITLE);
    }

    #[test]
    fn resize_split_clamps_and_repairs_focus() {
        let (mut workspace, _project, tab) = workspace_with_project("ratio");
        let pane = workspace.pane_of_tab(tab.tab).expect("panel");
        workspace
            .split_pane(pane, SplitAxis::Horizontal)
            .expect("split");
        assert!(workspace.resize_split(&[], 0.95));
        assert!(matches!(
            workspace.layout().ratio_at(&[]),
            Some(ratio) if (ratio - 0.9).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn repair_drops_dangling_tabs_and_orphan_panes() {
        let (mut workspace, project, tab) = workspace_with_project("repair");
        let pane = workspace.pane_of_tab(tab.tab).expect("panel");
        // Estado corrupto: pestaña en un panel inexistente y panel huérfano.
        let orphan = PaneId::new(999);
        workspace.panes.insert(
            orphan,
            Pane {
                tabs: vec![tab.tab],
                active: Some(tab.tab),
            },
        );
        workspace.panes.get_mut(&pane).expect("panel").tabs.clear();
        workspace.focus.tab = Some(TabId::new(9999));
        workspace.repair();

        assert!(workspace.tab(tab.tab).is_none());
        assert!(workspace.pane(orphan).is_none());
        assert!(workspace.project(project).is_some());
        assert_eq!(workspace.focus().tab, None);
    }

    #[test]
    fn repair_reassigns_duplicate_roots() {
        let (mut workspace, project, tab) = workspace_with_project("dup-root");
        let original = workspace.project(project).expect("proyecto").clone();
        let duplicate_id = ProjectId::new(999);
        workspace.projects.push(Project::new(
            duplicate_id,
            original.root.clone(),
            "duplicado".into(),
        ));
        // Una pestaña apuntando al duplicado debe conservarse y reasignarse.
        let pane = workspace.pane_of_tab(tab.tab).expect("panel");
        let mut copy = workspace.tab(tab.tab).expect("tab").clone();
        copy.project_id = duplicate_id;
        copy.terminal_id = TerminalId::new(1000);
        let copy_id = TabId::new(1000);
        workspace.tabs.insert(copy_id, copy);
        workspace
            .panes
            .get_mut(&pane)
            .expect("panel")
            .tabs
            .push(copy_id);
        workspace.repair();

        assert!(workspace.project(duplicate_id).is_none());
        assert_eq!(workspace.tab(copy_id).expect("copia").project_id, project);
    }

    #[cfg(windows)]
    #[test]
    fn repair_normalizes_verbatim_roots() {
        let (mut workspace, project, _tab) = workspace_with_project("repair-verbatim");
        let friendly = workspace.project(project).expect("proyecto").root.clone();
        let verbatim = PathBuf::from(format!(r"\\?\{}", friendly.display()));
        workspace.projects[0].root = verbatim;

        // Un duplicado con la forma amigable debe fusionarse con el verbatim.
        let duplicate_id = ProjectId::new(999);
        workspace.projects.push(Project::new(
            duplicate_id,
            friendly.clone(),
            "duplicado".into(),
        ));
        workspace.repair();

        assert_eq!(workspace.project(project).expect("proyecto").root, friendly);
        assert!(workspace.project(duplicate_id).is_none());
    }

    #[test]
    fn tab_agents_are_runtime_only_and_acknowledged_on_focus() {
        use axf_agents::{AgentKind, AgentReport, AgentSession, AgentState};
        use std::time::{Duration, SystemTime};

        let (mut workspace, _project, tab) = workspace_with_project("agents-runtime");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let mut live = AgentSession::new(1, AgentKind::OpenCode, 100, now);
        live.apply_attention(None, Some("permiso".into()), now);
        live.alive = true;
        let mut gone = AgentSession::new(2, AgentKind::Copilot, 101, now);
        gone.apply_report(&AgentReport::new(AgentState::Working), now);
        gone.process_gone(None, now);
        gone.acknowledge(now);
        assert!(workspace.set_tab_agents(tab.tab, vec![live, gone]));
        assert_eq!(workspace.tab_agents(tab.tab).map(<[_]>::len), Some(2));

        // El estado runtime no viaja en `session.json`.
        let value = serde_json::to_value(&workspace).expect("serializa");
        let tabs = value["tabs"].as_object().expect("tabs");
        for tab in tabs.values() {
            assert!(tab.get("agents").is_none(), "los agentes no se serializan");
        }

        assert!(workspace.acknowledge_tab_agents(tab.tab, now));
        let agents = workspace.tab_agents(tab.tab).expect("agentes");
        assert_eq!(agents.len(), 1, "la sesión sin proceso se retira");
        assert_eq!(agents[0].state, AgentState::Idle);
        assert!(workspace.clear_agents());
        assert!(workspace.tab_agents(tab.tab).is_some_and(<[_]>::is_empty));
    }

    #[test]
    fn duplicate_tab_drops_runtime_agents() {
        use axf_agents::{AgentKind, AgentSession};
        use std::time::SystemTime;

        let (mut workspace, _project, tab) = workspace_with_project("agents-duplicate");
        let session = AgentSession::new(1, AgentKind::OpenCode, 100, SystemTime::now());
        assert!(workspace.set_tab_agents(tab.tab, vec![session]));
        assert!(workspace.set_tab_resume_agent(tab.tab, Some("opencode".into())));
        let copy = workspace.duplicate_tab(tab.tab).expect("duplica");
        assert!(workspace.tab_agents(copy.tab).expect("copia").is_empty());
        assert_eq!(workspace.take_tab_resume_agent(copy.tab), None);
        assert_eq!(
            workspace.take_tab_resume_agent(tab.tab),
            Some("opencode".into())
        );
        assert_eq!(workspace.take_tab_resume_agent(tab.tab), None);
    }

    impl Workspace {
        fn tabs_for_test(&self, project: ProjectId) -> usize {
            self.tabs
                .values()
                .filter(|tab| tab.project_id == project)
                .count()
        }
    }

    #[test]
    fn branch_path_is_stable() {
        let (tree, [_a, b, _c]) = {
            let a = PaneId::new(1);
            let b = PaneId::new(2);
            let c = PaneId::new(3);
            let mut tree = PaneTree::leaf(a);
            tree.split(a, b, SplitAxis::Horizontal, 0.5);
            tree.split(b, c, SplitAxis::Vertical, 0.5);
            (tree, [a, b, c])
        };
        let path = tree.path_to(b).expect("camino");
        assert_eq!(path.first(), Some(&Branch::Second));
        assert_eq!(path.get(1), Some(&Branch::First));
    }
}
