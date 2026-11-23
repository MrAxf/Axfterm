#[test]
fn reconcile_adds_matches_and_closes_processes() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut next_id = 1;
    let mut sessions = Vec::new();
    let (changed, transitions) = reconcile_agents(
        &mut sessions,
        &[DetectedAgent {
            kind: AgentKind::OpenCode,
            pid: 42,
        }],
        true,
        None,
        true,
        now,
        &mut next_id,
    );
    assert!(changed);
    assert!(transitions.is_empty());
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].state, AgentState::Working);
    let (changed, transitions) =
        reconcile_agents(&mut sessions, &[], true, None, false, now, &mut next_id);
    assert!(changed);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].state, AgentState::Finished);
    assert_eq!(transitions.len(), 1);

    // Si el usuario lanza un segundo agente del mismo tipo, reutiliza la
    // única sesión de la terminal con su nuevo PID.
    let (changed, _) = reconcile_agents(
        &mut sessions,
        &[
            DetectedAgent {
                kind: AgentKind::OpenCode,
                pid: 99,
            },
            DetectedAgent {
                kind: AgentKind::OpenCode,
                pid: 100,
            },
        ],
        true,
        None,
        false,
        now,
        &mut next_id,
    );
    assert!(changed);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].pid, 99);
    assert!(sessions[0].alive);
}
#[test]
fn shell_exit_uses_the_exit_code_for_pending_sessions() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let mut next_id = 5;
    let mut sessions = vec![test_agent(
        1,
        AgentKind::OpenCode,
        42,
        AgentState::Working,
        90,
    )];
    let (_, transitions) = reconcile_agents(
        &mut sessions,
        &[],
        false,
        Some(axf_terminal::ExitInfo {
            code: Some(1),
            success: false,
        }),
        false,
        now,
        &mut next_id,
    );
    assert_eq!(sessions[0].state, AgentState::Error);
    assert_eq!(transitions.len(), 1);
}
#[test]
fn context_path_hides_the_root_and_compacts_parents() {
    let separator = std::path::MAIN_SEPARATOR;
    let root = std::path::PathBuf::from("proyecto");
    let inside = root.join("crates").join("app");
    let outside = std::path::PathBuf::from("otro").join("lugar");
    assert_eq!(
        context_path(&root, &inside),
        Some(format!("crates{separator}app"))
    );
    assert_eq!(context_path(&root, &root), None);
    assert_eq!(
        context_path(&root, &outside),
        Some(format!("…{separator}lugar"))
    );
}
