//! Estado global del sistema visual y proyección al tema de gpui-kit.

use std::rc::Rc;
use std::sync::OnceLock;

use crate::fonts::FontSet;
use crate::gpui::component::theme::{
    Theme as GpuiTheme, ThemeConfig, ThemeConfigColors, ThemeMode,
};
use crate::gpui::{App, Global, Pixels, SharedString, px};
use crate::metrics::UiMetrics;

use super::builtin;
use super::color::{Rgb, hsla_hex};
use super::document::{Theme, ThemeVariant};

/// Todo lo que necesita un componente para pintarse con el tema activo.
#[derive(Clone, Debug)]
pub struct ThemeState {
    /// Tema resuelto (UI + terminal).
    pub theme: Theme,
    /// Métricas derivadas del tamaño base y la escala.
    pub metrics: UiMetrics,
    /// Fuentes resueltas.
    pub fonts: FontSet,
    /// Tamaño base de la UI en píxeles lógicos.
    pub ui_font_size: Pixels,
    /// Tamaño de la fuente del terminal.
    pub mono_font_size: Pixels,
    /// Altura de línea explícita del terminal; `0` usa la natural.
    pub terminal_line_height: Pixels,
}

impl Global for ThemeState {}

impl ThemeState {
    /// Estado inicial con el tema One Dark y fuentes del sistema.
    pub fn initial() -> Self {
        let theme = builtin::builtin("One Dark").unwrap_or_else(Theme::fallback);
        let metrics = UiMetrics::scaled(14.0, 1.0, 1.0);
        Self {
            theme,
            metrics,
            fonts: FontSet::system(),
            ui_font_size: px(14.0),
            mono_font_size: px(13.0),
            terminal_line_height: px(0.0),
        }
    }

    /// Estado de emergencia si la app no instaló uno (tests, diagnósticos).
    pub fn fallback() -> Self {
        Self::initial()
    }

    /// Recalcula las métricas al cambiar el monitor o su escala.
    pub fn with_display_scale(&self, display_scale: f32) -> Self {
        let mut next = self.clone();
        next.metrics = next.metrics.with_display_scale(display_scale);
        next
    }
}

/// Estado activo o un fallback estático.
pub fn state(cx: &App) -> &ThemeState {
    static FALLBACK: OnceLock<ThemeState> = OnceLock::new();
    cx.try_global::<ThemeState>()
        .unwrap_or_else(|| FALLBACK.get_or_init(ThemeState::fallback))
}

/// Métricas activas.
pub fn metrics(cx: &App) -> UiMetrics {
    state(cx).metrics
}

/// Instala un estado y lo proyecta a gpui-kit y al terminal.
pub fn install(cx: &mut App, state: ThemeState) {
    apply_gpui(cx, &state);
    cx.set_global::<ThemeState>(state);
    cx.refresh_windows();
}

/// Inicializa el sistema visual si nadie lo hizo todavía.
pub fn init(cx: &mut App) {
    if !cx.has_global::<ThemeState>() {
        install(cx, ThemeState::initial());
    }
}

fn mode_for(variant: ThemeVariant) -> ThemeMode {
    match variant {
        ThemeVariant::Dark => ThemeMode::Dark,
        ThemeVariant::Light => ThemeMode::Light,
    }
}

fn color(value: crate::gpui::Hsla) -> Option<SharedString> {
    Some(SharedString::from(hsla_hex(value)))
}

/// Mezcla un color con el texto del tema para producir hover/pressed sin
/// depender de que el tema declare cada variante.
fn tinted(base: crate::gpui::Hsla, tint: crate::gpui::Hsla, amount: f32) -> Option<SharedString> {
    color(Rgb::from(base).mix(Rgb::from(tint), amount).to_hsla())
}

/// Proyecta el tema propio sobre gpui-kit para que sus componentes (input,
/// menú, diálogo, scrollbar) usen los mismos tokens.
pub fn apply_gpui(cx: &mut App, state: &ThemeState) {
    let mode = mode_for(state.theme.variant());
    let ui = state.theme.ui;

    let mut colors = ThemeConfigColors::default();
    colors.background = color(ui.background);
    colors.foreground = color(ui.text);
    colors.border = color(ui.border);
    colors.input = color(ui.border);
    colors.ring = color(ui.focus);
    colors.selection = color(ui.selection);
    colors.caret = color(ui.focus);

    colors.accent = color(ui.surface_hover);
    colors.accent_foreground = color(ui.text);
    colors.primary = color(ui.accent);
    colors.primary_foreground = color(ui.accent_text);
    colors.primary_hover = color(ui.accent_hover);
    colors.primary_active = color(ui.accent_active);
    colors.secondary = color(ui.surface_raised);
    colors.secondary_foreground = color(ui.text);
    colors.secondary_hover = color(ui.surface_hover);
    colors.secondary_active = color(ui.surface_active);
    colors.button = color(ui.surface_raised);
    colors.button_foreground = color(ui.text);
    colors.button_hover = color(ui.surface_hover);
    colors.button_active = color(ui.surface_active);
    colors.button_primary = color(ui.accent);
    colors.button_primary_foreground = color(ui.accent_text);
    colors.button_primary_hover = color(ui.accent_hover);
    colors.button_primary_active = color(ui.accent_active);
    colors.button_secondary = color(ui.surface_raised);
    colors.button_secondary_foreground = color(ui.text);
    colors.button_secondary_hover = color(ui.surface_hover);
    colors.button_secondary_active = color(ui.surface_active);
    colors.muted = color(ui.surface);
    colors.muted_foreground = color(ui.muted);

    colors.danger = color(ui.danger);
    colors.danger_foreground = color(ui.danger_text);
    colors.danger_hover = tinted(ui.danger, ui.text, 0.10);
    colors.danger_active = tinted(ui.danger, ui.text, 0.18);
    colors.warning = color(ui.warning);
    colors.warning_foreground = color(ui.warning_text);
    colors.warning_hover = tinted(ui.warning, ui.text, 0.10);
    colors.warning_active = tinted(ui.warning, ui.text, 0.18);
    colors.success = color(ui.success);
    colors.success_foreground = color(ui.success_text);
    colors.success_hover = tinted(ui.success, ui.text, 0.10);
    colors.success_active = tinted(ui.success, ui.text, 0.18);
    colors.info = color(ui.info);
    colors.info_foreground = color(ui.info_text);
    colors.info_hover = tinted(ui.info, ui.text, 0.10);
    colors.info_active = tinted(ui.info, ui.text, 0.18);

    colors.popover = color(ui.surface_raised);
    colors.popover_foreground = color(ui.text);
    colors.sidebar = color(ui.surface);
    colors.sidebar_foreground = color(ui.text);
    colors.sidebar_accent = color(ui.surface_hover);
    colors.sidebar_accent_foreground = color(ui.text);
    colors.sidebar_border = color(ui.border);
    colors.sidebar_primary = color(ui.accent);
    colors.sidebar_primary_foreground = color(ui.accent_text);

    colors.tab = color(ui.surface);
    colors.tab_bar = color(ui.surface);
    colors.tab_active = color(ui.background);
    colors.tab_active_foreground = color(ui.text);
    colors.tab_foreground = color(ui.muted);
    colors.tab_bar_segmented = color(ui.surface_raised);

    colors.title_bar = color(ui.surface);
    colors.title_bar_border = color(ui.border);
    colors.status_bar = color(ui.surface);
    colors.status_bar_border = color(ui.border);
    colors.window_border = color(ui.border);

    colors.group_box = color(ui.surface);
    colors.group_box_foreground = color(ui.text);
    colors.group_box_title_foreground = color(ui.muted);
    colors.accordion = color(ui.surface);
    colors.description_list_label = color(ui.surface);
    colors.description_list_label_foreground = color(ui.muted);

    colors.list = color(ui.surface);
    colors.list_hover = color(ui.surface_hover);
    colors.list_active = color(ui.surface_active);
    colors.list_active_border = color(ui.accent);
    colors.list_even = color(ui.surface);
    colors.list_head = color(ui.surface_raised);
    colors.list_highlight_foreground = color(ui.text);
    colors.table = color(ui.surface);
    colors.table_hover = color(ui.surface_hover);
    colors.table_active = color(ui.surface_active);
    colors.table_active_border = color(ui.accent);
    colors.table_even = color(ui.surface);
    colors.table_head = color(ui.surface_raised);
    colors.table_head_foreground = color(ui.muted);
    colors.table_foot = color(ui.surface);
    colors.table_foot_foreground = color(ui.muted);
    colors.table_row_border = color(ui.border);

    colors.scrollbar = color(ui.surface);
    colors.scrollbar_thumb = color(ui.border_strong);
    colors.scrollbar_thumb_hover = color(ui.muted);
    colors.slider_bar = color(ui.accent);
    colors.slider_thumb = color(ui.accent);
    colors.progress_bar = color(ui.accent);
    colors.switch = color(ui.switch_off);
    colors.switch_thumb = color(ui.text);
    colors.skeleton = color(ui.surface_raised);
    colors.link = color(ui.accent);
    colors.link_hover = color(ui.accent_hover);
    colors.link_active = color(ui.accent_active);
    colors.drop_target = color(ui.accent);
    colors.drag_border = color(ui.focus);
    colors.overlay = color(ui.overlay);

    let mut config = ThemeConfig::default();
    config.name = SharedString::from(state.theme.meta.name.clone());
    config.mode = mode;
    config.colors = colors;
    config.font_size = Some(f32::from(state.ui_font_size));
    config.font_family = Some(state.fonts.ui.clone());
    config.mono_font_size = Some(f32::from(state.mono_font_size));
    config.mono_font_family = Some(state.fonts.mono.clone());
    config.radius = Some(f32::from(state.metrics.radius_md()).round() as usize);
    config.radius_lg = Some(f32::from(state.metrics.radius_lg()).round() as usize);
    config.shadow = Some(false);
    config.highlight = None;

    {
        let theme = GpuiTheme::global_mut(cx);
        match mode {
            ThemeMode::Dark => theme.dark_theme = Rc::new(config),
            ThemeMode::Light => theme.light_theme = Rc::new(config),
        }
    }
    GpuiTheme::change(mode, None, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_uses_a_resolved_builtin() {
        let state = ThemeState::initial();
        assert_eq!(state.theme.variant(), ThemeVariant::Dark);
        assert!(state.ui_font_size > px(0.0));
        assert!(state.mono_font_size > px(0.0));
    }

    #[test]
    fn display_scale_updates_metrics_only() {
        let state = ThemeState::initial();
        let hi = state.with_display_scale(2.0);
        assert_eq!(hi.metrics.display_scale, 2.0);
        assert_eq!(hi.theme.meta, state.theme.meta);
    }
}
