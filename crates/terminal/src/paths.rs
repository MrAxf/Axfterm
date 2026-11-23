//! Rutas que se entregan a un proceso hijo.
//!
//! `std::fs::canonicalize` devuelve en Windows la forma verbatim
//! (`\\?\C:\...`). Usada como directorio de trabajo, PowerShell no puede
//! asociarla a una unidad y muestra
//! `Microsoft.PowerShell.Core\FileSystem::\\?\C:\...`. Quitar el prefijo
//! devuelve la ruta que el usuario espera sin perder el soporte de rutas
//! largas.

use std::path::{Path, PathBuf};

/// Longitud máxima (en unidades UTF-16) de una ruta Win32 sin `\\?\`.
#[cfg(windows)]
const MAX_PATH_WITHOUT_VERBATIM: usize = 259;

/// Devuelve la ruta sin el prefijo verbatim de Windows.
///
/// Solo se quita cuando el resultado sigue siendo una ruta absoluta normal
/// (`C:\...` o `\\servidor\recurso\...`) y no supera la longitud que admiten
/// las APIs Win32 sin prefijo; en cualquier otro caso se devuelve la entrada
/// intacta. Fuera de Windows es una copia.
pub fn normalize(path: &Path) -> PathBuf {
    #[cfg(windows)]
    if let Some(normalized) = path.to_str().and_then(strip_verbatim) {
        return PathBuf::from(normalized);
    }
    path.to_path_buf()
}

#[cfg(windows)]
fn strip_verbatim(path: &str) -> Option<String> {
    let rest = path.strip_prefix(r"\\?\")?;
    let stripped = match rest.strip_prefix(r"UNC\") {
        Some(unc) => format!(r"\\{unc}"),
        None => {
            let bytes = rest.as_bytes();
            let is_drive = bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && (bytes[2] == b'\\' || bytes[2] == b'/');
            if !is_drive {
                return None;
            }
            rest.to_owned()
        }
    };
    (stripped.encode_utf16().count() <= MAX_PATH_WITHOUT_VERBATIM).then_some(stripped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_paths_are_unchanged() {
        for path in ["", "relativa/axfterm", r"C:\Proyectos\axfterm"] {
            assert_eq!(normalize(Path::new(path)), PathBuf::from(path));
        }
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_drive_paths_lose_the_prefix() {
        assert_eq!(
            normalize(Path::new(r"\\?\C:\Proyectos\axfterm")),
            PathBuf::from(r"C:\Proyectos\axfterm")
        );
        assert_eq!(normalize(Path::new(r"\\?\c:\")), PathBuf::from(r"c:\"));
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_unc_paths_become_plain_unc() {
        assert_eq!(
            normalize(Path::new(r"\\?\UNC\servidor\recurso\carpeta")),
            PathBuf::from(r"\\servidor\recurso\carpeta")
        );
    }

    #[cfg(windows)]
    #[test]
    fn device_paths_are_kept_intact() {
        let volume = r"\\?\Volume{00000000-0000-0000-0000-000000000000}\carpeta";
        assert_eq!(normalize(Path::new(volume)), PathBuf::from(volume));
    }

    #[cfg(windows)]
    #[test]
    fn long_paths_keep_the_prefix() {
        let longest = format!(r"\\?\C:\{}", "a".repeat(MAX_PATH_WITHOUT_VERBATIM - 3));
        assert_eq!(
            normalize(Path::new(&longest)),
            PathBuf::from(longest.replace(r"\\?\C:\", r"C:\"))
        );

        let too_long = format!(r"\\?\C:\{}", "a".repeat(MAX_PATH_WITHOUT_VERBATIM));
        assert_eq!(normalize(Path::new(&too_long)), PathBuf::from(&too_long));
    }
}
