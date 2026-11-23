pub fn load_or_create(path: &Path) -> LoadedConfig {
    let source = match read_source(path) {
        Ok(Some(source)) => source,
        Ok(None) => return create_default_config(path),
        Err(ConfigError::FutureVersion { found, max }) => {
            return LoadedConfig {
                config: Config::default(),
                warning: Some(format!(
                    "config.toml usa schema_version {found}; esta build entiende hasta {max}. Se usan valores por defecto sin modificar el archivo."
                )),
                from_file: false,
                profiles_detected: false,
            };
        }
        Err(error) => return recover_config(path, error),
    };

    let discovery = profiles::discover();
    match parse_document(&source, &path.display().to_string(), &discovery) {
        Ok(parsed) => {
            let mut warning = None;
            if parsed.needs_write {
                let result = rewrite_document(&source, &parsed.config).and_then(|text| {
                    write_atomic(path, text.as_bytes()).map_err(|error| ConfigError::Io {
                        path: path.display().to_string(),
                        message: error.to_string(),
                    })
                });
                if let Err(error) = result {
                    warning = Some(format!(
                        "No se pudo guardar la migración de config.toml ({error}); se usa solo en memoria."
                    ));
                }
            }
            LoadedConfig {
                config: parsed.config,
                warning,
                from_file: true,
                profiles_detected: parsed.profiles_detected,
            }
        }
        Err(ConfigError::FutureVersion { found, max }) => LoadedConfig {
            config: Config::default(),
            warning: Some(format!(
                "config.toml usa schema_version {found}; esta build entiende hasta {max}. Se usan valores por defecto sin modificar el archivo."
            )),
            from_file: false,
            profiles_detected: false,
        },
        Err(error) => recover_config(path, error),
    }
}
fn create_default_config(path: &Path) -> LoadedConfig {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let discovery = profiles::discover();
    let template = default_template(&discovery);
    let config = parse_document(&template, "plantilla", &discovery)
        .map(|parsed| parsed.config)
        .unwrap_or_default();
    let warning = match write_atomic(path, template.as_bytes()) {
        Ok(()) => {
            tracing::info!("se creó config.toml con la plantilla y los perfiles detectados");
            None
        }
        Err(error) => Some(format!(
            "No se pudo escribir config.toml ({}); se usan los valores por defecto.",
            error
        )),
    };
    LoadedConfig {
        config,
        warning,
        from_file: false,
        profiles_detected: true,
    }
}
fn recover_config(path: &Path, error: ConfigError) -> LoadedConfig {
    let backup = backup_file(path);
    let detail = match backup {
        Ok(Some(backup)) => format!(
            "config.toml no se pudo leer ({error}). Copia conservada en {}.",
            backup.display()
        ),
        Ok(None) => format!("config.toml no se pudo leer ({error})."),
        Err(backup_error) => {
            format!("config.toml no se pudo leer ({error}) y la copia falló ({backup_error}).")
        }
    };
    LoadedConfig {
        config: Config::default(),
        warning: Some(detail),
        from_file: false,
        profiles_detected: false,
    }
}
fn rewrite_document(source: &str, config: &Config) -> Result<String, ConfigError> {
    use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

    let mut document: DocumentMut =
        source
            .parse()
            .map_err(|error: toml_edit::TomlError| ConfigError::Field {
                field: "config.toml".into(),
                message: error.to_string(),
            })?;
    if let Some(item) = document.get_mut("schema_version") {
        *item = value(i64::from(CONFIG_SCHEMA_VERSION));
    } else {
        document.insert("schema_version", value(i64::from(CONFIG_SCHEMA_VERSION)));
    }
    if !document.contains_key("terminal") {
        document.insert("terminal", Item::Table(Table::new()));
    }
    if let Some(Item::Table(terminal)) = document.get_mut("terminal") {
        terminal.insert(
            "default_profile",
            value(config.terminal.default_profile.as_str()),
        );
    }
    let mut profiles = ArrayOfTables::new();
    for profile in &config.profiles {
        let mut table = Table::new();
        table.insert("id", value(profile.id.as_str()));
        table.insert("name", value(profile.name.as_str()));
        table.insert("command", value(profile.command.as_str()));
        if let Some(icon) = &profile.icon {
            table.insert("icon", value(icon.display().to_string()));
        }
        profiles.push(table);
    }
    document.insert("profiles", Item::ArrayOfTables(profiles));
    Ok(document.to_string())
}
pub fn append_profiles(
    path: &Path,
    new_profiles: &[Profile],
) -> Result<Vec<ProfileId>, ConfigError> {
    use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

    let source = read_source(path)?.ok_or_else(|| ConfigError::Io {
        path: path.display().to_string(),
        message: "el archivo no existe".into(),
    })?;
    let mut document: DocumentMut =
        source
            .parse()
            .map_err(|error: toml_edit::TomlError| ConfigError::Field {
                field: "config.toml".into(),
                message: error.to_string(),
            })?;
    let mut profiles = match document.remove("profiles") {
        Some(Item::ArrayOfTables(profiles)) => profiles,
        _ => ArrayOfTables::new(),
    };
    let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut commands: std::collections::HashSet<String> = std::collections::HashSet::new();
    for table in profiles.iter() {
        if let Some(id) = table.get("id").and_then(Item::as_str) {
            ids.insert(id.to_owned());
        }
        if let Some(command) = table.get("command").and_then(Item::as_str) {
            commands.insert(command.trim().to_ascii_lowercase());
        }
    }
    let mut added = Vec::new();
    for profile in new_profiles {
        if ids.contains(profile.id.as_str())
            || commands.contains(&profile.command.trim().to_ascii_lowercase())
        {
            continue;
        }
        let mut table = Table::new();
        table.insert("id", value(profile.id.as_str()));
        table.insert("name", value(profile.name.as_str()));
        table.insert("command", value(profile.command.as_str()));
        if let Some(icon) = &profile.icon {
            table.insert("icon", value(icon.display().to_string()));
        }
        profiles.push(table);
        ids.insert(profile.id.as_str().to_owned());
        commands.insert(profile.command.trim().to_ascii_lowercase());
        added.push(profile.id.clone());
    }
    if added.is_empty() {
        document.insert("profiles", Item::ArrayOfTables(profiles));
        return Ok(added);
    }

    if !document.contains_key("terminal") {
        document.insert("terminal", Item::Table(Table::new()));
    }
    if let Some(Item::Table(terminal)) = document.get_mut("terminal") {
        if terminal
            .get("default_profile")
            .and_then(Item::as_str)
            .is_none()
            && let Some(id) = added.first()
        {
            terminal.insert("default_profile", value(id.as_str()));
        }
    }
    document.insert("profiles", Item::ArrayOfTables(profiles));
    write_atomic(path, document.to_string().as_bytes()).map_err(|error| ConfigError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(added)
}
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = path.with_extension(format!(
        "tmp.{}.{}.{suffix}",
        std::process::id(),
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp")
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}
pub fn backup_file(path: &Path) -> std::io::Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = crate::timestamp::stamp();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config.toml".into());
    for attempt in 0..100u32 {
        let candidate = if attempt == 0 {
            format!("{name}.bak-{stamp}")
        } else {
            format!("{name}.bak-{stamp}-{attempt}")
        };
        let backup = path.with_file_name(candidate);
        if backup.exists() {
            continue;
        }
        std::fs::rename(path, &backup)?;
        return Ok(Some(backup));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no queda un nombre de backup libre",
    ))
}
fn parse_error(error: toml::de::Error, source: &str, label: &str) -> ConfigError {
    let (line, column) = error
        .span()
        .map(|span| line_column(source, span.start))
        .unwrap_or((1, 1));
    ConfigError::Parse {
        path: label.to_string(),
        message: error.message().to_string(),
        line,
        column,
    }
}
pub(crate) fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map(|tail| tail.chars().count() + 1)
        .unwrap_or(1);
    (line, column)
}
