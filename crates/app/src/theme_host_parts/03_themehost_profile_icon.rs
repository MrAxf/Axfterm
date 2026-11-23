impl ThemeHost {
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
