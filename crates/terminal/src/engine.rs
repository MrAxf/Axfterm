//! Motor VT: hilo parser que posee `Term` y publica filas dañadas a la UI.
//!
//! El hilo de UI nunca toca `alacritty_terminal`. El parser recibe bytes y
//! comandos por un canal acotado, aplica el damage y envía un `ScreenFrame` con
//! las filas que cambiaron. Los tipos de alacritty quedan confinados aquí
//! (ADR 0001).

include!("engine_parts/01_scrollback_lines.rs");
include!("engine_parts/02_terminalcore_new.rs");
include!("engine_parts/03_trace_file.rs");
include!("engine_parts/04_percent_decode.rs");
