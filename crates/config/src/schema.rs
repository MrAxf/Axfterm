//! Esquema de `config.toml` (ADR 0004).
//!
//! El archivo es la interfaz de configuración de v0.1: se edita a mano y se
//! recarga en caliente. Los campos desconocidos son un error para que un typo
//! no se ignore en silencio. Los rangos se validan sin clamp silencioso.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::profiles::{
    self, CommandError, Discovery, MAX_PROFILE_COMMAND_CHARS, MAX_PROFILE_NAME_CHARS, MAX_PROFILES,
    Profile, ProfileId,
};

/// Versión de esquema que entiende esta build.
pub const CONFIG_SCHEMA_VERSION: u32 = 2;

/// Tope de tamaño de `config.toml` (ADR 0005).
pub const MAX_CONFIG_BYTES: usize = 1024 * 1024;

/// Cabecera de la plantilla del primer arranque (ADR 0006).
pub const CONFIG_TEMPLATE_HEADER: &str = r#"# Axfterm — configuración de usuario.
# El archivo es la interfaz: se recarga en caliente al guardar.
schema_version = 2

# system | light | dark
appearance = "system"

[ui]
# Nombre de un tema built-in o de un archivo en themes/.
theme = "One Dark"
# Fuente de la UI; vacío deja la cadena de fallback de la plataforma.
# font_family = "Segoe UI"
# Tamaño base en píxeles lógicos (11–24).
font_size = 14.0
# Escala global de la UI (0.8–2.0). El DPI del monitor se aplica aparte.
scale = 1.0

[terminal]
# Fuente monoespaciada independiente de la UI.
# font_family = "Cascadia Mono"
# Tamaño de celda en píxeles (8–32).
font_size = 13.0
# Multiplicador de altura de línea (1.0–2.5). 1.0 usa la altura natural.
line_height = 1.0
# Perfil que usan las pestañas nuevas. Debe existir en [[profiles]].
default_profile = "__default__"

[fonts]
# Archivos .ttf/.otf/.ttc o directorios con fuentes (Nerd Font, CJK, emoji).
sources = []

[keymap]
# Atajos: `chord = "accion"`. Los ids salen de docs/keymap.md y la pantalla
# de atajos (Ctrl+Alt+K). Los atajos de app requieren Ctrl/Alt/Win; Ctrl+C,
# Ctrl+D, Ctrl+Z, Tab, Esc y las flechas quedan siempre para la terminal.
# "ctrl-alt-shift-n" = "tab.new"
# Quita todos los atajos por defecto de una acción:
# unbind = ["app.quit"]
# Contextos: workspace, sidebar y terminal.
# [keymap.terminal]
# "ctrl-shift-c" = "terminal.copy"

[agents]
# Detección local de agentes (opencode y Copilot) por OSC 7501 y procesos.
enabled = true
# Intervalo del escaneo de procesos en milisegundos (1000–2000).
scan_interval_ms = 1500
# Avisos in-app y en el título al terminar, fallar o requerir atención.
notify_finished = true
notify_error = true
notify_attention = true
# Perfil de terminal preferido para abrir agentes (vacío = perfil por defecto).
# [agents.preferred_profiles]
# opencode = "pwsh"
# copilot = "pwsh"
"#;

/// Plantilla del primer arranque con los perfiles detectados.
pub fn default_template(discovery: &Discovery) -> String {
    let default_profile = discovery.default_profile();
    let mut text = CONFIG_TEMPLATE_HEADER.replace(
        "default_profile = \"__default__\"",
        &format!("default_profile = \"{default_profile}\""),
    );
    text.push_str("\n# Perfiles locales. `command` es una línea: ejecutable + argumentos.\n");
    text.push_str("# Si se omite `icon`, Windows usa el icono del ejecutable.\n");
    let profiles = discovery.profiles_only();
    #[derive(Serialize)]
    struct Document<'a> {
        profiles: &'a [Profile],
    }
    match toml::to_string(&Document {
        profiles: &profiles,
    }) {
        Ok(body) => text.push_str(&body),
        Err(error) => {
            tracing::error!("no se pudo serializar la plantilla de perfiles: {error}");
            text.push_str("[[profiles]]\nid = \"cmd\"\nname = \"Símbolo del sistema\"\ncommand = \"cmd /Q\"\n");
        }
    }
    text
}

/// Apariencia global.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// Sigue la apariencia del sistema.
    #[default]
    System,
    /// Fuerza la variante clara.
    Light,
    /// Fuerza la variante oscura.
    Dark,
}

impl Appearance {
    /// Etiqueta corta para la UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::Light => "Claro",
            Self::Dark => "Oscuro",
        }
    }
}

/// Configuración resuelta.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Versión del esquema.
    pub schema_version: u32,
    /// Apariencia clara/oscura/sistema.
    pub appearance: Appearance,
    /// Interfaz y tema.
    pub ui: UiConfig,
    /// Terminal.
    pub terminal: TerminalConfig,
    /// Fuentes adicionales.
    pub fonts: FontsConfig,
    /// Detección de agentes (ADR 0009).
    pub agents: AgentsConfig,
    /// Atajos de teclado (`[keymap]`), validados por la app contra el registro
    /// de acciones.
    pub keymap: toml::Table,
    /// Perfiles de terminal en orden de menú (ADR 0006).
    pub profiles: Vec<Profile>,
}

impl Default for Config {
    fn default() -> Self {
        let fallback = profiles::fallback_profile();
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            appearance: Appearance::System,
            ui: UiConfig::default(),
            terminal: TerminalConfig {
                default_profile: fallback.id.clone(),
                ..TerminalConfig::default()
            },
            fonts: FontsConfig::default(),
            agents: AgentsConfig::default(),
            keymap: toml::Table::new(),
            profiles: vec![fallback],
        }
    }
}

/// Sección `[ui]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    /// Tema activo por nombre.
    pub theme: String,
    /// Familia de la UI; `None` usa la cadena recomendada.
    pub font_family: Option<String>,
    /// Tamaño base de la fuente de UI en píxeles lógicos.
    pub font_size: f32,
    /// Escala global de la UI.
    pub scale: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "One Dark".into(),
            font_family: None,
            font_size: 14.0,
            scale: 1.0,
        }
    }
}

/// Sección `[terminal]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerminalConfig {
    /// Familia monoespaciada; `None` usa la cadena recomendada.
    pub font_family: Option<String>,
    /// Tamaño de fuente del terminal.
    pub font_size: f32,
    /// Multiplicador de altura de línea; 1.0 usa la altura natural del glifo.
    pub line_height: f32,
    /// Perfil que usan las pestañas nuevas.
    pub default_profile: ProfileId,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 13.0,
            line_height: 1.0,
            default_profile: ProfileId::fallback(),
        }
    }
}

/// Sección `[fonts]`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FontsConfig {
    /// Rutas de archivos o directorios; las relativas se resuelven contra el
    /// directorio de configuración.
    pub sources: Vec<PathBuf>,
}

/// Claves válidas de `[agents.preferred_profiles]` (nombre estable del agente).
pub const AGENT_PROFILE_KEYS: &[&str] = &["opencode", "copilot"];

/// Sección `[agents]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentsConfig {
    /// Detección activa.
    pub enabled: bool,
    /// Intervalo del escaneo de procesos en milisegundos (1000–2000).
    pub scan_interval_ms: u64,
    /// Avisar cuando una sesión termina.
    pub notify_finished: bool,
    /// Avisar cuando una sesión falla.
    pub notify_error: bool,
    /// Avisar cuando una sesión requiere al usuario.
    pub notify_attention: bool,
    /// Perfil con el que abrir cada agente. La clave es el nombre estable del
    /// agente (`opencode`, `copilot`) y el valor un id de `[[profiles]]`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub preferred_profiles: BTreeMap<String, ProfileId>,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_interval_ms: 1500,
            notify_finished: true,
            notify_error: true,
            notify_attention: true,
            preferred_profiles: BTreeMap::new(),
        }
    }
}

/// Error de configuración con la mejor localización disponible.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// TOML inválido con línea y columna.
    #[error("{path}: {message} (línea {line}, columna {column})")]
    Parse {
        /// Archivo.
        path: String,
        /// Mensaje de `toml`.
        message: String,
        /// Línea 1-based.
        line: usize,
        /// Columna 1-based.
        column: usize,
    },
    /// Valor fuera de rango o campo incoherente.
    #[error("config.toml: campo `{field}`: {message}")]
    Field {
        /// Ruta del campo (`ui.font_size`).
        field: String,
        /// Detalle accionable.
        message: String,
    },
    /// El archivo viene de una versión posterior.
    #[error(
        "config.toml usa schema_version {found}; esta build entiende hasta {max}. Actualiza Axfterm."
    )]
    FutureVersion {
        /// Versión encontrada.
        found: u32,
        /// Versión máxima soportada.
        max: u32,
    },
    /// Error de E/S.
    #[error("{path}: {message}")]
    Io {
        /// Archivo.
        path: String,
        /// Detalle.
        message: String,
    },
    /// Archivo por encima del tope.
    #[error("config.toml supera el límite de {limit} bytes")]
    TooLarge {
        /// Tope.
        limit: usize,
    },
}

/// Resultado de arrancar con `config.toml`.
#[derive(Clone, Debug)]
pub struct LoadedConfig {
    /// Configuración efectiva (siempre válida).
    pub config: Config,
    /// Aviso accionable para la UI.
    pub warning: Option<String>,
    /// `true` si se leyó el archivo del usuario.
    pub from_file: bool,
    /// `true` si se completaron perfiles con el descubrimiento local.
    pub profiles_detected: bool,
}

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

/// Carga estricta de un `config.toml` existente.
pub fn load(path: &Path) -> Result<Option<Config>, ConfigError> {
    let Some(source) = read_source(path)? else {
        return Ok(None);
    };
    let parsed = parse_document(&source, &path.display().to_string(), &profiles::discover())?;
    Ok(Some(parsed.config))
}

/// Parsea y valida el contenido de un `config.toml`, migrando v1 → v2 en
/// memoria y completando perfiles si faltan (ADR 0006).
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

/// Migra v1 → v2 y completa perfiles vacíos. Devuelve si hay que reescribir el
/// archivo y si se usó el descubrimiento local.
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

/// Arranque con recuperación (ADR 0004).
///
/// - Falta el archivo: descubre perfiles, escribe la plantilla y los usa.
/// - v1 o config sin perfiles: migra y reescribe una sola vez conservando
///   comentarios.
/// - Corrupto o demasiado grande: mueve a backup fechado y usa defaults.
/// - Versión futura: no toca el archivo y usa defaults con aviso.
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

/// Reescribe el documento migrado conservando comentarios y orden.
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

/// Añade perfiles sin duplicar id ni comando y conserva comentarios.
///
/// Lo usa el descubrimiento de WSL en segundo plano. Devuelve los ids añadidos.
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

/// Escribe un archivo de forma atómica (temporal + `sync` + rename).
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

/// Mueve `path` a un backup `*.bak-YYYYMMDD-HHMMSS`.
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
