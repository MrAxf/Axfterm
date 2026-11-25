use super::*;
use axf_ui::gpui::size;
#[cfg(feature = "studio")]
#[test]
fn studio_demo_isolates_powershell_profile_without_changing_normal_spawns() {
    let powershell = axf_terminal::SpawnSpec::new(
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        vec!["-NoLogo".into()],
    );
    assert_eq!(
        studio_spawn_spec(powershell.clone(), true),
        axf_terminal::SpawnSpec::new(
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            vec!["-NoLogo".into(), "-NoProfile".into()],
        )
    );
    assert_eq!(studio_spawn_spec(powershell.clone(), false), powershell);
    assert_eq!(
        studio_spawn_spec(
            axf_terminal::SpawnSpec::new("powershell.EXE", vec!["-noprofile".into()]),
            true,
        )
        .args,
        ["-noprofile"]
    );
    assert_eq!(
        studio_spawn_spec(axf_terminal::SpawnSpec::new("cmd.exe", vec![]), true),
        axf_terminal::SpawnSpec::new("cmd.exe", vec![])
    );
}
#[test]
fn split_keys_are_stable_and_unambiguous() {
    assert_eq!(split_key(&[]), "");
    assert_eq!(split_key(&[Branch::First]), "0");
    assert_eq!(split_key(&[Branch::Second]), "1");
    assert_eq!(
        split_key(&[Branch::Second, Branch::First, Branch::Second]),
        "101"
    );
}
#[test]
fn the_title_centers_on_the_window_and_avoids_the_chrome() {
    let typical = title_bar_title_geometry(px(1280.0), px(240.0), px(138.0), px(480.0), px(8.0));
    assert_eq!(typical.max_width, px(480.0));
    assert_eq!(typical.padding_left, px(0.0));
    assert_eq!(typical.padding_right, px(102.0));
    let narrow = title_bar_title_geometry(px(600.0), px(200.0), px(138.0), px(480.0), px(8.0));
    assert_eq!(narrow.max_width, px(184.0));
    assert_eq!(narrow.padding_right, px(62.0));

    // La sidebar ocupa más de media ventana: no hay centro real libre, así
    // que se centra en el hueco y se recorta al espacio disponible.
    let wide_sidebar =
        title_bar_title_geometry(px(900.0), px(480.0), px(138.0), px(480.0), px(8.0));
    assert_eq!(wide_sidebar.max_width, px(266.0));
    assert_eq!(wide_sidebar.padding_left, px(0.0));
    assert_eq!(wide_sidebar.padding_right, px(0.0));

    // Caso simétrico inverso, no alcanzable en Windows, pero la geometría
    // no debe depender de que la sidebar sea la franja más ancha.
    let wide_controls =
        title_bar_title_geometry(px(1280.0), px(120.0), px(200.0), px(480.0), px(8.0));
    assert_eq!(wide_controls.padding_left, px(80.0));
    assert_eq!(wide_controls.padding_right, px(0.0));
}
#[test]
fn tab_scroll_offsets_stay_within_the_scrollable_range() {
    let step = px(100.0);
    let max = px(250.0);
    assert_eq!(tab_scroll_offset(px(-200.0), max, step), px(-100.0));
    assert_eq!(tab_scroll_offset(px(-50.0), max, step), px(0.0));
    assert_eq!(tab_scroll_offset(px(-200.0), max, -step), px(-250.0));
    assert_eq!(tab_scroll_offset(px(0.0), max, -step), px(-100.0));
    assert_eq!(tab_scroll_offset(px(-10.0), px(0.0), step), px(0.0));
}
#[test]
fn powershell_uses_its_brand_icon_and_the_rest_falls_back() {
    assert_eq!(terminal_tab_icon(Some("pwsh")), icons::BRAND_POWERSHELL);
    assert_eq!(terminal_tab_icon(Some("pwsh.exe")), icons::BRAND_POWERSHELL);
    assert_eq!(
        terminal_tab_icon(Some(r"C:\Program Files\PowerShell\7\pwsh.exe")),
        icons::BRAND_POWERSHELL
    );
    assert_eq!(
        terminal_tab_icon(Some("powershell.exe")),
        icons::BRAND_POWERSHELL
    );
    assert_eq!(terminal_tab_icon(Some("cmd.exe")), icons::TERMINAL);
    assert_eq!(terminal_tab_icon(Some("/bin/bash")), icons::TERMINAL);
    assert_eq!(terminal_tab_icon(None), icons::TERMINAL);
}
#[test]
fn tab_drop_zones_prefer_the_nearest_edge_within_the_band() {
    let bounds = Bounds::new(point(px(100.0), px(50.0)), size(px(400.0), px(200.0)));
    let ratio = 0.25;
    assert_eq!(
        tab_drop_zone(point(px(110.0), px(150.0)), bounds, ratio),
        TabDropZone::Left
    );
    assert_eq!(
        tab_drop_zone(point(px(490.0), px(150.0)), bounds, ratio),
        TabDropZone::Right
    );
    assert_eq!(
        tab_drop_zone(point(px(300.0), px(60.0)), bounds, ratio),
        TabDropZone::Top
    );
    assert_eq!(
        tab_drop_zone(point(px(300.0), px(240.0)), bounds, ratio),
        TabDropZone::Bottom
    );
    assert_eq!(
        tab_drop_zone(point(px(300.0), px(150.0)), bounds, ratio),
        TabDropZone::Center
    );

    // En una esquina gana el borde más cercano.
    assert_eq!(
        tab_drop_zone(point(px(105.0), px(55.0)), bounds, ratio),
        TabDropZone::Left
    );
    assert!(bounds_contain(bounds, point(px(100.0), px(50.0))));
    assert!(!bounds_contain(bounds, point(px(99.0), px(150.0))));
}
#[test]
fn insert_index_accounts_for_removing_the_dragged_tab() {
    assert_eq!(adjusted_insert_index(Some(0), 2), 1);
    assert_eq!(adjusted_insert_index(Some(2), 0), 0);
    assert_eq!(adjusted_insert_index(Some(1), 1), 1);
    assert_eq!(adjusted_insert_index(None, usize::MAX), usize::MAX);
    assert_eq!(adjusted_insert_index(Some(0), usize::MAX), usize::MAX - 1);
}
#[test]
fn tab_widths_compress_uniformly_between_min_and_max() {
    let min = px(90.0);
    let max = px(160.0);

    // Cabe el ancho objetivo con holgura: no crece de más.
    assert_eq!(tab_slot_width(px(1000.0), 3, min, max), 160.0);
    // Se reparte el ancho disponible hasta el objetivo.
    assert_eq!(tab_slot_width(px(480.0), 3, min, max), 160.0);
    assert_eq!(tab_slot_width(px(300.0), 3, min, max), 100.0);
    // Por debajo del mínimo manda el mínimo y la strip hace scroll.
    assert_eq!(tab_slot_width(px(120.0), 3, min, max), 90.0);
    assert_eq!(tab_slot_width(px(120.0), 0, min, max), 0.0);
}
#[test]
fn insert_index_moves_at_each_tab_center() {
    // Tres pestañas de 200: los medios están en 100, 300 y 500.
    assert_eq!(insert_index_at(0.0, 200.0, 3), 0);
    assert_eq!(insert_index_at(99.0, 200.0, 3), 0);
    assert_eq!(insert_index_at(100.0, 200.0, 3), 1);
    assert_eq!(insert_index_at(299.0, 200.0, 3), 1);
    assert_eq!(insert_index_at(300.0, 200.0, 3), 2);
    assert_eq!(insert_index_at(599.0, 200.0, 3), 3);
    assert_eq!(insert_index_at(900.0, 200.0, 3), 3);
    assert_eq!(insert_index_at(10.0, 200.0, 0), 0);
    assert_eq!(insert_index_at(10.0, 0.0, 3), 0);
}
#[test]
fn tab_autoscroll_steps_once_and_clamps() {
    let left = px(100.0);
    let right = px(700.0);
    let edge = px(20.0);
    let nudge = px(15.0);
    let max = px(500.0);

    // Fuera de la franja no se mueve.
    assert_eq!(
        tab_autoscroll_offset(px(-100.0), max, px(400.0), left, right, edge, nudge),
        px(-100.0)
    );

    // Cada movimiento aplica exactamente un paso, nunca uno por pestaña.
    assert_eq!(
        tab_autoscroll_offset(px(-100.0), max, px(690.0), left, right, edge, nudge),
        px(-115.0)
    );
    assert_eq!(
        tab_autoscroll_offset(px(-100.0), max, px(110.0), left, right, edge, nudge),
        px(-85.0)
    );

    // Se acota a los extremos.
    assert_eq!(
        tab_autoscroll_offset(px(-495.0), max, px(690.0), left, right, edge, nudge),
        px(-500.0)
    );
    assert_eq!(
        tab_autoscroll_offset(px(-5.0), max, px(110.0), left, right, edge, nudge),
        px(0.0)
    );

    // Sin overflow no hay desplazamiento.
    assert_eq!(
        tab_autoscroll_offset(px(0.0), px(0.0), px(690.0), left, right, edge, nudge),
        px(0.0)
    );
}
#[test]
fn shell_path_titles_are_ignored() {
    assert!(title_is_shell_path(
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        Some("pwsh")
    ));
    assert!(title_is_shell_path(
        r"Administrador: C:\Program Files\PowerShell\7\pwsh.exe",
        Some("pwsh")
    ));
    assert!(title_is_shell_path("pwsh", Some("pwsh")));
    assert!(title_is_shell_path("/bin/bash", Some("/bin/bash")));
    assert!(!title_is_shell_path(r"C:\Proyectos\axfterm", Some("pwsh")));
    assert!(!title_is_shell_path("axfterm", Some("pwsh")));
    assert!(!title_is_shell_path("pwsh.exe", None));
}
#[test]
fn a_manual_tab_name_wins_over_osc_and_the_profile() {
    let mut tab = axf_workspace::Tab::new(
        ProjectId::new(1),
        TerminalId::new(2),
        ProfileId::new("pwsh"),
        "PowerShell 7",
    );
    assert_eq!(
        resolve_tab_title(
            &tab,
            Some(r"C:\Program Files\PowerShell\7\pwsh.exe"),
            Some("pwsh"),
            Some("adventure2")
        ),
        "PowerShell 7"
    );
    tab.custom_title = true;
    tab.title = "Mi shell".into();
    assert_eq!(
        resolve_tab_title(&tab, Some("C:\\otro"), Some("pwsh"), Some("adventure2")),
        "Mi shell"
    );
    tab.custom_title = false;
    tab.title = "PowerShell 7".into();
    assert_eq!(
        resolve_tab_title(&tab, Some("nvim ~/proyecto"), Some("pwsh"), None),
        "nvim ~/proyecto"
    );
    assert_eq!(
        resolve_tab_title(
            &tab,
            Some(r"C:\Program Files\PowerShell\7\pwsh.exe"),
            Some("pwsh"),
            None
        ),
        "PowerShell 7"
    );
    tab.auto_title.clear();
    assert_eq!(
        resolve_tab_title(&tab, None, None, Some("adventure2")),
        "PowerShell 7"
    );
    tab.title.clear();
    assert_eq!(
        resolve_tab_title(&tab, None, None, Some("adventure2")),
        "adventure2"
    );
    assert_eq!(resolve_tab_title(&tab, None, None, None), "Terminal");
}
fn sidebar_temp_project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("axfterm-sidebar-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("carpeta temporal");
    dir
}
fn sidebar_profile() -> ProfileId {
    ProfileId::new("shell")
}
const SIDEBAR_PROFILE_TITLE: &str = "Shell";
fn sidebar_workspace(name: &str) -> (Workspace, ProjectId, TabId) {
    let mut workspace = Workspace::new();
    let root = sidebar_temp_project(name);
    let opened = workspace
        .open_project(&root, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
        .expect("abre proyecto");
    let tab = opened.tab.expect("pestaña inicial");
    (workspace, opened.project, tab.tab)
}
#[test]
fn sidebar_view_shows_expanded_projects_and_their_tabs() {
    let (workspace, project, tab) = sidebar_workspace("view-all");
    let groups = sidebar_view(&workspace, SidebarSection::All);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].project, project);
    assert_eq!(groups[0].tabs, vec![tab]);
    assert!(groups[0].show_tabs);
    assert_eq!(
        sidebar_rows_from_groups(&groups),
        vec![SidebarRow::Project(project), SidebarRow::Tab(tab)]
    );
}
#[test]
fn sidebar_view_keeps_empty_projects() {
    let mut workspace = Workspace::new();
    let root = sidebar_temp_project("empty-project");
    let (project, _) = workspace
        .register_project(&root)
        .expect("registra proyecto vacío");
    let groups = sidebar_view(&workspace, SidebarSection::All);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].project, project);
    assert!(groups[0].tabs.is_empty());
    assert_eq!(
        sidebar_rows_from_groups(&groups),
        vec![SidebarRow::Project(project)]
    );
    assert!(sidebar_view(&workspace, SidebarSection::Agents).is_empty());
}
fn test_agent(
    id: AgentSessionId,
    kind: AgentKind,
    pid: u32,
    state: AgentState,
    seconds: u64,
) -> AgentSession {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
    let mut session = AgentSession::new(id, kind, pid, now);
    let mut report = AgentReport::new(state);
    report.message = Some(format!("tarea {id}"));
    session.apply_report(&report, now);
    session
}
#[test]
fn sidebar_agents_filter_keeps_agent_tabs_and_orders_by_priority() {
    let (mut workspace, project, tab) = sidebar_workspace("agents-section");
    let pane = workspace.pane_of_tab(tab).expect("panel");
    let second = workspace
        .new_tab(pane, project, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
        .expect("segunda pestaña")
        .tab;
    let _third_without_agent = workspace
        .new_tab(pane, project, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
        .expect("tercera pestaña sin agente")
        .tab;
    workspace.set_tab_agents(
        tab,
        vec![test_agent(
            1,
            AgentKind::OpenCode,
            10,
            AgentState::Working,
            1,
        )],
    );
    workspace.set_tab_agents(
        second,
        vec![test_agent(
            2,
            AgentKind::Copilot,
            11,
            AgentState::NeedsAttention,
            2,
        )],
    );
    let _ = workspace.set_project_expanded(project, false);
    let groups = sidebar_view(&workspace, SidebarSection::Agents);
    assert_eq!(groups.len(), 1);
    assert!(groups[0].show_tabs, "en Agentes siempre se despliegan");
    assert_eq!(groups[0].tabs, vec![second, tab], "atención primero");
    let rows = sidebar_rows_from_groups(&groups);
    assert_eq!(
        rows,
        vec![
            SidebarRow::Project(project),
            SidebarRow::Tab(second),
            SidebarRow::Tab(tab),
        ]
    );
}
#[test]
fn palette_attention_jump_cycles_only_tabs_that_need_attention() {
    let (mut workspace, project, tab) = sidebar_workspace("attention-jump");
    let pane = workspace.pane_of_tab(tab).expect("panel");
    let second = workspace
        .new_tab(pane, project, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
        .expect("segunda pestaña")
        .tab;
    let _third = workspace
        .new_tab(pane, project, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
        .expect("tercera pestaña")
        .tab;
    workspace.set_tab_agents(
        tab,
        vec![test_agent(
            1,
            AgentKind::OpenCode,
            10,
            AgentState::NeedsAttention,
            1,
        )],
    );
    workspace.set_tab_agents(
        second,
        vec![test_agent(
            2,
            AgentKind::Copilot,
            11,
            AgentState::NeedsAttention,
            2,
        )],
    );
    let attention = attention_tabs(&workspace);
    assert_eq!(attention, vec![tab, second]);
    assert_eq!(next_attention(&attention, Some(tab)), Some(second));
    assert_eq!(next_attention(&attention, Some(second)), Some(tab));
    assert_eq!(next_attention(&attention, None), Some(tab));
    assert_eq!(next_attention(&[], None), None);
}
#[test]
fn sidebar_sections_toggle_between_all_and_agents() {
    assert_eq!(SidebarSection::All.toggled(), SidebarSection::Agents);
    assert_eq!(SidebarSection::Agents.toggled(), SidebarSection::All);
    assert_eq!(SidebarSection::All.label(), "Todas");
    assert_eq!(SidebarSection::Agents.label(), "Agentes");
}
#[test]
fn sidebar_agent_metadata_shows_state_before_truncated_detail() {
    let mut session = test_agent(1, AgentKind::Copilot, 1, AgentState::NeedsAttention, 1);
    session.progress = Some(52);
    assert_eq!(
        sidebar_agent_metadata(&session, Some("orbit".into())),
        ["Copilot", "atención", "orbit", "52%", "tarea 1"]
    );
}
#[test]
fn agent_states_map_to_tab_status_and_icons() {
    assert_eq!(
        tab_status_from_state(AgentState::NeedsAttention),
        TabStatus::Attention
    );
    assert_eq!(
        tab_status_from_state(AgentState::Finished),
        TabStatus::Finished
    );
    assert_eq!(tab_status_from_state(AgentState::Error), TabStatus::Error);
    assert_eq!(
        tab_status_from_state(AgentState::Working),
        TabStatus::Working
    );
    assert_eq!(agent_icon(AgentKind::OpenCode), icons::BRAND_OPENCODE);
    assert_eq!(agent_icon(AgentKind::Copilot), icons::BRAND_COPILOT);
}
#[test]
fn collapsed_project_shows_its_most_urgent_agent_state() {
    assert_eq!(most_urgent_agent_state([]), None);
    assert_eq!(
        most_urgent_agent_state([AgentState::Idle, AgentState::Working, AgentState::Error]),
        Some(AgentState::Working)
    );
    assert_eq!(
        most_urgent_agent_state([AgentState::Working, AgentState::NeedsAttention]),
        Some(AgentState::NeedsAttention)
    );
}
#[test]
fn durations_are_compact() {
    assert_eq!(format_duration(Duration::from_secs(9)), "9s");
    assert_eq!(format_duration(Duration::from_secs(61)), "1m 01s");
    assert_eq!(format_duration(Duration::from_secs(3_601)), "1h 00m");
}
#[test]
fn osc_status_maps_to_agent_states_and_attention() {
    let mut status = ProgramStatus {
        state: ProgramState::Blocked,
        kind: Some("auth".into()),
        progress: Some(5),
        app: Some("copilot".into()),
        title: None,
        message: Some("Login".into()),
        id: None,
    };
    let report = agent_report_from_status(&status);
    assert_eq!(report.state, AgentState::NeedsAttention);
    assert_eq!(report.attention, Some(AttentionKind::Auth));
    assert!(!report.clear);
    status.state = ProgramState::Clear;
    let report = agent_report_from_status(&status);
    assert!(report.clear);
    assert_eq!(report.state, AgentState::Idle);
}
