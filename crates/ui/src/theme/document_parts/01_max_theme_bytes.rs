use super::TerminalPalette;
use super::color::Rgb;
use crate::gpui::Hsla;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub const MAX_THEME_BYTES: usize = 256 * 1024;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeVariant {
    /// Oscura.
    #[default]
    Dark,
    /// Clara.
    Light,
}
impl ThemeVariant {
    /// Etiqueta para la UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Oscuro",
            Self::Light => "Claro",
        }
    }

    /// ¿Es oscura?
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    /// Variante opuesta.
    pub fn opposite(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeMeta {
    /// Nombre visible y clave de selección.
    pub name: String,
    /// Autor opcional.
    pub author: Option<String>,
    /// Familia para emparejar variantes (`One Dark`/`One Light`).
    pub family: String,
    /// Variante.
    pub variant: ThemeVariant,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiTokens {
    /// Fondo de la ventana.
    pub background: Hsla,
    /// Superficie de sidebar, barras y filas.
    pub surface: Hsla,
    /// Superficie elevada (popovers, tarjetas).
    pub surface_raised: Hsla,
    /// Borde normal.
    pub border: Hsla,
    /// Borde de énfasis (separadores, scrollbar).
    pub border_strong: Hsla,
    /// Texto principal.
    pub text: Hsla,
    /// Texto secundario.
    pub muted: Hsla,
    /// Texto sutil (metadatos).
    pub subtle: Hsla,
    /// Color de acento.
    pub accent: Hsla,
    /// Texto legible sobre el acento.
    pub accent_text: Hsla,
    /// Error.
    pub danger: Hsla,
    /// Texto sobre error.
    pub danger_text: Hsla,
    /// Advertencia.
    pub warning: Hsla,
    /// Texto sobre advertencia.
    pub warning_text: Hsla,
    /// Éxito.
    pub success: Hsla,
    /// Texto sobre éxito.
    pub success_text: Hsla,
    /// Información.
    pub info: Hsla,
    /// Texto sobre información.
    pub info_text: Hsla,
    /// Fondo de selección.
    pub selection: Hsla,
    /// Anillo de foco.
    pub focus: Hsla,
    /// Velo de modal.
    pub overlay: Hsla,
}
impl UiTokens {
    /// Tokens neutros para tests y para el arranque sin tema.
    pub fn placeholder() -> Self {
        let color = Hsla::default();
        Self {
            background: color,
            surface: color,
            surface_raised: color,
            border: color,
            border_strong: color,
            text: color,
            muted: color,
            subtle: color,
            accent: color,
            accent_text: color,
            danger: color,
            danger_text: color,
            warning: color,
            warning_text: color,
            success: color,
            success_text: color,
            info: color,
            info_text: color,
            selection: color,
            focus: color,
            overlay: color,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Metadatos.
    pub meta: ThemeMeta,
    /// Tokens de UI.
    pub ui: UiTokens,
    /// Paleta del terminal.
    pub terminal: TerminalPalette,
}
impl Theme {
    /// Nombre visible.
    pub fn name(&self) -> &str {
        &self.meta.name
    }

    /// Variante.
    pub fn variant(&self) -> ThemeVariant {
        self.meta.variant
    }

    /// Documento TOML equivalente, listo para exportar.
    pub fn to_toml(&self) -> Result<String, ThemeError> {
        let document = ThemeDocument::from_theme(self);
        let body = toml::to_string_pretty(&document).map_err(|error| ThemeError::Serialize {
            message: error.to_string(),
        })?;
        Ok(format!(
            "# Tema de Axfterm exportado desde la aplicación.\n# Editable a mano; se recarga en caliente.\n\n{body}"
        ))
    }

    /// Tema neutro de emergencia si un built-in no parsea.
    pub fn fallback() -> Self {
        let background = Rgb::rgb(0x1e, 0x21, 0x27);
        let surface = Rgb::rgb(0x21, 0x24, 0x2b);
        let text = Rgb::rgb(0xab, 0xb2, 0xbf);
        let accent = Rgb::rgb(0x61, 0xaf, 0xef);
        let danger = Rgb::rgb(0xe0, 0x6c, 0x75);
        let warning = Rgb::rgb(0xe5, 0xc0, 0x7b);
        let success = Rgb::rgb(0x98, 0xc3, 0x79);
        let ansi = [
            0x001e_2127,
            0x00e0_6c75,
            0x0098_c379,
            0x00e5_c07b,
            0x0061_afef,
            0x00c6_78dd,
            0x0056_b6c2,
            0x00ab_b2bf,
            0x005c_6370,
            0x00e0_6c75,
            0x0098_c379,
            0x00e5_c07b,
            0x0061_afef,
            0x00c6_78dd,
            0x0056_b6c2,
            0x00ff_ffff,
        ];
        Self {
            meta: ThemeMeta {
                name: "One Dark".into(),
                author: None,
                family: "One".into(),
                variant: ThemeVariant::Dark,
            },
            ui: UiTokens {
                background: background.to_hsla(),
                surface: surface.to_hsla(),
                surface_raised: surface.lighten(0.06).to_hsla(),
                border: Rgb::rgb(0x3a, 0x3f, 0x4b).to_hsla(),
                border_strong: Rgb::rgb(0x4b, 0x52, 0x60).to_hsla(),
                text: text.to_hsla(),
                muted: Rgb::rgb(0x7d, 0x85, 0x94).to_hsla(),
                subtle: Rgb::rgb(0x5c, 0x63, 0x70).to_hsla(),
                accent: accent.to_hsla(),
                accent_text: accent.readable_text().to_hsla(),
                danger: danger.to_hsla(),
                danger_text: danger.readable_text().to_hsla(),
                warning: warning.to_hsla(),
                warning_text: warning.readable_text().to_hsla(),
                success: success.to_hsla(),
                success_text: success.readable_text().to_hsla(),
                info: accent.to_hsla(),
                info_text: accent.readable_text().to_hsla(),
                selection: accent.to_hsla(),
                focus: accent.to_hsla(),
                overlay: Rgb::rgba(0, 0, 0, 150).to_hsla(),
            },
            terminal: TerminalPalette::new(
                text.to_hsla(),
                background.to_hsla(),
                accent.to_hsla(),
                Rgb::rgba(0x61, 0xaf, 0xef, 90).to_hsla(),
                Rgb::rgb(0x7d, 0x85, 0x94).to_hsla(),
                ansi.map(|hex| {
                    Rgb {
                        r: (hex >> 16) as u8,
                        g: (hex >> 8) as u8,
                        b: hex as u8,
                        a: 255,
                    }
                    .to_hsla()
                }),
            ),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeDocument {
    /// Metadatos.
    pub meta: ThemeMetaDocument,
    /// Tokens de UI.
    pub ui: ThemeUiDocument,
    /// Paleta del terminal.
    pub terminal: ThemeTerminalDocument,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeMetaDocument {
    /// Nombre visible.
    pub name: String,
    /// Autor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Familia para emparejar variantes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// `light` u `dark`.
    pub variant: ThemeVariant,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeUiDocument {
    /// Fondo.
    pub background: String,
    /// Superficie.
    pub surface: String,
    /// Superficie elevada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_raised: Option<String>,
    /// Borde.
    pub border: String,
    /// Borde fuerte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_strong: Option<String>,
    /// Texto.
    pub text: String,
    /// Texto secundario.
    pub muted: String,
    /// Texto sutil.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtle: Option<String>,
    /// Acento.
    pub accent: String,
    /// Texto sobre acento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_text: Option<String>,
    /// Error.
    pub danger: String,
    /// Advertencia.
    pub warning: String,
    /// Éxito.
    pub success: String,
    /// Información.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<String>,
    /// Selección.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<String>,
    /// Foco.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeTerminalDocument {
    /// Texto por defecto.
    pub foreground: String,
    /// Fondo por defecto.
    pub background: String,
    /// Cursor.
    pub cursor: String,
    /// Selección.
    pub selection: String,
    /// Texto atenuado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim_foreground: Option<String>,
    /// ANSI 0–15.
    pub ansi: Vec<String>,
}
impl ThemeDocument {
    /// Construye el documento exportable desde un tema resuelto.
    pub fn from_theme(theme: &Theme) -> Self {
        let ui = &theme.ui;
        let terminal = &theme.terminal;
        Self {
            meta: ThemeMetaDocument {
                name: theme.meta.name.clone(),
                author: theme.meta.author.clone(),
                family: Some(theme.meta.family.clone()),
                variant: theme.meta.variant,
            },
            ui: ThemeUiDocument {
                background: hex(ui.background),
                surface: hex(ui.surface),
                surface_raised: Some(hex(ui.surface_raised)),
                border: hex(ui.border),
                border_strong: Some(hex(ui.border_strong)),
                text: hex(ui.text),
                muted: hex(ui.muted),
                subtle: Some(hex(ui.subtle)),
                accent: hex(ui.accent),
                accent_text: Some(hex(ui.accent_text)),
                danger: hex(ui.danger),
                warning: hex(ui.warning),
                success: hex(ui.success),
                info: Some(hex(ui.info)),
                selection: Some(hex(ui.selection)),
                focus: Some(hex(ui.focus)),
            },
            terminal: ThemeTerminalDocument {
                foreground: hex(terminal.foreground),
                background: hex(terminal.background),
                cursor: hex(terminal.cursor),
                selection: hex(terminal.selection),
                dim_foreground: Some(hex(terminal.dim_foreground())),
                ansi: (0..16)
                    .map(|index| hex(terminal.ansi(index as u8)))
                    .collect(),
            },
        }
    }
}
fn hex(color: Hsla) -> String {
    Rgb::from(color).to_hex()
}
#[derive(Clone, Debug, thiserror::Error)]
pub enum ThemeError {
    /// TOML inválido.
    #[error("{label}: {message} (línea {line}, columna {column})")]
    Parse {
        /// Archivo o nombre.
        label: String,
        /// Mensaje de `toml`.
        message: String,
        /// Línea 1-based.
        line: usize,
        /// Columna 1-based.
        column: usize,
    },
    /// Uno o más campos inválidos.
    #[error("{label}: tema inválido:\n{problems}")]
    Invalid {
        /// Archivo o nombre.
        label: String,
        /// Lista de problemas con campo y línea.
        problems: String,
    },
    /// Error de E/S.
    #[error("{path}: {message}")]
    Io {
        /// Archivo.
        path: String,
        /// Detalle.
        message: String,
    },
    /// Archivo por encima del tope.
    #[error("{path}: supera el límite de {limit} bytes")]
    TooLarge {
        /// Archivo.
        path: String,
        /// Tope.
        limit: usize,
    },
    /// No se pudo serializar al exportar.
    #[error("no se pudo serializar el tema: {message}")]
    Serialize {
        /// Detalle.
        message: String,
    },
}
#[derive(Clone, Debug, Default)]
pub struct ThemeCatalog {
    /// Temas cargados, ordenados por nombre.
    pub themes: Vec<Theme>,
    /// Errores legibles, uno por archivo.
    pub errors: Vec<ThemeError>,
}
impl ThemeCatalog {
    /// Busca un tema por nombre exacto.
    pub fn find(&self, name: &str) -> Option<&Theme> {
        self.themes.iter().find(|theme| theme.meta.name == name)
    }

    /// Busca la variante de la misma familia; si no hay, la del nombre dado.
    pub fn counterpart(&self, family: &str, variant: ThemeVariant) -> Option<&Theme> {
        self.themes
            .iter()
            .find(|theme| theme.meta.family == family && theme.meta.variant == variant)
    }

    /// Añade temas sin duplicar nombres (los built-in ganan).
    pub fn extend_unique(&mut self, themes: impl IntoIterator<Item = Theme>) {
        for theme in themes {
            if self.find(&theme.meta.name).is_none() {
                self.themes.push(theme);
            }
        }
        self.themes.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
    }
}
pub fn parse_str(source: &str, label: &str) -> Result<Theme, ThemeError> {
    let document: ThemeDocument =
        toml::from_str(source).map_err(|error| parse_error(error, source, label))?;
    resolve(document, source, label)
}
