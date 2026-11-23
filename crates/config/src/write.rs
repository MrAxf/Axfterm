//! Escritura de `config.toml` desde el panel de ajustes (ADR 0007).
//!
//! El documento se edita sobre el texto actual con `toml_edit`: solo se tocan
//! las secciones cuyo valor difiere del borrador base, se conservan comentarios
//! y orden, y el resultado se vuelve a parsear antes de escribirse de forma
//! atómica. Si algo falla no se aplica nada.

include!("write_parts/01_read_config_text.rs");
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const BASE: &str = r#"# cabecera del usuario
schema_version = 2

appearance = "system"

[ui]
# tema preferido
theme = "One Dark"
font_size = 14.0
scale = 1.0

[terminal]
font_size = 13.0
line_height = 1.0
default_profile = "a"

[fonts]
sources = []

[agents]
enabled = true
scan_interval_ms = 1500
notify_finished = true
notify_error = true
notify_attention = true

[keymap]
# mis atajos
"ctrl-alt-q" = "tab.new"

[[profiles]]
# perfil base
id = "a"
name = "A"
command = "cmd /Q"
"#;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "axfterm-write-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("crea dir temporal");
        dir
    }

    fn base_config() -> Config {
        parse_str(BASE, "base.toml").expect("config base válida")
    }

    fn prepare(name: &str) -> (PathBuf, Config) {
        let dir = temp_dir(name);
        let path = dir.join("config.toml");
        std::fs::write(&path, BASE).expect("escribe base");
        (dir, base_config())
    }

    #[test]
    fn round_trip_preserves_comments_order_and_untouched_sections() {
        let (dir, base) = prepare("roundtrip");
        let path = dir.join("config.toml");
        let mut draft = base.clone();
        draft.ui.font_size = 16.0;
        let text = write_config(&path, &base, &draft).expect("escribe");
        assert!(text.contains("# cabecera del usuario"));
        assert!(text.contains("# tema preferido"));
        assert!(text.contains("# mis atajos"));
        assert!(text.contains("# perfil base"));
        assert!(text.contains("font_size = 16.0"));
        assert!(
            text.find("[ui]").expect("sección ui")
                < text.find("[terminal]").expect("sección terminal"),
            "el orden de secciones se conserva"
        );
        let on_disk = std::fs::read_to_string(&path).expect("lee");
        assert_eq!(on_disk, text);
        let reparsed = parse_str(&on_disk, "releído").expect("sigue siendo válido");
        assert_eq!(reparsed.ui.font_size, 16.0);
        assert_eq!(reparsed.profiles, base.profiles);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unchanged_sections_are_byte_identical() {
        let (dir, base) = prepare("untouched");
        let path = dir.join("config.toml");
        let mut draft = base.clone();
        draft.appearance = crate::schema::Appearance::Dark;
        write_config(&path, &base, &draft).expect("escribe");
        let text = std::fs::read_to_string(&path).expect("lee");
        assert!(text.contains("[terminal]\nfont_size = 13.0\nline_height = 1.0"));
        assert!(text.contains("# mis atajos"));
        assert!(text.contains("# perfil base"));
        assert!(text.contains("appearance = \"dark\""));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn profile_edits_keep_the_profile_comment_and_order() {
        let (dir, base) = prepare("profiles");
        let path = dir.join("config.toml");
        let mut draft = base.clone();
        draft.profiles[0].name = "Mi shell".into();
        draft.profiles.push(Profile::new("b", "B", "pwsh -NoLogo"));
        write_config(&path, &base, &draft).expect("escribe");
        let text = std::fs::read_to_string(&path).expect("lee");
        assert!(text.contains("# perfil base"));
        assert!(text.contains("name = \"Mi shell\""));
        assert!(
            text.find("id = \"a\"").expect("perfil a") < text.find("id = \"b\"").expect("perfil b")
        );
        let reparsed = parse_str(&text, "releído").expect("válido");
        assert_eq!(reparsed.profiles, draft.profiles);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn keymap_diff_adds_and_removes_entries_without_touching_comments() {
        let (dir, base) = prepare("keymap");
        let path = dir.join("config.toml");
        let mut draft = base.clone();
        draft.keymap.remove("ctrl-alt-q");
        draft
            .keymap
            .insert("ctrl-alt-w".into(), toml::Value::String("tab.new".into()));
        write_config(&path, &base, &draft).expect("escribe");
        let text = std::fs::read_to_string(&path).expect("lee");
        assert!(text.contains("# mis atajos"));
        assert!(!text.contains("ctrl-alt-q"));
        assert!(text.contains("ctrl-alt-w ="));
        let reparsed = parse_str(&text, "releído").expect("válido");
        assert_eq!(flatten(&reparsed.keymap), flatten(&draft.keymap));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn agents_and_font_sources_are_written_only_when_they_change() {
        let (dir, base) = prepare("agents");
        let path = dir.join("config.toml");
        let mut draft = base.clone();
        draft.agents.scan_interval_ms = 1200;
        draft
            .agents
            .preferred_profiles
            .insert("opencode".into(), base.terminal.default_profile.clone());
        draft.fonts.sources = vec![PathBuf::from("fonts")];
        write_config(&path, &base, &draft).expect("escribe");
        let text = std::fs::read_to_string(&path).expect("lee");
        assert!(text.contains("scan_interval_ms = 1200"));
        assert!(text.contains("[agents.preferred_profiles]"));
        assert!(text.contains("opencode = \"a\""));
        assert!(text.contains("sources = [\"fonts\"]"));
        let reparsed = parse_str(&text, "releído").expect("válido");
        assert_eq!(reparsed.agents, draft.agents);
        assert_eq!(reparsed.fonts, draft.fonts);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_draft_is_rejected_and_leaves_the_file_untouched() {
        let (dir, base) = prepare("invalid");
        let path = dir.join("config.toml");
        let original = BASE.to_owned();
        let mut draft = base.clone();
        draft.ui.font_size = 99.0;
        let error = write_config(&path, &base, &draft).expect_err("rango inválido");
        assert!(matches!(error, ConfigError::Field { ref field, .. } if field == "ui.font_size"));
        assert_eq!(std::fs::read_to_string(&path).expect("lee"), original);
        let _ = std::fs::remove_dir_all(dir);
    }
}
