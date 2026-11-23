//! Modelo puro del workspace.
//!
//! Todas las operaciones conservan los invariantes de ADR 0003: una pestaña
//! pertenece a un único panel, el foco apunta a una pestaña viva y siempre hay
//! al menos un panel. No hay estado duplicado del proyecto actual: se deriva
//! del foco.

include!("model_parts/01_workspaceerror.rs");
include!("model_parts/02_workspace_new.rs");
include!("model_parts/03_workspace_move_tab.rs");
include!("model_parts/04_workspace_counters.rs");
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Branch, SplitAxis};

    fn temp_project(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("axfterm-workspace-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("carpeta temporal");
        dir
    }

    fn profile() -> ProfileId {
        ProfileId::new("shell")
    }

    const PROFILE_TITLE: &str = "Shell";

    fn workspace_with_project(name: &str) -> (Workspace, ProjectId, NewTab) {
        let mut workspace = Workspace::new();
        let root = temp_project(name);
        let opened = workspace
            .open_project(&root, &profile(), PROFILE_TITLE)
            .expect("abre proyecto");
        let tab = opened.tab.expect("pestaña inicial");
        (workspace, opened.project, tab)
    }

    #[test]
    fn empty_workspace_is_valid() {
        let workspace = Workspace::new();
        assert_eq!(workspace.layout().leaves().len(), 1);
        assert_eq!(workspace.tab_count(), 0);
        assert_eq!(workspace.focus().tab, None);
        assert_eq!(workspace.current_project(), None);
    }

    #[test]
    fn open_project_creates_tab_only_once() {
        let mut workspace = Workspace::new();
        let root = temp_project("open-once");
        let first = workspace
            .open_project(&root, &profile(), PROFILE_TITLE)
            .expect("primer open");
        assert!(first.tab.is_some());
        let second = workspace
            .open_project(&root, &profile(), PROFILE_TITLE)
            .expect("segundo open");
        assert_eq!(second.project, first.project);
        assert!(second.tab.is_none());
        assert_eq!(workspace.tab_count(), 1);
    }

    #[test]
    fn registering_the_same_root_is_deduplicated() {
        let root = temp_project("dedupe");
        let mut workspace = Workspace::new();
        let (first, added) = workspace.register_project(&root).expect("registra");
        assert!(added);
        let canonical = std::fs::canonicalize(&root).expect("canonicaliza");
        let (second, added) = workspace
            .register_project(canonical.join("."))
            .expect("registra otra forma");
        assert!(!added);
        assert_eq!(first, second);
        assert_eq!(workspace.projects().len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn registered_root_is_not_verbatim() {
        let root = temp_project("friendly-root");
        let mut workspace = Workspace::new();
        let (project, _) = workspace.register_project(&root).expect("registra");
        let stored = workspace.project(project).expect("proyecto").root.clone();
        let expected =
            axf_terminal::paths::normalize(&std::fs::canonicalize(&root).expect("canonicaliza"));
        assert_eq!(stored, expected);
        assert!(!stored.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn split_and_close_collapse_the_tree() {
        let (mut workspace, _project, tab) = workspace_with_project("split");
        let pane = workspace.pane_of_tab(tab.tab).expect("panel original");
        let new_pane = workspace
            .split_pane(pane, SplitAxis::Horizontal)
            .expect("divide");
        assert_eq!(workspace.layout().leaves().len(), 2);
        assert_eq!(workspace.focus().pane, new_pane);

        let closed = workspace.close_pane(new_pane).expect("cierra panel");
        assert!(closed.is_empty());
        assert_eq!(workspace.layout().leaves(), vec![pane]);
        assert_eq!(workspace.focus().pane, pane);
    }

    #[test]
    fn closing_last_pane_keeps_an_empty_pane() {
        let (mut workspace, _project, tab) = workspace_with_project("last-pane");
        let pane = workspace.pane_of_tab(tab.tab).expect("panel");
        let closed = workspace.close_pane(pane).expect("cierra");
        assert_eq!(closed, vec![tab.tab]);
        assert_eq!(workspace.layout().leaves(), vec![pane]);
        assert_eq!(workspace.tab_count(), 0);
        assert_eq!(workspace.focus().tab, None);
    }

    #[test]
    fn moving_a_tab_keeps_project_and_terminal() {
        let (mut workspace, project, first) = workspace_with_project("move-tab");
        let original = workspace.tab(first.tab).expect("tab").clone();
        let pane = workspace.pane_of_tab(first.tab).expect("panel");
        let second_pane = workspace
            .split_pane(pane, SplitAxis::Vertical)
            .expect("split");
        let second = workspace
            .new_tab(second_pane, project, &profile(), PROFILE_TITLE)
            .expect("nueva tab");

        workspace
            .move_tab(first.tab, second_pane, 0)
            .expect("mueve");
        let moved = workspace.tab(first.tab).expect("sigue viva");
        assert_eq!(moved.project_id, original.project_id);
        assert_eq!(moved.terminal_id, original.terminal_id);
        assert_eq!(moved.profile_id, original.profile_id);
        assert_eq!(workspace.focus().tab, Some(first.tab));
        assert_eq!(workspace.focus().pane, second_pane);
        // El panel origen queda vacío y se colapsa.
        assert_eq!(workspace.layout().leaves(), vec![second_pane]);
        assert_eq!(
            workspace.pane(second_pane).expect("panel").tabs,
            vec![first.tab, second.tab]
        );
    }

    #[test]
    fn split_pane_with_tab_creates_the_pane_on_the_requested_side() {
        let (mut workspace, project, first) = workspace_with_project("split-with-tab");
        let pane = workspace.pane_of_tab(first.tab).expect("panel");
        let second = workspace
            .new_tab(pane, project, &profile(), PROFILE_TITLE)
            .expect("segunda");
        let terminal = workspace.tab(second.tab).expect("tab").terminal_id;

        let new_pane = workspace
            .split_pane_with_tab(pane, SplitAxis::Horizontal, Branch::First, second.tab)
            .expect("divide con pestaña");

        assert_eq!(workspace.layout().leaves(), vec![new_pane, pane]);
        assert_eq!(workspace.focus().pane, new_pane);
        assert_eq!(workspace.focus().tab, Some(second.tab));
        assert_eq!(
            workspace.tab(second.tab).expect("tab").terminal_id,
            terminal
        );
        assert_eq!(
            workspace.pane(pane).expect("panel original").tabs,
            vec![first.tab]
        );
    }

    #[test]
    fn split_pane_with_the_only_tab_collapses_the_empty_source() {
        let (mut workspace, _project, first) = workspace_with_project("split-only-tab");
        let pane = workspace.pane_of_tab(first.tab).expect("panel");

        let new_pane = workspace
            .split_pane_with_tab(pane, SplitAxis::Vertical, Branch::Second, first.tab)
            .expect("divide con pestaña");

        assert_eq!(workspace.layout().leaves(), vec![new_pane]);
        assert_eq!(workspace.focus().pane, new_pane);
        assert_eq!(workspace.focus().tab, Some(first.tab));
        assert_eq!(
            workspace.pane(new_pane).expect("panel nuevo").tabs,
            vec![first.tab]
        );
    }

    #[test]
    fn duplicate_preserves_profile_and_project() {
        let (mut workspace, project, first) = workspace_with_project("duplicate-profile");
        let original = workspace.tab(first.tab).expect("tab").clone();
        let copy = workspace.duplicate_tab(first.tab).expect("duplica");
        let duplicate = workspace.tab(copy.tab).expect("copia");
        assert_eq!(duplicate.project_id, project);
        assert_eq!(duplicate.profile_id, original.profile_id);
        assert_eq!(duplicate.auto_title, original.auto_title);
        assert_ne!(duplicate.terminal_id, original.terminal_id);
    }

    #[test]
    fn closing_active_tab_focuses_right_neighbor_then_left() {
        let (mut workspace, project, first) = workspace_with_project("neighbors");
        let pane = workspace.pane_of_tab(first.tab).expect("panel");
        let second = workspace
            .new_tab(pane, project, &profile(), PROFILE_TITLE)
            .expect("segunda");
        let third = workspace
            .new_tab(pane, project, &profile(), PROFILE_TITLE)
            .expect("tercera");
        workspace.focus_tab(second.tab).expect("foco");

        workspace.close_tab(second.tab).expect("cierra");
        assert_eq!(workspace.focus().tab, Some(third.tab));

        workspace.close_tab(third.tab).expect("cierra");
        assert_eq!(workspace.focus().tab, Some(first.tab));
    }

    #[test]
    fn removing_project_closes_all_its_tabs_and_keeps_others() {
        let (mut workspace, project_a, _) = workspace_with_project("remove-a");
        let root_b = temp_project("remove-b");
        let opened_b = workspace
            .open_project(&root_b, &profile(), PROFILE_TITLE)
            .expect("proyecto b");
        let project_b = opened_b.project;
        let tab_b = opened_b.tab.expect("tab b").tab;

        let closed = workspace.remove_project(project_a).expect("quita a");
        assert!(!closed.is_empty());
        assert_eq!(workspace.tabs_for_test(project_a), 0);
        assert_eq!(workspace.current_project(), Some(project_b));
        assert_eq!(workspace.focus().tab, Some(tab_b));
        assert!(workspace.project(project_a).is_none());
    }

    #[test]
    fn current_project_is_derived_from_focus_after_move() {
        let (mut workspace, project_a, tab_a) = workspace_with_project("focus-a");
        let root_b = temp_project("focus-b");
        let opened_b = workspace
            .open_project(&root_b, &profile(), PROFILE_TITLE)
            .expect("proyecto b");
        let project_b = opened_b.project;
        let tab_b = opened_b.tab.expect("tab b").tab;

        assert_eq!(workspace.current_project(), Some(project_b));
        workspace.focus_tab(tab_a.tab).expect("enfoca a");
        assert_eq!(workspace.current_project(), Some(project_a));

        let pane = workspace.pane_of_tab(tab_b).expect("panel b");
        workspace
            .move_tab(tab_b, pane, usize::MAX)
            .expect("reordena");
        assert_eq!(workspace.current_project(), Some(project_b));
    }

    #[test]
    fn rename_blocks_osc_and_reset_restores_the_auto_title() {
        let (mut workspace, _project, tab) = workspace_with_project("rename");
        assert!(workspace.set_tab_title(tab.tab, "desde OSC"));
        assert_eq!(workspace.tab(tab.tab).expect("tab").title, "desde OSC");

        workspace.rename_tab(tab.tab, "Mi shell").expect("renombra");
        assert!(!workspace.set_tab_title(tab.tab, "otra vez OSC"));
        assert_eq!(workspace.tab(tab.tab).expect("tab").title, "Mi shell");

        workspace.reset_tab_title(tab.tab).expect("reset");
        let title = &workspace.tab(tab.tab).expect("tab").title;
        assert_eq!(*title, PROFILE_TITLE);
    }

    #[test]
    fn resize_split_clamps_and_repairs_focus() {
        let (mut workspace, _project, tab) = workspace_with_project("ratio");
        let pane = workspace.pane_of_tab(tab.tab).expect("panel");
        workspace
            .split_pane(pane, SplitAxis::Horizontal)
            .expect("split");
        assert!(workspace.resize_split(&[], 0.95));
        assert!(matches!(
            workspace.layout().ratio_at(&[]),
            Some(ratio) if (ratio - 0.9).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn repair_drops_dangling_tabs_and_orphan_panes() {
        let (mut workspace, project, tab) = workspace_with_project("repair");
        let pane = workspace.pane_of_tab(tab.tab).expect("panel");
        // Estado corrupto: pestaña en un panel inexistente y panel huérfano.
        let orphan = PaneId::new(999);
        workspace.panes.insert(
            orphan,
            Pane {
                tabs: vec![tab.tab],
                active: Some(tab.tab),
            },
        );
        workspace.panes.get_mut(&pane).expect("panel").tabs.clear();
        workspace.focus.tab = Some(TabId::new(9999));
        workspace.repair();

        assert!(workspace.tab(tab.tab).is_none());
        assert!(workspace.pane(orphan).is_none());
        assert!(workspace.project(project).is_some());
        assert_eq!(workspace.focus().tab, None);
    }

    #[test]
    fn repair_reassigns_duplicate_roots() {
        let (mut workspace, project, tab) = workspace_with_project("dup-root");
        let original = workspace.project(project).expect("proyecto").clone();
        let duplicate_id = ProjectId::new(999);
        workspace.projects.push(Project::new(
            duplicate_id,
            original.root.clone(),
            "duplicado".into(),
        ));
        // Una pestaña apuntando al duplicado debe conservarse y reasignarse.
        let pane = workspace.pane_of_tab(tab.tab).expect("panel");
        let mut copy = workspace.tab(tab.tab).expect("tab").clone();
        copy.project_id = duplicate_id;
        copy.terminal_id = TerminalId::new(1000);
        let copy_id = TabId::new(1000);
        workspace.tabs.insert(copy_id, copy);
        workspace
            .panes
            .get_mut(&pane)
            .expect("panel")
            .tabs
            .push(copy_id);
        workspace.repair();

        assert!(workspace.project(duplicate_id).is_none());
        assert_eq!(workspace.tab(copy_id).expect("copia").project_id, project);
    }

    #[cfg(windows)]
    #[test]
    fn repair_normalizes_verbatim_roots() {
        let (mut workspace, project, _tab) = workspace_with_project("repair-verbatim");
        let friendly = workspace.project(project).expect("proyecto").root.clone();
        let verbatim = PathBuf::from(format!(r"\\?\{}", friendly.display()));
        workspace.projects[0].root = verbatim;

        // Un duplicado con la forma amigable debe fusionarse con el verbatim.
        let duplicate_id = ProjectId::new(999);
        workspace.projects.push(Project::new(
            duplicate_id,
            friendly.clone(),
            "duplicado".into(),
        ));
        workspace.repair();

        assert_eq!(workspace.project(project).expect("proyecto").root, friendly);
        assert!(workspace.project(duplicate_id).is_none());
    }

    #[test]
    fn tab_agents_are_runtime_only_and_acknowledged_on_focus() {
        use axf_agents::{AgentKind, AgentReport, AgentSession, AgentState};
        use std::time::{Duration, SystemTime};

        let (mut workspace, _project, tab) = workspace_with_project("agents-runtime");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let mut live = AgentSession::new(1, AgentKind::OpenCode, 100, now);
        live.apply_attention(None, Some("permiso".into()), now);
        live.alive = true;
        let mut gone = AgentSession::new(2, AgentKind::Copilot, 101, now);
        gone.apply_report(&AgentReport::new(AgentState::Working), now);
        gone.process_gone(None, now);
        gone.acknowledge(now);
        assert!(workspace.set_tab_agents(tab.tab, vec![live, gone]));
        assert_eq!(workspace.tab_agents(tab.tab).map(<[_]>::len), Some(2));

        // El estado runtime no viaja en `session.json`.
        let value = serde_json::to_value(&workspace).expect("serializa");
        let tabs = value["tabs"].as_object().expect("tabs");
        for tab in tabs.values() {
            assert!(tab.get("agents").is_none(), "los agentes no se serializan");
        }

        assert!(workspace.acknowledge_tab_agents(tab.tab, now));
        let agents = workspace.tab_agents(tab.tab).expect("agentes");
        assert_eq!(agents.len(), 1, "la sesión sin proceso se retira");
        assert_eq!(agents[0].state, AgentState::Idle);
        assert!(workspace.clear_agents());
        assert!(workspace.tab_agents(tab.tab).is_some_and(<[_]>::is_empty));
    }

    #[test]
    fn duplicate_tab_drops_runtime_agents() {
        use axf_agents::{AgentKind, AgentSession};
        use std::time::SystemTime;

        let (mut workspace, _project, tab) = workspace_with_project("agents-duplicate");
        let session = AgentSession::new(1, AgentKind::OpenCode, 100, SystemTime::now());
        assert!(workspace.set_tab_agents(tab.tab, vec![session]));
        assert!(workspace.set_tab_resume_agent(tab.tab, Some("opencode".into())));
        let copy = workspace.duplicate_tab(tab.tab).expect("duplica");
        assert!(workspace.tab_agents(copy.tab).expect("copia").is_empty());
        assert_eq!(workspace.take_tab_resume_agent(copy.tab), None);
        assert_eq!(
            workspace.take_tab_resume_agent(tab.tab),
            Some("opencode".into())
        );
        assert_eq!(workspace.take_tab_resume_agent(tab.tab), None);
    }

    impl Workspace {
        fn tabs_for_test(&self, project: ProjectId) -> usize {
            self.tabs
                .values()
                .filter(|tab| tab.project_id == project)
                .count()
        }
    }

    #[test]
    fn branch_path_is_stable() {
        let (tree, [_a, b, _c]) = {
            let a = PaneId::new(1);
            let b = PaneId::new(2);
            let c = PaneId::new(3);
            let mut tree = PaneTree::leaf(a);
            tree.split(a, b, SplitAxis::Horizontal, 0.5);
            tree.split(b, c, SplitAxis::Vertical, 0.5);
            (tree, [a, b, c])
        };
        let path = tree.path_to(b).expect("camino");
        assert_eq!(path.first(), Some(&Branch::Second));
        assert_eq!(path.get(1), Some(&Branch::First));
    }
}
