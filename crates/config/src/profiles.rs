//! Perfiles de terminal, tokenizer de `command` y descubrimiento local.
//!
//! El crate no conoce GPUI ni abre PTYs: solo describe qué programa arrancar.
//! La app traduce [`CommandLine`] a `axf_terminal::SpawnSpec` y resuelve los
//! iconos. Las reglas del tokenizer están en ADR 0006 y `docs/profiles.md`.

include!("profiles_parts/01_max_profiles.rs");
include!("profiles_parts/02_discover_wsl_windows.rs");
