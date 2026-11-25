use crate::keymap::Keymap;
use axf_config::{Appearance, Config, ConfigError, Profile, ProfileId};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SettingsSection {
    Appearance,
    Profiles,
    Shortcuts,
    Agents,
}
impl SettingsSection {
    /// Orden de navegación y de `Ctrl+Tab`.
    pub(crate) const ALL: &'static [Self] = &[
        Self::Appearance,
        Self::Profiles,
        Self::Shortcuts,
        Self::Agents,
    ];

    /// Etiqueta de la barra lateral.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Apariencia",
            Self::Profiles => "Perfiles",
            Self::Shortcuts => "Atajos",
            Self::Agents => "Agentes",
        }
    }

    /// Frase de ayuda de la cabecera.
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Appearance => "Tema, tipografía y escala de la aplicación y la terminal.",
            Self::Profiles => "Qué programas se pueden abrir como terminal.",
            Self::Shortcuts => "Atajos de la aplicación y conflictos con la terminal.",
            Self::Agents => "Detección local de OpenCode y Copilot.",
        }
    }

    /// Icono Tabler de la sección.
    pub(crate) fn icon(self) -> &'static str {
        match self {
            Self::Appearance => axf_ui::icons::PALETTE,
            Self::Profiles => axf_ui::icons::USER,
            Self::Shortcuts => axf_ui::icons::KEYBOARD,
            Self::Agents => axf_ui::icons::BRAND_OPENCODE,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Conflict {
    /// Texto actual de `config.toml`.
    pub(crate) source: String,
    /// Resumen legible del cambio (para el aviso).
    pub(crate) summary: String,
}
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingDelete {
    pub(crate) id: ProfileId,
    pub(crate) name: String,
    pub(crate) in_use: usize,
}
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
    pub(crate) shortcut_query: String,
    /// Acción esperando la siguiente pulsación de tecla.
    pub(crate) capturing: Option<String>,
    pub(crate) detect: DetectState,
    pub(crate) pending_delete: Option<PendingDelete>,
    /// Confirmación de restauración de valores por defecto.
    pub(crate) pending_defaults: bool,
}
