//! Colores de marca de terceros.
//!
//! Es la única excepción a la regla de "ningún color literal fuera del tema":
//! estos colores identifican a un producto externo (Rust, OpenCode, Copilot) y
//! no forman parte de la identidad visual de Axfterm. Las marcas monocromas se
//! tintan con el token de texto para que sigan siendo visibles.

use crate::gpui::{Hsla, rgb};

/// Una marca con su color oficial.
#[derive(Clone, Copy, Debug)]
pub struct BrandColor {
    /// Nombre legible de la marca.
    pub name: &'static str,
    /// Color oficial en formato `0xRRGGBB`.
    pub rgb: u32,
    /// Si la marca es monocroma y debe tintarse con el tema.
    pub monochrome: bool,
}

/// Rust.
pub const RUST: BrandColor = BrandColor {
    name: "Rust",
    rgb: 0xF74C00,
    monochrome: false,
};

/// OpenCode (marca monocroma).
pub const OPENCODE: BrandColor = BrandColor {
    name: "OpenCode",
    rgb: 0x000000,
    monochrome: true,
};

/// GitHub Copilot (marca monocroma).
pub const GITHUB_COPILOT: BrandColor = BrandColor {
    name: "GitHub Copilot",
    rgb: 0x000000,
    monochrome: true,
};

/// Devuelve el color de la marca listo para `svg().text_color(...)`.
pub fn hsla(brand: BrandColor) -> Hsla {
    rgb(brand.rgb).into()
}

/// Color de marca o el token de texto si la marca es monocroma.
pub fn tint(brand: BrandColor, text: Hsla) -> Hsla {
    if brand.monochrome { text } else { hsla(brand) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brands_have_opaque_rgb() {
        assert!(!RUST.name.is_empty());
        assert_eq!(hsla(RUST).a, 1.0);
        assert!(!OPENCODE.name.is_empty());
    }

    #[test]
    fn monochrome_brands_use_the_text_token() {
        let text = rgb(0xabb2bf).into();
        assert_eq!(tint(OPENCODE, text), text);
        assert_eq!(tint(GITHUB_COPILOT, text), text);
        assert_ne!(tint(RUST, text), text);
    }
}
