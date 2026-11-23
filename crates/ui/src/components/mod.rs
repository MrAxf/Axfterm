//! Componentes propios de Axfterm.
//!
//! Son deliberadamente pequeños: composición de `div`/`svg` con los tokens y
//! métricas de `axf_ui`. Los componentes complejos (input, menú contextual,
//! diálogo, scrollbar) se proyectan desde gpui-kit con el mismo tema.

pub mod button;
pub mod empty;
pub mod icon;
pub mod input;
pub mod kbd;
pub mod menu;
pub mod modal;
pub mod pill;
pub mod tab;
pub mod toast;
pub mod toggle;
pub mod tooltip;

use crate::gpui::{App, ClickEvent, Hsla, Pixels, Window};
use crate::metrics::UiMetrics;
use crate::theme::UiTokens;

/// Callback de una acción de componente.
pub type ClickHandler = std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Callback de un toggle con el nuevo valor.
pub type ToggleHandler = std::rc::Rc<dyn Fn(&bool, &mut Window, &mut App)>;

pub use button::{Button, ButtonVariant, IconButton};
pub use empty::EmptyState;
pub use icon::{Icon, IconSize, ProfileIcon, ProfileIconSource};
pub use input::TextField;
pub use kbd::Kbd;
pub use menu::{Menu, MenuItem};
pub use modal::Modal;
pub use pill::{Pill, StatusPill, TabStatus};
pub use tab::Tab;
pub use toast::Toast;
pub use toggle::Toggle;
pub use tooltip::text_tooltip;

/// Tamaño de un control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    /// Chip o icon-button compacto.
    Xs,
    /// Botón pequeño.
    Sm,
    /// Botón por defecto.
    #[default]
    Md,
    /// Botón grande.
    Lg,
}

impl ControlSize {
    /// Altura del control.
    pub fn height(self, metrics: UiMetrics) -> Pixels {
        match self {
            Self::Xs => metrics.control_height_xs(),
            Self::Sm => metrics.control_height_sm(),
            Self::Md => metrics.control_height_md(),
            Self::Lg => metrics.control_height_lg(),
        }
    }

    /// Icono adecuado al tamaño.
    pub fn icon_size(self) -> IconSize {
        match self {
            Self::Xs => IconSize::Xs,
            Self::Sm => IconSize::Sm,
            Self::Md => IconSize::Md,
            Self::Lg => IconSize::Lg,
        }
    }

    /// Multiplicador del tamaño de fuente base.
    pub fn text_scale(self) -> f32 {
        match self {
            Self::Xs => 0.6875,
            Self::Sm => 0.75,
            Self::Md => 0.8125,
            Self::Lg => 0.875,
        }
    }

    /// Padding horizontal.
    pub fn padding_x(self, metrics: UiMetrics) -> Pixels {
        match self {
            Self::Xs => metrics.spacing_xs(),
            Self::Sm => metrics.spacing_sm(),
            Self::Md => metrics.spacing_md(),
            Self::Lg => metrics.spacing_lg(),
        }
    }
}

/// Tono semántico de un control o píldora.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    /// Neutro.
    #[default]
    Neutral,
    /// Acento.
    Accent,
    /// Error.
    Danger,
    /// Advertencia.
    Warning,
    /// Éxito.
    Success,
    /// Información.
    Info,
}

impl Tone {
    /// Color de fondo/símbolo.
    pub fn color(self, ui: UiTokens) -> Hsla {
        match self {
            Self::Neutral => ui.muted,
            Self::Accent => ui.accent,
            Self::Danger => ui.danger,
            Self::Warning => ui.warning,
            Self::Success => ui.success,
            Self::Info => ui.info,
        }
    }

    /// Color de texto legible sobre `color(ui)` para badges sólidos.
    pub fn text(self, ui: UiTokens) -> Hsla {
        match self {
            Self::Neutral => ui.background,
            Self::Accent => ui.accent_text,
            Self::Danger => ui.danger_text,
            Self::Warning => ui.warning_text,
            Self::Success => ui.success_text,
            Self::Info => ui.info_text,
        }
    }
}
