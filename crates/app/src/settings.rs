//! Modelo del panel de ajustes (ADR 0007).
//!
//! El panel trabaja sobre un borrador de `Config` independiente del activo.
//! Este módulo concentra las operaciones, la validación y la política de
//! conflictos; el shell solo pinta y traduce eventos. Nada aquí toca disco.

include!("settings_parts/01_settingssection.rs");
include!("settings_parts/02_settingsstate_new.rs");
include!("settings_parts/03_settingsstate_unbind_action.rs");
include!("settings_parts/04_step.rs");
