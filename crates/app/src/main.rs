//! Punto de entrada de Axfterm.
//!
//! Carga config, fuentes y tema, luego la sesión, y abre la ventana principal.
//! El modelo de workspace vive en `axf_workspace`; este crate solo conecta ese
//! modelo con entidades de terminal y vistas.

mod actions;
mod font_select;
mod fuzzy;
mod history;
mod keymap;
mod keys;
mod palette;
mod shell;
mod theme_host;

use std::path::PathBuf;

use axf_terminal::{SendBacktab, SendTab, TERMINAL_CONTEXT};
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
        OpenSearchPalette,
        ToggleSidebarAgents,
        ShowThemeInfo,
        ToggleThemeVariant,
        NextTheme,
        PreviousTheme,
        ReloadThemes,
        OpenThemesFolder,
        ImportTheme,
        ExportTheme,
        AgentNextAttention,
        ShowShortcuts,
        PaletteNext,
        PalettePrevious,
        PaletteAccept,
        PaletteCancel,
        PaletteSwitchMode,
        PaletteFilterAll,
        PaletteFilterTabs,
        PaletteFilterPanes,
        PaletteFilterProjects,
        PaletteFilterProfiles,
        PaletteFilterAgents,
    ]
);

/// Contexto de teclado del shell completo.
pub(crate) const WORKSPACE_CONTEXT: &str = "Workspace";

/// Contexto de teclado de la sidebar.
pub(crate) const SIDEBAR_CONTEXT: &str = "Sidebar";

/// Contexto de teclado de la paleta.
pub(crate) const PALETTE_CONTEXT: &str = "Palette";

/// La paleta se navega con las flechas aunque el foco esté en su input.
///
/// El `Input` de gpui-kit declara el contexto `Input`, más profundo que
/// `Palette`, y sus bindings de `up`/`down` mueven el cursor del texto. Estos
/// bindings se registran después con un predicado de descendiente para que, en
/// la paleta, las flechas muevan la selección y no el cursor.
const PALETTE_INPUT_CONTEXT: &str = "Palette > Input";

/// Contexto de teclado de la pantalla de atajos.
pub(crate) const SHORTCUTS_CONTEXT: &str = "Shortcuts";

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
            let theme_host = ThemeHost::bootstrap(cx);
            let keymap = keymap::Keymap::resolve(&theme_host.settings.keymap);
            let mut keymap_errors = keymap.errors.clone();
            keymap_errors.extend(keymap.install(cx));
            keymap_errors.extend(keymap.warnings.clone());
            init_internal_keymap(cx);

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
                cx.new(|cx| {
                    Shell::new(
                        window,
                        cx,
                        startup.clone(),
                        theme_host,
                        keymap,
                        keymap_errors,
                    )
                })
            }) {
                eprintln!("axfterm: no se pudo abrir la ventana: {error}");
                std::process::exit(1);
            }
        });
}

fn init_internal_keymap(cx: &mut App) {
    // Los atajos de app salen del registro y de `[keymap]`; aquí quedan solo
    // las teclas internas de las superficies modales y la entrega de Tab a la
    // terminal. Tab y Shift+Tab también se enlazan en Terminal porque gpui-kit
    // registra navegación global de foco (`tab` / `shift-tab` sin contexto),
    // que se resolvería antes de `on_key_down` (AGENTS.md: el foco decide).
    cx.bind_keys([
        KeyBinding::new("tab", SendTab, Some(TERMINAL_CONTEXT)),
        KeyBinding::new("shift-tab", SendBacktab, Some(TERMINAL_CONTEXT)),
        KeyBinding::new("escape", PaletteCancel, Some(SHORTCUTS_CONTEXT)),
    ]);
    // Dos niveles: la paleta como contenedor y la paleta con el input enfocado.
    // El segundo gana al binding `up`/`down` de `Input` por orden de registro.
    macro_rules! palette_key {
        ($key:expr, $action:expr) => {
            cx.bind_keys([
                KeyBinding::new($key, $action, Some(PALETTE_CONTEXT)),
                KeyBinding::new($key, $action, Some(PALETTE_INPUT_CONTEXT)),
            ]);
        };
    }
    palette_key!("down", PaletteNext);
    palette_key!("up", PalettePrevious);
    palette_key!("tab", PaletteNext);
    palette_key!("shift-tab", PalettePrevious);
    palette_key!("ctrl-n", PaletteNext);
    palette_key!("ctrl-p", PalettePrevious);
    palette_key!("enter", PaletteAccept);
    palette_key!("escape", PaletteCancel);
    palette_key!("ctrl-tab", PaletteSwitchMode);
    palette_key!("ctrl-shift-tab", PaletteSwitchMode);
    palette_key!("ctrl-1", PaletteFilterAll);
    palette_key!("ctrl-2", PaletteFilterTabs);
    palette_key!("ctrl-3", PaletteFilterPanes);
    palette_key!("ctrl-4", PaletteFilterProjects);
    palette_key!("ctrl-5", PaletteFilterProfiles);
    palette_key!("ctrl-6", PaletteFilterAgents);
}
