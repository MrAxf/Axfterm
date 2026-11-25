use crate::ids::IdCounters;
use crate::layout::{Branch, PaneTree, SplitAxis};
use crate::project::{Project, ProjectHost, SidebarState};
use crate::tab::{Focus, Pane, ResumeAgent, Tab};
use crate::{PaneId, ProjectId, TabId, TerminalId};
use axf_agents::AgentSession;
use axf_config::ProfileId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
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
    /// Host o ruta remota inválida.
    #[error("proyecto remoto inválido: {message}")]
    InvalidHost {
        /// Motivo accionable.
        message: String,
    },
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewTab {
    /// Pestaña creada.
    pub tab: TabId,
    /// Terminal asociada (nueva, sin PTY todavía).
    pub terminal: TerminalId,
    /// Panel que la contiene.
    pub pane: PaneId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenedProject {
    /// Proyecto registrado o existente.
    pub project: ProjectId,
    /// Pestaña nueva, solo cuando el proyecto no tenía ninguna.
    pub tab: Option<NewTab>,
}
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
