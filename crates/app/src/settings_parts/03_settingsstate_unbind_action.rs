impl SettingsState {
    /// Quita todos los atajos de una acción.
    pub(crate) fn unbind_action(&mut self, action_id: &str) -> Result<(), String> {
        if crate::actions::by_id(action_id).is_none() {
            return Err(format!("La acción `{action_id}` no existe."));
        }
        let mut candidate = self.draft.keymap.clone();
        remove_action_entries(&mut candidate, action_id);
        set_unbind(&mut candidate, action_id, true);
        let resolved = Keymap::resolve(&candidate);
        if !resolved.errors.is_empty() {
            return Err(resolved.errors.join(" "));
        }
        self.draft.keymap = candidate;
        self.capturing = None;
        Ok(())
    }

    /// Restaura los atajos por defecto de una acción.
    pub(crate) fn reset_action(&mut self, action_id: &str) -> Result<(), String> {
        if crate::actions::by_id(action_id).is_none() {
            return Err(format!("La acción `{action_id}` no existe."));
        }
        let mut candidate = self.draft.keymap.clone();
        remove_action_entries(&mut candidate, action_id);
        set_unbind(&mut candidate, action_id, false);
        self.draft.keymap = candidate;
        Ok(())
    }

    /// Empieza a capturar la siguiente tecla para una acción.
    pub(crate) fn begin_capture(&mut self, action_id: &str) {
        self.capturing = Some(action_id.to_owned());
        self.notice = Some((
            "Pulsa la combinación nueva; Esc cancela y Supr quita el atajo.".into(),
            false,
        ));
    }

    /// Cancela la captura.
    pub(crate) fn cancel_capture(&mut self) {
        self.capturing = None;
        self.notice = None;
    }

    // --- Avanzado ------------------------------------------------------

    /// Restaura los valores por defecto conservando los perfiles del usuario.
    pub(crate) fn restore_defaults(&mut self) {
        let profiles = self.draft.profiles.clone();
        let mut next = Config {
            profiles,
            keymap: toml::Table::new(),
            ..Config::default()
        };
        if !next
            .profiles
            .iter()
            .any(|profile| profile.id == next.terminal.default_profile)
            && let Some(first) = next.profiles.first()
        {
            next.terminal.default_profile = first.id.clone();
        }
        self.draft = next;
        self.profile_cursor = 0;
        self.pending_defaults = false;
        self.notice = Some((
            "Valores por defecto restaurados en el borrador; guarda para aplicarlos.".into(),
            false,
        ));
    }

    // --- Validación y conflicto ---------------------------------------

    /// Valida el borrador completo con las mismas reglas que el arranque.
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if let Err(error) = self.draft.validate() {
            errors.push(error.to_string());
        }
        let keymap = Keymap::resolve(&self.draft.keymap);
        errors.extend(keymap.errors);
        errors
    }

    /// Registra el resultado de una validación previa al guardado.
    pub(crate) fn set_errors(&mut self, errors: Vec<String>) {
        self.errors = errors;
    }

    /// Aviso para la cabecera o el pie del panel.
    pub(crate) fn set_notice(&mut self, text: String, error: bool) {
        self.notice = Some((text, error));
    }

    /// Texto que se muestra en el conflicto.
    pub(crate) fn conflict_message(&self) -> Option<&str> {
        self.conflict
            .as_ref()
            .map(|conflict| conflict.summary.as_str())
    }

    /// Aplica un cambio externo de `config.toml`.
    pub(crate) fn external_change(
        &mut self,
        source: String,
        parsed: Result<Config, ConfigError>,
    ) -> ExternalOutcome {
        if self.baseline.as_deref() == Some(source.as_str()) {
            return ExternalOutcome::Ignored;
        }
        match parsed {
            Ok(config) => {
                if self.is_dirty() {
                    self.conflict = Some(Conflict {
                        source,
                        summary: "config.toml cambió fuera del panel. Recarga el archivo o sigue editando; guardar pedirá confirmación para sobrescribirlo.".into(),
                    });
                    self.overwrite = false;
                    ExternalOutcome::Conflict
                } else {
                    self.adopt(config, Some(source));
                    ExternalOutcome::Reloaded
                }
            }
            Err(error) => {
                let message = format!(
                    "El cambio externo de config.toml no es válido ({error}); se conserva el borrador."
                );
                self.notice = Some((message.clone(), true));
                ExternalOutcome::Invalid(message)
            }
        }
    }

    /// Recarga el archivo externo descartando el borrador.
    pub(crate) fn reload_external(&mut self, config: Config, source: String) {
        self.adopt(config, Some(source));
        self.notice = Some((
            "config.toml recargado; el borrador se descartó.".into(),
            false,
        ));
    }

    /// Mantiene el borrador tras un conflicto.
    pub(crate) fn keep_editing(&mut self) {
        self.conflict = None;
        self.overwrite = false;
        self.notice = Some((
            "Sigue editando. Guardar pedirá confirmación para sobrescribir el cambio externo."
                .into(),
            false,
        ));
    }

    /// Autoriza explícitamente sobrescribir el cambio externo.
    pub(crate) fn allow_overwrite(&mut self) {
        self.overwrite = true;
        self.conflict = None;
        self.notice = Some(("Guardado autorizado sobre el cambio externo.".into(), false));
    }

    /// Registra el texto guardado y convierte el borrador en la base activa.
    pub(crate) fn commit_saved(&mut self, written: String) {
        self.base = self.draft.clone();
        self.baseline = Some(written);
        self.conflict = None;
        self.overwrite = false;
        self.errors.clear();
        self.notice = Some(("Configuración guardada.".into(), false));
    }

    fn adopt(&mut self, config: Config, source: Option<String>) {
        self.base = config.clone();
        self.draft = config;
        self.baseline = source;
        self.conflict = None;
        self.overwrite = false;
        self.errors.clear();
        self.profile_cursor = 0;
    }
}
