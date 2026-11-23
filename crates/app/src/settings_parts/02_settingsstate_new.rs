impl SettingsState {
    /// Abre un borrador a partir de la configuración activa.
    pub(crate) fn new(base: Config, baseline: Option<String>) -> Self {
        Self {
            section: SettingsSection::Appearance,
            draft: base.clone(),
            base,
            baseline,
            errors: Vec::new(),
            notice: None,
            conflict: None,
            overwrite: false,
            profile_cursor: 0,
            shortcut_cursor: 0,
            shortcut_query: String::new(),
            capturing: None,
            detect: DetectState::Idle,
            pending_delete: None,
            pending_defaults: false,
        }
    }

    /// `true` si el borrador difiere de la configuración con la que se abrió.
    pub(crate) fn is_dirty(&self) -> bool {
        self.draft != self.base
    }

    /// Cambia de sección y reinicia los cursores dependientes.
    pub(crate) fn set_section(&mut self, section: SettingsSection) {
        if self.section == section {
            return;
        }
        self.section = section;
        self.errors.clear();
        self.capturing = None;
        self.pending_delete = None;
        self.pending_defaults = false;
        if section != SettingsSection::Profiles {
            self.profile_cursor = 0;
        }
        if section != SettingsSection::Shortcuts {
            self.shortcut_cursor = 0;
        }
    }

    /// Mueve la sección `delta` posiciones.
    pub(crate) fn move_section(&mut self, delta: isize) {
        self.set_section(self.section.next(delta));
    }

    // --- Apariencia y métricas -----------------------------------------

    /// Fija la apariencia global.
    pub(crate) fn set_appearance(&mut self, appearance: Appearance) {
        self.draft.appearance = appearance;
    }

    /// Fija el tema; la variante del catálogo manda sobre la apariencia.
    pub(crate) fn set_theme(&mut self, name: &str, dark: bool) {
        self.draft.ui.theme = name.to_owned();
        self.draft.appearance = if dark {
            Appearance::Dark
        } else {
            Appearance::Light
        };
    }

    /// Familia de la UI; `None` deja la cadena del sistema.
    pub(crate) fn set_ui_font(&mut self, family: Option<String>) {
        self.draft.ui.font_family = family;
    }

    /// Familia de la terminal.
    pub(crate) fn set_terminal_font(&mut self, family: Option<String>) {
        self.draft.terminal.font_family = family;
    }

    /// Tamaño base de la UI (11–24, paso 1).
    pub(crate) fn step_ui_font_size(&mut self, delta: f32) {
        self.draft.ui.font_size = step(self.draft.ui.font_size, delta, 1.0, 11.0, 24.0);
    }

    /// Escala de la UI (0.8–2.0, paso 0.05).
    pub(crate) fn step_ui_scale(&mut self, delta: f32) {
        self.draft.ui.scale = step(self.draft.ui.scale, delta, 0.05, 0.8, 2.0);
    }

    /// Tamaño de la terminal (8–32, paso 1).
    pub(crate) fn step_terminal_font_size(&mut self, delta: f32) {
        self.draft.terminal.font_size = step(self.draft.terminal.font_size, delta, 1.0, 8.0, 32.0);
    }

    /// Altura de línea (1.0–2.5, paso 0.05).
    pub(crate) fn step_line_height(&mut self, delta: f32) {
        self.draft.terminal.line_height =
            step(self.draft.terminal.line_height, delta, 0.05, 1.0, 2.5);
    }

    // --- Perfiles ------------------------------------------------------

    /// Perfiles en el orden actual del borrador.
    pub(crate) fn profiles(&self) -> &[Profile] {
        &self.draft.profiles
    }

    /// Perfil actualmente seleccionado en la lista.
    pub(crate) fn selected_profile(&self) -> Option<&Profile> {
        self.draft.profiles.get(self.profile_cursor)
    }

    /// Añade un perfil vacío con id único y lo selecciona.
    pub(crate) fn add_profile(&mut self) -> ProfileId {
        let id = self.unique_profile_id("perfil");
        let name = format!("Perfil {}", self.draft.profiles.len() + 1);
        let command = axf_config::fallback_profile().command;
        self.draft
            .profiles
            .push(Profile::new(id.clone(), name, command));
        self.profile_cursor = self.draft.profiles.len() - 1;
        id
    }

    /// Aplica los campos editados a un perfil, sin cambiar su id.
    pub(crate) fn update_profile(
        &mut self,
        id: &ProfileId,
        name: String,
        command: String,
        icon: Option<PathBuf>,
    ) -> Result<(), String> {
        let Some(profile) = self
            .draft
            .profiles
            .iter_mut()
            .find(|profile| &profile.id == id)
        else {
            return Err(format!("El perfil `{id}` ya no existe."));
        };
        profile.name = name;
        profile.command = command;
        profile.icon = icon;
        Ok(())
    }

    /// Duplica el perfil indicado al final de la lista.
    pub(crate) fn duplicate_profile(&mut self, id: &ProfileId) -> Result<ProfileId, String> {
        let Some(profile) = self
            .draft
            .profiles
            .iter()
            .find(|profile| &profile.id == id)
            .cloned()
        else {
            return Err(format!("El perfil `{id}` ya no existe."));
        };
        let new_id = self.unique_profile_id(&format!("{}-copia", profile.id));
        let mut copy = profile;
        copy.id = new_id.clone();
        copy.name = format!("{} (copia)", copy.name);
        self.draft.profiles.push(copy);
        self.profile_cursor = self.draft.profiles.len() - 1;
        Ok(new_id)
    }

    /// Borra un perfil si las reglas lo permiten.
    ///
    /// No se puede borrar el último ni el predeterminado. El uso por pestañas
    /// abiertas se comprueba en el shell antes de llamar aquí.
    pub(crate) fn delete_profile(&mut self, id: &ProfileId) -> Result<(), String> {
        if self.draft.profiles.len() <= 1 {
            return Err("Debe quedar al menos un perfil.".into());
        }
        if &self.draft.terminal.default_profile == id {
            return Err("Elige otro perfil predeterminado antes de borrar este.".into());
        }
        let Some(index) = self
            .draft
            .profiles
            .iter()
            .position(|profile| &profile.id == id)
        else {
            return Err(format!("El perfil `{id}` ya no existe."));
        };
        self.draft.profiles.remove(index);
        self.draft
            .agents
            .preferred_profiles
            .retain(|_, profile| profile != id);
        self.profile_cursor = self.profile_cursor.min(self.draft.profiles.len() - 1);
        Ok(())
    }

    /// Mueve un perfil una posición en el orden de la lista.
    pub(crate) fn move_profile(&mut self, id: &ProfileId, delta: isize) -> bool {
        let Some(index) = self
            .draft
            .profiles
            .iter()
            .position(|profile| &profile.id == id)
        else {
            return false;
        };
        let target = index as isize + delta;
        if target < 0 || target >= self.draft.profiles.len() as isize {
            return false;
        }
        self.draft.profiles.swap(index, target as usize);
        self.profile_cursor = target as usize;
        true
    }

    /// Marca un perfil como predeterminado.
    pub(crate) fn set_default_profile(&mut self, id: &ProfileId) -> Result<(), String> {
        if !self.draft.profiles.iter().any(|profile| &profile.id == id) {
            return Err(format!("El perfil `{id}` no existe."));
        }
        self.draft.terminal.default_profile = id.clone();
        Ok(())
    }

    /// Añade perfiles detectados sin duplicar id ni comando.
    pub(crate) fn merge_detected(&mut self, detected: Vec<Profile>) -> usize {
        let mut ids: HashSet<String> = self
            .draft
            .profiles
            .iter()
            .map(|profile| profile.id.as_str().to_owned())
            .collect();
        let mut commands: HashSet<String> = self
            .draft
            .profiles
            .iter()
            .map(|profile| normalized_command(&profile.command))
            .collect();
        let mut added = 0;
        for profile in detected {
            if ids.contains(profile.id.as_str())
                || commands.contains(&normalized_command(&profile.command))
            {
                continue;
            }
            ids.insert(profile.id.as_str().to_owned());
            commands.insert(normalized_command(&profile.command));
            self.draft.profiles.push(profile);
            added += 1;
        }
        added
    }

    fn unique_profile_id(&self, base: &str) -> ProfileId {
        let base = slugify(base);
        if !self
            .draft
            .profiles
            .iter()
            .any(|profile| profile.id.as_str() == base)
        {
            return ProfileId::new(base);
        }
        for suffix in 2..1000 {
            let candidate = format!("{base}-{suffix}");
            if !self
                .draft
                .profiles
                .iter()
                .any(|profile| profile.id.as_str() == candidate)
            {
                return ProfileId::new(candidate);
            }
        }
        ProfileId::new(format!("{base}-{}", self.draft.profiles.len() + 1))
    }

    // --- Fuentes -------------------------------------------------------

    /// Añade un origen de fuentes deduplicado; devuelve `false` si ya estaba.
    pub(crate) fn add_font_source(&mut self, path: &Path, config_dir: &Path) -> bool {
        let stored = relative_source(path, config_dir);
        if self.draft.fonts.sources.contains(&stored) {
            return false;
        }
        if self.draft.fonts.sources.len() >= 16 {
            return false;
        }
        self.draft.fonts.sources.push(stored);
        true
    }

    /// Quita un origen por índice.
    pub(crate) fn remove_font_source(&mut self, index: usize) {
        if index < self.draft.fonts.sources.len() {
            self.draft.fonts.sources.remove(index);
        }
    }

    // --- Agentes -------------------------------------------------------

    /// Activa o desactiva la detección.
    pub(crate) fn set_agents_enabled(&mut self, enabled: bool) {
        self.draft.agents.enabled = enabled;
    }

    /// Intervalo de escaneo (1000–2000, paso 100).
    pub(crate) fn step_scan_interval(&mut self, delta: f32) {
        self.draft.agents.scan_interval_ms = step(
            self.draft.agents.scan_interval_ms as f32,
            delta,
            100.0,
            1000.0,
            2000.0,
        ) as u64;
    }

    /// Activa o desactiva un aviso de agente.
    pub(crate) fn set_notify_finished(&mut self, enabled: bool) {
        self.draft.agents.notify_finished = enabled;
    }

    /// Aviso de error.
    pub(crate) fn set_notify_error(&mut self, enabled: bool) {
        self.draft.agents.notify_error = enabled;
    }

    /// Aviso de atención.
    pub(crate) fn set_notify_attention(&mut self, enabled: bool) {
        self.draft.agents.notify_attention = enabled;
    }

    /// Agentes con perfil preferido editable.
    pub(crate) const AGENT_KEYS: &'static [&'static str] = &["opencode", "copilot"];

    /// Perfil preferido de un agente.
    pub(crate) fn preferred_profile(&self, agent: &str) -> Option<&ProfileId> {
        self.draft.agents.preferred_profiles.get(agent)
    }

    /// Cicla el perfil preferido entre «ninguno» y los perfiles actuales.
    pub(crate) fn cycle_preferred_profile(&mut self, agent: &str, delta: isize) {
        let mut options: Vec<Option<ProfileId>> = vec![None];
        options.extend(
            self.draft
                .profiles
                .iter()
                .map(|profile| Some(profile.id.clone())),
        );
        let current = options
            .iter()
            .position(|option| option.as_ref() == self.preferred_profile(agent))
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(options.len() as isize) as usize;
        match options.swap_remove(next) {
            Some(id) => {
                self.draft
                    .agents
                    .preferred_profiles
                    .insert(agent.to_owned(), id);
            }
            None => {
                self.draft.agents.preferred_profiles.remove(agent);
            }
        }
    }

    // --- Atajos --------------------------------------------------------

    /// Acciones visibles en la sección, filtradas por la consulta.
    pub(crate) fn shortcut_actions(&self) -> Vec<&'static crate::actions::ActionMeta> {
        let query = self.shortcut_query.trim();
        crate::actions::all()
            .filter(|action| action.enabled)
            .filter(|action| {
                query.is_empty()
                    || crate::fuzzy::score(query, action.label).is_some()
                    || crate::fuzzy::score(query, action.id).is_some()
                    || action
                        .keywords
                        .iter()
                        .any(|keyword| crate::fuzzy::score(query, keyword).is_some())
            })
            .collect()
    }

    /// `true` si la acción tiene un atajo efectivo.
    #[cfg(test)]
    pub(crate) fn has_binding(&self, action: &str) -> bool {
        Keymap::resolve(&self.draft.keymap)
            .chords_for(action)
            .is_some()
    }

    /// Chords visibles de una acción.
    #[cfg(test)]
    pub(crate) fn chords_for(&self, action: &str) -> Option<String> {
        Keymap::resolve(&self.draft.keymap).chords_for(action)
    }

    /// Asigna un atajo, sustituyendo los anteriores de esa acción.
    pub(crate) fn set_binding(&mut self, action_id: &str, chord: &str) -> Result<(), String> {
        let Some(meta) = crate::actions::by_id(action_id) else {
            return Err(format!("La acción `{action_id}` no existe."));
        };
        Keymap::validate_for(action_id, chord)?;
        let normalized = normalize_chord(chord);
        let resolved = Keymap::resolve(&self.draft.keymap);
        if let Some(owner) = resolved.chord_owner(&normalized)
            && owner != action_id
        {
            let label = crate::actions::by_id(owner)
                .map(|meta| meta.label)
                .unwrap_or(owner);
            return Err(format!(
                "`{}` ya está asignado a «{label}»; quita ese atajo antes.",
                crate::keys::format_chord(&normalized)
            ));
        }
        let mut candidate = self.draft.keymap.clone();
        remove_action_entries(&mut candidate, action_id);
        let is_default = normalize_chord(meta.default_chord) == normalized;
        if !is_default {
            candidate.insert(
                normalized.clone(),
                toml::Value::String(action_id.to_owned()),
            );
        }
        set_unbind(&mut candidate, action_id, !is_default);
        let resolved = Keymap::resolve(&candidate);
        if !resolved.errors.is_empty() {
            return Err(resolved.errors.join(" "));
        }
        self.draft.keymap = candidate;
        self.capturing = None;
        Ok(())
    }
}
