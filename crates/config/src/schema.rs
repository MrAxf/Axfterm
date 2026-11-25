//! Esquema de `config.toml` (ADR 0004).
//!
//! El archivo es la interfaz de configuración de v0.1: se edita a mano y se
//! recarga en caliente. Los campos desconocidos son un error para que un typo
//! no se ignore en silencio. Los rangos se validan sin clamp silencioso.

include!("schema_parts/01_config_schema_version.rs");
include!("schema_parts/02_config_validate.rs");
include!("schema_parts/03_load_or_create.rs");
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "axfterm-config-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("crea dir temporal");
        dir
    }

    #[test]
    fn template_parses_and_is_valid() {
        let discovery = profiles::discover();
        let template = default_template(&discovery);
        let config = parse_str(&template, "plantilla").expect("plantilla válida");
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.ui.theme, "One Dark");
        assert_eq!(config.terminal.font_size, 13.0);
        assert_eq!(config.terminal.scrollback_lines, 10_000);
        assert!(!config.profiles.is_empty());
        assert!(
            config
                .profiles
                .iter()
                .any(|profile| profile.id == config.terminal.default_profile)
        );
    }

    #[test]
    fn keymap_section_is_kept_without_being_interpreted_by_the_schema() {
        let source = r#"schema_version = 2
[terminal]
default_profile = "a"
[keymap]
"ctrl-alt-q" = "tab.new"
[keymap.terminal]
"ctrl-shift-c" = "terminal.copy"
[[profiles]]
id = "a"
name = "A"
command = "cmd /Q"
"#;
        let config = parse_str(source, "keymap.toml").expect("config válida");
        let keymap = config.keymap;
        assert_eq!(
            keymap.get("ctrl-alt-q").and_then(toml::Value::as_str),
            Some("tab.new")
        );
        assert!(
            keymap
                .get("terminal")
                .and_then(toml::Value::as_table)
                .and_then(|terminal| terminal.get("ctrl-shift-c"))
                .and_then(toml::Value::as_str)
                == Some("terminal.copy")
        );
    }

    #[test]
    fn profiles_are_validated_by_field() {
        let mut empty = Config {
            profiles: Vec::new(),
            ..Config::default()
        };
        empty.terminal.default_profile = ProfileId::new("a");
        let error = empty.validate().expect_err("sin perfiles");
        assert!(matches!(error, ConfigError::Field { field, .. } if field == "profiles"));

        let duplicate = r#"schema_version = 2

[terminal]
default_profile = "a"

[[profiles]]
id = "a"
name = "A"
command = "cmd"

[[profiles]]
id = "a"
name = "Otra"
command = "cmd"
"#;
        let error = parse_str(duplicate, "config.toml").expect_err("id duplicado");
        assert!(matches!(error, ConfigError::Field { field, .. } if field.ends_with(".id")));

        let missing_default = r#"schema_version = 2

[terminal]
default_profile = "b"

[[profiles]]
id = "a"
name = "A"
command = "cmd"
"#;
        let error = parse_str(missing_default, "config.toml").expect_err("default inexistente");
        assert!(
            matches!(error, ConfigError::Field { field, .. } if field == "terminal.default_profile")
        );

        let bad_command = r#"schema_version = 2

[terminal]
default_profile = "a"

[[profiles]]
id = "a"
name = "A"
command = "cmd \"sin cerrar"
"#;
        let error = parse_str(bad_command, "config.toml").expect_err("comando inválido");
        assert!(matches!(error, ConfigError::Field { field, .. } if field.ends_with(".command")));
    }

    #[test]
    fn v1_config_is_migrated_and_rewritten_without_losing_comments() {
        let dir = temp_dir("migrate-v1");
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "# comentario del usuario\nschema_version = 1\n\n[ui]\n# mi tema\ntheme = \"Monokai\"\n",
        )
        .expect("escribe");
        let loaded = load_or_create(&path);
        assert_eq!(loaded.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert!(loaded.profiles_detected);
        assert!(!loaded.config.profiles.is_empty());
        let written = std::fs::read_to_string(&path).expect("lee");
        assert!(written.contains("# comentario del usuario"));
        assert!(written.contains("# mi tema"));
        assert!(written.contains("schema_version = 2"));
        assert!(written.contains("[[profiles]]"));
        assert!(written.contains("default_profile"));
        let reloaded = load_or_create(&path);
        assert!(!reloaded.profiles_detected, "no se redetecta dos veces");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_default_uses_the_first_existing_profile() {
        let source = r#"schema_version = 2

[terminal]
font_size = 13.0

[[profiles]]
id = "mio"
name = "Mi shell"
command = "cmd /Q"
"#;
        let config = parse_str(source, "config.toml").expect("migra el default");
        assert_eq!(config.terminal.default_profile, ProfileId::new("mio"));
    }

    #[test]
    fn v2_without_profiles_gets_discovery_once() {
        let dir = temp_dir("add-profiles");
        let path = dir.join("config.toml");
        std::fs::write(&path, "schema_version = 2\n[terminal]\nfont_size = 13.0\n")
            .expect("escribe");
        let loaded = load_or_create(&path);
        assert!(loaded.profiles_detected);
        assert!(!loaded.config.profiles.is_empty());
        let written = std::fs::read_to_string(&path).expect("lee");
        assert!(written.contains("[[profiles]]"));
        assert!(written.contains("default_profile"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn append_profiles_keeps_existing_content_and_deduplicates() {
        let dir = temp_dir("append-profiles");
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "# cabecera\nschema_version = 2\n\n[terminal]\ndefault_profile = \"a\"\n\n# perfil base\n[[profiles]]\nid = \"a\"\nname = \"A\"\ncommand = \"cmd /Q\"\n",
        )
        .expect("escribe");
        let added = append_profiles(
            &path,
            &[
                Profile::new("a", "Repetido por id", "otra"),
                Profile::new("b", "Otra vez el mismo comando", "cmd /q"),
                Profile::new("wsl-ubuntu", "Ubuntu (WSL)", "wsl -d \"Ubuntu\""),
            ],
        )
        .expect("añade");
        assert_eq!(added, vec![ProfileId::new("wsl-ubuntu")]);
        let written = std::fs::read_to_string(&path).expect("lee");
        assert!(written.contains("# cabecera"));
        assert!(written.contains("# perfil base"));
        assert!(written.contains("id = \"wsl-ubuntu\""));
        assert!(!written.contains("Otra vez el mismo comando"));
        let config = parse_str(&written, "config.toml").expect("sigue siendo válido");
        assert_eq!(config.profiles.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_fields_are_rejected_with_location() {
        let source = "schema_version = 1\n[ui]\nfont_sise = 14.0\n";
        let error = parse_str(source, "config.toml").expect_err("typo debe fallar");
        match error {
            ConfigError::Parse { line, column, .. } => {
                assert_eq!(line, 3);
                assert!(column > 0);
            }
            other => panic!("error inesperado: {other}"),
        }
    }

    #[test]
    fn ranges_are_reported_by_field() {
        let source = "schema_version = 1\n[ui]\nfont_size = 99.0\n";
        let error = parse_str(source, "config.toml").expect_err("fuera de rango");
        match error {
            ConfigError::Field { field, .. } => assert_eq!(field, "ui.font_size"),
            other => panic!("error inesperado: {other}"),
        }

        let source = "schema_version = 2\n[terminal]\nscrollback_lines = 60000\n";
        let error = parse_str(source, "config.toml").expect_err("scrollback fuera de rango");
        match error {
            ConfigError::Field { field, .. } => assert_eq!(field, "terminal.scrollback_lines"),
            other => panic!("error inesperado: {other}"),
        }
    }

    #[test]
    fn agents_interval_is_validated_with_the_field_name() {
        let config = Config {
            agents: AgentsConfig {
                scan_interval_ms: 500,
                ..AgentsConfig::default()
            },
            ..Config::default()
        };
        let error = config.validate().expect_err("intervalo fuera de rango");
        match error {
            ConfigError::Field { field, .. } => assert_eq!(field, "agents.scan_interval_ms"),
            other => panic!("error inesperado: {other}"),
        }
        assert!(Config::default().validate().is_ok());
        assert_eq!(Config::default().agents.scan_interval_ms, 1500);
    }

    #[test]
    fn future_schema_is_not_rewritten() {
        let dir = temp_dir("future");
        let path = dir.join("config.toml");
        std::fs::write(&path, "schema_version = 9\n").expect("escribe");
        let loaded = load_or_create(&path);
        assert!(loaded.warning.is_some());
        assert_eq!(
            std::fs::read_to_string(&path).expect("lee"),
            "schema_version = 9\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_file_is_backed_up() {
        let dir = temp_dir("corrupt");
        let path = dir.join("config.toml");
        std::fs::write(&path, "esto no es toml = =").expect("escribe");
        let loaded = load_or_create(&path);
        assert!(loaded.warning.is_some());
        assert!(!path.exists());
        let backups: Vec<_> = std::fs::read_dir(&dir)
            .expect("lee")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".bak-"))
            .collect();
        assert_eq!(backups.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn atomic_write_replaces_content() {
        let dir = temp_dir("atomic");
        let path = dir.join("config.toml");
        write_atomic(&path, b"a = 1\n").expect("primera escritura");
        write_atomic(&path, b"a = 2\n").expect("segunda escritura");
        assert_eq!(std::fs::read_to_string(&path).expect("lee"), "a = 2\n");
        let _ = std::fs::remove_dir_all(dir);
    }
}
