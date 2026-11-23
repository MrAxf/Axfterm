//! Fuente de assets embebida.
//!
//! Sirve primero los iconos propios (Tabler/simple-icons vendorizados) y delega
//! en los assets por defecto de gpui-kit, para que los componentes de la
//! librería sigan encontrando sus iconos.

use std::borrow::Cow;

use gpui_kit::{AssetSource, SharedString};

use crate::icons;

macro_rules! own_assets {
    ($($path:expr => $file:literal),* $(,)?) => {
        const OWN_ASSETS: &[(&str, &[u8])] = &[
            $(($path, include_bytes!($file))),*
        ];
    };
}

// Pipeline de SVG embebido: una sola tabla es la fuente de verdad. Los assets
// se vendorizan con `scripts/update-icons.ps1`.
own_assets! {
    icons::TERMINAL => "../assets/icons/tabler/terminal-2.svg",
    icons::SIDEBAR => "../assets/icons/tabler/layout-sidebar.svg",
    icons::SETTINGS => "../assets/icons/tabler/settings.svg",
    icons::PLUS => "../assets/icons/tabler/plus.svg",
    icons::CLOSE => "../assets/icons/tabler/x.svg",
    icons::WINDOW_MINIMIZE => "../assets/icons/tabler/minus.svg",
    icons::WINDOW_MAXIMIZE => "../assets/icons/tabler/square.svg",
    icons::CHEVRON_DOWN => "../assets/icons/tabler/chevron-down.svg",
    icons::CHEVRON_RIGHT => "../assets/icons/tabler/chevron-right.svg",
    icons::CHEVRON_LEFT => "../assets/icons/tabler/chevron-left.svg",
    icons::CHEVRON_UP => "../assets/icons/tabler/chevron-up.svg",
    icons::FOLDER => "../assets/icons/tabler/folder.svg",
    icons::FOLDER_OPEN => "../assets/icons/tabler/folder-open.svg",
    icons::SPLIT_HORIZONTAL => "../assets/icons/tabler/layout-columns.svg",
    icons::SPLIT_VERTICAL => "../assets/icons/tabler/layout-rows.svg",
    icons::RENAME => "../assets/icons/tabler/pencil.svg",
    icons::DUPLICATE => "../assets/icons/tabler/copy.svg",
    icons::ZOOM => "../assets/icons/tabler/maximize.svg",
    icons::RESTART => "../assets/icons/tabler/refresh.svg",
    icons::SEARCH => "../assets/icons/tabler/search.svg",
    icons::SUN => "../assets/icons/tabler/sun.svg",
    icons::MOON => "../assets/icons/tabler/moon.svg",
    icons::PALETTE => "../assets/icons/tabler/palette.svg",
    icons::MENU => "../assets/icons/tabler/menu-2.svg",
    icons::KEYBOARD => "../assets/icons/tabler/keyboard.svg",
    icons::DEVICE_DESKTOP => "../assets/icons/tabler/device-desktop.svg",
    icons::EXTERNAL_LINK => "../assets/icons/tabler/external-link.svg",
    icons::FILE_IMPORT => "../assets/icons/tabler/file-import.svg",
    icons::FILE_EXPORT => "../assets/icons/tabler/file-export.svg",
    icons::CHECK => "../assets/icons/tabler/check.svg",
    icons::CIRCLE => "../assets/icons/tabler/circle.svg",
    icons::ALERT_TRIANGLE => "../assets/icons/tabler/alert-triangle.svg",
    icons::INFO_CIRCLE => "../assets/icons/tabler/info-circle.svg",
    icons::PLAYER_PLAY => "../assets/icons/tabler/player-play.svg",
    icons::PLAYER_PAUSE => "../assets/icons/tabler/player-pause.svg",
    icons::STOP => "../assets/icons/tabler/player-stop.svg",
    icons::LOADER => "../assets/icons/tabler/loader-2.svg",
    icons::BELL => "../assets/icons/tabler/bell.svg",
    icons::BELL_OFF => "../assets/icons/tabler/bell-off.svg",
    icons::BRAND_POWERSHELL => "../assets/icons/tabler/brand-powershell.svg",
    icons::RUST => "../assets/icons/simple/rust.svg",
    icons::BRAND_OPENCODE => "../assets/icons/simple/opencode.svg",
    icons::BRAND_COPILOT => "../assets/icons/simple/githubcopilot.svg",
}

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
    fn every_icon_constant_is_served() {
        for path in icons::ALL {
            let bytes = Assets.load(path).expect("asset propio debe cargar");
            assert!(bytes.is_some(), "falta el asset {path}");
        }
    }

    #[test]
    fn asset_table_and_icon_list_agree() {
        assert_eq!(OWN_ASSETS.len(), icons::ALL.len());
        for path in icons::ALL {
            assert!(
                OWN_ASSETS.iter().any(|(asset, _)| asset == path),
                "no está en la tabla: {path}"
            );
        }
    }

    #[test]
    fn lists_vendored_icons_under_their_prefix() {
        let paths = Assets.list("icons/").expect("listado de iconos propios");
        for path in icons::ALL {
            assert!(paths.iter().any(|listed| listed == path), "no lista {path}");
        }
    }

    #[test]
    fn unknown_asset_is_not_served() {
        assert!(Assets.load("icons/tabler/no-existe.svg").is_err());
    }
}
