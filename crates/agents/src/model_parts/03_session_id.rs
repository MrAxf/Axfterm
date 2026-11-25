const MAX_OBSERVED_SESSIONS: usize = 16;

/// Sesión reportada por OSC 7501, con el título que muestra su pestaña.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportedSession {
    /// Id reanudable de la sesión.
    pub id: String,
    /// Título visible reportado por el agente.
    pub title: Option<String>,
}

impl AgentSession {
    /// Id de la sesión que corresponde al título visible del terminal.
    ///
    /// OpenCode comparte su tira de pestañas entre terminales del mismo
    /// directorio y reporta todas las sesiones; el título del terminal
    /// distingue la que está en pantalla. Si el título no permite decidir, se
    /// usa la única sesión observada o el último id válido.
    pub fn resume_session_id(&self, terminal_title: Option<&str>) -> Option<&str> {
        let title = terminal_title.map(str::trim).filter(|title| !title.is_empty());
        if let Some(title) = title {
            let matches: Vec<&str> = self
                .observed_sessions
                .iter()
                .filter(|known| {
                    known
                        .title
                        .as_deref()
                        .is_some_and(|candidate| title_matches(title, candidate))
                })
                .map(|known| known.id.as_str())
                .collect();
            if let [id] = matches.as_slice() {
                return Some(id);
            }
        }
        if let [only] = self.observed_sessions.as_slice() {
            return Some(only.id.as_str());
        }
        self.external_id.as_deref()
    }
}

fn title_matches(terminal_title: &str, session_title: &str) -> bool {
    let session_title = session_title.trim();
    if session_title.is_empty() {
        return false;
    }
    if terminal_title == session_title {
        return true;
    }
    terminal_title
        .strip_suffix(session_title)
        .is_some_and(|prefix| {
            prefix
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_alphanumeric())
        })
}

impl AgentKind {
    /// `true` si `id` identifica una sesión reanudable de este agente.
    ///
    /// OpenCode publica el id de sesión (`ses_…`) en sus registros OSC 7501 y
    /// Copilot usa un UUID. Se rechaza cualquier otro valor (por ejemplo el
    /// registro sintético `opencode`) antes de construir un comando.
    pub fn is_resumable_session_id(self, id: &str) -> bool {
        let id = id.trim();
        match self {
            Self::OpenCode => {
                let Some(suffix) = id.strip_prefix("ses_") else {
                    return false;
                };
                (27..=32).contains(&id.len())
                    && !suffix.is_empty()
                    && suffix
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
            }
            Self::Copilot => {
                id.len() == 36
                    && id.chars().enumerate().all(|(index, character)| {
                        if matches!(index, 8 | 13 | 18 | 23) {
                            character == '-'
                        } else {
                            character.is_ascii_hexdigit()
                        }
                    })
            }
        }
    }
}
