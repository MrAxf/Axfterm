//! Modelo de workspace de Axfterm: proyectos, árbol de paneles, pestañas,
//! foco y persistencia de sesión.
//!
//! El crate es puro: no depende de GPUI ni arranca PTYs. La app traduce las
//! operaciones del modelo a entidades de terminal y vistas. Esto permite testear
//! los invariantes de ADR 0003 sin ventana ni procesos.

mod ids;
mod layout;
mod model;
mod project;
mod session;
mod tab;

pub use ids::{IdCounters, PaneId, ProjectId, TabId, TerminalId};
pub use layout::{Branch, MIN_RATIO, PaneTree, SplitAxis};
pub use model::{NewTab, OpenedProject, Workspace, WorkspaceError};
pub use project::{Project, SidebarSection, SidebarSide, SidebarState};
pub use session::{
    MAX_SESSION_BYTES, SESSION_VERSION, SessionError, SessionFile, SessionLoad, backup_session,
    load_or_recover, load_session, save_session,
};
pub use tab::{Focus, Pane, Tab};
