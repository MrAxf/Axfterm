//! Atajos configurables: lectura de `[keymap]`, validación e instalación.
//!
//! El registro de acciones (`crate::actions`) es la fuente de los ids y de los
//! chords por defecto. `config.toml` solo añade alternativas o quita atajos;
//! nunca se roba una tecla en silencio.

use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use axf_ui::gpui::{App, KeyBinding, KeyBindingContextPredicate, NoAction, SharedString};

use crate::actions;

/// Contextos aceptados en `[keymap.*]`.
const CONTEXTS: &[(&str, &str)] = &[
    ("workspace", crate::WORKSPACE_CONTEXT),
    ("sidebar", crate::SIDEBAR_CONTEXT),
    ("terminal", axf_terminal::TERMINAL_CONTEXT),
];

/// Teclas que pertenecen siempre a la terminal.
const TERMINAL_RESERVED: &[&str] = &[
    "ctrl-c",
    "ctrl-d",
    "ctrl-z",
    "tab",
    "shift-tab",
    "esc",
    "escape",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
    "backspace",
];

/// Un chord que se va a instalar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeymapBinding {
    pub(crate) chord: String,
    pub(crate) action: String,
    pub(crate) contexts: Vec<&'static str>,
}

/// Un chord por defecto que se desactiva.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KeymapUnbind {
    chord: String,
    action: String,
    contexts: Vec<&'static str>,
}

/// Keymap resuelto: lo que se instala en GPUI y lo que muestra la ayuda.
#[derive(Clone, Debug, Default)]
pub(crate) struct Keymap {
    bindings: Vec<KeymapBinding>,
    unbinds: Vec<KeymapUnbind>,
    effective: BTreeMap<String, Vec<String>>,
    /// Errores accionables de `config.toml`.
    pub(crate) errors: Vec<String>,
    /// Avisos que no impiden arrancar.
    pub(crate) warnings: Vec<String>,
}

impl Keymap {
    /// Solo los atajos por defecto del registro.
    pub(crate) fn defaults() -> Self {
        let mut keymap = Self::default();
        for action in actions::all() {
            if !action.enabled {
                continue;
            }
            keymap.bindings.push(KeymapBinding {
                chord: action.default_chord.to_owned(),
                action: action.id.to_owned(),
                contexts: action.contexts.to_vec(),
            });
            keymap
                .effective
                .entry(action.id.to_owned())
                .or_default()
                .push(action.default_chord.to_owned());
        }
        keymap
    }

    /// Resuelve `[keymap]` sobre los atajos por defecto.
    pub(crate) fn resolve(table: &toml::Table) -> Self {
        let mut keymap = Self::defaults();
        keymap.apply_table(table);
        keymap
    }

    /// Chords efectivos por id de acción.
    #[cfg(test)]
    pub(crate) fn effective(&self) -> &BTreeMap<String, Vec<String>> {
        &self.effective
    }

    /// Chords visibles de una acción, ya formateados para la UI.
    pub(crate) fn chords_for(&self, id: &str) -> Option<String> {
        let chords = self.effective.get(id)?;
        if chords.is_empty() {
            return None;
        }
        Some(
            chords
                .iter()
                .map(|chord| crate::keys::format_chord(chord))
                .collect::<Vec<_>>()
                .join(" · "),
        )
    }

    fn apply_table(&mut self, table: &toml::Table) {
        let mut entries: Vec<(&String, &toml::Value)> = table.iter().collect();
        entries.sort_by(|left, right| left.0.cmp(right.0));

        let mut seen: HashMap<(String, String), String> = HashMap::new();
        for (key, value) in entries {
            if key == "unbind" {
                self.apply_unbind_list(value, None, false);
                continue;
            }
            if let Some(section) = value.as_table() {
                let Some((name, context)) = CONTEXTS.iter().find(|(name, _)| *name == key) else {
                    if key == "settings" {
                        self.errors.push(
                            "config.toml: el contexto `settings` está reservado para el panel de ajustes (M7)."
                                .into(),
                        );
                    } else {
                        self.errors.push(format!(
                            "config.toml: contexto `{key}` desconocido en [keymap]; usa `workspace`, `sidebar` o `terminal`."
                        ));
                    }
                    continue;
                };
                self.apply_section(section, context, name, &mut seen);
                continue;
            }
            let Some(action_id) = value.as_str() else {
                self.errors.push(format!(
                    "config.toml: [keymap] `{key}` debe ser un id de acción entre comillas."
                ));
                continue;
            };
            self.add_binding(key, action_id, None, &mut seen);
        }
    }

    fn apply_section(
        &mut self,
        section: &toml::Table,
        context: &'static str,
        context_label: &str,
        seen: &mut HashMap<(String, String), String>,
    ) {
        let mut entries: Vec<(&String, &toml::Value)> = section.iter().collect();
        entries.sort_by(|left, right| left.0.cmp(right.0));
        for (key, value) in entries {
            if key == "unbind" {
                self.apply_unbind_list(value, Some(context), true);
                continue;
            }
            let Some(action) = value.as_str() else {
                self.errors.push(format!(
                    "config.toml: [keymap.{context_label}] `{key}` debe ser un id de acción entre comillas."
                ));
                continue;
            };
            self.add_binding(key, action, Some(context), seen);
        }
    }

    fn apply_unbind_list(
        &mut self,
        value: &toml::Value,
        context: Option<&'static str>,
        in_section: bool,
    ) {
        let where_ = if in_section {
            "en [keymap.<contexto>]"
        } else {
            "en [keymap]"
        };
        let Some(items) = value.as_array() else {
            self.errors.push(format!(
                "config.toml: `unbind` {where_} debe ser un array de ids."
            ));
            return;
        };
        for item in items {
            let Some(id) = item.as_str() else {
                self.errors.push(format!(
                    "config.toml: `unbind` {where_} solo admite ids entre comillas."
                ));
                continue;
            };
            let Some(meta) = actions::by_id(id) else {
                self.errors.push(format!(
                    "config.toml: `unbind` apunta a una acción desconocida `{id}`."
                ));
                continue;
            };
            let contexts: Vec<&'static str> = match context {
                Some(context) => vec![context],
                None => meta.contexts.to_vec(),
            };
            for context in &contexts {
                self.unbinds.push(KeymapUnbind {
                    chord: meta.default_chord.to_owned(),
                    action: id.to_owned(),
                    contexts: vec![*context],
                });
            }
            if context.is_none() {
                self.effective.remove(id);
            }
        }
    }

    fn add_binding(
        &mut self,
        chord: &str,
        action_id: &str,
        context: Option<&'static str>,
        seen: &mut HashMap<(String, String), String>,
    ) {
        let Some(meta) = actions::by_id(action_id) else {
            self.errors.push(format!(
                "config.toml: [keymap] `{chord}` apunta a una acción desconocida `{action_id}`."
            ));
            return;
        };
        if let Err(error) = validate_chord(chord, meta, context) {
            self.errors
                .push(format!("config.toml: [keymap] `{chord}`: {error}"));
            return;
        }
        let normalized = normalize_chord(chord);
        let contexts: Vec<&'static str> = match context {
            Some(context) => vec![context],
            None => meta.contexts.to_vec(),
        };
        let mut duplicate = false;
        for context in &contexts {
            let key = ((*context).to_owned(), normalized.clone());
            if let Some(previous) = seen.get(&key) {
                if previous != action_id {
                    self.errors.push(format!(
                        "config.toml: [keymap] `{chord}` está asignado a `{previous}` y a `{action_id}` en el mismo contexto."
                    ));
                    duplicate = true;
                }
            } else {
                seen.insert(key, action_id.to_owned());
            }
        }
        if duplicate {
            return;
        }

        // Un atajo nuevo que coincide con el de otra acción la pisa. No se
        // bloquea (el usuario manda), pero se explica y queda registrado.
        for other in actions::all() {
            if other.id == action_id {
                continue;
            }
            if normalize_chord(other.default_chord) == normalized
                && self
                    .effective
                    .get(other.id)
                    .is_some_and(|chords| !chords.is_empty())
            {
                self.warnings.push(format!(
                    "config.toml: `{chord}` pasa de `{}` a `{action_id}`; añade `unbind = [\"{}\"]` si quieres quitar el anterior.",
                    other.id, other.id
                ));
            }
        }

        self.bindings.push(KeymapBinding {
            chord: normalized.clone(),
            action: action_id.to_owned(),
            contexts,
        });
        let chords = self.effective.entry(action_id.to_owned()).or_default();
        if !chords
            .iter()
            .any(|existing| normalize_chord(existing) == normalized)
        {
            chords.push(normalized);
        }
    }

    /// Instala defaults + cambios sobre el keymap de GPUI.
    ///
    /// Se llama después de `axf_ui::init`. Al recargar se vuelve a llamar: los
    /// bindings añadidos después ganan a los anteriores y los `unbind` se
    /// aplican como `NoAction`.
    pub(crate) fn install(&self, cx: &mut App) -> Vec<String> {
        let mut errors = Vec::new();
        let mut bindings = Vec::new();
        for spec in &self.bindings {
            let Some(action) = actions::action_for(&spec.action) else {
                errors.push(format!(
                    "No se pudo construir la acción `{}` al instalar atajos.",
                    spec.action
                ));
                continue;
            };
            for context in &spec.contexts {
                let Some(predicate) = KeyBindingContextPredicate::parse(context).ok() else {
                    errors.push(format!("Contexto de teclado inválido `{context}`."));
                    continue;
                };
                match KeyBinding::load(
                    &spec.chord,
                    action.boxed_clone(),
                    Some(Rc::new(predicate)),
                    false,
                    None,
                    cx,
                ) {
                    Ok(binding) => bindings.push(binding),
                    Err(error) => errors.push(error.to_string()),
                }
            }
        }
        for spec in &self.unbinds {
            for context in &spec.contexts {
                let Some(predicate) = KeyBindingContextPredicate::parse(context).ok() else {
                    continue;
                };
                match KeyBinding::load(
                    &spec.chord,
                    Box::new(NoAction),
                    Some(Rc::new(predicate)),
                    false,
                    Some(SharedString::from(spec.action.clone())),
                    cx,
                ) {
                    Ok(binding) => bindings.push(binding),
                    Err(error) => errors.push(error.to_string()),
                }
            }
        }
        if !bindings.is_empty() {
            cx.bind_keys(bindings);
        }
        errors
    }
}

fn normalize_chord(chord: &str) -> String {
    chord
        .trim()
        .to_ascii_lowercase()
        .replace('+', "-")
        .replace(' ', "")
}

fn validate_chord(
    chord: &str,
    meta: &actions::ActionMeta,
    context: Option<&str>,
) -> Result<(), String> {
    let chord = chord.trim();
    if chord.is_empty() {
        return Err("está vacío".into());
    }
    if chord.chars().any(char::is_whitespace) {
        return Err("no puede contener espacios; usa `-` entre modificador y tecla".into());
    }
    let parts: Vec<&str> = chord.split('-').collect();
    if parts.iter().any(|part| part.is_empty()) {
        return Err("tiene un tramo vacío (¿un `-` de más?)".into());
    }
    let modifiers: Vec<&str> = parts[..parts.len().saturating_sub(1)].to_vec();
    if modifiers.iter().any(|part| {
        !matches!(
            part.to_ascii_lowercase().as_str(),
            "ctrl" | "control" | "alt" | "shift" | "cmd" | "super" | "meta" | "win" | "fn"
        )
    }) {
        return Err(
            "solo admite ctrl, alt, shift, cmd/super/meta/win y fn antes de la tecla".into(),
        );
    }
    let normalized = normalize_chord(chord);
    if TERMINAL_RESERVED.contains(&normalized.as_str()) {
        return Err(
            "está reservado para la terminal (Ctrl+C/D/Z, Tab, Esc, flechas y navegación); ese atajo no se puede robar."
                .into(),
        );
    }
    let has_ctrl = modifiers
        .iter()
        .any(|part| part.eq_ignore_ascii_case("ctrl") || part.eq_ignore_ascii_case("control"));
    let has_alt = modifiers
        .iter()
        .any(|part| part.eq_ignore_ascii_case("alt"));
    let has_platform = modifiers.iter().any(|part| {
        part.eq_ignore_ascii_case("cmd")
            || part.eq_ignore_ascii_case("super")
            || part.eq_ignore_ascii_case("meta")
            || part.eq_ignore_ascii_case("win")
    });
    let has_fn = modifiers.iter().any(|part| part.eq_ignore_ascii_case("fn"));
    let terminal_only = context == Some(axf_terminal::TERMINAL_CONTEXT)
        || (meta.contexts.len() == 1 && meta.contexts[0] == axf_terminal::TERMINAL_CONTEXT);
    if !terminal_only && !has_ctrl && !has_alt && !has_platform && !has_fn {
        return Err("necesita Ctrl, Alt o Win; solo Shift no basta para una acción de app".into());
    }
    if !has_ctrl && !has_alt && !has_platform && !has_fn {
        return Err("necesita al menos un modificador (Ctrl, Alt o Win)".into());
    }
    Ok(())
}

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
        for action in actions::all() {
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
        let actual = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            actual, expected,
            "docs/keymap.md está desactualizado; regenera el documento desde el registro"
        );
    }
}
