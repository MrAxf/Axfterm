//! Punto de entrada de Axfterm.
//!
//! Arranca GPUI con el tema oscuro base y abre la ventana principal. El shell
//! real (proyectos, paneles y terminales) llega en milestones posteriores.

mod shell;

use axf_ui::{Assets, icons};
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::{
    App, AppContext, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, actions, px,
    size,
};

use shell::Shell;

actions!(axfterm, [Quit, ToggleSidebar]);

/// Contexto de teclado del shell completo.
pub(crate) const WORKSPACE_CONTEXT: &str = "Workspace";

#[expect(
    clippy::print_stderr,
    reason = "fallo fatal de arranque, antes de que exista logging configurado"
)]
fn main() {
    gpui_kit::application().with_assets(Assets).run(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        init_keymap(cx);

        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Axfterm".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(900.0), px(560.0))),
            ..Default::default()
        };

        if let Err(error) = gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| Shell::new(window, cx))
        }) {
            eprintln!("axfterm: no se pudo abrir la ventana: {error}");
            std::process::exit(1);
        }
    });
}

fn init_keymap(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-b", ToggleSidebar, Some(WORKSPACE_CONTEXT)),
        KeyBinding::new("ctrl-q", Quit, Some(WORKSPACE_CONTEXT)),
    ]);
}
