//! Galería visual del sistema de diseño.
//!
//! ```powershell
//! cargo run -p axf_ui --features gallery --example gallery
//! ```
//!
//! Teclas: `1`..`9` cambian de sección, flechas navegan y `Esc` cierra el
//! modal. La app la reutiliza como superficie de Axfterm Studio.

fn main() {
    axf_ui::gallery::run_standalone();
}
