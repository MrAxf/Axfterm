//! Sesiones de agente, estados y agregados por pestaña.
//!
//! El modelo es runtime-only: no se serializa en `session.json`. Cada proceso
//! detectado es una `AgentSession`; una pestaña puede tener varias.

include!("model_parts/01_agentsessionid.rs");
include!("model_parts/03_session_id.rs");
include!("model_parts/02_aggregate.rs");
