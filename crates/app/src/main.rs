//! Punto de entrada de Axfterm.
//!
//! Carga config, fuentes y tema, luego la sesión, y abre la ventana principal.
//! El modelo de workspace vive en `axf_workspace`; este crate solo conecta ese
//! modelo con entidades de terminal y vistas.

mod history;
mod keys;
mod shell;
mod theme_host;

use std::path::PathBuf;

use axf_terminal::{CopySelection, PasteSelection, SendBacktab, SendTab, TERMINAL_CONTEXT};
use axf_ui::Assets;
use axf_ui::gpui;
use axf_ui::gpui::{
    App, AppContext, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, actions,
    point, px, size,
};

use shell::Shell;
use theme_host::ThemeHost;

actions!(
    axfterm,
    [
        Quit,
        ToggleSidebar,
        OpenProject,
        NewTab,
        CloseTab,
        NextTab,
        PreviousTab,
        MoveTabNextPane,
        SplitHorizontal,
        SplitVertical,
        ClosePane,
        FocusNextPane,
        FocusPreviousPane,
        RenameTab,
        DuplicateTab,
        ToggleZoom,
        RestartTab,
        FocusSidebar,
        OpenCommandPalette,
        ToggleSidebarAgents,
        ShowThemeInfo,
        ToggleThemeVariant,
        NextTheme,
        PreviousTheme,
        ReloadThemes,
        OpenThemesFolder,
        ImportTheme,
        ExportTheme,
    ]
);

/// Contexto de teclado del shell completo.
pub(crate) const WORKSPACE_CONTEXT: &str = "Workspace";

/// Contexto de teclado de la sidebar.
pub(crate) const SIDEBAR_CONTEXT: &str = "Sidebar";

#[expect(
    clippy::print_stderr,
    reason = "fallo fatal de arranque, antes de que exista logging configurado"
)]
fn main() {
    let startup = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| !path.as_os_str().is_empty());

    axf_ui::gpui::application()
        .with_assets(Assets)
        .run(move |cx| {
            axf_ui::init(cx);
            init_keymap(cx);
            let theme_host = ThemeHost::bootstrap(cx);

            // 1280x800 es el tamaño objetivo, pero a 150 % de escala física esa
            // medida no cabe en una pantalla de 1920x1080. Se recorta al área
            // visible del monitor manteniendo el mínimo de la ventana.
            let (width, height) = match cx.primary_display() {
                Some(display) => {
                    let visible = display.visible_bounds().size;
                    (
                        (visible.width * 0.96).min(px(1280.0)).max(px(900.0)),
                        (visible.height * 0.96).min(px(800.0)).max(px(560.0)),
                    )
                }
                None => (px(1280.0), px(800.0)),
            };
            let bounds = Bounds::centered(None, size(width, height), cx);
            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // Chrome propio: la barra nativa se oculta y el shell dibuja la
                // suya (icono, nombre, toggle de sidebar y controles). El
                // arrastre y los controles siguen siendo nativos.
                titlebar: Some(TitlebarOptions {
                    title: Some("Axfterm".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(9.0), px(9.0))),
                }),
                app_owns_titlebar_drag: true,
                window_min_size: Some(size(px(900.0), px(560.0))),
                ..Default::default()
            };

            let startup = startup.clone();
            if let Err(error) = axf_ui::gpui::open_window(window_options, cx, move |window, cx| {
                cx.new(|cx| Shell::new(window, cx, startup.clone(), theme_host))
            }) {
                eprintln!("axfterm: no se pudo abrir la ventana: {error}");
                std::process::exit(1);
            }
        });
}

fn init_keymap(cx: &mut App) {
    // Las acciones de app se resuelven antes que el PTY, también con la
    // terminal enfocada (AGENTS.md, orden de dispatch). Los atajos llevan
    // modificador y no roban Ctrl+C/D/Z, Tab, Esc ni flechas sin modificar.
    macro_rules! bind_all {
        ($key:expr, $action:expr) => {
            cx.bind_keys([
                KeyBinding::new($key, $action, Some(WORKSPACE_CONTEXT)),
                KeyBinding::new($key, $action, Some(SIDEBAR_CONTEXT)),
                KeyBinding::new($key, $action, Some(TERMINAL_CONTEXT)),
            ]);
        };
    }
    bind_all!(keys::QUIT, Quit);
    bind_all!(keys::TOGGLE_SIDEBAR, ToggleSidebar);
    bind_all!(keys::OPEN_PROJECT, OpenProject);
    bind_all!(keys::NEW_TAB, NewTab);
    bind_all!(keys::CLOSE_TAB, CloseTab);
    bind_all!(keys::NEXT_TAB, NextTab);
    bind_all!(keys::PREVIOUS_TAB, PreviousTab);
    bind_all!(keys::MOVE_TAB_NEXT_PANE, MoveTabNextPane);
    bind_all!(keys::SPLIT_HORIZONTAL, SplitHorizontal);
    bind_all!(keys::SPLIT_VERTICAL, SplitVertical);
    bind_all!(keys::CLOSE_PANE, ClosePane);
    bind_all!(keys::FOCUS_NEXT_PANE, FocusNextPane);
    bind_all!(keys::FOCUS_PREVIOUS_PANE, FocusPreviousPane);
    bind_all!(keys::RENAME_TAB, RenameTab);
    bind_all!(keys::DUPLICATE_TAB, DuplicateTab);
    bind_all!(keys::TOGGLE_ZOOM, ToggleZoom);
    bind_all!(keys::RESTART_TAB, RestartTab);
    bind_all!(keys::FOCUS_SIDEBAR, FocusSidebar);
    bind_all!(keys::COMMAND_PALETTE, OpenCommandPalette);
    bind_all!(keys::TOGGLE_SIDEBAR_AGENTS, ToggleSidebarAgents);

    // Tema y fuentes (M3). El modal es la superficie de descubrimiento sin
    // convertirse en una GUI de ajustes: el archivo sigue siendo la fuente.
    bind_all!(keys::SHOW_THEME_INFO, ShowThemeInfo);
    bind_all!(keys::TOGGLE_THEME_VARIANT, ToggleThemeVariant);
    bind_all!(keys::NEXT_THEME, NextTheme);
    bind_all!(keys::PREVIOUS_THEME, PreviousTheme);
    bind_all!(keys::RELOAD_THEMES, ReloadThemes);
    bind_all!(keys::OPEN_THEMES_FOLDER, OpenThemesFolder);
    bind_all!(keys::IMPORT_THEME, ImportTheme);
    bind_all!(keys::EXPORT_THEME, ExportTheme);

    // Copiar/pegar viven en el contexto de la terminal para no interceptar los
    // mismos atajos en un input de renombrado. Tab y Shift+Tab también se
    // enlazan en Terminal porque gpui-kit registra navegación global de foco
    // (`tab` / `shift-tab` sin contexto) en `gpui_kit::init`, que se resolvería
    // antes de `on_key_down` (AGENTS.md: el foco decide).
    cx.bind_keys([
        KeyBinding::new(keys::COPY_SELECTION, CopySelection, Some(TERMINAL_CONTEXT)),
        KeyBinding::new(
            keys::PASTE_SELECTION,
            PasteSelection,
            Some(TERMINAL_CONTEXT),
        ),
        KeyBinding::new("tab", SendTab, Some(TERMINAL_CONTEXT)),
        KeyBinding::new("shift-tab", SendBacktab, Some(TERMINAL_CONTEXT)),
    ]);
}
