//! Esquema TOML de temas y resolución a tokens.
//!
//! Un tema es un archivo independiente en `themes/` con `[meta]`, tokens de UI
//! y la paleta del terminal. El loader valida todos los campos y señala el
//! campo y la línea del primer problema.

include!("document_parts/01_max_theme_bytes.rs");
include!("document_parts/02_load_file.rs");
