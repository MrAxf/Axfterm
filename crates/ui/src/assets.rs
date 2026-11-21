//! Fuente de assets embebida.
//!
//! Sirve primero los iconos propios (Tabler vendorizado) y delega en los
//! assets por defecto de gpui-kit, para que los componentes de la librería
//! sigan encontrando sus iconos.

use std::borrow::Cow;

use gpui_kit::{AssetSource, SharedString};

use crate::icons;

/// Iconos propios embebidos en el binario, indexados por su ruta de asset.
const OWN_ASSETS: &[(&str, &[u8])] = &[
    (
        icons::TERMINAL,
        include_bytes!("../assets/icons/tabler/terminal-2.svg"),
    ),
    (
        icons::SIDEBAR,
        include_bytes!("../assets/icons/tabler/layout-sidebar.svg"),
    ),
    (
        icons::SETTINGS,
        include_bytes!("../assets/icons/tabler/settings.svg"),
    ),
    (
        icons::PLUS,
        include_bytes!("../assets/icons/tabler/plus.svg"),
    ),
];

/// Fuente de assets de Axfterm.
#[derive(Debug, Default, Clone, Copy)]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        // Los assets propios van primero: el `load` por defecto de gpui-kit
        // devuelve error para rutas desconocidas y cortaría el fallback.
        if let Some((_, bytes)) = OWN_ASSETS.iter().find(|(asset, _)| *asset == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }

        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = OWN_ASSETS
            .iter()
            .map(|(asset, _)| *asset)
            .filter(|asset| asset.starts_with(path))
            .map(SharedString::from)
            .collect();

        paths.extend(gpui_kit::assets::Assets.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_vendored_tabler_icons() {
        for (path, _) in OWN_ASSETS {
            let bytes = Assets.load(path).expect("asset propio debe cargar");
            assert!(bytes.is_some(), "falta el asset {path}");
        }
    }

    #[test]
    fn lists_vendored_icons_under_their_prefix() {
        let paths = Assets
            .list("icons/tabler/")
            .expect("listado de iconos propios");
        for (path, _) in OWN_ASSETS {
            assert!(paths.iter().any(|listed| listed == path), "no lista {path}");
        }
    }

    #[test]
    fn unknown_asset_is_not_served() {
        assert!(Assets.load("icons/tabler/no-existe.svg").is_err());
    }
}
