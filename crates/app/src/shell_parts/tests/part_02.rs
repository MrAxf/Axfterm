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
#[test]
fn file_drag_hover_rearms_only_when_the_target_changes() {
    let mut state = FileDragHoverState::default();
    let tab = FileDragHover::Tab(TabId::new(1));
    let other = FileDragHover::Tab(TabId::new(2));
    let seq = state.observe(tab, true).expect("arma el temporizador");
    assert!(state.is_current(tab, seq));
    // Moverse dentro del mismo objetivo no reinicia la espera.
    assert_eq!(state.observe(tab, true), None);
    assert!(state.is_current(tab, seq));
    // Otro objetivo cancela el anterior por secuencia.
    let next = state.observe(other, true).expect("arma el nuevo objetivo");
    assert!(!state.is_current(tab, seq));
    assert!(state.is_current(other, next));
    // Salir de un objetivo que no está armado no afecta al vigente.
    assert_eq!(state.observe(tab, false), None);
    assert!(state.is_current(other, next));
    // Salir del objetivo vigente lo desarma.
    assert_eq!(state.observe(other, false), None);
    assert!(!state.is_current(other, next));
}
#[test]
fn clearing_file_drag_hover_invalidates_pending_timers() {
    let mut state = FileDragHoverState::default();
    let tab = FileDragHover::Tab(TabId::new(7));
    let seq = state.observe(tab, true).expect("arma el temporizador");
    state.clear();
    assert!(!state.is_current(tab, seq));
    let next = state.observe(tab, true).expect("vuelve a armar");
    assert_ne!(next, seq);
}
#[test]
fn file_drag_rows_outside_the_viewport_do_not_count() {
    let row = Bounds::new(point(px(20.0), px(120.0)), size(px(200.0), px(32.0)));
    let viewport = Bounds::new(point(px(0.0), px(100.0)), size(px(240.0), px(200.0)));
    assert!(file_drag_over(
        row,
        point(px(30.0), px(130.0)),
        Some(viewport)
    ));
    // La fila existe por encima del viewport, pero está oculta por el scroll.
    assert!(!file_drag_over(
        row,
        point(px(30.0), px(90.0)),
        Some(viewport)
    ));
    // Sin viewport medido todavía mandan solo los bounds de la fila.
    assert!(file_drag_over(row, point(px(30.0), px(130.0)), None));
}
#[test]
fn resume_command_uses_the_profile_override_and_keeps_continue() {
    assert_eq!(
        agent_resume_command(Some("opencode2"), AgentKind::OpenCode),
        "opencode2 --continue"
    );
    assert_eq!(
        agent_resume_command(Some("  bun x opencode  "), AgentKind::OpenCode),
        "bun x opencode --continue"
    );
    assert_eq!(
        agent_resume_command(None, AgentKind::OpenCode),
        "opencode --continue"
    );
    assert_eq!(
        agent_resume_command(None, AgentKind::Copilot),
        "copilot --continue"
    );
}
