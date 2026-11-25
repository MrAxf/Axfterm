use crate::profiles::Profile;
use crate::schema::{
    CONFIG_SCHEMA_VERSION, CONFIG_TEMPLATE_HEADER, Config, ConfigError, parse_str, write_atomic,
};
use std::collections::BTreeMap;
use std::path::Path;
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, Value, value};
pub fn read_config_text(path: &Path) -> Result<Option<String>, ConfigError> {
    crate::schema::read_source(path)
}
pub fn write_config(path: &Path, base: &Config, draft: &Config) -> Result<String, ConfigError> {
    draft.validate()?;
    let source = read_config_text(path)?;
    write_document(path, source.as_deref(), base, draft)
}

/// Escribe solo si el archivo sigue siendo exactamente `expected`.
///
/// Cierra la carrera entre la comprobación del borrador y la escritura: si el
/// archivo cambió en ese intervalo, no se toca.
pub fn write_config_checked(
    path: &Path,
    expected: &str,
    base: &Config,
    draft: &Config,
) -> Result<String, ConfigError> {
    draft.validate()?;
    let source = read_config_text(path)?;
    if source.as_deref().unwrap_or_default() != expected {
        return Err(ConfigError::Conflict {
            path: path.display().to_string(),
        });
    }
    write_document(path, source.as_deref(), base, draft)
}

fn write_document(
    path: &Path,
    source: Option<&str>,
    base: &Config,
    draft: &Config,
) -> Result<String, ConfigError> {
    let label = path.display().to_string();
    let mut document = match source {
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
        set_value(
            table,
            "scrollback_lines",
            Value::from(i64::from(draft.terminal.scrollback_lines)),
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
    update_profile_agent_commands(table, profile);
}

fn update_profile_agent_commands(table: &mut Table, profile: &Profile) {
    if profile.agent_commands.is_empty() {
        table.remove("agent_commands");
        return;
    }
    if table
        .get("agent_commands")
        .is_none_or(|item| !item.is_table())
    {
        let mut commands = Table::new();
        commands.set_implicit(false);
        table.insert("agent_commands", Item::Table(commands));
    }
    let Some(commands) = table.get_mut("agent_commands").and_then(Item::as_table_mut) else {
        return;
    };
    let stale: Vec<String> = commands
        .iter()
        .map(|(agent, _)| agent.to_owned())
        .filter(|agent| !profile.agent_commands.contains_key(agent))
        .collect();
    for agent in stale {
        commands.remove(&agent);
    }
    for (agent, command) in &profile.agent_commands {
        set_value(commands, agent, Value::from(command.as_str()));
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
