//! Registro único de acciones de la aplicación.
//!
//! La paleta, los atajos, la ayuda y los menús leen de aquí; los keybindings se
//! resuelven por `id` desde `config.toml`. Un `id` es estable y visible en la
//! configuración: renombrarlo rompe el archivo del usuario.

include!("actions_parts/01_actioncategory.rs");
include!("actions_parts/02_shortcuts_markdown.rs");
