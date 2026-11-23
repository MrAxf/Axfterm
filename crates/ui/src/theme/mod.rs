//! Tema de Axfterm: tokens UI y paleta de terminal.
//!
//! `axf_ui` no conoce `axf_terminal`: aquí solo se exponen colores resueltos.
//! El render del terminal pide un color por índice ANSI y esta paleta decide.

pub mod builtin;
pub mod color;
pub mod document;
pub mod state;

mod ansi;

use crate::gpui::component::theme::Theme as GpuiTheme;
use crate::gpui::{Hsla, rgb};

use ansi::ANSI_DARK;

pub use color::{ColorError, Rgb, contrast_ratio, hsla_hex};
pub use document::{
    MAX_THEME_BYTES, Theme, ThemeCatalog, ThemeDocument, ThemeError, ThemeMeta, ThemeUiDocument,
    ThemeVariant, UiTokens, load_dir, load_file, parse_str,
};
pub use state::{ThemeState, install, metrics, state};

/// Paleta que usa el grid de terminal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalPalette {
    /// Texto por defecto (`SGR 39`).
    pub foreground: Hsla,
    /// Fondo por defecto (`SGR 49`).
    pub background: Hsla,
    /// Color del cursor.
    pub cursor: Hsla,
    /// Fondo de selección.
    pub selection: Hsla,
    dim_foreground: Hsla,
    ansi: [Hsla; 16],
}

impl TerminalPalette {
    /// Construye una paleta completa desde un tema.
    pub fn new(
        foreground: Hsla,
        background: Hsla,
        cursor: Hsla,
        selection: Hsla,
        dim_foreground: Hsla,
        ansi: [Hsla; 16],
    ) -> Self {
        Self {
            foreground,
            background,
            cursor,
            selection,
            dim_foreground,
            ansi,
        }
    }

    /// Deriva la paleta del tema activo de gpui-kit y la base ANSI oscura.
    pub fn from_theme(theme: &GpuiTheme) -> Self {
        Self {
            foreground: theme.colors.foreground,
            background: theme.colors.background,
            cursor: theme.colors.caret,
            selection: theme.colors.selection,
            dim_foreground: theme.colors.muted_foreground,
            ansi: ANSI_DARK.map(|hex| rgb(hex).into()),
        }
    }

    /// Texto atenuado (`SGR 2`).
    pub fn dim_foreground(&self) -> Hsla {
        self.dim_foreground
    }

    /// Paleta neutra para tests y diagnósticos sin tema activo.
    ///
    /// No se usa en runtime: la app siempre pasa la paleta derivada del tema.
    #[doc(hidden)]
    pub fn placeholder() -> Self {
        let color = Hsla::default();
        Self {
            foreground: color,
            background: color,
            cursor: color,
            selection: color,
            dim_foreground: color,
            ansi: [color; 16],
        }
    }

    /// Resuelve un color indexado de 256 colores.
    pub fn ansi(&self, index: u8) -> Hsla {
        match index {
            0..=15 => self.ansi[usize::from(index)],
            16..=231 => {
                let value = u32::from(index) - 16;
                let levels = [0u8, 95, 135, 175, 215, 255];
                let r = levels[(value / 36) as usize];
                let g = levels[((value % 36) / 6) as usize];
                let b = levels[(value % 6) as usize];
                rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)).into()
            }
            232..=255 => {
                let level = 8 + 10 * (u32::from(index) - 232);
                rgb(level << 16 | level << 8 | level).into()
            }
        }
    }

    /// Resuelve un índice de `NamedColor` (incluye fg/bg/cursor y dim).
    pub fn named(&self, index: usize) -> Hsla {
        match index {
            0..=15 => self.ansi(index as u8),
            256 | 267 => self.foreground,
            257 => self.background,
            258 => self.cursor,
            259..=266 => self.ansi((index - 259) as u8),
            268 => self.dim_foreground,
            other => self.ansi(other as u8),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_16_and_256_resolve() {
        let palette = TerminalPalette::from_theme(&GpuiTheme::default());
        assert_ne!(palette.ansi(1), palette.ansi(2));
        assert_eq!(palette.ansi(16), rgb(0).into());
        assert_eq!(palette.ansi(255), rgb(0xeeeeee).into());
        assert_eq!(palette.named(257), palette.background);
        assert_eq!(palette.named(258), palette.cursor);
    }
}
