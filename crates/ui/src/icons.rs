//! Rutas de los iconos vendorizados.
//!
//! Las rutas son las claves que el `AssetSource` registrado en la aplicación
//! entrega a GPUI. Nunca se construyen a mano en otro sitio.

/// Marca de Axfterm (monocroma, tintada por el tema).
pub const AXFTERM: &str = "brand/axfterm-mark.svg";

/// Terminal (pestañas y sidebar).
pub const TERMINAL: &str = "icons/tabler/terminal-2.svg";

/// Sidebar (acción de alternarla).
pub const SIDEBAR: &str = "icons/tabler/layout-sidebar.svg";

/// Ajustes.
pub const SETTINGS: &str = "icons/tabler/settings.svg";

/// Nuevo (proyecto, pestaña, panel).
pub const PLUS: &str = "icons/tabler/plus.svg";

/// Cerrar pestaña o panel.
pub const CLOSE: &str = "icons/tabler/x.svg";

/// Minimizar la ventana.
pub const WINDOW_MINIMIZE: &str = "icons/tabler/minus.svg";

/// Maximizar la ventana.
pub const WINDOW_MAXIMIZE: &str = "icons/tabler/square.svg";

/// Desplegar un proyecto de la sidebar.
pub const CHEVRON_DOWN: &str = "icons/tabler/chevron-down.svg";

/// Plegar un proyecto de la sidebar.
pub const CHEVRON_RIGHT: &str = "icons/tabler/chevron-right.svg";

/// Chevron de overflow a la izquierda.
pub const CHEVRON_LEFT: &str = "icons/tabler/chevron-left.svg";

/// Chevron hacia arriba (menús, scroll).
pub const CHEVRON_UP: &str = "icons/tabler/chevron-up.svg";

/// Proyecto de la sidebar.
pub const FOLDER: &str = "icons/tabler/folder.svg";

/// Carpeta abierta / abrir carpeta de temas.
pub const FOLDER_OPEN: &str = "icons/tabler/folder-open.svg";

/// Split de izquierda a derecha.
pub const SPLIT_HORIZONTAL: &str = "icons/tabler/layout-columns.svg";

/// Split de arriba a abajo.
pub const SPLIT_VERTICAL: &str = "icons/tabler/layout-rows.svg";

/// Renombrar.
pub const RENAME: &str = "icons/tabler/pencil.svg";

/// Duplicar.
pub const DUPLICATE: &str = "icons/tabler/copy.svg";

/// Zoom temporal del panel.
pub const ZOOM: &str = "icons/tabler/maximize.svg";

/// Rearrancar una terminal restaurada.
pub const RESTART: &str = "icons/tabler/refresh.svg";

/// Buscar.
pub const SEARCH: &str = "icons/tabler/search.svg";

/// Variante clara.
pub const SUN: &str = "icons/tabler/sun.svg";

/// Variante oscura.
pub const MOON: &str = "icons/tabler/moon.svg";

/// Tema / paleta.
pub const PALETTE: &str = "icons/tabler/palette.svg";

/// Menú.
pub const MENU: &str = "icons/tabler/menu-2.svg";

/// Teclado / atajos.
pub const KEYBOARD: &str = "icons/tabler/keyboard.svg";

/// Perfil / persona.
pub const USER: &str = "icons/tabler/user.svg";

/// Tipografía / fuentes.
pub const TYPOGRAPHY: &str = "icons/tabler/typography.svg";

/// Sistema / apariencia del sistema.
pub const DEVICE_DESKTOP: &str = "icons/tabler/device-desktop.svg";

/// Enlace externo.
pub const EXTERNAL_LINK: &str = "icons/tabler/external-link.svg";

/// Importar.
pub const FILE_IMPORT: &str = "icons/tabler/file-import.svg";

/// Exportar.
pub const FILE_EXPORT: &str = "icons/tabler/file-export.svg";

/// Confirmación.
pub const CHECK: &str = "icons/tabler/check.svg";

/// Punto neutro / inactivo.
pub const CIRCLE: &str = "icons/tabler/circle.svg";

/// Aviso / atención.
pub const ALERT_TRIANGLE: &str = "icons/tabler/alert-triangle.svg";

/// Información.
pub const INFO_CIRCLE: &str = "icons/tabler/info-circle.svg";

/// Proceso en ejecución.
pub const PLAYER_PLAY: &str = "icons/tabler/player-play.svg";

/// Proceso pausado.
pub const PLAYER_PAUSE: &str = "icons/tabler/player-pause.svg";

/// Proceso detenido.
pub const STOP: &str = "icons/tabler/player-stop.svg";

/// Trabajo en curso.
pub const LOADER: &str = "icons/tabler/loader-2.svg";

/// Campana de notificación.
pub const BELL: &str = "icons/tabler/bell.svg";

/// Notificaciones silenciadas.
pub const BELL_OFF: &str = "icons/tabler/bell-off.svg";

/// PowerShell (Tabler, outline tintado por token).
pub const BRAND_POWERSHELL: &str = "icons/tabler/brand-powershell.svg";

/// Marca de Rust (simple-icons, a color de marca).
pub const RUST: &str = "icons/simple/rust.svg";

/// Marca de OpenCode (simple-icons; monocroma).
pub const BRAND_OPENCODE: &str = "icons/simple/opencode.svg";

/// Marca de GitHub Copilot (simple-icons; monocroma).
pub const BRAND_COPILOT: &str = "icons/simple/githubcopilot.svg";

/// Todos los iconos vendorizados, para tests y herramienta de actualización.
pub const ALL: &[&str] = &[
    AXFTERM,
    TERMINAL,
    SIDEBAR,
    SETTINGS,
    PLUS,
    CLOSE,
    WINDOW_MINIMIZE,
    WINDOW_MAXIMIZE,
    CHEVRON_DOWN,
    CHEVRON_RIGHT,
    CHEVRON_LEFT,
    CHEVRON_UP,
    FOLDER,
    FOLDER_OPEN,
    SPLIT_HORIZONTAL,
    SPLIT_VERTICAL,
    RENAME,
    DUPLICATE,
    ZOOM,
    RESTART,
    SEARCH,
    SUN,
    MOON,
    PALETTE,
    MENU,
    KEYBOARD,
    USER,
    TYPOGRAPHY,
    DEVICE_DESKTOP,
    EXTERNAL_LINK,
    FILE_IMPORT,
    FILE_EXPORT,
    CHECK,
    CIRCLE,
    ALERT_TRIANGLE,
    INFO_CIRCLE,
    PLAYER_PLAY,
    PLAYER_PAUSE,
    STOP,
    LOADER,
    BELL,
    BELL_OFF,
    BRAND_POWERSHELL,
    RUST,
    BRAND_OPENCODE,
    BRAND_COPILOT,
];
