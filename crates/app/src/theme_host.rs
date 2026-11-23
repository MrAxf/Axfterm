//! Carga y aplicación de config, temas y fuentes.
//!
//! Mantiene el catálogo de temas, el tema activo y el watcher. La app solo
//! consume este módulo; el modelo de workspace no sabe nada de estilo.

include!("theme_host_parts/01_applyreport.rs");
include!("theme_host_parts/02_themehost_bootstrap.rs");
include!("theme_host_parts/03_themehost_profile_icon.rs");
include!("theme_host_parts/04_target_variant.rs");
