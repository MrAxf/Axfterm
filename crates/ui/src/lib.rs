//! Sistema visual de Axfterm: tokens, temas, iconos y componentes propios.

pub mod assets;
pub mod brand;
pub mod components;
pub mod fonts;
pub mod icons;
pub mod metrics;
pub mod profile_icon;
pub mod theme;

pub use assets::Assets;

/// GPUI reexportado para que el resto de crates no dependan de `gpui-kit`
/// directamente (ver AGENTS.md: GPUI se encapsula aquí).
///
/// Uso: `use axf_ui::gpui::{div, px, App};`
pub use gpui_kit as gpui;

/// Inicializa GPUI y el sistema visual de Axfterm.
///
/// Sustituye a `gpui_kit::init` en la app: además de los componentes y del
/// tema de la librería, instala el tema propio y las métricas.
pub fn init(cx: &mut gpui::App) {
    gpui::init(cx);
    theme::state::init(cx);
}
