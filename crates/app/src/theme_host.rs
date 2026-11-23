//! Carga y aplicación de config, temas y fuentes.
//!
//! Mantiene el catálogo de temas, el tema activo y el watcher. La app solo
//! consume este módulo; el modelo de workspace no sabe nada de estilo.

use std::collections::HashMap;
use std::path::PathBuf;

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

/// Resultado de aplicar una recarga.
#[derive(Clone, Debug, Default)]
pub(crate) struct ApplyReport {
    /// Se aplicó una configuración nueva.
    pub(crate) applied: bool,
    /// Avisos informativos listos para mostrar.
    pub(crate) messages: Vec<String>,
    /// Errores accionables (config/tema ignorado).
    pub(crate) errors: Vec<String>,
}

/// Perfil listo para arrancar.
#[derive(Clone, Debug)]
pub(crate) struct PreparedProfile {
    /// Programa y argumentos resueltos.
    pub(crate) spec: SpawnSpec,
}

/// Host del sistema visual de la ventana.
#[derive(Debug)]
pub(crate) struct ThemeHost {
    pub(crate) settings: Config,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) config_dir: PathBuf,
    pub(crate) themes_dir: Option<PathBuf>,
    pub(crate) catalog: ThemeCatalog,
    pub(crate) selected: String,
    pub(crate) preview_variant: Option<ThemeVariant>,
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

impl ThemeHost {
    /// Arranca el sistema visual: config, catálogo, fuentes y watcher.
    pub(crate) fn bootstrap(cx: &mut App) -> Self {
        let config_path = axf_config::config_path().ok();
        let themes_dir = axf_config::themes_dir().ok();
        let config_dir = axf_config::config_dir().unwrap_or_else(|_| PathBuf::from("."));
        let LoadedConfig {
            config,
            warning,
            from_file: _,
            profiles_detected,
        } = config_path
            .as_deref()
            .map(axf_config::load_or_create)
            .unwrap_or(LoadedConfig {
                config: Config::default(),
                warning: Some(
                    "No se pudo resolver el directorio de configuración; se usan defaults.".into(),
                ),
                from_file: false,
                profiles_detected: false,
            });

        let mut notes = Vec::new();
        if let Some(warning) = warning {
            notes.push(warning);
        }

        let mut catalog = axf_ui::theme::builtin::load_builtins();
        if let Some(dir) = themes_dir.as_deref() {
            let user = axf_ui::theme::load_dir(dir);
            catalog.extend_unique(user.themes);
            notes.extend(user.errors.into_iter().map(|error| error.to_string()));
        } else {
            notes.push("No se pudo resolver themes/; solo hay temas built-in.".into());
        }

        let sources = resolved_font_sources(&config, &config_dir);
        let prepared = fonts::read_sources(&sources);
        let loaded = prepared.files.len();
        let font_errors = fonts::install(cx, prepared.files);
        let installed = cx.text_system().all_font_names();
        let mut fonts = fonts::resolve(
            &installed,
            config.ui.font_family.as_deref(),
            config.terminal.font_family.as_deref(),
            loaded,
            prepared.errors,
        );
        fonts.errors.extend(font_errors);
        notes.extend(fonts.warnings.iter().cloned());
        notes.extend(fonts.errors.iter().cloned());

        let selected = config.ui.theme.clone();
        let watcher = match (config_path.as_deref(), themes_dir.as_deref()) {
            (Some(config), Some(themes)) => match axf_config::watch(config, themes) {
                Ok(watcher) => Some(watcher),
                Err(error) => {
                    notes.push(format!("Hot reload desactivado: {error}"));
                    None
                }
            },
            _ => None,
        };

        let mut host = Self {
            settings: config,
            config_path,
            config_dir: config_dir.clone(),
            themes_dir,
            catalog,
            selected,
            preview_variant: None,
            watcher,
            fonts,
            display_scale: 1.0,
            revision: 0,
            notes,
            profile_icons: HashMap::new(),
            icons: IconCache::new(config_dir.join("icons").join("cache")),
            profiles_detected,
        };
        if let Some(theme) = host.catalog.find(&host.selected).cloned() {
            host.selected = theme.meta.name.clone();
        } else {
            host.notes.push(format!(
                "El tema `{}` no existe; se usa el tema por defecto.",
                host.selected
            ));
            host.selected = "One Dark".into();
        }
        host.apply_current(cx);
        host
    }

    /// Tema activo según apariencia, preview y catálogo.
    pub(crate) fn resolve_theme(&self, system_dark: bool) -> (Theme, Option<String>) {
        let target = self
            .preview_variant
            .unwrap_or_else(|| target_variant(self.settings.appearance, system_dark));
        let selected = self
            .catalog
            .find(&self.selected)
            .cloned()
            .unwrap_or_else(Theme::fallback);
        if selected.variant() == target {
            return (selected, None);
        }
        if let Some(counterpart) = self
            .catalog
            .counterpart(&selected.meta.family, target)
            .cloned()
        {
            return (counterpart, None);
        }
        let fallback = self
            .catalog
            .themes
            .iter()
            .find(|theme| theme.variant() == target)
            .cloned()
            .unwrap_or_else(Theme::fallback);
        let warning = Some(format!(
            "`{}` no tiene variante {}; se usa `{}`.",
            selected.meta.name,
            target.label().to_lowercase(),
            fallback.meta.name
        ));
        (fallback, warning)
    }

    /// Aplica el tema resuelto y recalcula métricas. Devuelve avisos nuevos.
    pub(crate) fn apply_current(&mut self, cx: &mut App) -> Vec<String> {
        let system_dark = system_is_dark(cx);
        let (theme, warning) = self.resolve_theme(system_dark);
        let mut messages = Vec::new();
        if let Some(warning) = warning {
            messages.push(warning);
        }
        let metrics = UiMetrics::scaled(
            self.settings.ui.font_size,
            self.settings.ui.scale,
            self.display_scale,
        );
        let ui_font_size = metrics.font_size;
        let mono_font_size = px(self.settings.terminal.font_size);
        let terminal_line_height = if (self.settings.terminal.line_height - 1.0).abs() < 0.001 {
            px(0.0)
        } else {
            px(f32::from(mono_font_size) * self.settings.terminal.line_height)
        };
        theme::install(
            cx,
            ThemeState {
                theme,
                metrics,
                fonts: self.fonts.clone(),
                ui_font_size,
                mono_font_size,
                terminal_line_height,
            },
        );
        self.revision = self.revision.wrapping_add(1);
        messages
    }

    /// Actualiza la escala DPI del monitor activo.
    pub(crate) fn sync_display_scale(&mut self, scale: f32, cx: &mut App) -> bool {
        if !scale.is_finite() || scale <= 0.0 || (scale - self.display_scale).abs() < 0.001 {
            return false;
        }
        self.display_scale = scale;
        let _ = self.apply_current(cx);
        true
    }

    /// Cambia el tema activo en memoria y lo aplica.
    pub(crate) fn select_theme(&mut self, name: &str, cx: &mut App) -> bool {
        let Some(theme) = self.catalog.find(name) else {
            self.notes
                .push(format!("El tema `{name}` no existe en el catálogo."));
            return false;
        };
        let variant = theme.variant();
        self.selected = name.to_string();
        self.settings.ui.theme = name.to_string();
        self.settings.appearance = match variant {
            ThemeVariant::Dark => Appearance::Dark,
            ThemeVariant::Light => Appearance::Light,
        };
        self.preview_variant = None;
        let messages = self.apply_current(cx);
        self.notes.extend(messages);
        true
    }

    /// Fuerza la variante clara/oscura para previsualizar.
    pub(crate) fn set_preview_variant(&mut self, variant: Option<ThemeVariant>, cx: &mut App) {
        self.preview_variant = variant;
        let messages = self.apply_current(cx);
        self.notes.extend(messages);
    }

    /// Alterna la variante de preview.
    pub(crate) fn toggle_preview_variant(&mut self, cx: &mut App) {
        let current = theme::state(cx).theme.variant();
        self.set_preview_variant(Some(current.opposite()), cx);
    }

    /// Recarga config y temas. Conserva la última config válida ante errores.
    pub(crate) fn reload(&mut self, event: WatchEvent, cx: &mut App) -> ApplyReport {
        let mut report = ApplyReport::default();
        if event.config {
            match self.config_path.as_deref().map(axf_config::load) {
                Some(Ok(Some(config))) => {
                    let sources_changed = resolved_font_sources(&config, &self.config_dir)
                        != resolved_font_sources(&self.settings, &self.config_dir);
                    self.settings = config;
                    self.profile_icons.clear();
                    report.applied = true;
                    if sources_changed {
                        report.messages.extend(self.reload_fonts(cx));
                    } else {
                        report.messages.extend(self.refresh_font_resolution(cx));
                    }
                }
                Some(Ok(None)) => {
                    report.errors.push(
                        "config.toml desapareció; se conserva la última configuración válida."
                            .into(),
                    );
                }
                Some(Err(error)) => {
                    report.errors.push(format!(
                        "Cambio de config.toml ignorado: {error}. Se conserva la última válida."
                    ));
                }
                None => {}
            }
        }
        if event.themes {
            let mut catalog = axf_ui::theme::builtin::load_builtins();
            if let Some(dir) = self.themes_dir.as_deref() {
                let user = axf_ui::theme::load_dir(dir);
                catalog.extend_unique(user.themes);
                report
                    .errors
                    .extend(user.errors.into_iter().map(|error| error.to_string()));
            }
            self.catalog = catalog;
            if self.catalog.find(&self.selected).is_none() {
                report.errors.push(format!(
                    "El tema `{}` ya no existe; se usa el tema por defecto.",
                    self.selected
                ));
                self.selected = "One Dark".into();
                self.settings.ui.theme = self.selected.clone();
            }
            report.applied = true;
        }
        if event.config && report.applied {
            // La config es la fuente de verdad al recargar: adopta el tema
            // elegido en el archivo y descarta una previsualización de sesión.
            self.preview_variant = None;
            let desired = self.settings.ui.theme.clone();
            if self.catalog.find(&desired).is_some() {
                self.selected = desired;
            } else {
                report.errors.push(format!(
                    "El tema `{desired}` no existe; se usa el tema por defecto."
                ));
                self.selected = "One Dark".into();
                self.settings.ui.theme = self.selected.clone();
            }
        }
        if report.applied {
            let messages = self.apply_current(cx);
            report.messages.extend(messages);
        }
        report
    }

    /// Relee las fuentes configuradas y las registra.
    fn reload_fonts(&mut self, cx: &mut App) -> Vec<String> {
        let sources = resolved_font_sources(&self.settings, &self.config_dir);
        let prepared = fonts::read_sources(&sources);
        let loaded = prepared.files.len();
        let errors = fonts::install(cx, prepared.files);
        let installed = cx.text_system().all_font_names();
        let mut resolved = fonts::resolve(
            &installed,
            self.settings.ui.font_family.as_deref(),
            self.settings.terminal.font_family.as_deref(),
            loaded,
            prepared.errors,
        );
        resolved.errors.extend(errors);
        let mut messages = resolved.warnings.clone();
        messages.extend(resolved.errors.clone());
        self.fonts = resolved;
        messages
    }

    /// Re-resuelve familias tras un cambio de config sin releer archivos.
    fn refresh_font_resolution(&mut self, cx: &mut App) -> Vec<String> {
        let installed = cx.text_system().all_font_names();
        let resolved = fonts::resolve(
            &installed,
            self.settings.ui.font_family.as_deref(),
            self.settings.terminal.font_family.as_deref(),
            self.fonts.loaded,
            self.fonts.errors.clone(),
        );
        let mut messages = resolved.warnings.clone();
        messages.extend(resolved.errors.clone());
        self.fonts = resolved;
        messages
    }

    /// Perfiles en orden de menú.
    pub(crate) fn profiles(&self) -> &[Profile] {
        &self.settings.profiles
    }

    /// Perfil por defecto.
    pub(crate) fn default_profile(&self) -> Option<&Profile> {
        self.settings.default_profile()
    }

    /// Perfil por id.
    pub(crate) fn profile(&self, id: &ProfileId) -> Option<&Profile> {
        self.settings.profile(id)
    }

    /// Nombre visible de un perfil.
    pub(crate) fn profile_name(&self, id: &ProfileId) -> Option<String> {
        self.profile(id).map(|profile| profile.name.clone())
    }

    /// Resuelve el perfil a un `SpawnSpec` y su icono.
    ///
    /// Un comando mal formado o un ejecutable ausente es un error accionable;
    /// nunca se cae a otro shell (ADR 0006). Un icono que falla solo produce un
    /// aviso y el fallback Tabler.
    pub(crate) fn prepare_profile(&mut self, id: &ProfileId) -> Result<PreparedProfile, String> {
        let profile = self
            .profile(id)
            .cloned()
            .ok_or_else(|| format!("El perfil `{id}` no existe en config.toml."))?;
        let line = profile.command_line().map_err(|error| {
            format!(
                "El perfil `{}` tiene un comando inválido: {error}. Revisa config.toml.",
                profile.name
            )
        })?;
        let executable = axf_config::resolve_executable(&line.program).ok_or_else(|| {
            format!(
                "El perfil `{}` apunta a `{}`, pero no se encontró. Revisa `command` en config.toml.",
                profile.name, line.program
            )
        })?;
        let _icon = self.resolve_icon(&profile, Some(&executable));
        Ok(PreparedProfile {
            spec: SpawnSpec::new(executable.to_string_lossy().into_owned(), line.args),
        })
    }

    /// Icono de un perfil para pestañas y menús.
    pub(crate) fn profile_icon(&mut self, id: &ProfileId) -> ProfileIconSource {
        let Some(profile) = self.profile(id).cloned() else {
            return ProfileIconSource::fallback();
        };
        if let Some(cached) = self.profile_icons.get(&profile.id) {
            return ProfileIconSource::from_path(cached.clone());
        }
        let executable = profile
            .command_line()
            .ok()
            .and_then(|line| axf_config::resolve_executable(&line.program));
        let path = self.resolve_icon(&profile, executable.as_deref());
        ProfileIconSource::from_path(path)
    }

    fn resolve_icon(
        &mut self,
        profile: &Profile,
        executable: Option<&std::path::Path>,
    ) -> Option<PathBuf> {
        if let Some(cached) = self.profile_icons.get(&profile.id) {
            return cached.clone();
        }
        let manual = profile.icon.as_deref();
        let result = self.icons.resolve(manual, executable, &self.config_dir);
        let path = match result {
            Ok(path) => path,
            Err(error) => {
                // Un icono ausente no es un fallo de la app: se cae al icono
                // genérico sin interrumpir al usuario.
                tracing::debug!(
                    "icono del perfil `{}` no disponible ({error}); se usa el genérico",
                    profile.name
                );
                None
            }
        };
        self.profile_icons.insert(profile.id.clone(), path.clone());
        path
    }

    /// Fusiona los perfiles WSL detectados en segundo plano con la config viva.
    pub(crate) fn merge_wsl_profiles(
        &mut self,
        discovered: Vec<DetectedProfile>,
        cx: &mut App,
    ) -> Result<Vec<ProfileId>, String> {
        let Some(path) = self.config_path.clone() else {
            return Err("No se pudo resolver config.toml para añadir WSL.".into());
        };
        let profiles: Vec<Profile> = discovered
            .into_iter()
            .map(|detected| detected.profile)
            .collect();
        if profiles.is_empty() {
            return Ok(Vec::new());
        }
        let added = axf_config::append_profiles(&path, &profiles)
            .map_err(|error| format!("No se pudieron guardar los perfiles WSL: {error}"))?;
        if added.is_empty() {
            return Ok(added);
        }
        self.profile_icons.clear();
        let report = self.reload(
            WatchEvent {
                config: true,
                themes: false,
            },
            cx,
        );
        if !report.errors.is_empty() {
            return Err(report.errors.join(" "));
        }
        Ok(added)
    }

    /// Nombres y variantes para el selector de temas.
    pub(crate) fn theme_names(&self) -> Vec<(String, ThemeVariant)> {
        self.catalog
            .themes
            .iter()
            .map(|theme| (theme.meta.name.clone(), theme.meta.variant))
            .collect()
    }

    /// Importa un archivo de tema validándolo antes de copiarlo.
    pub(crate) fn import_theme(
        &mut self,
        source: &std::path::Path,
        cx: &mut App,
    ) -> Result<PathBuf, String> {
        let Some(dir) = self.themes_dir.clone() else {
            return Err("No se pudo resolver el directorio de temas.".into());
        };
        let theme = axf_ui::theme::load_file(source).map_err(|error| error.to_string())?;
        if self.catalog.find(&theme.meta.name).is_some() {
            return Err(format!(
                "Ya existe un tema llamado `{}`; usa otro nombre.",
                theme.meta.name
            ));
        }
        let file_name = source
            .file_name()
            .ok_or_else(|| "El archivo no tiene nombre.".to_string())?;
        let destination = dir.join(file_name);
        if destination.exists() {
            return Err(format!("Ya existe {}", destination.display()));
        }
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("No se pudo crear {}: {error}", dir.display()))?;
        std::fs::copy(source, &destination)
            .map_err(|error| format!("No se pudo copiar el tema: {error}"))?;
        self.notes
            .push(format!("Tema `{}` importado.", theme.meta.name));
        let report = self.reload(
            WatchEvent {
                config: false,
                themes: true,
            },
            cx,
        );
        self.notes.extend(report.errors);
        self.notes.extend(report.messages);
        Ok(destination)
    }

    /// Exporta el tema activo a un archivo TOML.
    pub(crate) fn export_theme(
        &self,
        destination: &std::path::Path,
        cx: &App,
    ) -> Result<PathBuf, String> {
        let theme = theme::state(cx).theme.clone();
        let toml = theme.to_toml().map_err(|error| error.to_string())?;
        let mut path = destination.to_path_buf();
        if path.extension().is_none() {
            path.set_extension("toml");
        }
        axf_config::write_atomic(&path, toml.as_bytes())
            .map_err(|error| format!("No se pudo escribir {}: {error}", path.display()))?;
        Ok(path)
    }
}

fn target_variant(appearance: Appearance, system_dark: bool) -> ThemeVariant {
    match appearance {
        Appearance::Light => ThemeVariant::Light,
        Appearance::Dark => ThemeVariant::Dark,
        Appearance::System if system_dark => ThemeVariant::Dark,
        Appearance::System => ThemeVariant::Light,
    }
}

fn system_is_dark(cx: &App) -> bool {
    matches!(
        cx.window_appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

fn resolved_font_sources(config: &Config, base: &std::path::Path) -> Vec<PathBuf> {
    config
        .fonts
        .sources
        .iter()
        .map(|source| config.resolve_font_source(base, source))
        .collect()
}
