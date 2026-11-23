//! Píldoras de estado: color, icono y texto, nunca solo color.

use crate::gpui::prelude::FluentBuilder as _;
use crate::gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div};
use crate::theme;

use super::Tone;
use super::icon::{Icon, IconSize};

/// Estado visual de una pestaña o agente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabStatus {
    /// Proceso hijo en ejecución.
    Running,
    /// Agente trabajando.
    Working,
    /// Agente esperando al usuario.
    Attention,
    /// Agente terminado.
    Finished,
    /// Agente con error.
    Error,
    /// Agente inactivo.
    Idle,
    /// Proceso terminado.
    Stopped,
}

impl TabStatus {
    /// Etiqueta accesible.
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "En ejecución",
            Self::Working => "Trabajando",
            Self::Attention => "Atención",
            Self::Finished => "Terminado",
            Self::Error => "Error",
            Self::Idle => "Inactivo",
            Self::Stopped => "Detenido",
        }
    }

    /// Tono del estado.
    pub fn tone(self) -> Tone {
        match self {
            Self::Running | Self::Finished => Tone::Success,
            Self::Working => Tone::Accent,
            Self::Attention => Tone::Warning,
            Self::Error => Tone::Danger,
            Self::Idle | Self::Stopped => Tone::Neutral,
        }
    }

    /// `true` cuando el icono debe girar (trabajo en curso).
    pub const fn is_spinning(self) -> bool {
        matches!(self, Self::Working)
    }

    /// Icono que distingue el estado sin depender del color.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Running => crate::icons::PLAYER_PLAY,
            Self::Working => crate::icons::LOADER,
            Self::Attention => crate::icons::ALERT_TRIANGLE,
            Self::Finished => crate::icons::CHECK,
            Self::Error => crate::icons::CLOSE,
            Self::Idle => crate::icons::CIRCLE,
            Self::Stopped => crate::icons::STOP,
        }
    }
}

/// Píldora con tono y contenido.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct Pill {
    label: SharedString,
    tone: Tone,
    icon: Option<&'static str>,
}

impl Pill {
    /// Crea una píldora.
    pub fn new(label: impl Into<SharedString>, tone: Tone) -> Self {
        Self {
            label: label.into(),
            tone,
            icon: None,
        }
    }

    /// Icono a la izquierda.
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
}

impl RenderOnce for Pill {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let color = self.tone.color(ui);
        div()
            .flex()
            .items_center()
            .gap(metrics.spacing_xxs())
            .h(metrics.control_height_xs())
            .px(metrics.spacing_xs())
            .rounded(metrics.radius_full())
            .bg(color.opacity(0.18))
            .text_color(ui.text)
            .text_size(metrics.font_size * 0.6875)
            .when_some(self.icon, |this, icon| {
                this.child(Icon::new(icon).size(IconSize::Xs).color(color))
            })
            .child(self.label.clone())
    }
}

/// Píldora de estado con etiqueta e icono.
#[derive(IntoElement)]
#[expect(
    missing_debug_implementations,
    reason = "components con callbacks no derivan Debug"
)]
pub struct StatusPill {
    status: TabStatus,
}

impl StatusPill {
    /// Crea la píldora de un estado.
    pub fn new(status: TabStatus) -> Self {
        Self { status }
    }

    /// Estado representado.
    pub fn status(&self) -> TabStatus {
        self.status
    }
}

impl RenderOnce for StatusPill {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        Pill::new(self.status.label(), self.status.tone())
            .icon(self.status.icon())
            .render(window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_has_its_own_label_and_icon() {
        let states = [
            TabStatus::Running,
            TabStatus::Working,
            TabStatus::Attention,
            TabStatus::Finished,
            TabStatus::Error,
            TabStatus::Idle,
            TabStatus::Stopped,
        ];
        let mut labels = std::collections::HashSet::new();
        let mut icons = std::collections::HashSet::new();
        for state in states {
            assert!(!state.label().is_empty());
            assert!(labels.insert(state.label()), "etiqueta duplicada");
            assert!(icons.insert(state.icon()), "icono duplicado");
        }
        assert_eq!(labels.len(), states.len());
        assert_eq!(icons.len(), states.len());
    }
}
