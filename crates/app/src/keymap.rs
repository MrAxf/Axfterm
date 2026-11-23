//! Atajos configurables: lectura de `[keymap]`, validación e instalación.
//!
//! El registro de acciones (`crate::actions`) es la fuente de los ids y de los
//! chords por defecto. `config.toml` solo añade alternativas o quita atajos;
//! nunca se roba una tecla en silencio.

include!("keymap_parts/01_contexts.rs");
#[cfg(test)]
mod tests {
    use super::*;

    fn table(source: &str) -> toml::Table {
        let document: toml::Table = toml::from_str(&format!("[keymap]\n{source}")).expect("toml");
        document
            .get("keymap")
            .and_then(toml::Value::as_table)
            .cloned()
            .expect("tabla keymap")
    }

    #[test]
    fn palette_input_context_predicate_is_valid() {
        assert!(
            KeyBindingContextPredicate::parse(crate::PALETTE_INPUT_CONTEXT).is_ok(),
            "el predicado de descendiente de la paleta debe parsear"
        );
    }

    #[test]
    fn defaults_cover_every_registered_action() {
        let keymap = Keymap::defaults();
        for action in actions::all().filter(|action| action.enabled) {
            assert!(
                keymap.effective().contains_key(action.id),
                "falta {}",
                action.id
            );
            assert!(keymap.chords_for(action.id).is_some());
        }
        assert!(keymap.errors.is_empty());
    }

    #[test]
    fn default_chords_are_unique_per_context() {
        let keymap = Keymap::defaults();
        let mut seen = std::collections::HashSet::new();
        for binding in &keymap.bindings {
            for context in &binding.contexts {
                assert!(
                    seen.insert((context, binding.chord.as_str())),
                    "{} se repite en {context}",
                    binding.chord
                );
            }
        }
    }

    #[test]
    fn terminal_keys_are_never_stolen() {
        let keymap = Keymap::defaults();
        for binding in &keymap.bindings {
            let normalized = normalize_chord(&binding.chord);
            assert!(
                !matches!(
                    normalized.as_str(),
                    "ctrl-c" | "ctrl-d" | "ctrl-z" | "tab" | "esc"
                ),
                "{} roba una tecla de terminal",
                binding.chord
            );
        }
    }

    #[test]
    fn config_adds_alternative_chords() {
        let keymap = Keymap::resolve(&table(r#""ctrl-alt-shift-t" = "tab.new""#));
        assert!(keymap.errors.is_empty());
        assert_eq!(
            keymap.chords_for("tab.new").as_deref(),
            Some("Ctrl+Shift+T · Ctrl+Alt+Shift+T")
        );
        assert!(
            keymap.bindings.iter().any(|binding| {
                binding.chord == "ctrl-alt-shift-t" && binding.action == "tab.new"
            })
        );
    }

    #[test]
    fn duplicate_chords_in_the_same_context_are_rejected() {
        let keymap = Keymap::resolve(&table(
            "\"ctrl-alt-q\" = \"tab.new\"\nterminal = { \"ctrl-alt-q\" = \"tab.close\" }",
        ));
        assert!(!keymap.errors.is_empty());
    }

    #[test]
    fn unknown_actions_and_reserved_chords_are_explained() {
        let keymap = Keymap::resolve(&table(
            "\"ctrl-alt-q\" = \"nope.missing\"\n\"ctrl-c\" = \"tab.close\"",
        ));
        assert_eq!(keymap.errors.len(), 2);
        assert!(
            keymap
                .errors
                .iter()
                .any(|error| error.contains("desconocida"))
        );
        assert!(
            keymap
                .errors
                .iter()
                .any(|error| error.contains("reservado"))
        );
    }

    #[test]
    fn invalid_chords_and_shift_only_are_rejected() {
        let keymap = Keymap::resolve(&table(
            "\"ctrl-alt-\" = \"tab.new\"\n\"shift-t\" = \"tab.close\"",
        ));
        assert_eq!(keymap.errors.len(), 2);
        assert!(keymap.errors.iter().any(|error| error.contains("vacío")));
        assert!(keymap.errors.iter().any(|error| error.contains("Ctrl")));
    }

    #[test]
    fn unbind_removes_default_chords_and_settings_is_reserved() {
        let keymap = Keymap::resolve(&table(
            "unbind = [\"app.quit\"]\nsettings = { \"ctrl-alt-s\" = \"tab.new\" }",
        ));
        assert!(keymap.chords_for("app.quit").is_none());
        assert!(
            keymap
                .unbinds
                .iter()
                .any(|unbind| unbind.action == "app.quit")
        );
        assert!(keymap.errors.iter().any(|error| error.contains("settings")));
    }

    #[test]
    fn context_bindings_only_touch_their_context() {
        let keymap = Keymap::resolve(&table(
            "terminal = { \"ctrl-shift-c\" = \"terminal.copy\" }",
        ));
        assert!(keymap.errors.is_empty());
        let binding = keymap
            .bindings
            .iter()
            .find(|binding| binding.chord == "ctrl-shift-c" && binding.action == "terminal.copy")
            .expect("binding");
        assert_eq!(binding.contexts, vec![axf_terminal::TERMINAL_CONTEXT]);
    }

    #[test]
    fn default_shortcuts_doc_matches_generated_markdown() {
        let expected = actions::shortcuts_markdown(Keymap::defaults().effective());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/keymap.md");
        if std::env::var_os("AXF_WRITE_KEYMAP_DOC").is_some() {
            std::fs::write(&path, &expected).expect("regenera docs/keymap.md");
        }
        let actual = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            actual, expected,
            "docs/keymap.md está desactualizado; regenera el documento desde el registro"
        );
    }
}
