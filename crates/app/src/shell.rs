//! Shell de la ventana: sidebar de proyectos, árbol de paneles, pestañas y
//! terminales.
//!
//! El modelo (`axf_workspace`) es la fuente de verdad. Este módulo traduce sus
//! operaciones a entidades `TerminalView`, guarda el mapa `TerminalId -> PTY` y
//! persiste la sesión sin bloquear el hilo de UI.
#![expect(
    clippy::let_unit_value,
    reason = "WeakEntity::update devuelve () en la versión fijada de gpui-pre"
)]
include!("shell_parts/01_max_tabs.rs");
include!("shell_parts/02_shell_new.rs");
include!("shell_parts/03_shell_watch_config.rs");
include!("shell_parts/04_shell_palette_next.rs");
include!("shell_parts/05_shell_save_settings.rs");
include!("shell_parts/06_shell_split_horizontal.rs");
include!("shell_parts/07_shell_ensure_terminal.rs");
include!("shell_parts/08_reconcile_agents.rs");
include!("shell_parts/09_render_for_shell_render.rs");
include!("shell_parts/10_shell_render_title_bar.rs");
include!("shell_parts/11_shell_render_sidebar.rs");
include!("shell_parts/12_shell_render_sidebar_tab.rs");
include!("shell_parts/13_shell_sidebar_key.rs");
include!("shell_parts/14_shell_render_tab_strip.rs");
include!("shell_parts/15_shell_render_tab.rs");
include!("shell_parts/16_shell_new_tab_in_pane.rs");
include!("shell_parts/17_shell_render_palette.rs");
include!("shell_parts/18_shell_settings_selects.rs");
include!("shell_parts/19_shell_render_settings_shell.rs");
include!("shell_parts/20_shell_render_settings_appearance.rs");
include!("shell_parts/21_shell_render_settings_profiles.rs");
include!("shell_parts/22_shell_render_settings_profiles_shortcuts.rs");
include!("shell_parts/23_shell_render_settings_agents.rs");
include!("shell_parts/24_dragghost.rs");
include!("shell_parts/26_shell_render_settings_footer.rs");
include!("shell_parts/27_shell_settings_profiles.rs");
include!("shell_parts/28_shell_settings_step.rs");
include!("shell_parts/29_shell_render_settings_typography.rs");
include!("shell_parts/30_shell_wsl.rs");
include!("shell_parts/31_shell_wsl_projects.rs");
include!("shell_parts/32_shell_render_wsl.rs");
include!("shell_parts/33_shell_file_drag.rs");
#[cfg(feature = "studio")]
include!("shell_parts/25_studio_surface.rs");
#[cfg(test)]
mod tests {
    include!("shell_parts/tests/part_01.rs");
    include!("shell_parts/tests/part_02.rs");
}
