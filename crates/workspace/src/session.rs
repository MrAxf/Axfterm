//! Persistencia de la sesión (`session.json`).
//!
//! Formato versionado, escritura atómica y recuperación sin pérdida del archivo
//! original (ADR 0004). La sesión es de la app: no se recarga en caliente.

include!("session_parts/01_session_version.rs");
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::SplitAxis;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("axfterm-session-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("carpeta temporal");
        dir
    }

    fn profile() -> ProfileId {
        ProfileId::new("shell")
    }

    fn sample_workspace(name: &str) -> (Workspace, PathBuf) {
        let root = temp_dir(name);
        let mut workspace = Workspace::new();
        let opened = workspace
            .open_project(&root, &profile(), "Shell")
            .expect("proyecto");
        let tab = opened.tab.expect("tab").tab;
        let pane = workspace.pane_of_tab(tab).expect("panel");
        workspace
            .split_pane(pane, SplitAxis::Vertical)
            .expect("split");
        (workspace, root)
    }

    #[test]
    fn round_trip_preserves_empty_projects_and_layout() {
        let (mut workspace, root) = sample_workspace("roundtrip");
        let empty_root = temp_dir("roundtrip-empty");
        let (empty, _) = workspace.register_project(&empty_root).expect("vacío");
        let tab_before = workspace.tab_count();
        let leaves_before = workspace.layout().leaves();
        let session = SessionFile::current(workspace);
        let path = root.join("session.json");

        save_session(&path, &session).expect("guarda");
        let loaded = load_session(&path, &profile())
            .expect("carga")
            .expect("existe");
        assert_eq!(loaded.version, SESSION_VERSION);
        assert_eq!(loaded.workspace.tab_count(), tab_before);
        assert_eq!(loaded.workspace.layout().leaves(), leaves_before);
        assert!(loaded.workspace.project(empty).is_some());
    }

    #[test]
    fn atomic_save_leaves_no_temporary_file() {
        let (workspace, root) = sample_workspace("atomic");
        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .expect("lee dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "temporales: {leftovers:?}");
    }

    #[test]
    fn v1_session_migrates_to_v2_with_the_default_profile() {
        let (workspace, root) = sample_workspace("migrate-v1");
        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("lee")).expect("json");
        value["version"] = serde_json::json!(1);
        if let Some(tabs) = value["workspace"]["tabs"].as_object_mut() {
            for tab in tabs.values_mut() {
                if let Some(object) = tab.as_object_mut() {
                    object.remove("profile_id");
                    object.remove("auto_title");
                }
            }
        }
        std::fs::write(&path, serde_json::to_vec_pretty(&value).expect("json")).expect("escribe");

        let default = ProfileId::new("pwsh");
        let loaded = load_session(&path, &default)
            .expect("carga")
            .expect("existe");
        assert_eq!(loaded.version, SESSION_VERSION);
        let tabs: Vec<_> = loaded
            .workspace
            .project_tabs(loaded.workspace.projects()[0].id);
        assert!(!tabs.is_empty());
        for tab in tabs {
            let tab = loaded.workspace.tab(tab).expect("tab");
            assert_eq!(tab.profile_id, default);
            assert!(!tab.auto_title.is_empty());
        }
    }

    #[test]
    fn future_version_is_rejected_without_touching_the_file() {
        let (workspace, root) = sample_workspace("future");
        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("lee")).expect("json");
        value["version"] = serde_json::json!(SESSION_VERSION + 1);
        std::fs::write(&path, serde_json::to_vec_pretty(&value).expect("json")).expect("escribe");

        let load = load_or_recover(&path, &profile());
        assert!(load.session.is_none());
        assert!(load.warning.is_some());
        assert!(path.exists(), "una versión futura no se mueve");
    }

    #[test]
    fn corrupt_session_is_backed_up() {
        let root = temp_dir("corrupt");
        let path = root.join("session.json");
        std::fs::write(&path, b"{ esto no es json").expect("escribe");

        let load = load_or_recover(&path, &profile());
        assert!(load.session.is_none());
        assert!(load.warning.is_some());
        assert!(!path.exists());
        let backups: Vec<_> = std::fs::read_dir(&root)
            .expect("lee dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".bak-"))
            .collect();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn oversized_session_is_rejected_before_parsing() {
        let root = temp_dir("oversized");
        let path = root.join("session.json");
        let file = std::fs::File::create(&path).expect("crea");
        file.set_len(MAX_SESSION_BYTES + 1).expect("trunca");
        let error = load_session(&path, &profile()).expect_err("debe rechazar");
        assert!(matches!(error, SessionError::TooLarge { .. }));
    }

    #[test]
    fn missing_version_is_an_error() {
        let root = temp_dir("missing-version");
        let path = root.join("session.json");
        std::fs::write(&path, br#"{"workspace":{}}"#).expect("escribe");
        let error = load_session(&path, &profile()).expect_err("debe rechazar");
        assert!(matches!(error, SessionError::MissingVersion));
    }

    #[test]
    fn timestamp_has_the_expected_shape() {
        let stamp = timestamp_suffix(UNIX_EPOCH);
        assert_eq!(stamp, "19700101-000000");
        let stamp = timestamp_suffix(UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000));
        assert_eq!(stamp, "20231114-221320");
    }

    #[test]
    fn stress_four_panes_twenty_tabs_three_projects_round_trip() {
        let mut workspace = Workspace::new();
        let mut projects = Vec::new();
        for index in 0..3 {
            let root = temp_dir(&format!("stress-{index}"));
            let opened = workspace
                .open_project(&root, &profile(), "Shell")
                .expect("proyecto");
            projects.push(opened.project);
        }
        // Un proyecto queda vacío en la sidebar (el tercero no tiene pestaña
        // propia todavía): se registra y debe sobrevivir a la sesión.
        let empty_root = temp_dir("stress-empty");
        let (empty, _) = workspace.register_project(&empty_root).expect("vacío");

        let mut pane = workspace.focus().pane;
        for axis in [
            SplitAxis::Horizontal,
            SplitAxis::Vertical,
            SplitAxis::Horizontal,
        ] {
            pane = workspace.split_pane(pane, axis).expect("split");
        }
        let leaves = workspace.layout().leaves();
        assert_eq!(leaves.len(), 4);

        while workspace.tab_count() < 20 {
            let project = projects[workspace.tab_count() % projects.len()];
            let pane = leaves[workspace.tab_count() % leaves.len()];
            workspace
                .new_tab(pane, project, &profile(), "Shell")
                .expect("pestaña");
        }
        assert_eq!(workspace.tab_count(), 20);

        // Mezcla de proyectos en un panel y movimiento entre paneles.
        let first = workspace.project_tabs(projects[0])[0];
        let target = leaves[3];
        workspace.move_tab(first, target, 0).expect("mueve");
        assert_eq!(workspace.focus().tab, Some(first));
        assert_eq!(workspace.focus().pane, target);

        let root = temp_dir("stress-session");
        let path = root.join("session.json");
        let before_leaves = workspace.layout().leaves();
        let before_tabs = workspace.tab_count();
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let loaded = load_session(&path, &profile())
            .expect("carga")
            .expect("existe")
            .workspace;
        assert_eq!(loaded.tab_count(), before_tabs);
        assert_eq!(loaded.layout().leaves(), before_leaves);
        assert_eq!(loaded.projects().len(), 4);
        assert!(loaded.project(empty).is_some());
    }

    #[test]
    fn round_trip_preserves_focus_on_a_later_pane() {
        let mut workspace = Workspace::new();
        let root = temp_dir("focus-later");
        let project = workspace
            .open_project(&root, &profile(), "Shell")
            .expect("proyecto")
            .project;
        let first = workspace.focus().pane;
        let second = workspace
            .split_pane(first, SplitAxis::Horizontal)
            .expect("split");
        let third = workspace
            .split_pane(second, SplitAxis::Vertical)
            .expect("split");
        workspace
            .new_tab(third, project, &profile(), "Shell")
            .expect("tab tercero");
        workspace.focus_pane(third).expect("foco tercero");
        let tab = workspace.focus().tab.expect("tab");

        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let loaded = load_session(&path, &profile())
            .expect("carga")
            .expect("existe")
            .workspace;
        assert_eq!(loaded.focus().pane, third);
        assert_eq!(loaded.focus().tab, Some(tab));
    }

    #[test]
    fn loaded_session_is_repaired() {
        let (workspace, root) = sample_workspace("repair-load");
        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("lee")).expect("json");
        // Arranca un ratio imposible y un foco a un panel inexistente.
        value["workspace"]["layout"]["value"]["ratio"] = serde_json::json!(99.0);
        value["workspace"]["focus"]["pane"] = serde_json::json!(9999);
        std::fs::write(&path, serde_json::to_vec_pretty(&value).expect("json")).expect("escribe");

        let loaded = load_session(&path, &profile())
            .expect("carga")
            .expect("existe");
        assert_eq!(
            loaded.workspace.layout().ratio_at(&[]),
            Some(1.0 - crate::layout::MIN_RATIO)
        );
        let focus_pane = loaded.workspace.focus().pane;
        assert!(loaded.workspace.layout().leaves().contains(&focus_pane));
    }
}
