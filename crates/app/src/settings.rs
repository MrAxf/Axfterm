//! Modelo del panel de ajustes (ADR 0007).
//!
//! El panel trabaja sobre un borrador de `Config` independiente del activo.
//! Este módulo concentra las operaciones, la validación y la política de
//! conflictos; el shell solo pinta y traduce eventos. Nada aquí toca disco.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use axf_config::{Appearance, Config, ConfigError, Profile, ProfileId};

use crate::keymap::Keymap;

/// Secciones del panel, en el orden de la barra lateral.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SettingsSection {
    Appearance,
    Terminal,
    Profiles,
    Fonts,
    Shortcuts,
    Agents,
    Advanced,
}

impl SettingsSection {
    /// Orden de navegación y de `Ctrl+1..7`.
    pub(crate) const ALL: &'static [Self] = &[
        Self::Appearance,
        Self::Terminal,
        Self::Profiles,
        Self::Fonts,
        Self::Shortcuts,
        Self::Agents,
        Self::Advanced,
    ];

    /// Etiqueta de la barra lateral.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Apariencia",
            Self::Terminal => "Terminal",
            Self::Profiles => "Perfiles",
            Self::Fonts => "Fuentes",
            Self::Shortcuts => "Atajos",
            Self::Agents => "Agentes",
            Self::Advanced => "Avanzado",
        }
    }

    /// Frase de ayuda de la cabecera.
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Appearance => "Tema, variante clara u oscura y escala de la interfaz.",
            Self::Terminal => "Fuente monoespaciada y métricas de la terminal.",
            Self::Profiles => "Qué programas se pueden abrir como terminal.",
            Self::Fonts => "Archivos y carpetas con fuentes adicionales.",
            Self::Shortcuts => "Atajos de la aplicación y conflictos con la terminal.",
            Self::Agents => "Detección local de opencode y Copilot.",
            Self::Advanced => "Rutas, versión de la configuración y valores por defecto.",
        }
    }

    /// Icono Tabler de la sección.
    pub(crate) fn icon(self) -> &'static str {
        match self {
            Self::Appearance => axf_ui::icons::PALETTE,
            Self::Terminal => axf_ui::icons::TERMINAL,
            Self::Profiles => axf_ui::icons::USER,
            Self::Fonts => axf_ui::icons::TYPOGRAPHY,
            Self::Shortcuts => axf_ui::icons::KEYBOARD,
            Self::Agents => axf_ui::icons::BRAND_OPENCODE,
            Self::Advanced => axf_ui::icons::SETTINGS,
        }
    }

    /// Índice en `ALL`.
    pub(crate) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or(0)
    }

    /// Siguiente sección con vuelta.
    pub(crate) fn next(self, delta: isize) -> Self {
        let count = Self::ALL.len() as isize;
        let index = (self.index() as isize + delta).rem_euclid(count) as usize;
        Self::ALL[index]
    }
}

/// Estado del descubrimiento de terminales.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum DetectState {
    /// Nunca se ha lanzado o se descartó el resultado.
    #[default]
    Idle,
    /// Descubrimiento en curso.
    Running,
    /// Perfiles nuevos listos para añadir.
    Ready(Vec<Profile>),
}

/// Cambio externo pendiente de resolver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Conflict {
    /// Texto actual de `config.toml`.
    pub(crate) source: String,
    /// Resumen legible del cambio (para el aviso).
    pub(crate) summary: String,
}

/// Resultado de aplicar un cambio externo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExternalOutcome {
    /// Es la escritura propia del panel; el watcher no debe hacer nada.
    Ignored,
    /// El borrador estaba limpio y se recargó.
    Reloaded,
    /// Hay cambios sin guardar: se pidió decisión al usuario.
    Conflict,
    /// El archivo externo no es válido; se conserva el borrador.
    Invalid(String),
}

/// Perfil pendiente de borrado cuando lo usa alguna pestaña.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingDelete {
    pub(crate) id: ProfileId,
    pub(crate) name: String,
    pub(crate) in_use: usize,
}

/// Borrador y estado de interacción del panel.
#[derive(Clone, Debug)]
pub(crate) struct SettingsState {
    pub(crate) section: SettingsSection,
    /// Configuración activa cuando se abrió el panel.
    pub(crate) base: Config,
    /// Configuración que se está editando.
    pub(crate) draft: Config,
    /// Texto de `config.toml` al abrir o tras guardar.
    pub(crate) baseline: Option<String>,
    pub(crate) errors: Vec<String>,
    pub(crate) notice: Option<(String, bool)>,
    pub(crate) conflict: Option<Conflict>,
    /// El usuario pidió guardar sobre un cambio externo.
    pub(crate) overwrite: bool,
    pub(crate) profile_cursor: usize,
    pub(crate) shortcut_cursor: usize,
    pub(crate) shortcut_query: String,
    /// Acción esperando la siguiente pulsación de tecla.
    pub(crate) capturing: Option<String>,
    pub(crate) detect: DetectState,
    pub(crate) pending_delete: Option<PendingDelete>,
    /// Confirmación de restauración de valores por defecto.
    pub(crate) pending_defaults: bool,
}

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

fn step(value: f32, delta: f32, increment: f32, min: f32, max: f32) -> f32 {
    let stepped = (value + delta * increment).clamp(min, max);
    let snapped = (stepped / increment).round() * increment;
    // Redondeo decimal para que 1.0500001 sea 1.05 y la validación y la UI
    // muestren el mismo número.
    (snapped * 1000.0).round() / 1000.0
}

fn normalized_command(command: &str) -> String {
    command.trim().to_ascii_lowercase()
}

fn relative_source(path: &Path, config_dir: &Path) -> PathBuf {
    path.strip_prefix(config_dir)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() || character == '_' {
            slug.push(character);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_owned();
    let mut slug = if slug.is_empty() {
        "perfil".to_owned()
    } else {
        slug
    };
    if !slug
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
    {
        slug.insert(0, 'p');
    }
    slug.chars().take(48).collect()
}

fn normalize_chord(chord: &str) -> String {
    chord
        .trim()
        .to_ascii_lowercase()
        .replace('+', "-")
        .replace(' ', "")
}

/// Quita las entradas explícitas de una acción en `[keymap]` y sus contextos.
fn remove_action_entries(table: &mut toml::Table, action_id: &str) {
    let flat: Vec<String> = table
        .iter()
        .filter(|(_, value)| value.as_str() == Some(action_id))
        .map(|(key, _)| key.clone())
        .collect();
    for key in flat {
        table.remove(&key);
    }
    let sections: Vec<String> = table
        .iter()
        .filter(|(_, value)| value.is_table())
        .map(|(key, _)| key.clone())
        .collect();
    for key in sections {
        if let Some(toml::Value::Table(section)) = table.get_mut(&key) {
            section.retain(|_, value| value.as_str() != Some(action_id));
        }
    }
    table.retain(|_, value| {
        value
            .as_table()
            .map(|section| !section.is_empty())
            .unwrap_or(true)
    });
}

/// Añade o quita una acción de la lista `unbind`.
fn set_unbind(table: &mut toml::Table, action_id: &str, enabled: bool) {
    let mut items: Vec<toml::Value> = table
        .get("unbind")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    items.retain(|value| value.as_str() != Some(action_id));
    if enabled {
        items.push(toml::Value::String(action_id.to_owned()));
    }
    if items.is_empty() {
        table.remove("unbind");
    } else {
        table.insert("unbind".into(), toml::Value::Array(items));
    }
}

#[cfg(test)]
mod tests {
    use axf_config::{AgentsConfig, Profile};

    use super::*;

    fn base() -> Config {
        Config {
            profiles: vec![
                Profile::new("a", "A", "cmd /Q"),
                Profile::new("b", "B", "pwsh -NoLogo"),
            ],
            terminal: axf_config::TerminalConfig {
                default_profile: ProfileId::new("a"),
                ..axf_config::TerminalConfig::default()
            },
            ..Config::default()
        }
    }

    fn state() -> SettingsState {
        SettingsState::new(base(), None)
    }

    #[test]
    fn sections_cycle_in_order() {
        assert_eq!(
            SettingsSection::Appearance.next(1),
            SettingsSection::Terminal
        );
        assert_eq!(
            SettingsSection::Appearance.next(-1),
            SettingsSection::Advanced
        );
        assert_eq!(SettingsSection::ALL.len(), 7);
    }

    #[test]
    fn numeric_steps_clamp_and_snap() {
        let mut state = state();
        state.step_ui_font_size(100.0);
        assert_eq!(state.draft.ui.font_size, 24.0);
        state.step_ui_scale(-100.0);
        assert_eq!(state.draft.ui.scale, 0.8);
        state.step_line_height(1.0);
        assert_eq!(state.draft.terminal.line_height, 1.05);
        state.step_scan_interval(-99.0);
        assert_eq!(state.draft.agents.scan_interval_ms, 1000);
    }

    #[test]
    fn add_duplicate_and_delete_follow_profile_rules() {
        let mut state = state();
        let id = state.add_profile();
        assert_eq!(state.profiles().len(), 3);
        assert_eq!(
            state.selected_profile().map(|profile| &profile.id),
            Some(&id)
        );

        let copy = state.duplicate_profile(&id).expect("duplica");
        assert_eq!(state.profiles().len(), 4);
        assert_eq!(
            state.profiles().last().map(|profile| profile.name.as_str()),
            Some("Perfil 3 (copia)")
        );

        let error = state
            .delete_profile(&ProfileId::new("a"))
            .expect_err("el predeterminado está protegido");
        assert!(error.contains("predeterminado"));

        state
            .set_default_profile(&ProfileId::new("b"))
            .expect("mueve el default");
        state.delete_profile(&ProfileId::new("a")).expect("borra");
        assert_eq!(state.profiles().len(), 3);
        assert!(
            state
                .profiles()
                .iter()
                .all(|profile| profile.id != ProfileId::new("a"))
        );
        let _ = copy;
    }

    #[test]
    fn deleting_a_profile_removes_its_agent_preference() {
        let mut state = state();
        state
            .draft
            .agents
            .preferred_profiles
            .insert("opencode".into(), ProfileId::new("b"));
        state
            .set_default_profile(&ProfileId::new("a"))
            .expect("default a");
        state.delete_profile(&ProfileId::new("b")).expect("borra b");
        assert!(state.draft.agents.preferred_profiles.is_empty());
    }

    #[test]
    fn last_profile_cannot_be_deleted() {
        let mut state = state();
        state.draft.profiles.truncate(1);
        state.draft.terminal.default_profile = ProfileId::new("a");
        let error = state
            .delete_profile(&ProfileId::new("a"))
            .expect_err("el último perfil no se puede borrar");
        assert!(error.contains("al menos un perfil"));
    }

    #[test]
    fn merge_detected_deduplicates_id_and_command() {
        let mut state = state();
        let added = state.merge_detected(vec![
            Profile::new("a", "Repetido", "otro"),
            Profile::new("c", "Mismo comando", "CMD /q"),
            Profile::new("nu", "Nu", "nu"),
        ]);
        assert_eq!(added, 1);
        assert_eq!(state.profiles().last().map(|p| p.id.as_str()), Some("nu"));
    }

    #[test]
    fn rebind_replaces_chords_and_marks_unbind() {
        let mut state = state();
        state
            .set_binding("tab.new", "ctrl-alt-shift-n")
            .expect("rebind");
        let chords = state.chords_for("tab.new").expect("sigue teniendo atajo");
        assert_eq!(chords, "Ctrl+Alt+Shift+N");
        let unbind = state
            .draft
            .keymap
            .get("unbind")
            .and_then(toml::Value::as_array);
        assert!(
            unbind.is_some_and(|items| items.iter().any(|item| item.as_str() == Some("tab.new")))
        );
        assert_eq!(
            state
                .draft
                .keymap
                .get("ctrl-alt-shift-n")
                .and_then(toml::Value::as_str),
            Some("tab.new")
        );
    }

    #[test]
    fn assigning_a_chord_of_another_action_is_explained() {
        let mut state = state();
        let error = state
            .set_binding("tab.close", "ctrl-shift-t")
            .expect_err("el chord de Nueva terminal está ocupado");
        assert!(error.contains("Nueva terminal") || error.contains("tab.new"));
    }

    #[test]
    fn reserved_terminal_keys_are_rejected() {
        let mut state = state();
        assert!(state.set_binding("tab.new", "ctrl-d").is_err());
    }

    #[test]
    fn unbind_and_reset_remove_the_action_shortcut() {
        let mut state = state();
        state.unbind_action("tab.new").expect("unbind");
        assert!(!state.has_binding("tab.new"));
        assert!(
            state
                .draft
                .keymap
                .get("unbind")
                .and_then(toml::Value::as_array)
                .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("tab.new")))
        );
        state.reset_action("tab.new").expect("reset");
        assert!(state.has_binding("tab.new"));
        assert!(state.draft.keymap.is_empty());
    }

    #[test]
    fn preferred_profile_cycles_through_none_and_profiles() {
        let mut state = state();
        assert_eq!(state.preferred_profile("opencode"), None);
        state.cycle_preferred_profile("opencode", 1);
        assert_eq!(
            state.preferred_profile("opencode"),
            Some(&ProfileId::new("a"))
        );
        state.cycle_preferred_profile("opencode", 1);
        assert_eq!(
            state.preferred_profile("opencode"),
            Some(&ProfileId::new("b"))
        );
        state.cycle_preferred_profile("opencode", 1);
        assert_eq!(state.preferred_profile("opencode"), None);
    }

    #[test]
    fn invalid_agent_profile_blocks_validation() {
        let mut state = state();
        state.draft.agents = AgentsConfig {
            preferred_profiles: [("opencode".into(), ProfileId::new("missing"))]
                .into_iter()
                .collect(),
            ..AgentsConfig::default()
        };
        let errors = state.validate();
        assert!(
            errors
                .iter()
                .any(|error| error.contains("preferred_profiles"))
        );
    }

    #[test]
    fn restore_defaults_keeps_profiles_and_a_valid_default() {
        let mut state = state();
        state.set_theme("Monokai", true);
        state.draft.ui.font_size = 20.0;
        state.restore_defaults();
        assert_eq!(state.draft.profiles, base().profiles);
        assert_eq!(state.draft.ui.theme, "One Dark");
        assert_eq!(state.draft.appearance, Appearance::System);
        assert_eq!(state.draft.ui.font_size, 14.0);
        assert!(state.validate().is_empty());
    }

    #[test]
    fn external_change_reloads_a_clean_draft_and_conflicts_with_a_dirty_one() {
        let mut state = state();
        let mut external = base();
        external.ui.font_size = 18.0;
        let source = "schema_version = 2".to_owned();
        assert_eq!(
            state.external_change(source.clone(), Ok(external.clone())),
            ExternalOutcome::Reloaded
        );
        assert_eq!(state.draft.ui.font_size, 18.0);
        assert!(!state.is_dirty());

        state.draft.ui.font_size = 20.0;
        let mut newer = external.clone();
        newer.ui.font_size = 19.0;
        assert_eq!(
            state.external_change("otro texto".into(), Ok(newer)),
            ExternalOutcome::Conflict
        );
        assert!(state.conflict.is_some());
        assert!(state.is_dirty());
        state.allow_overwrite();
        assert!(state.overwrite);
        assert!(state.conflict.is_none());
    }

    #[test]
    fn own_write_is_ignored_by_the_watcher() {
        let mut state = state();
        let written = "contenido escrito".to_owned();
        state.commit_saved(written.clone());
        assert_eq!(
            state.external_change(written, Ok(base())),
            ExternalOutcome::Ignored
        );
    }

    #[test]
    fn invalid_external_file_keeps_the_draft_and_explains() {
        let mut state = state();
        let outcome = state.external_change(
            "roto".into(),
            Err(ConfigError::Field {
                field: "ui.font_size".into(),
                message: "fuera de rango".into(),
            }),
        );
        assert!(matches!(outcome, ExternalOutcome::Invalid(_)));
        assert!(state.notice.as_ref().is_some_and(|(_, error)| *error));
        assert_eq!(state.draft.ui.font_size, 14.0);
    }

    #[test]
    fn font_sources_are_stored_relative_to_config() {
        let mut state = state();
        let config_dir = PathBuf::from("C:/config");
        assert!(state.add_font_source(&config_dir.join("fonts").join("nerd.ttf"), &config_dir));
        assert_eq!(
            state.draft.fonts.sources,
            vec![PathBuf::from("fonts").join("nerd.ttf")]
        );
        assert!(!state.add_font_source(&config_dir.join("fonts").join("nerd.ttf"), &config_dir));
        state.remove_font_source(0);
        assert!(state.draft.fonts.sources.is_empty());
    }
}
