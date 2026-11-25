/// Comando base de un agente, con el override opcional del perfil.
///
/// El perfil puede cambiar el ejecutable (por ejemplo `opencode2` en WSL); sin
/// override se usa el nombre estable del agente.
fn agent_base_command(override_command: Option<&str>, agent: AgentKind) -> &str {
    override_command
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .unwrap_or_else(|| agent.as_str())
}

/// Línea que se escribe al shell para abrir el agente en una pestaña nueva.
fn agent_launch_command(override_command: Option<&str>, agent: AgentKind) -> String {
    agent_base_command(override_command, agent).to_owned()
}

/// Comando de reanudación con el override opcional del perfil.
///
/// Si el agente reportó una sesión reanudable se abre exactamente esa; el id se
/// valida antes de escribirlo en el shell. Sin id se conserva el fallback
/// `--continue`, que reanuda la última sesión de ese directorio.
fn agent_resume_command(
    override_command: Option<&str>,
    agent: AgentKind,
    session: Option<&str>,
) -> String {
    let base = agent_base_command(override_command, agent);
    match session.filter(|id| agent.is_resumable_session_id(id)) {
        Some(id) if agent == AgentKind::OpenCode => format!("{base} --session {id}"),
        Some(id) => format!("{base} --resume={id}"),
        None => format!("{base} --continue"),
    }
}
