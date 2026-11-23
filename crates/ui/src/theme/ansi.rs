//! Paleta ANSI de respaldo para `TerminalPalette::from_theme`.
//!
//! Los temas de M3 traen su propia paleta de 16 colores; esta base oscura solo
//! se usa cuando no hay un tema propio (tests, diagnósticos y compatibilidad).
//! La variante clara vive en los TOML built-in y de usuario.

/// Colores ANSI 0–15 de la variante oscura (base One Dark).
pub(crate) const ANSI_DARK: [u32; 16] = [
    0x001e_2127, // black
    0x00e0_6c75, // red
    0x0098_c379, // green
    0x00e5_c07b, // yellow
    0x0061_afef, // blue
    0x00c6_78dd, // magenta
    0x0056_b6c2, // cyan
    0x00ab_b2bf, // white
    0x005c_6370, // bright black
    0x00e0_6c75, // bright red
    0x0098_c379, // bright green
    0x00e5_c07b, // bright yellow
    0x0061_afef, // bright blue
    0x00c6_78dd, // bright magenta
    0x0056_b6c2, // bright cyan
    0x00ff_ffff, // bright white
];
