//! Detección local de agentes de IA (opencode, GitHub Copilot) y su máquina de
//! estados. Sin dependencia de GPUI.
//!
//! - `process`: snapshot de procesos y matcher puro con tests deterministas.
//! - `model`: sesiones runtime, estados, transiciones y agregados por pestaña.
//!
//! El parser OSC 7501 vive en `axf_terminal` porque es protocolo de terminal;
//! la app normaliza sus reportes a `AgentReport` y los aplica aquí. El estado
//! nunca se serializa en `session.json` (ADR 0009).

mod model;
mod process;

pub use model::{
    AgentAggregate, AgentKind, AgentReport, AgentSession, AgentSessionId, AgentState,
    AgentTransition, AttentionKind, ReportedSession, aggregate,
};
pub use process::{DetectedAgent, ProcessInfo, ProcessScanner, ProcessSnapshot};
