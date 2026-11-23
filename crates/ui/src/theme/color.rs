//! Color de tema: parseo `#RRGGBB[AA]`, conversión a GPUI y contraste.
//!
//! Los colores literales solo existen en los temas built-in; la UI consume
//! siempre tokens resueltos.

use crate::gpui::{Hsla, rgba};

/// Error de un color de tema.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("color inválido `{value}`: {message}")]
pub struct ColorError {
    /// Texto original.
    pub value: String,
    /// Motivo accionable.
    pub message: String,
}

/// Color RGBA de 8 bits, independiente de GPUI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Rojo.
    pub r: u8,
    /// Verde.
    pub g: u8,
    /// Azul.
    pub b: u8,
    /// Alfa (255 = opaco).
    pub a: u8,
}

impl Rgb {
    /// Color opaco.
    #[expect(
        clippy::self_named_constructors,
        reason = "constructor canónico del tipo Rgb"
    )]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Color con alfa.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parsea `#rgb`, `#rgba`, `#rrggbb` o `#rrggbbaa`.
    pub fn parse(value: &str) -> Result<Self, ColorError> {
        let original = value.to_string();
        let hex = value.trim().trim_start_matches('#');
        let expand = |digit: u8| -> u8 { digit * 17 };
        let nibble = |byte: u8| -> Result<u8, ColorError> {
            match byte {
                b'0'..=b'9' => Ok(byte - b'0'),
                b'a'..=b'f' => Ok(byte - b'a' + 10),
                b'A'..=b'F' => Ok(byte - b'A' + 10),
                _ => Err(ColorError {
                    value: original.clone(),
                    message: "se esperaban dígitos hexadecimales".into(),
                }),
            }
        };
        let mut digits = Vec::with_capacity(8);
        for byte in hex.as_bytes() {
            digits.push(nibble(*byte)?);
        }
        match digits.len() {
            3 => Ok(Self::rgb(
                expand(digits[0]),
                expand(digits[1]),
                expand(digits[2]),
            )),
            4 => Ok(Self::rgba(
                expand(digits[0]),
                expand(digits[1]),
                expand(digits[2]),
                expand(digits[3]),
            )),
            6 => Ok(Self::rgb(
                digits[0] << 4 | digits[1],
                digits[2] << 4 | digits[3],
                digits[4] << 4 | digits[5],
            )),
            8 => Ok(Self::rgba(
                digits[0] << 4 | digits[1],
                digits[2] << 4 | digits[3],
                digits[4] << 4 | digits[5],
                digits[6] << 4 | digits[7],
            )),
            _ => Err(ColorError {
                value: original,
                message: "usa #RGB, #RGBA, #RRGGBB o #RRGGBBAA".into(),
            }),
        }
    }

    /// Color como `#RRGGBB` o `#RRGGBBAA` si tiene alfa.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Convierte a color de GPUI.
    pub fn to_hsla(self) -> Hsla {
        let packed = u32::from(self.r) << 24
            | u32::from(self.g) << 16
            | u32::from(self.b) << 8
            | u32::from(self.a);
        rgba(packed).into()
    }

    /// Mezcla con `other` en proporción `t` (0 = self, 1 = other).
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| -> u8 {
            (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8
        };
        Self {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: lerp(self.a, other.a),
        }
    }

    /// Aclara hacia blanco.
    pub fn lighten(self, t: f32) -> Self {
        self.mix(Self::rgb(255, 255, 255), t)
    }

    /// Oscurece hacia negro.
    pub fn darken(self, t: f32) -> Self {
        self.mix(Self::rgb(0, 0, 0), t)
    }

    /// Compone sobre un fondo opaco (alfa recto).
    pub fn over(self, background: Self) -> Self {
        if self.a == 255 {
            return self;
        }
        let alpha = f32::from(self.a) / 255.0;
        let blend = |fg: u8, bg: u8| -> u8 {
            (f32::from(fg) * alpha + f32::from(bg) * (1.0 - alpha)).round() as u8
        };
        Self::rgb(
            blend(self.r, background.r),
            blend(self.g, background.g),
            blend(self.b, background.b),
        )
    }

    /// Luminancia relativa sRGB (WCAG 2.1).
    pub fn luminance(self) -> f64 {
        let channel = |value: u8| -> f64 {
            let value = f64::from(value) / 255.0;
            if value <= 0.040_45 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// Ratio de contraste WCAG contra un fondo.
    pub fn contrast(self, background: Self) -> f64 {
        let foreground = self.over(background).luminance();
        let background = background.over(background).luminance();
        let (lighter, darker) = if foreground >= background {
            (foreground, background)
        } else {
            (background, foreground)
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Texto legible (negro o blanco) sobre este color.
    pub fn readable_text(self) -> Self {
        let black = Self::rgb(0, 0, 0);
        let white = Self::rgb(255, 255, 255);
        if black.contrast(self) >= white.contrast(self) {
            black
        } else {
            white
        }
    }
}

impl From<Hsla> for Rgb {
    fn from(color: Hsla) -> Self {
        let rgba = color.to_rgb();
        let channel = |value: f32| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
        Self {
            r: channel(rgba.r),
            g: channel(rgba.g),
            b: channel(rgba.b),
            a: channel(rgba.a),
        }
    }
}

/// Color de GPUI como hex legible.
pub fn hsla_hex(color: Hsla) -> String {
    Rgb::from(color).to_hex()
}

/// Ratio de contraste entre dos colores resueltos.
pub fn contrast_ratio(foreground: Hsla, background: Hsla) -> f64 {
    Rgb::from(foreground).contrast(Rgb::from(background))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_short_and_long_hex() {
        assert_eq!(Rgb::parse("#fff").expect("3"), Rgb::rgb(255, 255, 255));
        assert_eq!(Rgb::parse("#0000").expect("4"), Rgb::rgba(0, 0, 0, 0));
        assert_eq!(Rgb::parse("61afef").expect("6"), Rgb::rgb(0x61, 0xaf, 0xef));
        assert_eq!(
            Rgb::parse("#61afef80").expect("8"),
            Rgb::rgba(0x61, 0xaf, 0xef, 0x80)
        );
    }

    #[test]
    fn rejects_invalid_colors() {
        for value in ["", "#12", "#gggggg", "azul", "#12345"] {
            let error = Rgb::parse(value).expect_err("debe fallar");
            assert_eq!(error.value, value);
        }
    }

    #[test]
    fn hex_round_trips() {
        let color = Rgb::rgba(0x12, 0x34, 0x56, 0x78);
        assert_eq!(color.to_hex(), "#12345678");
        assert_eq!(Rgb::parse(&color.to_hex()).expect("parse"), color);
        assert_eq!(Rgb::rgb(1, 2, 3).to_hex(), "#010203");
    }

    #[test]
    fn contrast_meets_aa_for_black_on_white() {
        let white = Rgb::rgb(255, 255, 255);
        let black = Rgb::rgb(0, 0, 0);
        assert!((black.contrast(white) - 21.0).abs() < 0.01);
        assert!(black.contrast(white) >= 4.5);
        assert_eq!(white.readable_text(), black);
        assert_eq!(black.readable_text(), white);
    }

    #[test]
    fn alpha_is_composited_before_contrast() {
        let transparent = Rgb::rgba(0, 0, 0, 0);
        let white = Rgb::rgb(255, 255, 255);
        assert!((transparent.contrast(white) - 1.0).abs() < 0.01);
    }
}
