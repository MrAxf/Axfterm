use axf_config::{
    Appearance, Config, ConfigWatcher, DetectedProfile, LoadedConfig, Profile, ProfileId,
    WatchEvent,
};
use axf_terminal::SpawnSpec;
use axf_ui::components::ProfileIconSource;
use axf_ui::fonts::{self, FontSet};
use axf_ui::gpui::{App, WindowAppearance, px};
use axf_ui::metrics::UiMetrics;
use axf_ui::profile_icon::IconCache;
use axf_ui::theme::{self, Theme, ThemeCatalog, ThemeState, ThemeVariant};
use std::collections::HashMap;
use std::path::PathBuf;
#[derive(Clone, Debug, Default)]
pub(crate) struct ApplyReport {
    /// Se aplicó una configuración nueva.
    pub(crate) applied: bool,
    /// Avisos informativos listos para mostrar.
    pub(crate) messages: Vec<String>,
    /// Errores accionables (config/tema ignorado).
    pub(crate) errors: Vec<String>,
}
#[derive(Clone, Debug)]
pub(crate) struct PreparedProfile {
    /// Programa y argumentos resueltos.
    pub(crate) spec: SpawnSpec,
}
#[derive(Debug)]
pub(crate) struct ThemeHost {
    pub(crate) settings: Config,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) config_dir: PathBuf,
    pub(crate) themes_dir: Option<PathBuf>,
    pub(crate) catalog: ThemeCatalog,
    pub(crate) selected: String,
    pub(crate) preview_variant: Option<ThemeVariant>,
    /// Borrador visual del panel de ajustes; `None` muestra `settings`.
    pub(crate) preview: Option<Config>,
    pub(crate) watcher: Option<ConfigWatcher>,
    pub(crate) fonts: FontSet,
    pub(crate) display_scale: f32,
    pub(crate) revision: u64,
    pub(crate) notes: Vec<String>,
    /// Caché de iconos ya resueltos por perfil.
    profile_icons: HashMap<ProfileId, Option<PathBuf>>,
    /// Caché acotada de iconos extraídos de ejecutables.
    icons: IconCache,
    /// `true` cuando los perfiles vienen del descubrimiento local inicial.
    pub(crate) profiles_detected: bool,
}
