impl Config {
    /// Valida rangos y coherencia. No toca el disco.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version > CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::FutureVersion {
                found: self.schema_version,
                max: CONFIG_SCHEMA_VERSION,
            });
        }
        if self.schema_version < 1 {
            return Err(ConfigError::Field {
                field: "schema_version".into(),
                message: "debe ser un entero mayor o igual que 1".into(),
            });
        }
        if self.ui.theme.trim().is_empty() {
            return Err(ConfigError::Field {
                field: "ui.theme".into(),
                message: "no puede estar vacío".into(),
            });
        }
        if !(11.0..=24.0).contains(&self.ui.font_size) {
            return Err(ConfigError::Field {
                field: "ui.font_size".into(),
                message: format!("{} está fuera del rango 11–24", self.ui.font_size),
            });
        }
        if !(0.8..=2.0).contains(&self.ui.scale) {
            return Err(ConfigError::Field {
                field: "ui.scale".into(),
                message: format!("{} está fuera del rango 0.8–2.0", self.ui.scale),
            });
        }
        if !(8.0..=32.0).contains(&self.terminal.font_size) {
            return Err(ConfigError::Field {
                field: "terminal.font_size".into(),
                message: format!("{} está fuera del rango 8–32", self.terminal.font_size),
            });
        }
        if !(1.0..=2.5).contains(&self.terminal.line_height) {
            return Err(ConfigError::Field {
                field: "terminal.line_height".into(),
                message: format!("{} está fuera del rango 1.0–2.5", self.terminal.line_height),
            });
        }
        if self.fonts.sources.len() > 16 {
            return Err(ConfigError::Field {
                field: "fonts.sources".into(),
                message: "admite como máximo 16 rutas".into(),
            });
        }
        if !(1000..=2000).contains(&self.agents.scan_interval_ms) {
            return Err(ConfigError::Field {
                field: "agents.scan_interval_ms".into(),
                message: format!(
                    "{} está fuera del rango 1000–2000",
                    self.agents.scan_interval_ms
                ),
            });
        }
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::Field {
                field: "schema_version".into(),
                message: format!(
                    "esta build usa el esquema {CONFIG_SCHEMA_VERSION}; guarda el archivo para migrarlo"
                ),
            });
        }
        self.validate_profiles()?;
        self.validate_agent_profiles()?;
        Ok(())
    }

    fn validate_agent_profiles(&self) -> Result<(), ConfigError> {
        for (agent, profile) in &self.agents.preferred_profiles {
            if !AGENT_PROFILE_KEYS.contains(&agent.as_str()) {
                return Err(ConfigError::Field {
                    field: format!("agents.preferred_profiles.{agent}"),
                    message: format!("agente desconocido; usa {}", AGENT_PROFILE_KEYS.join(" o ")),
                });
            }
            if !self
                .profiles
                .iter()
                .any(|candidate| &candidate.id == profile)
            {
                return Err(ConfigError::Field {
                    field: format!("agents.preferred_profiles.{agent}"),
                    message: format!("`{profile}` no existe en [[profiles]]"),
                });
            }
        }
        Ok(())
    }

    fn validate_profiles(&self) -> Result<(), ConfigError> {
        if self.profiles.is_empty() {
            return Err(ConfigError::Field {
                field: "profiles".into(),
                message: "declara al menos un perfil".into(),
            });
        }
        if self.profiles.len() > MAX_PROFILES {
            return Err(ConfigError::Field {
                field: "profiles".into(),
                message: format!(
                    "admite como máximo {MAX_PROFILES} perfiles; hay {}",
                    self.profiles.len()
                ),
            });
        }
        let mut seen = std::collections::HashSet::new();
        for (index, profile) in self.profiles.iter().enumerate() {
            let prefix = format!("profiles[{index}]");
            if !profile.id.is_valid_slug() {
                return Err(ConfigError::Field {
                    field: format!("{prefix}.id"),
                    message: "usa un slug único: minúsculas, dígitos, `-` o `_`".into(),
                });
            }
            if !seen.insert(profile.id.as_str().to_owned()) {
                return Err(ConfigError::Field {
                    field: format!("{prefix}.id"),
                    message: format!("el id `{}` está repetido", profile.id),
                });
            }
            if profile.name.trim().is_empty() {
                return Err(ConfigError::Field {
                    field: format!("{prefix}.name"),
                    message: "no puede estar vacío".into(),
                });
            }
            if profile.name.chars().count() > MAX_PROFILE_NAME_CHARS {
                return Err(ConfigError::Field {
                    field: format!("{prefix}.name"),
                    message: format!("admite como máximo {MAX_PROFILE_NAME_CHARS} caracteres"),
                });
            }
            if profile.command.trim().is_empty() {
                return Err(ConfigError::Field {
                    field: format!("{prefix}.command"),
                    message: "no puede estar vacío".into(),
                });
            }
            if profile.command.chars().count() > MAX_PROFILE_COMMAND_CHARS {
                return Err(ConfigError::Field {
                    field: format!("{prefix}.command"),
                    message: format!("admite como máximo {MAX_PROFILE_COMMAND_CHARS} caracteres"),
                });
            }
            profile.command_line().map_err(|error| match error {
                CommandError::UnterminatedQuote { quote } => ConfigError::Field {
                    field: format!("{prefix}.command"),
                    message: format!("comilla {quote} sin cerrar"),
                },
                other => ConfigError::Field {
                    field: format!("{prefix}.command"),
                    message: other.to_string(),
                },
            })?;
        }
        if !self
            .profiles
            .iter()
            .any(|profile| profile.id == self.terminal.default_profile)
        {
            return Err(ConfigError::Field {
                field: "terminal.default_profile".into(),
                message: format!(
                    "`{}` no existe en [[profiles]]",
                    self.terminal.default_profile
                ),
            });
        }
        Ok(())
    }

    /// Perfil por id.
    pub fn profile(&self, id: &ProfileId) -> Option<&Profile> {
        self.profiles.iter().find(|profile| &profile.id == id)
    }

    /// Perfil por defecto ya validado.
    pub fn default_profile(&self) -> Option<&Profile> {
        self.profile(&self.terminal.default_profile)
    }

    /// Perfil preferido para un agente (`opencode` o `copilot`).
    pub fn agent_profile(&self, agent: &str) -> Option<&Profile> {
        self.agents
            .preferred_profiles
            .get(agent)
            .and_then(|id| self.profile(id))
    }

    /// Resuelve una ruta de fuente relativa contra el directorio de config.
    pub fn resolve_font_source(&self, base: &Path, source: &Path) -> PathBuf {
        if source.is_absolute() {
            source.to_path_buf()
        } else {
            base.join(source)
        }
    }

    /// Resuelve la ruta de icono de un perfil contra el directorio de config.
    pub fn resolve_profile_icon(&self, base: &Path, profile: &Profile) -> Option<PathBuf> {
        profile.icon.as_ref().map(|icon| {
            if icon.is_absolute() {
                icon.clone()
            } else {
                base.join(icon)
            }
        })
    }
}
pub fn load(path: &Path) -> Result<Option<Config>, ConfigError> {
    let Some(source) = read_source(path)? else {
        return Ok(None);
    };
    let parsed = parse_document(&source, &path.display().to_string(), &profiles::discover())?;
    Ok(Some(parsed.config))
}
pub fn parse_str(source: &str, label: &str) -> Result<Config, ConfigError> {
    parse_document(source, label, &profiles::discover()).map(|parsed| parsed.config)
}
struct ParsedDocument {
    config: Config,
    needs_write: bool,
    profiles_detected: bool,
}
pub(crate) fn read_source(path: &Path) -> Result<Option<String>, ConfigError> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ConfigError::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            });
        }
    };
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err(ConfigError::TooLarge {
            limit: MAX_CONFIG_BYTES,
        });
    }
    std::fs::read_to_string(path)
        .map(Some)
        .map_err(|error| ConfigError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })
}
fn parse_document(
    source: &str,
    label: &str,
    discovery: &Discovery,
) -> Result<ParsedDocument, ConfigError> {
    let mut value: toml::Value =
        toml::from_str(source).map_err(|error| parse_error(error, source, label))?;
    let migration_needed = migration_needed(&value)?;
    if migration_needed {
        // Un typo o un tipo incorrecto debe señalar línea y columna. La
        // migración trabaja sobre `toml::Value` y pierde el span, así que se
        // valida la forma original antes de migrar.
        if let Err(error) = toml::from_str::<Config>(source) {
            return Err(parse_error(error, source, label));
        }
    }
    let (needs_write, profiles_detected) = migrate_value(&mut value, discovery)?;
    let config: Config = if migration_needed {
        value
            .try_into()
            .map_err(|error: toml::de::Error| parse_error(error, source, label))?
    } else {
        toml::from_str(source).map_err(|error| parse_error(error, source, label))?
    };
    config.validate()?;
    Ok(ParsedDocument {
        config,
        needs_write,
        profiles_detected,
    })
}
fn migration_needed(value: &toml::Value) -> Result<bool, ConfigError> {
    let version = match value.get("schema_version") {
        Some(version) => version.as_integer().ok_or_else(|| ConfigError::Field {
            field: "schema_version".into(),
            message: "debe ser un entero mayor o igual que 1".into(),
        })?,
        None => 1,
    };
    if version < 1 {
        return Err(ConfigError::Field {
            field: "schema_version".into(),
            message: "debe ser un entero mayor o igual que 1".into(),
        });
    }
    let version = u32::try_from(version).unwrap_or(u32::MAX);
    if version > CONFIG_SCHEMA_VERSION {
        return Err(ConfigError::FutureVersion {
            found: version,
            max: CONFIG_SCHEMA_VERSION,
        });
    }
    let has_profiles = value
        .get("profiles")
        .and_then(toml::Value::as_array)
        .is_some_and(|profiles| !profiles.is_empty());
    let has_default = value
        .get("terminal")
        .and_then(toml::Value::as_table)
        .and_then(|terminal| terminal.get("default_profile"))
        .and_then(toml::Value::as_str)
        .is_some_and(|id| !id.is_empty());
    Ok(version < CONFIG_SCHEMA_VERSION || !has_profiles || !has_default)
}
fn migrate_value(
    value: &mut toml::Value,
    discovery: &Discovery,
) -> Result<(bool, bool), ConfigError> {
    let version_value = value.get("schema_version");
    let version = match version_value {
        Some(version) => version.as_integer().ok_or_else(|| ConfigError::Field {
            field: "schema_version".into(),
            message: "debe ser un entero mayor o igual que 1".into(),
        })?,
        None => 1,
    };
    if version < 1 {
        return Err(ConfigError::Field {
            field: "schema_version".into(),
            message: "debe ser un entero mayor o igual que 1".into(),
        });
    }
    let version = u32::try_from(version).unwrap_or(u32::MAX);
    if version > CONFIG_SCHEMA_VERSION {
        return Err(ConfigError::FutureVersion {
            found: version,
            max: CONFIG_SCHEMA_VERSION,
        });
    }
    let mut needs_write = version < CONFIG_SCHEMA_VERSION;
    let table = value.as_table_mut().ok_or_else(|| ConfigError::Field {
        field: "config.toml".into(),
        message: "la raíz debe ser una tabla TOML".into(),
    })?;
    if needs_write {
        table.insert(
            "schema_version".to_owned(),
            toml::Value::Integer(i64::from(CONFIG_SCHEMA_VERSION)),
        );
    }

    let has_profiles = table
        .get("profiles")
        .and_then(toml::Value::as_array)
        .is_some_and(|profiles| !profiles.is_empty());
    let profiles_detected = !has_profiles;
    if profiles_detected {
        let mut profiles = Vec::new();
        for profile in discovery.profiles_only() {
            let value = toml::Value::try_from(profile).map_err(|error| ConfigError::Field {
                field: "profiles".into(),
                message: format!("no se pudo preparar el perfil detectado: {error}"),
            })?;
            profiles.push(value);
        }
        table.insert("profiles".to_owned(), toml::Value::Array(profiles));
        needs_write = true;
    }

    let first_profile = table
        .get("profiles")
        .and_then(toml::Value::as_array)
        .and_then(|profiles| profiles.first())
        .and_then(|profile| profile.get("id"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned);

    let terminal = table
        .entry("terminal".to_owned())
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let terminal = terminal.as_table_mut().ok_or_else(|| ConfigError::Field {
        field: "terminal".into(),
        message: "debe ser una tabla".into(),
    })?;
    let has_default = terminal
        .get("default_profile")
        .and_then(toml::Value::as_str)
        .is_some_and(|id| !id.is_empty());
    if !has_default {
        terminal.insert(
            "default_profile".into(),
            toml::Value::String(
                first_profile.unwrap_or_else(|| discovery.default_profile().as_str().to_owned()),
            ),
        );
        needs_write = true;
    }

    Ok((needs_write, profiles_detected))
}
