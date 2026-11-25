fn snap(value: f32, increment: f32, min: f32, max: f32) -> f32 {
    let clamped = value.clamp(min, max);
    let snapped = (clamped / increment).round() * increment;
    // Redondeo decimal para que 1.0500001 sea 1.05 y la validación y la UI
    // muestren el mismo número.
    (snapped * 1000.0).round() / 1000.0
}
fn step(value: f32, delta: f32, increment: f32, min: f32, max: f32) -> f32 {
    snap(value + delta * increment, increment, min, max)
}
fn normalized_command(command: &str) -> String {
    command.trim().to_ascii_lowercase()
}
fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() || character == '_' {
            slug.push(character);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_owned();
    let mut slug = if slug.is_empty() {
        "perfil".to_owned()
    } else {
        slug
    };
    if !slug
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
    {
        slug.insert(0, 'p');
    }
    slug.chars().take(48).collect()
}
fn normalize_chord(chord: &str) -> String {
    chord
        .trim()
        .to_ascii_lowercase()
        .replace('+', "-")
        .replace(' ', "")
}
fn remove_action_entries(table: &mut toml::Table, action_id: &str) {
    let flat: Vec<String> = table
        .iter()
        .filter(|(_, value)| value.as_str() == Some(action_id))
        .map(|(key, _)| key.clone())
        .collect();
    for key in flat {
        table.remove(&key);
    }
    let sections: Vec<String> = table
        .iter()
        .filter(|(_, value)| value.is_table())
        .map(|(key, _)| key.clone())
        .collect();
    for key in sections {
        if let Some(toml::Value::Table(section)) = table.get_mut(&key) {
            section.retain(|_, value| value.as_str() != Some(action_id));
        }
    }
    table.retain(|_, value| {
        value
            .as_table()
            .map(|section| !section.is_empty())
            .unwrap_or(true)
    });
}
fn set_unbind(table: &mut toml::Table, action_id: &str, enabled: bool) {
    let mut items: Vec<toml::Value> = table
        .get("unbind")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    items.retain(|value| value.as_str() != Some(action_id));
    if enabled {
        items.push(toml::Value::String(action_id.to_owned()));
    }
    if items.is_empty() {
        table.remove("unbind");
    } else {
        table.insert("unbind".into(), toml::Value::Array(items));
    }
}
#[cfg(test)]
mod tests {
    use axf_config::{AgentsConfig, Profile};
    use std::path::PathBuf;

    use super::*;

    fn base() -> Config {
        Config {
            profiles: vec![
                Profile::new("a", "A", "cmd /Q"),
                Profile::new("b", "B", "pwsh -NoLogo"),
            ],
            terminal: axf_config::TerminalConfig {
                default_profile: ProfileId::new("a"),
                ..axf_config::TerminalConfig::default()
            },
            ..Config::default()
        }
    }

    fn state() -> SettingsState {
        SettingsState::new(base(), None)
    }

    #[test]
    fn sections_cycle_in_order() {
        assert_eq!(
            SettingsSection::Appearance.next(1),
            SettingsSection::Profiles
        );
        assert_eq!(SettingsSection::Appearance.next(-1), SettingsSection::Agents);
        assert_eq!(SettingsSection::ALL.len(), 4);
    }

    #[test]
    fn numeric_values_clamp_and_snap() {
        let mut state = state();
        state.set_ui_font_size(100.0);
        assert_eq!(state.draft.ui.font_size, 24.0);
        state.set_ui_scale(-100.0);
        assert_eq!(state.draft.ui.scale, 0.8);
        state.set_ui_scale(1.234);
        assert_eq!(state.draft.ui.scale, 1.25);
        state.set_terminal_font_size(100.0);
        assert_eq!(state.draft.terminal.font_size, 32.0);
        state.set_line_height(1.0);
        assert_eq!(state.draft.terminal.line_height, 1.0);
        state.step_scan_interval(-99.0);
        assert_eq!(state.draft.agents.scan_interval_ms, 1000);
    }

    #[test]
    fn add_duplicate_and_delete_follow_profile_rules() {
        let mut state = state();
        let profile = state.new_profile();
        let id = profile.id.clone();
        assert_eq!(state.profiles().len(), 2);
        state.insert_profile(profile).expect("inserta");
        assert_eq!(state.profiles().len(), 3);
        assert_eq!(
            state.profiles().last().map(|profile| &profile.id),
            Some(&id)
        );

        let copy = state.duplicate_profile(&id).expect("duplica");
        assert_eq!(state.profiles().len(), 4);
        assert_eq!(
            state.profiles().last().map(|profile| profile.name.as_str()),
            Some("Perfil 3 (copia)")
        );

        let error = state
            .delete_profile(&ProfileId::new("a"))
            .expect_err("el predeterminado está protegido");
        assert!(error.contains("predeterminado"));

        state
            .set_default_profile(&ProfileId::new("b"))
            .expect("mueve el default");
        state.delete_profile(&ProfileId::new("a")).expect("borra");
        assert_eq!(state.profiles().len(), 3);
        assert!(
            state
                .profiles()
                .iter()
                .all(|profile| profile.id != ProfileId::new("a"))
        );
        let _ = copy;
    }

    #[test]
    fn a_duplicate_profile_id_is_rejected() {
        let mut state = state();
        let error = state
            .insert_profile(Profile::new("a", "Otra", "cmd"))
            .expect_err("el id ya existe");
        assert!(error.contains("ya existe"));
    }

    #[test]
    fn deleting_a_profile_removes_its_agent_preference() {
        let mut state = state();
        state
            .draft
            .agents
            .preferred_profiles
            .insert("opencode".into(), ProfileId::new("b"));
        state
            .set_default_profile(&ProfileId::new("a"))
            .expect("default a");
        state.delete_profile(&ProfileId::new("b")).expect("borra b");
        assert!(state.draft.agents.preferred_profiles.is_empty());
    }

    #[test]
    fn last_profile_cannot_be_deleted() {
        let mut state = state();
        state.draft.profiles.truncate(1);
        state.draft.terminal.default_profile = ProfileId::new("a");
        let error = state
            .delete_profile(&ProfileId::new("a"))
            .expect_err("el último perfil no se puede borrar");
        assert!(error.contains("al menos un perfil"));
    }

    #[test]
    fn merge_detected_deduplicates_id_and_command() {
        let mut state = state();
        let added = state.merge_detected(vec![
            Profile::new("a", "Repetido", "otro"),
            Profile::new("c", "Mismo comando", "CMD /q"),
            Profile::new("nu", "Nu", "nu"),
        ]);
        assert_eq!(added, 1);
        assert_eq!(state.profiles().last().map(|p| p.id.as_str()), Some("nu"));
    }

    #[test]
    fn rebind_replaces_chords_and_marks_unbind() {
        let mut state = state();
        state
            .set_binding("tab.new", "ctrl-alt-shift-n")
            .expect("rebind");
        let chords = state.chords_for("tab.new").expect("sigue teniendo atajo");
        assert_eq!(chords, "Ctrl+Alt+Shift+N");
        let unbind = state
            .draft
            .keymap
            .get("unbind")
            .and_then(toml::Value::as_array);
        assert!(
            unbind.is_some_and(|items| items.iter().any(|item| item.as_str() == Some("tab.new")))
        );
        assert_eq!(
            state
                .draft
                .keymap
                .get("ctrl-alt-shift-n")
                .and_then(toml::Value::as_str),
            Some("tab.new")
        );
    }

    #[test]
    fn assigning_a_chord_of_another_action_is_explained() {
        let mut state = state();
        let error = state
            .set_binding("tab.close", "ctrl-shift-t")
            .expect_err("el chord de Nueva terminal está ocupado");
        assert!(error.contains("Nueva terminal") || error.contains("tab.new"));
    }

    #[test]
    fn reserved_terminal_keys_are_rejected() {
        let mut state = state();
        assert!(state.set_binding("tab.new", "ctrl-d").is_err());
    }

    #[test]
    fn unbind_and_reset_remove_the_action_shortcut() {
        let mut state = state();
        state.unbind_action("tab.new").expect("unbind");
        assert!(!state.has_binding("tab.new"));
        assert!(
            state
                .draft
                .keymap
                .get("unbind")
                .and_then(toml::Value::as_array)
                .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("tab.new")))
        );
        state.reset_action("tab.new").expect("reset");
        assert!(state.has_binding("tab.new"));
        assert!(state.draft.keymap.is_empty());
    }

    #[test]
    fn preferred_profile_can_be_set_and_cleared() {
        let mut state = state();
        assert_eq!(state.preferred_profile("opencode"), None);
        state.set_preferred_profile("opencode", Some(ProfileId::new("b")));
        assert_eq!(
            state.preferred_profile("opencode"),
            Some(&ProfileId::new("b"))
        );
        state.set_preferred_profile("opencode", None);
        assert_eq!(state.preferred_profile("opencode"), None);
    }

    #[test]
    fn invalid_agent_profile_blocks_validation() {
        let mut state = state();
        state.draft.agents = AgentsConfig {
            preferred_profiles: [("opencode".into(), ProfileId::new("missing"))]
                .into_iter()
                .collect(),
            ..AgentsConfig::default()
        };
        let errors = state.validate();
        assert!(
            errors
                .iter()
                .any(|error| error.contains("preferred_profiles"))
        );
    }

    #[test]
    fn restore_defaults_keeps_profiles_fonts_and_a_valid_default() {
        let mut state = state();
        state.set_theme("Monokai", true);
        state.draft.ui.font_size = 20.0;
        state.draft.fonts.sources = vec![std::path::PathBuf::from("fonts")];
        state.restore_defaults();
        assert_eq!(state.draft.profiles, base().profiles);
        assert_eq!(state.draft.ui.theme, "One Dark");
        assert_eq!(state.draft.appearance, Appearance::System);
        assert_eq!(state.draft.ui.font_size, 14.0);
        assert_eq!(
            state.draft.fonts.sources,
            vec![std::path::PathBuf::from("fonts")]
        );
        assert!(state.validate().is_empty());
    }

    #[test]
    fn external_change_reloads_a_clean_draft_and_conflicts_with_a_dirty_one() {
        let mut state = state();
        let mut external = base();
        external.ui.font_size = 18.0;
        let source = "schema_version = 2".to_owned();
        assert_eq!(
            state.external_change(source.clone(), Ok(external.clone())),
            ExternalOutcome::Reloaded
        );
        assert_eq!(state.draft.ui.font_size, 18.0);
        assert!(!state.is_dirty());

        state.draft.ui.font_size = 20.0;
        let mut newer = external.clone();
        newer.ui.font_size = 19.0;
        assert_eq!(
            state.external_change("otro texto".into(), Ok(newer)),
            ExternalOutcome::Conflict
        );
        assert!(state.conflict.is_some());
        assert!(state.is_dirty());
        state.allow_overwrite();
        assert!(state.overwrite);
        assert!(state.conflict.is_none());
    }

    #[test]
    fn own_write_is_ignored_by_the_watcher() {
        let mut state = state();
        let written = "contenido escrito".to_owned();
        state.commit_saved(written.clone());
        assert_eq!(
            state.external_change(written, Ok(base())),
            ExternalOutcome::Ignored
        );
    }

    #[test]
    fn saved_notice_is_hidden_after_a_new_edit() {
        let mut state = state();
        state.set_notice("Error anterior".into(), true);
        state.commit_saved("config".into());
        assert_eq!(
            state.visible_notice().map(|(text, _)| text.as_str()),
            Some("Configuración guardada.")
        );
        let larger = state.draft.ui.font_size + 1.0;
        state.set_ui_font_size(larger);
        assert!(state.is_dirty());
        assert!(state.visible_notice().is_none());
    }

    #[test]
    fn invalid_external_file_keeps_the_draft_and_explains() {
        let mut state = state();
        let outcome = state.external_change(
            "roto".into(),
            Err(ConfigError::Field {
                field: "ui.font_size".into(),
                message: "fuera de rango".into(),
            }),
        );
        assert!(matches!(outcome, ExternalOutcome::Invalid(_)));
        assert!(state.notice.as_ref().is_some_and(|(_, error)| *error));
        assert_eq!(state.draft.ui.font_size, 14.0);
    }

    #[test]
    fn settings_manual_gate_changes_survive_config_reload() {
        let mut state = state();
        state.set_theme("Monokai", true);
        state.set_ui_font(Some("Segoe UI".into()));
        state.set_terminal_font(Some("Cascadia Mono".into()));
        state
            .update_profile(
                &ProfileId::new("a"),
                "Shell de prueba".into(),
                "cmd /Q".into(),
                None,
            )
            .expect("actualiza perfil");
        state
            .set_default_profile(&ProfileId::new("b"))
            .expect("cambia perfil predeterminado");
        state
            .set_binding("tab.new", "ctrl-alt-shift-n")
            .expect("cambia atajo");
        state.set_agents_enabled(false);
        state.step_scan_interval(3.0);
        state.set_preferred_profile("opencode", Some(ProfileId::new("b")));
        state.draft.fonts.sources = vec![PathBuf::from("fonts")];

        assert!(state.validate().is_empty());
        let dir = std::env::temp_dir().join(format!(
            "axfterm-settings-roundtrip-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("crea directorio temporal");
        let path = dir.join("config.toml");
        axf_config::write_config(&path, &state.base, &state.draft).expect("guarda borrador");
        let text = std::fs::read_to_string(&path).expect("lee config guardada");
        let reloaded = axf_config::parse_str(&text, "config.toml").expect("recarga config");

        assert_eq!(reloaded.ui.theme, "Monokai");
        assert_eq!(reloaded.ui.font_family.as_deref(), Some("Segoe UI"));
        assert_eq!(
            reloaded.terminal.font_family.as_deref(),
            Some("Cascadia Mono")
        );
        assert_eq!(reloaded.profiles[0].name, "Shell de prueba");
        assert_eq!(reloaded.terminal.default_profile, ProfileId::new("b"));
        assert_eq!(reloaded.fonts.sources, vec![PathBuf::from("fonts")]);
        assert_eq!(reloaded.agents.scan_interval_ms, 1800);
        assert!(!reloaded.agents.enabled);
        assert_eq!(
            reloaded.agents.preferred_profiles.get("opencode"),
            Some(&ProfileId::new("b"))
        );
        assert_eq!(
            crate::keymap::Keymap::resolve(&reloaded.keymap).chords_for("tab.new"),
            Some("Ctrl+Alt+Shift+N".into())
        );
        std::fs::remove_dir_all(dir).expect("limpia directorio temporal");
    }
}
