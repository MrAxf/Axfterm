//! Pestañas y paneles.

use axf_agents::{AgentAggregate, AgentSession, aggregate};
use axf_config::ProfileId;
use serde::{Deserialize, Serialize};

use crate::{ProjectId, TabId, TerminalId};

/// Pestaña de un panel.
///
/// `project_id`, `terminal_id` y `profile_id` no cambian al mover la pestaña
/// entre paneles (ADR 0003/0006). `title` es el título visible; `auto_title` es
/// el nombre del perfil que se restaura al quitar un nombre manual.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    /// Proyecto al que pertenece la pestaña.
    pub project_id: ProjectId,
    /// Terminal (PTY) ligada a la pestaña.
    pub terminal_id: TerminalId,
    /// Perfil con el que se arranca el PTY.
    pub profile_id: ProfileId,
    /// Título visible persistido.
    pub title: String,
    /// Título automático (nombre del perfil).
    #[serde(default)]
    pub auto_title: String,
    /// Si el usuario renombró la pestaña.
    #[serde(default)]
    pub custom_title: bool,
    /// Agente activo al cerrar la sesión (`"opencode"` o `"copilot"`), para
    /// reanudar automáticamente su última sesión al reabrir la pestaña.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_agent: Option<String>,
    /// Sesiones de agentes detectadas en esta pestaña.
    ///
    /// Es estado runtime-only (ADR 0009): no se serializa en `session.json`.
    #[serde(skip)]
    pub agents: Vec<AgentSession>,
}

impl Tab {
    /// Crea una pestaña con su perfil y título inicial.
    pub fn new(
        project_id: ProjectId,
        terminal_id: TerminalId,
        profile_id: ProfileId,
        title: impl Into<String>,
    ) -> Self {
        let title = title.into();
        Self {
            project_id,
            terminal_id,
            profile_id,
            title: title.clone(),
            auto_title: title,
            custom_title: false,
            resume_agent: None,
            agents: Vec::new(),
        }
    }

    /// Agregado de las sesiones, si hay alguna.
    pub fn agent_aggregate(&self) -> Option<AgentAggregate> {
        aggregate(&self.agents)
    }

    /// `true` si la pestaña tiene al menos un agente.
    pub fn has_agents(&self) -> bool {
        !self.agents.is_empty()
    }
}

/// Paneles: lista de pestañas y cuál está activa.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pane {
    /// Pestañas en orden de la tab strip.
    pub tabs: Vec<TabId>,
    /// Pestaña activa; `None` en un panel vacío.
    pub active: Option<TabId>,
}

impl Pane {
    /// Panel vacío válido.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Índice de la pestaña activa dentro de `tabs`.
    pub fn active_index(&self) -> Option<usize> {
        let active = self.active?;
        self.tabs.iter().position(|tab| *tab == active)
    }

    /// Repara `active` si apunta a una pestaña que ya no existe.
    pub fn repair_active(&mut self) {
        if let Some(active) = self.active
            && !self.tabs.contains(&active)
        {
            self.active = self.tabs.first().copied();
        }
        if self.active.is_none() {
            self.active = self.tabs.first().copied();
        }
    }
}

/// Foco del workspace: panel y, si existe, pestaña activa.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Focus {
    /// Panel enfocado.
    pub pane: crate::PaneId,
    /// Pestaña enfocada (siempre la activa del panel).
    pub tab: Option<TabId>,
}
