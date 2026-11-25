use std::time::{Duration, SystemTime};
pub type AgentSessionId = u64;
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
            Self::OpenCode => "OpenCode",
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
    /// Id de sesión reportado por el agente, para reanudarla al reabrir.
    pub external_id: Option<String>,
    /// Sesiones observadas por OSC y su título, para elegir la del terminal.
    pub observed_sessions: Vec<ReportedSession>,
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
            external_id: None,
            observed_sessions: Vec::new(),
            started_at: now,
            last_activity: now,
            reported: false,
            alive: true,
        }
    }

    /// Aplica un reporte OSC 7501. El reporte reemplaza el registro completo.
    pub fn apply_report(&mut self, report: &AgentReport, now: SystemTime) -> AgentTransition {
        let from = self.state;
        let was_reported = self.reported;
        self.reported = true;
        self.last_activity = now;
        self.app = report.app.clone();
        self.title = report.title.clone();
        // El registro raíz no trae id; conservamos el último id de sesión
        // válido para poder reanudar exactamente esta conversación. OpenCode
        // comparte la tira de pestañas entre terminales del mismo directorio,
        // así que también guardamos el título para elegir la sesión actual.
        if let Some(id) = report
            .id
            .as_deref()
            .filter(|id| self.kind.is_resumable_session_id(id))
        {
            self.external_id = Some(id.to_owned());
            if let Some(known) = self
                .observed_sessions
                .iter_mut()
                .find(|known| known.id == id)
            {
                if report.title.is_some() {
                    known.title = report.title.clone();
                }
            } else {
                self.observed_sessions.push(ReportedSession {
                    id: id.to_owned(),
                    title: report.title.clone(),
                });
                if self.observed_sessions.len() > MAX_OBSERVED_SESSIONS {
                    self.observed_sessions.remove(0);
                }
            }
        }

        // OpenCode 2 no emite `done`: al acabar un turno reporta `idle` en el
        // registro raíz y `clear` en el registro de la sesión. Para que el
        // aviso de «terminado» exista, ambos cierran el trabajo en curso como
        // `Finished`. Un `idle`/`clear` posterior conserva el resultado
        // pendiente hasta que el usuario enfoque la pestaña. Exigimos que el
        // `Working` viniera de un reporte OSC: el arranque de la TUI produce
        // salida y no debe confundirse con una tarea terminada.
        let settles_opencode = self.kind == AgentKind::OpenCode
            && was_reported
            && from == AgentState::Working
            && (report.clear || report.state == AgentState::Idle);
        let keeps_finished = self.kind == AgentKind::OpenCode
            && from == AgentState::Finished
            && (report.clear || report.state == AgentState::Idle);
        if keeps_finished {
            self.attention = None;
            self.progress = None;
            self.message = if report.clear {
                None
            } else {
                report.message.clone()
            };
            return AgentTransition {
                from,
                to: from,
                notify: false,
            };
        }

        if report.clear {
            self.state = if settles_opencode {
                AgentState::Finished
            } else {
                AgentState::Idle
            };
            self.attention = None;
            self.progress = None;
            self.message = None;
            return AgentTransition::new(from, self.state);
        }

        self.state = if settles_opencode {
            AgentState::Finished
        } else {
            report.state
        };
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
