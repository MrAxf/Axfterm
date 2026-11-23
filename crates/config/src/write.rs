//! Escritura de `config.toml` desde el panel de ajustes (ADR 0007).
//!
//! El documento se edita sobre el texto actual con `toml_edit`: solo se tocan
//! las secciones cuyo valor difiere del borrador base, se conservan comentarios
//! y orden, y el resultado se vuelve a parsear antes de escribirse de forma
//! atómica. Si algo falla no se aplica nada.

use std::collections::BTreeMap;
use std::path::Path;

use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, Value, value};

use crate::profiles::Profile;
use crate::schema::{
    CONFIG_SCHEMA_VERSION, CONFIG_TEMPLATE_HEADER, Config, ConfigError, parse_str, write_atomic,
};

/// Texto actual de `config.toml`, si existe.
///
/// El panel lo usa como huella para detectar cambios externos y para no
/// reaccionar dos veces al evento del watcher provocado por su propia escritura.
pub fn read_config_text(path: &Path) -> Result<Option<String>, ConfigError> {
    crate::schema::read_source(path)
}

/// Escribe `draft` sobre el documento actual.
///
/// `base` es la configuración de la que salió el borrador. Solo se actualizan
/// los campos que difieren entre `base` y `draft`; el resto del archivo,
/// incluidos comentarios y orden, queda intacto. Devuelve el texto escrito.
pub fn write_config(path: &Path, base: &Config, draft: &Config) -> Result<String, ConfigError> {
    draft.validate()?;
    let source = read_config_text(path)?;
    let label = path.display().to_string();
    let mut document = match source.as_deref() {
        Some(source) => source
            .parse::<DocumentMut>()
            .map_err(|error| ConfigError::Field {
                field: "config.toml".into(),
                message: error.to_string(),
            })?,
        None => CONFIG_TEMPLATE_HEADER
            .parse::<DocumentMut>()
            .unwrap_or_else(|_| DocumentMut::new()),
    };

    apply_scalars(&mut document, base, draft);
    apply_section_scalars(&mut document, base, draft);
    apply_fonts(&mut document, base, draft);
    apply_agents(&mut document, base, draft);
    apply_keymap(&mut document, &base.keymap, &draft.keymap);
    apply_profiles(&mut document, base, draft);

    let text = document.to_string();
    let parsed = parse_str(&text, &label).map_err(|error| ConfigError::Field {
        field: "config.toml".into(),
        message: format!("la escritura produjo un archivo inválido: {error}"),
    })?;
    if !equivalent(&parsed, draft) {
        return Err(ConfigError::Field {
            field: "config.toml".into(),
            message: "la escritura no refleja el borrador; no se aplicó nada".into(),
        });
    }
    write_atomic(path, text.as_bytes()).map_err(|error| ConfigError::Io {
        path: label,
        message: error.to_string(),
    })?;
    Ok(text)
}

fn apply_scalars(document: &mut DocumentMut, base: &Config, draft: &Config) {
    // La build siempre escribe la versión que entiende, aunque el archivo
    // viniera de una migración v1 → v2.
    set_value(
        document.as_table_mut(),
        "schema_version",
        Value::from(i64::from(CONFIG_SCHEMA_VERSION)),
    );
    let text = match draft.appearance {
        crate::schema::Appearance::System => "system",
        crate::schema::Appearance::Light => "light",
        crate::schema::Appearance::Dark => "dark",
    };
    if base.appearance != draft.appearance
        || document.get("appearance").and_then(Item::as_str) != Some(text)
    {
        set_value(document.as_table_mut(), "appearance", Value::from(text));
    }
}

fn apply_section_scalars(document: &mut DocumentMut, base: &Config, draft: &Config) {
    if base.ui != draft.ui {
        let table = ensure_table(document, "ui");
        set_value(table, "theme", Value::from(draft.ui.theme.as_str()));
        set_optional_string(table, "font_family", &draft.ui.font_family);
        set_value(
            table,
            "font_size",
            Value::from(f64::from(draft.ui.font_size)),
        );
        set_value(table, "scale", Value::from(f64::from(draft.ui.scale)));
    }
    if base.terminal != draft.terminal {
        let table = ensure_table(document, "terminal");
        set_optional_string(table, "font_family", &draft.terminal.font_family);
        set_value(
            table,
            "font_size",
            Value::from(f64::from(draft.terminal.font_size)),
        );
        set_value(
            table,
            "line_height",
            Value::from(f64::from(draft.terminal.line_height)),
        );
    }
    // El perfil por defecto debe existir siempre y coincidir con el borrador,
    // aunque el resto de `[terminal]` no haya cambiado.
    let table = ensure_table(document, "terminal");
    if table.get("default_profile").and_then(Item::as_str)
        != Some(draft.terminal.default_profile.as_str())
    {
        set_value(
            table,
            "default_profile",
            Value::from(draft.terminal.default_profile.as_str()),
        );
    }
}

fn apply_fonts(document: &mut DocumentMut, base: &Config, draft: &Config) {
    if base.fonts == draft.fonts {
        return;
    }
    let table = ensure_table(document, "fonts");
    let mut array = Array::new();
    for source in &draft.fonts.sources {
        array.push(source.display().to_string());
    }
    set_value(table, "sources", Value::Array(array));
}

fn apply_agents(document: &mut DocumentMut, base: &Config, draft: &Config) {
    if base.agents == draft.agents {
        return;
    }
    let table = ensure_table(document, "agents");
    set_value(table, "enabled", Value::from(draft.agents.enabled));
    set_value(
        table,
        "scan_interval_ms",
        Value::from(draft.agents.scan_interval_ms as i64),
    );
    set_value(
        table,
        "notify_finished",
        Value::from(draft.agents.notify_finished),
    );
    set_value(
        table,
        "notify_error",
        Value::from(draft.agents.notify_error),
    );
    set_value(
        table,
        "notify_attention",
        Value::from(draft.agents.notify_attention),
    );
    if draft.agents.preferred_profiles.is_empty() {
        table.remove("preferred_profiles");
    } else {
        let mut profiles = Table::new();
        profiles.set_implicit(false);
        for (agent, profile) in &draft.agents.preferred_profiles {
            profiles.insert(agent, value(profile.as_str()));
        }
        if let Some(item) = table.get_mut("preferred_profiles") {
            *item = Item::Table(profiles);
        } else {
            table.insert("preferred_profiles", Item::Table(profiles));
        }
    }
}

fn apply_keymap(document: &mut DocumentMut, base: &toml::Table, draft: &toml::Table) {
    let current: toml::Table = document
        .to_string()
        .parse::<toml::Value>()
        .ok()
        .and_then(|value| value.get("keymap").and_then(toml::Value::as_table).cloned())
        .unwrap_or_default();
    let before = flatten(base);
    let after = flatten(draft);
    let current_flat = flatten(&current);
    if before == after && current_flat == after {
        return;
    }
    let table = ensure_table(document, "keymap");
    let first_prefix = first_key_prefix(table);
    for path in before.keys() {
        if !after.contains_key(path) {
            remove_table_path(table, path);
        }
    }
    for (path, new) in &after {
        if current_flat.get(path) == Some(new) && before.get(path) == Some(new) {
            continue;
        }
        let Ok(new) = edit_value(new) else {
            continue;
        };
        set_table_path(table, path.clone(), new);
    }
    if let Some(prefix) = first_prefix {
        restore_first_prefix(table, &prefix);
    }
}

fn first_key_prefix(table: &Table) -> Option<String> {
    let key = table.iter().next().map(|(key, _)| key)?;
    table
        .key(key)?
        .leaf_decor()
        .prefix()
        .and_then(|prefix| prefix.as_str())
        .map(str::to_owned)
        .filter(|prefix| !prefix.trim().is_empty())
}

fn restore_first_prefix(table: &mut Table, prefix: &str) {
    let Some(key) = table.iter().next().map(|(key, _)| key.to_owned()) else {
        return;
    };
    let Some(mut key) = table.key_mut(&key) else {
        return;
    };
    let existing = key
        .leaf_decor()
        .prefix()
        .and_then(|prefix| prefix.as_str())
        .unwrap_or_default();
    if existing.trim().is_empty() {
        key.leaf_decor_mut().set_prefix(prefix.to_owned());
    }
}

fn apply_profiles(document: &mut DocumentMut, base: &Config, draft: &Config) {
    let has_document_profiles = matches!(
        document.get("profiles"),
        Some(Item::ArrayOfTables(profiles)) if !profiles.is_empty()
    );
    if base.profiles == draft.profiles && has_document_profiles {
        return;
    }
    let mut existing: BTreeMap<String, Table> = BTreeMap::new();
    if let Some(Item::ArrayOfTables(profiles)) = document.remove("profiles") {
        for table in profiles.iter() {
            if let Some(id) = table.get("id").and_then(Item::as_str) {
                existing.insert(id.to_owned(), table.clone());
            }
        }
    }
    let mut profiles = ArrayOfTables::new();
    for profile in &draft.profiles {
        let mut table = existing.remove(profile.id.as_str()).unwrap_or_default();
        update_profile_table(&mut table, profile);
        profiles.push(table);
    }
    if let Some(item) = document.get_mut("profiles") {
        *item = Item::ArrayOfTables(profiles);
    } else {
        document.insert("profiles", Item::ArrayOfTables(profiles));
    }
}

fn update_profile_table(table: &mut Table, profile: &Profile) {
    set_value(table, "id", Value::from(profile.id.as_str()));
    set_value(table, "name", Value::from(profile.name.as_str()));
    set_value(table, "command", Value::from(profile.command.as_str()));
    match profile.icon.as_deref() {
        Some(icon) => set_value(table, "icon", Value::from(icon.display().to_string())),
        None => {
            table.remove("icon");
        }
    }
}

fn set_optional_string(table: &mut Table, key: &str, new: &Option<String>) {
    match new {
        Some(text) => set_value(table, key, Value::from(text.as_str())),
        None => {
            table.remove(key);
        }
    }
}

/// Reemplaza un valor conservando el decorado (comentario de línea) del
/// existente.
fn set_value(table: &mut Table, key: &str, new: Value) {
    if let Some(item) = table.get_mut(key)
        && item.is_value()
    {
        // Mutar el valor existente conserva el decorado de la clave: el
        // comentario y el espaciado previos no se pierden al reemplazarlo.
        let decor = item
            .as_value()
            .map(|value| value.decor().clone())
            .unwrap_or_default();
        *item = Item::Value(new);
        if let Some(value) = item.as_value_mut() {
            *value.decor_mut() = decor;
        }
        return;
    }
    table.insert(key, Item::Value(new));
}
fn ensure_table<'a>(document: &'a mut DocumentMut, key: &str) -> &'a mut Table {
    if !document.contains_key(key) {
        document.insert(key, Item::Table(Table::new()));
    }
    if !document[key].is_table() {
        document[key] = Item::Table(Table::new());
    }
    document[key]
        .as_table_mut()
        .expect("la tabla acaba de crearse")
}

fn flatten(table: &toml::Table) -> BTreeMap<Vec<String>, toml::Value> {
    let mut flat = BTreeMap::new();
    flatten_into(table, &mut Vec::new(), &mut flat);
    flat
}

fn flatten_into(
    table: &toml::Table,
    prefix: &mut Vec<String>,
    flat: &mut BTreeMap<Vec<String>, toml::Value>,
) {
    for (key, value) in table {
        prefix.push(key.clone());
        if let toml::Value::Table(nested) = value {
            flatten_into(nested, prefix, flat);
        } else {
            flat.insert(prefix.clone(), value.clone());
        }
        prefix.pop();
    }
}

fn edit_value(value: &toml::Value) -> Result<Value, ConfigError> {
    Ok(match value {
        toml::Value::String(text) => Value::from(text.as_str()),
        toml::Value::Integer(integer) => Value::from(*integer),
        toml::Value::Float(float) => Value::from(*float),
        toml::Value::Boolean(boolean) => Value::from(*boolean),
        toml::Value::Datetime(datetime) => Value::from(datetime.to_string()),
        toml::Value::Array(items) => {
            let mut array = Array::new();
            for item in items {
                array.push(edit_value(item)?);
            }
            Value::Array(array)
        }
        toml::Value::Table(_) => {
            return Err(ConfigError::Field {
                field: "keymap".into(),
                message: "las tablas anidadas no se admiten como valor de atajo".into(),
            });
        }
    })
}

fn set_table_path(table: &mut Table, path: Vec<String>, new: Value) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut table = table;
    for key in parents {
        if !table.contains_key(key) || !table[key].is_table() {
            table.insert(key, Item::Table(Table::new()));
        }
        table = table[key]
            .as_table_mut()
            .expect("la tabla acaba de crearse");
    }
    set_value(table, last, new);
}

fn remove_table_path(table: &mut Table, path: &[String]) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut table = table;
    for key in parents {
        let Some(nested) = table.get_mut(key).and_then(Item::as_table_mut) else {
            return;
        };
        table = nested;
    }
    table.remove(last);
}

fn equivalent(left: &Config, right: &Config) -> bool {
    left.schema_version == right.schema_version
        && left.appearance == right.appearance
        && left.ui == right.ui
        && left.terminal == right.terminal
        && left.fonts == right.fonts
        && left.agents == right.agents
        && left.profiles == right.profiles
        && flatten(&left.keymap) == flatten(&right.keymap)
}

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
