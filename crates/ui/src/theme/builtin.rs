//! Temas built-in embebidos en el binario.
//!
//! Son archivos TOML normales: el mismo loader que los temas de usuario los
//! valida y los tests garantizan que todos parsean.

use super::{Theme, ThemeCatalog, parse_str};

/// Un tema embebido y su fuente TOML.
#[derive(Clone, Copy, Debug)]
pub struct BuiltinTheme {
    /// Nombre visible (debe coincidir con `[meta].name`).
    pub name: &'static str,
    /// Contenido TOML.
    pub source: &'static str,
}

/// Temas incluidos en v0.1.
pub const BUILTIN_THEMES: &[BuiltinTheme] = &[
    BuiltinTheme {
        name: "One Dark",
        source: include_str!("../../assets/themes/one-dark.toml"),
    },
    BuiltinTheme {
        name: "One Light",
        source: include_str!("../../assets/themes/one-light.toml"),
    },
    BuiltinTheme {
        name: "Monokai",
        source: include_str!("../../assets/themes/monokai.toml"),
    },
    BuiltinTheme {
        name: "Monokai Light",
        source: include_str!("../../assets/themes/monokai-light.toml"),
    },
    BuiltinTheme {
        name: "Cobalt2",
        source: include_str!("../../assets/themes/cobalt2.toml"),
    },
    BuiltinTheme {
        name: "Cobalt2 Light",
        source: include_str!("../../assets/themes/cobalt2-light.toml"),
    },
];

/// Carga los built-in; un built-in roto se omite (los tests lo detectan).
pub fn load_builtins() -> ThemeCatalog {
    let mut catalog = ThemeCatalog::default();
    for builtin in BUILTIN_THEMES {
        match parse_str(builtin.source, builtin.name) {
            Ok(theme) => catalog.themes.push(theme),
            Err(error) => catalog.errors.push(error),
        }
    }
    catalog
}

/// Busca un built-in por nombre.
pub fn builtin(name: &str) -> Option<Theme> {
    BUILTIN_THEMES
        .iter()
        .find(|builtin| builtin.name == name)
        .and_then(|builtin| parse_str(builtin.source, builtin.name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::color::Rgb;

    #[test]
    fn every_builtin_parses() {
        let catalog = load_builtins();
        assert!(
            catalog.errors.is_empty(),
            "built-in inválido: {:?}",
            catalog.errors
        );
        assert_eq!(catalog.themes.len(), BUILTIN_THEMES.len());
        for builtin in BUILTIN_THEMES {
            assert!(
                catalog.find(builtin.name).is_some(),
                "falta {}",
                builtin.name
            );
        }
    }

    #[test]
    fn required_families_have_both_variants() {
        let catalog = load_builtins();
        for family in ["One", "Monokai", "Cobalt2"] {
            assert!(
                catalog
                    .themes
                    .iter()
                    .any(|theme| theme.meta.family == family && theme.meta.variant.is_dark()),
                "falta la variante oscura de {family}"
            );
            assert!(
                catalog
                    .themes
                    .iter()
                    .any(|theme| theme.meta.family == family && !theme.meta.variant.is_dark()),
                "falta la variante clara de {family}"
            );
        }
    }

    #[test]
    fn builtin_text_passes_aa_contrast() {
        for entry in BUILTIN_THEMES {
            let theme = builtin(entry.name).expect("built-in parsea");
            let background = Rgb::from(theme.ui.background);
            let text = Rgb::from(theme.ui.text);
            let muted = Rgb::from(theme.ui.muted);
            assert!(
                text.contrast(background) >= 4.5,
                "{}: texto {:.2}",
                entry.name,
                text.contrast(background)
            );
            assert!(
                muted.contrast(background) >= 4.5,
                "{}: muted {:.2}",
                entry.name,
                muted.contrast(background)
            );
            let accent = Rgb::from(theme.ui.accent);
            let accent_text = Rgb::from(theme.ui.accent_text);
            assert!(
                accent_text.contrast(accent) >= 4.5,
                "{}: acento {:.2}",
                entry.name,
                accent_text.contrast(accent)
            );
            let focus = Rgb::from(theme.ui.focus);
            assert!(
                focus.contrast(background) >= 3.0,
                "{}: foco {:.2}",
                entry.name,
                focus.contrast(background)
            );
        }
    }

    #[test]
    fn builtin_interaction_surfaces_are_distinct_and_readable() {
        for entry in BUILTIN_THEMES {
            let theme = builtin(entry.name).expect("built-in parsea");
            assert_ne!(
                theme.ui.surface_hover, theme.ui.surface_raised,
                "{}: hover y superficie elevada no pueden coincidir",
                entry.name
            );
            assert_ne!(
                theme.ui.accent_hover, theme.ui.accent,
                "{}: hover de acento no puede coincidir con el acento",
                entry.name
            );
            assert_ne!(
                theme.ui.switch_off, theme.ui.accent,
                "{}: el switch apagado no puede usar el acento",
                entry.name
            );

            let text = Rgb::from(theme.ui.text);
            for (label, surface) in [
                ("surface_hover", theme.ui.surface_hover),
                ("surface_active", theme.ui.surface_active),
            ] {
                assert!(
                    text.contrast(Rgb::from(surface)) >= 4.5,
                    "{}: texto sobre {label}",
                    entry.name
                );
            }
            let accent_text = Rgb::from(theme.ui.accent_text);
            for (label, surface) in [
                ("accent_hover", theme.ui.accent_hover),
                ("accent_active", theme.ui.accent_active),
            ] {
                assert!(
                    accent_text.contrast(Rgb::from(surface)) >= 4.5,
                    "{}: texto de acento sobre {label}",
                    entry.name
                );
            }
        }
    }

    #[test]
    fn builtin_selected_text_remains_readable_on_ui_surfaces() {
        for entry in BUILTIN_THEMES {
            let theme = builtin(entry.name).expect("built-in parsea");
            let text = Rgb::from(theme.ui.text);
            let selection = Rgb::from(theme.ui.selection);
            for surface in [
                theme.ui.background,
                theme.ui.surface,
                theme.ui.surface_raised,
            ] {
                let selected = selection.over(Rgb::from(surface));
                assert!(
                    text.contrast(selected) >= 4.5,
                    "{}: texto seleccionado {:.2}",
                    entry.name,
                    text.contrast(selected)
                );
            }
        }
    }
}
