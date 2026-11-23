//! Sesiones de agente, estados y agregados por pestaña.
//!
//! El modelo es runtime-only: no se serializa en `session.json`. Cada proceso
//! detectado es una `AgentSession`; una pestaña puede tener varias.

use std::time::{Duration, SystemTime};

/// Identificador runtime de una sesión.
pub type AgentSessionId = u64;

/// Agente reconocido por la detección local.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentKind {
    /// `opencode` (también el alias `opencode2`).
    OpenCode,
    /// GitHub Copilot (`copilot` y `copilot-runtime`).
    Copilot,
}

impl AgentKind {
    /// Nombre estable para logs y configuración.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenCode => "opencode",
            Self::Copilot => "copilot",
        }
    }

    /// Etiqueta visible.
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenCode => "opencode",
            Self::Copilot => "Copilot",
        }
    }

    /// Comando de consola para reanudar la última sesión del agente al reabrir.
    pub const fn resume_command(self) -> &'static str {
        match self {
            Self::OpenCode => "opencode --continue",
            Self::Copilot => "copilot --continue",
        }
    }

    /// Clave estable en `session.json` para reanudar el agente.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "opencode" | "opencode2" => Some(Self::OpenCode),
            "copilot" => Some(Self::Copilot),
            _ => None,
        }
    }

    /// Agente a partir del `app` de OSC 7501.
    pub fn from_app(app: &str) -> Option<Self> {
        match app.trim().to_ascii_lowercase().as_str() {
            "opencode" | "opencode2" | "opencode-ai" => Some(Self::OpenCode),
            "copilot" | "github-copilot" | "copilot-cli" => Some(Self::Copilot),
            _ => None,
        }
    }
}

/// Estado visible de un agente.
///
/// El orden de la declaración es la prioridad de presentación:
/// `NeedsAttention > Working > Error > Finished > Idle > Stopped`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentState {
    /// El proceso terminó y no dejó un resultado visible.
    #[default]
    Stopped,
    /// Proceso vivo sin trabajo activo.
    Idle,
    /// Terminó con un resultado que el usuario aún no ha visto.
    Finished,
    /// Falló.
    Error,
    /// Procesando.
    Working,
    /// Requiere intervención del usuario.
    NeedsAttention,
}

impl AgentState {
    /// Prioridad de presentación cuando varias sesiones compiten.
    pub const fn priority(self) -> u8 {
        match self {
            Self::NeedsAttention => 5,
            Self::Working => 4,
            Self::Error => 3,
            Self::Finished => 2,
            Self::Idle => 1,
            Self::Stopped => 0,
        }
    }

    /// Etiqueta visible.
    pub const fn label(self) -> &'static str {
        match self {
            Self::NeedsAttention => "Atención",
            Self::Working => "Trabajando",
            Self::Error => "Error",
            Self::Finished => "Terminado",
            Self::Idle => "Inactivo",
            Self::Stopped => "Detenido",
        }
    }

    /// `true` si es un resultado pendiente de revisar.
    pub const fn is_terminal_alert(self) -> bool {
        matches!(self, Self::Finished | Self::Error | Self::NeedsAttention)
    }

    /// `true` si ya no representa trabajo en curso.
    pub const fn is_settled(self) -> bool {
        matches!(
            self,
            Self::Finished | Self::Error | Self::Stopped | Self::Idle
        )
    }

    /// Estado a partir de la clave de OSC 7501. `clear` no es un estado.
    pub fn from_protocol(state: &str) -> Option<Self> {
        match state.trim().to_ascii_lowercase().as_str() {
            "idle" => Some(Self::Idle),
            "working" => Some(Self::Working),
            "blocked" => Some(Self::NeedsAttention),
            "done" => Some(Self::Finished),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Motivo de una atención bloqueada, según OSC 7501.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttentionKind {
    /// Aprobación de una acción.
    Permission,
    /// Respuesta escrita.
    Question,
    /// Login o credenciales.
    Auth,
}

impl AttentionKind {
    /// Etiqueta visible.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Permission => "permiso",
            Self::Question => "pregunta",
            Self::Auth => "autenticación",
        }
    }

    /// Clave del protocolo.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Permission => "permission",
            Self::Question => "question",
            Self::Auth => "auth",
        }
    }

    /// Kind de OSC 7501; cualquier otro valor cuenta como `None`.
    pub fn from_protocol(kind: &str) -> Option<Self> {
        match kind.trim().to_ascii_lowercase().as_str() {
            "permission" => Some(Self::Permission),
            "question" => Some(Self::Question),
            "auth" => Some(Self::Auth),
            _ => None,
        }
    }
}

/// Informe normalizado que llega desde el parser OSC 7501.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentReport {
    /// Estado nuevo.
    pub state: AgentState,
    /// `clear` retira el registro y deja la sesión inactiva.
    pub clear: bool,
    /// Motivo de la atención (`blocked`).
    pub attention: Option<AttentionKind>,
    /// Progreso 0–100, si lo hay.
    pub progress: Option<u8>,
    /// Nombre estable del programa (`app`).
    pub app: Option<String>,
    /// Etiqueta corta (`title`).
    pub title: Option<String>,
    /// Una línea de detalle (`msg`).
    pub message: Option<String>,
    /// Identificador de registro del protocolo, si lo trae.
    pub id: Option<String>,
}

impl AgentReport {
    /// Informe de estado simple.
    pub fn new(state: AgentState) -> Self {
        Self {
            state,
            clear: false,
            attention: None,
            progress: None,
            app: None,
            title: None,
            message: None,
            id: None,
        }
    }

    /// Informe que retira el registro.
    pub fn clear() -> Self {
        let mut report = Self::new(AgentState::Idle);
        report.clear = true;
        report
    }
}

/// Cambio de estado que la app puede convertir en aviso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentTransition {
    /// Estado anterior.
    pub from: AgentState,
    /// Estado nuevo.
    pub to: AgentState,
    /// `true` si merece un aviso (atención, terminado o error).
    pub notify: bool,
}

impl AgentTransition {
    fn new(from: AgentState, to: AgentState) -> Self {
        Self {
            from,
            to,
            notify: to.is_terminal_alert() && from != to,
        }
    }
}

/// Sesión de un proceso de agente.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentSession {
    /// Identificador runtime.
    pub id: AgentSessionId,
    /// Agente.
    pub kind: AgentKind,
    /// PID del proceso detectado; `0` mientras solo se conoce por OSC.
    pub pid: u32,
    /// Estado actual.
    pub state: AgentState,
    /// Motivo de la atención.
    pub attention: Option<AttentionKind>,
    /// Detalle de una línea.
    pub message: Option<String>,
    /// Progreso 0–100.
    pub progress: Option<u8>,
    /// Etiqueta reportada por el programa.
    pub title: Option<String>,
    /// `app` reportada por el programa.
    pub app: Option<String>,
    /// Inicio de la sesión.
    pub started_at: SystemTime,
    /// Última actividad o reporte.
    pub last_activity: SystemTime,
    /// `true` si el CLI habló OSC 7501 al menos una vez.
    pub reported: bool,
    /// `true` si el proceso apareció en el último escaneo.
    pub alive: bool,
}

impl AgentSession {
    /// Sesión nueva en estado inactivo.
    pub fn new(id: AgentSessionId, kind: AgentKind, pid: u32, now: SystemTime) -> Self {
        Self {
            id,
            kind,
            pid,
            state: AgentState::Idle,
            attention: None,
            message: None,
            progress: None,
            title: None,
            app: None,
            started_at: now,
            last_activity: now,
            reported: false,
            alive: true,
        }
    }

    /// Aplica un reporte OSC 7501. El reporte reemplaza el registro completo.
    pub fn apply_report(&mut self, report: &AgentReport, now: SystemTime) -> AgentTransition {
        let from = self.state;
        self.reported = true;
        self.last_activity = now;
        self.app = report.app.clone();
        self.title = report.title.clone();

        if report.clear {
            self.state = AgentState::Idle;
            self.attention = None;
            self.progress = None;
            self.message = None;
            return AgentTransition::new(from, self.state);
        }

        self.state = report.state;
        self.attention = (report.state == AgentState::NeedsAttention)
            .then_some(report.attention)
            .flatten();
        self.progress = report.progress.filter(|_| {
            matches!(
                report.state,
                AgentState::Working | AgentState::NeedsAttention
            )
        });
        self.message = report.message.clone();
        if report.state.is_settled() {
            self.progress = None;
        }
        AgentTransition::new(from, self.state)
    }

    /// Señal de atención sin OSC 7501 (bell, OSC 9/777/1337).
    pub fn apply_attention(
        &mut self,
        kind: Option<AttentionKind>,
        message: Option<String>,
        now: SystemTime,
    ) -> AgentTransition {
        let from = self.state;
        self.state = AgentState::NeedsAttention;
        self.attention = kind;
        if message.is_some() {
            self.message = message;
        }
        self.progress = None;
        self.last_activity = now;
        AgentTransition::new(from, self.state)
    }

    /// Actividad de salida del PTY como señal de trabajo (fallback).
    pub fn apply_output_activity(&mut self, now: SystemTime) -> AgentTransition {
        let from = self.state;
        if matches!(self.state, AgentState::Idle | AgentState::Stopped) {
            self.state = AgentState::Working;
            self.last_activity = now;
        }
        AgentTransition::new(from, self.state)
    }

    /// Quiescencia de la salida (fallback): vuelve a inactivo.
    pub fn apply_quiescence(&mut self, now: SystemTime) -> AgentTransition {
        let from = self.state;
        if self.state == AgentState::Working {
            self.state = AgentState::Idle;
            self.last_activity = now;
        }
        AgentTransition::new(from, self.state)
    }

    /// Cierra la sesión cuando el proceso desaparece.
    ///
    /// `done` y `error` sobreviven; `working` se interpreta como terminado y
    /// `blocked` o `idle` se marcan como detenidos (ADR 0009).
    pub fn process_gone(
        &mut self,
        shell_success: Option<bool>,
        now: SystemTime,
    ) -> AgentTransition {
        let from = self.state;
        self.alive = false;
        self.progress = None;
        self.last_activity = now;
        if from.is_terminal_alert() {
            return AgentTransition {
                from,
                to: from,
                notify: false,
            };
        }
        self.state = match (from, shell_success) {
            (_, Some(false)) => AgentState::Error,
            (AgentState::Working, _) => AgentState::Finished,
            _ => AgentState::Stopped,
        };
        self.attention = None;
        AgentTransition::new(from, self.state)
    }

    /// El proceso del shell terminó: cierra los estados en curso con su código.
    pub fn shell_exited(&mut self, success: bool, now: SystemTime) -> AgentTransition {
        let from = self.state;
        if from.is_terminal_alert() {
            self.alive = false;
            return AgentTransition {
                from,
                to: from,
                notify: false,
            };
        }
        self.state = if success {
            AgentState::Finished
        } else {
            AgentState::Error
        };
        self.alive = false;
        self.attention = None;
        self.progress = None;
        self.last_activity = now;
        AgentTransition::new(from, self.state)
    }

    /// Limpia atención, terminado y error al enfocar su pestaña.
    pub fn acknowledge(&mut self, now: SystemTime) -> bool {
        if !self.state.is_terminal_alert() {
            return false;
        }
        self.state = AgentState::Idle;
        self.attention = None;
        self.progress = None;
        self.message = None;
        self.last_activity = now;
        true
    }

    /// `true` si la sesión debe retirarse al enfocar (proceso ya inexistente).
    pub fn is_stale_on_focus(&self) -> bool {
        !self.alive && !self.state.is_terminal_alert()
    }

    /// Duración viva hasta `now`.
    pub fn duration(&self, now: SystemTime) -> Duration {
        now.duration_since(self.started_at).unwrap_or_default()
    }

    /// `true` si requiere al usuario.
    pub fn needs_attention(&self) -> bool {
        self.state == AgentState::NeedsAttention
    }
}

/// Resumen de las sesiones de una pestaña.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentAggregate {
    /// Número de sesiones.
    pub total: usize,
    /// Sesión dominante (mayor prioridad; empate, la más reciente).
    pub dominant: AgentSession,
    /// Estado agregado.
    pub state: AgentState,
    /// Conteo por estado en orden de prioridad.
    pub breakdown: Vec<(AgentState, usize)>,
}

/// Calcula el agregado de una pestaña; `None` sin sesiones.
pub fn aggregate(sessions: &[AgentSession]) -> Option<AgentAggregate> {
    let dominant = sessions
        .iter()
        .max_by(|left, right| {
            left.state
                .priority()
                .cmp(&right.state.priority())
                .then(left.last_activity.cmp(&right.last_activity))
                .then(left.id.cmp(&right.id))
        })?
        .clone();
    let mut breakdown = Vec::new();
    for state in [
        AgentState::NeedsAttention,
        AgentState::Working,
        AgentState::Error,
        AgentState::Finished,
        AgentState::Idle,
        AgentState::Stopped,
    ] {
        let count = sessions
            .iter()
            .filter(|session| session.state == state)
            .count();
        if count > 0 {
            breakdown.push((state, count));
        }
    }
    Some(AgentAggregate {
        total: sessions.len(),
        state: dominant.state,
        dominant,
        breakdown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn session(id: u64, kind: AgentKind, pid: u32, seconds: u64) -> AgentSession {
        AgentSession::new(id, kind, pid, at(seconds))
    }

    #[test]
    fn priority_follows_the_adr() {
        let ordered = [
            AgentState::Stopped,
            AgentState::Idle,
            AgentState::Finished,
            AgentState::Error,
            AgentState::Working,
            AgentState::NeedsAttention,
        ];
        let mut priorities: Vec<u8> = ordered.iter().map(|state| state.priority()).collect();
        let sorted = {
            let mut copy = priorities.clone();
            copy.sort_unstable();
            copy
        };
        assert_eq!(priorities, sorted);
        priorities.dedup();
        assert_eq!(priorities.len(), ordered.len());
        assert!(AgentState::NeedsAttention.priority() > AgentState::Working.priority());
        assert!(AgentState::Working.priority() > AgentState::Error.priority());
        assert!(AgentState::Error.priority() > AgentState::Finished.priority());
        assert!(AgentState::Finished.priority() > AgentState::Idle.priority());
        assert!(AgentState::Idle.priority() > AgentState::Stopped.priority());
    }

    #[test]
    fn protocol_states_and_kinds_map_without_guessing() {
        assert_eq!(
            AgentState::from_protocol("blocked"),
            Some(AgentState::NeedsAttention)
        );
        assert_eq!(
            AgentState::from_protocol("done"),
            Some(AgentState::Finished)
        );
        assert_eq!(AgentState::from_protocol("clear"), None);
        assert_eq!(AgentState::from_protocol("desconocido"), None);
        assert_eq!(
            AttentionKind::from_protocol("permission"),
            Some(AttentionKind::Permission)
        );
        assert_eq!(AttentionKind::from_protocol("otro"), None);
        assert_eq!(AgentKind::from_app("opencode2"), Some(AgentKind::OpenCode));
        assert_eq!(AgentKind::from_app("copilot"), Some(AgentKind::Copilot));
        assert_eq!(AgentKind::from_app("make"), None);
        assert_eq!(AgentKind::from_name("opencode"), Some(AgentKind::OpenCode));
        assert_eq!(AgentKind::from_name("copilot"), Some(AgentKind::Copilot));
        assert_eq!(AgentKind::OpenCode.resume_command(), "opencode --continue");
        assert_eq!(AgentKind::Copilot.resume_command(), "copilot --continue");
    }

    #[test]
    fn a_report_replaces_the_whole_record() {
        let mut session = session(1, AgentKind::OpenCode, 10, 1);
        let mut report = AgentReport::new(AgentState::Working);
        report.app = Some("opencode".into());
        report.message = Some("Compilando".into());
        report.progress = Some(40);
        let transition = session.apply_report(&report, at(2));
        assert!(!transition.notify);
        assert_eq!(transition.to, AgentState::Working);
        assert_eq!(session.progress, Some(40));
        assert_eq!(session.message.as_deref(), Some("Compilando"));

        let done = AgentReport::new(AgentState::Finished);
        let transition = session.apply_report(&done, at(3));
        assert!(transition.notify);
        assert_eq!(transition.from, AgentState::Working);
        assert_eq!(transition.to, AgentState::Finished);

        let idle = AgentReport::new(AgentState::Idle);
        session.apply_report(&idle, at(3));
        assert_eq!(session.app, None);
        assert_eq!(session.message, None);
        assert_eq!(session.progress, None);
    }

    #[test]
    fn blocked_reports_keep_kind_and_progress() {
        let mut session = session(1, AgentKind::Copilot, 11, 1);
        let mut report = AgentReport::new(AgentState::NeedsAttention);
        report.attention = Some(AttentionKind::Question);
        report.progress = Some(10);
        report.message = Some("¿Sobrescribir?".into());
        session.apply_report(&report, at(2));
        assert_eq!(session.attention, Some(AttentionKind::Question));
        assert_eq!(session.progress, Some(10));
        assert!(session.needs_attention());
    }

    #[test]
    fn done_and_error_survive_the_process_but_working_does_not() {
        let mut done = session(1, AgentKind::OpenCode, 10, 1);
        done.apply_report(&AgentReport::new(AgentState::Finished), at(2));
        let transition = done.process_gone(Some(true), at(3));
        assert_eq!(done.state, AgentState::Finished);
        assert!(!transition.notify);
        assert!(!done.alive);

        let mut working = session(2, AgentKind::OpenCode, 10, 1);
        working.apply_report(&AgentReport::new(AgentState::Working), at(2));
        working.process_gone(None, at(3));
        assert_eq!(working.state, AgentState::Finished);

        let mut idle = session(3, AgentKind::OpenCode, 10, 1);
        idle.process_gone(None, at(3));
        assert_eq!(idle.state, AgentState::Stopped);
    }

    #[test]
    fn shell_exit_uses_the_exit_code_as_fallback() {
        let mut session = session(1, AgentKind::OpenCode, 10, 1);
        session.apply_report(&AgentReport::new(AgentState::Working), at(2));
        let transition = session.shell_exited(false, at(3));
        assert_eq!(session.state, AgentState::Error);
        assert!(transition.notify);

        let mut success = AgentSession::new(2, AgentKind::OpenCode, 12, at(1));
        success.shell_exited(true, at(4));
        assert_eq!(success.state, AgentState::Finished);
    }

    #[test]
    fn attention_latches_until_acknowledged() {
        let mut session = session(1, AgentKind::OpenCode, 10, 1);
        session.apply_attention(None, Some("campana".into()), at(2));
        assert_eq!(session.state, AgentState::NeedsAttention);
        session.apply_output_activity(at(3));
        assert_eq!(session.state, AgentState::NeedsAttention);
        assert!(!session.is_stale_on_focus());
        assert!(session.acknowledge(at(4)));
        assert_eq!(session.state, AgentState::Idle);
        assert_eq!(session.message, None);
        assert!(!session.acknowledge(at(5)));
    }

    #[test]
    fn focus_removes_sessions_whose_process_is_gone() {
        let mut live = session(1, AgentKind::OpenCode, 10, 1);
        live.alive = true;
        let mut gone = session(2, AgentKind::OpenCode, 11, 1);
        gone.process_gone(None, at(2));
        assert!(!gone.state.is_terminal_alert() && gone.is_stale_on_focus());

        let mut settled = session(3, AgentKind::OpenCode, 12, 1);
        settled.apply_report(&AgentReport::new(AgentState::Finished), at(2));
        settled.process_gone(Some(true), at(3));
        // Terminado sin proceso se retira al enfocar, no antes.
        assert!(!settled.is_stale_on_focus());
        settled.acknowledge(at(4));
        assert!(!settled.alive);
        assert!(settled.is_stale_on_focus());
    }

    #[test]
    fn aggregate_picks_the_highest_priority_and_breaks_down_states() {
        let mut working = session(1, AgentKind::OpenCode, 10, 1);
        working.apply_report(&AgentReport::new(AgentState::Working), at(2));
        let mut attention = session(2, AgentKind::Copilot, 11, 1);
        attention.apply_attention(Some(AttentionKind::Permission), None, at(3));
        let aggregate = aggregate(&[working, attention]).expect("agregado");
        assert_eq!(aggregate.total, 2);
        assert_eq!(aggregate.state, AgentState::NeedsAttention);
        assert_eq!(aggregate.dominant.kind, AgentKind::Copilot);
        assert_eq!(
            aggregate.breakdown,
            vec![(AgentState::NeedsAttention, 1), (AgentState::Working, 1),]
        );
    }

    #[test]
    fn aggregate_ties_go_to_the_most_recent_session() {
        let first = session(1, AgentKind::OpenCode, 10, 1);
        let second = session(2, AgentKind::OpenCode, 11, 2);
        let aggregate = aggregate(&[first, second]).expect("agregado");
        assert_eq!(aggregate.dominant.id, 2);
    }
}
