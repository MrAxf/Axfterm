//! Configuración de Axfterm: rutas, esquema TOML, defaults y hot reload.
//!
//! El crate no conoce GPUI ni el terminal: expone datos y errores tipados. La
//! app traduce esta configuración a tema, fuentes y terminal.

pub mod paths;
pub mod profiles;
pub mod schema;
pub mod watch;
mod write;

mod timestamp;

pub use paths::{
    PathError, config_dir, config_path, ensure_config_dir, scrollback_dir, session_path, themes_dir,
};
pub use profiles::{
    CommandError, CommandLine, DetectedProfile, Discovery, MAX_ICON_BYTES,
    MAX_PROFILE_COMMAND_CHARS, MAX_PROFILE_NAME_CHARS, MAX_PROFILES, Profile, ProfileId, discover,
    discover_wsl, fallback_profile, resolve_executable, tokenize,
};
pub use schema::{
    AGENT_PROFILE_KEYS, AgentsConfig, Appearance, CONFIG_SCHEMA_VERSION, CONFIG_TEMPLATE_HEADER,
    Config, ConfigError, FontsConfig, LoadedConfig, MAX_CONFIG_BYTES, TerminalConfig, UiConfig,
    append_profiles, default_template, load, load_or_create, parse_str, write_atomic,
};
pub use watch::{ConfigWatcher, DEBOUNCE, WatchError, WatchEvent, watch};
pub use write::{read_config_text, write_config};
