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
            preview: None,
            preview_previous_theme: None,
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

    /// Configuración que pinta la ventana: el borrador visual o la activa.
    pub(crate) fn visual(&self) -> &Config {
        self.preview.as_ref().unwrap_or(&self.settings)
    }

    /// Tema activo según apariencia, preview y catálogo.
    pub(crate) fn resolve_theme(&self, system_dark: bool) -> (Theme, Option<String>) {
        let visual = self.visual();
        let target = self
            .preview_variant
            .unwrap_or_else(|| target_variant(visual.appearance, system_dark));
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
            self.visual().ui.font_size,
            self.visual().ui.scale,
            self.display_scale,
        );
        let ui_font_size = metrics.font_size;
        let mono_font_size = px(self.visual().terminal.font_size);
        let terminal_line_height = if (self.visual().terminal.line_height - 1.0).abs() < 0.001 {
            px(0.0)
        } else {
            px(f32::from(mono_font_size) * self.visual().terminal.line_height)
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
                    self.preview = None;
                    self.preview_previous_theme = None;
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
        let sources = resolved_font_sources(self.visual(), &self.config_dir);
        let prepared = fonts::read_sources(&sources);
        let loaded = prepared.files.len();
        let errors = fonts::install(cx, prepared.files);
        let installed = cx.text_system().all_font_names();
        let mut resolved = fonts::resolve(
            &installed,
            self.visual().ui.font_family.as_deref(),
            self.visual().terminal.font_family.as_deref(),
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
            self.visual().ui.font_family.as_deref(),
            self.visual().terminal.font_family.as_deref(),
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

    /// Familias instaladas, ordenadas y sin duplicados.
    pub(crate) fn installed_fonts(&self, cx: &App) -> Vec<String> {
        let mut names = cx.text_system().all_font_names();
        names.sort();
        names.dedup();
        names
    }

    /// Previsualiza el borrador visual sin escribir ni tocar la config activa.
    ///
    /// Solo relee fuentes cuando cambian sus orígenes; el resto de campos se
    /// aplica con las métricas ya resueltas.
    pub(crate) fn preview_settings(&mut self, draft: &Config, cx: &mut App) -> Vec<String> {
        let sources_changed = resolved_font_sources(draft, &self.config_dir)
            != resolved_font_sources(self.visual(), &self.config_dir);
        let families_changed = draft.ui.font_family != self.visual().ui.font_family
            || draft.terminal.font_family != self.visual().terminal.font_family;
        if self.preview.is_none() {
            self.preview_previous_theme = Some(self.selected.clone());
        }
        self.preview = Some(draft.clone());
        self.preview_variant = None;
        if self.catalog.find(&draft.ui.theme).is_some() {
            self.selected = draft.ui.theme.clone();
        }
        let mut messages = if sources_changed {
            self.reload_fonts(cx)
        } else if families_changed {
            self.refresh_font_resolution(cx)
        } else {
            Vec::new()
        };
        messages.extend(self.apply_current(cx));
        messages
    }

    /// Descarta la previsualización y vuelve a la configuración activa.
    pub(crate) fn clear_preview(&mut self, cx: &mut App) -> Vec<String> {
        let Some(previous) = self.preview.take() else {
            return Vec::new();
        };
        if let Some(previous_theme) = self.preview_previous_theme.take() {
            self.selected = previous_theme;
        }
        let sources_changed = resolved_font_sources(&previous, &self.config_dir)
            != resolved_font_sources(&self.settings, &self.config_dir);
        let mut messages = if sources_changed {
            self.reload_fonts(cx)
        } else {
            self.refresh_font_resolution(cx)
        };
        messages.extend(self.apply_current(cx));
        messages
    }

    /// Convierte el borrador guardado en la configuración activa.
    ///
    /// Se llama una sola vez tras escribir el archivo; el watcher reconocerá
    /// el contenido y no repetirá el trabajo.
    pub(crate) fn commit_settings(&mut self, draft: Config, cx: &mut App) -> Vec<String> {
        let sources_changed = resolved_font_sources(&draft, &self.config_dir)
            != resolved_font_sources(&self.settings, &self.config_dir);
        self.preview = None;
        self.preview_previous_theme = None;
        self.preview_variant = None;
        self.settings = draft;
        self.profile_icons.clear();
        if self.catalog.find(&self.settings.ui.theme).is_some() {
            self.selected = self.settings.ui.theme.clone();
        }
        let mut messages = if sources_changed {
            self.reload_fonts(cx)
        } else {
            self.refresh_font_resolution(cx)
        };
        messages.extend(self.apply_current(cx));
        messages
    }

    /// Resuelve el perfil a un `SpawnSpec` y su icono.
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
        let program = executable.to_string_lossy().into_owned();
        let wsl_distro = axf_config::wsl_distro_from(&program, &line.args);
        Ok(PreparedProfile {
            spec: SpawnSpec::new(program, line.args),
            wsl_distro,
        })
    }
}
