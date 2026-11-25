use crate::profiles::{
    self, CommandError, Discovery, MAX_PROFILE_COMMAND_CHARS, MAX_PROFILE_NAME_CHARS, MAX_PROFILES,
    Profile, ProfileId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
pub const CONFIG_SCHEMA_VERSION: u32 = 2;
pub const MAX_CONFIG_BYTES: usize = 1024 * 1024;
/// Líneas de scrollback configurables por terminal (ADR 0005).
pub const MAX_SCROLLBACK_LINES: u32 = 50_000;
/// Tope efectivo de memoria por terminal; puede recortar las líneas pedidas.
pub const MAX_SCROLLBACK_BYTES: u64 = 32 * 1024 * 1024;
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
# Líneas de historial por terminal (0–50000). Además hay un tope de 32 MiB
# por terminal que puede recortar este número en anchos grandes.
scrollback_lines = 10000
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerminalConfig {
    /// Familia monoespaciada; `None` usa la cadena recomendada.
    pub font_family: Option<String>,
    /// Tamaño de fuente del terminal.
    pub font_size: f32,
    /// Multiplicador de altura de línea; 1.0 usa la altura natural del glifo.
    pub line_height: f32,
    /// Líneas de historial por terminal (0–50000).
    pub scrollback_lines: u32,
    /// Perfil que usan las pestañas nuevas.
    pub default_profile: ProfileId,
}
impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 13.0,
            line_height: 1.0,
            scrollback_lines: 10_000,
            default_profile: ProfileId::fallback(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FontsConfig {
    /// Rutas de archivos o directorios; las relativas se resuelven contra el
    /// directorio de configuración.
    pub sources: Vec<PathBuf>,
}
pub const AGENT_PROFILE_KEYS: &[&str] = &["opencode", "copilot"];
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
    /// El archivo cambió entre la comprobación y la escritura.
    #[error(
        "config.toml cambió fuera del panel mientras se guardaba; no se escribió nada. Recarga o confirma la sobrescritura."
    )]
    Conflict {
        /// Archivo afectado.
        path: String,
    },
}
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
