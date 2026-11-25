//! Rutas que se entregan a un proceso hijo.
//!
//! `std::fs::canonicalize` devuelve en Windows la forma verbatim
//! (`\\?\C:\...`). Usada como directorio de trabajo, PowerShell no puede
//! asociarla a una unidad y muestra
//! `Microsoft.PowerShell.Core\FileSystem::\\?\C:\...`. Quitar el prefijo
//! devuelve la ruta que el usuario espera sin perder el soporte de rutas
//! largas.

use std::path::{Path, PathBuf};

/// Cómo se cita una ruta soltada sobre la terminal.
///
/// El shell del perfil decide el dialecto: PowerShell/cmd/Nu usan comillas
/// dobles, los shells POSIX (Git Bash, WSL) comillas simples. En WSL además se
/// traduce la ruta de Windows al punto de montaje `/mnt/<unidad>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropPathStyle {
    /// Rutas Win32 con comillas dobles.
    #[default]
    Windows,
    /// Rutas POSIX con comillas simples (Git Bash y similares).
    Posix,
    /// Rutas POSIX traducidas desde Windows (`C:\...` -> `/mnt/c/...`).
    Wsl,
}

impl DropPathStyle {
    /// Infiere el dialecto a partir del ejecutable del perfil.
    pub fn for_program(program: &str) -> Self {
        let name = program
            .rsplit(['\\', '/'])
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(program);
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        match stem.to_ascii_lowercase().as_str() {
            "wsl" => Self::Wsl,
            "bash" | "sh" | "zsh" | "dash" | "fish" | "ksh" => Self::Posix,
            _ => Self::Windows,
        }
    }
}

/// Formatea las rutas de un «drop» para insertarlas en el prompt.
///
/// Cada ruta se cita y se separa con un espacio; el resultado termina en
/// espacio para que el usuario pueda seguir escribiendo. Las rutas con
/// caracteres de control se descartan: un nombre con salto de línea no debe
/// ejecutar nada al pegarse.
pub fn format_dropped_paths(paths: &[PathBuf], style: DropPathStyle) -> String {
    let mut text = String::new();
    for path in paths {
        let normalized = normalize(path);
        let Some(raw) = normalized.to_str() else {
            continue;
        };
        if raw.is_empty() || raw.chars().any(char::is_control) {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        match style {
            DropPathStyle::Windows => quote_windows(raw, &mut text),
            DropPathStyle::Posix => quote_posix(&raw.replace('\\', "/"), &mut text),
            DropPathStyle::Wsl => quote_posix(&wsl_path(raw), &mut text),
        }
    }
    if !text.is_empty() {
        text.push(' ');
    }
    text
}

fn quote_windows(path: &str, text: &mut String) {
    // Win32 prohíbe `"` en nombres de archivo, así que basta con envolver.
    text.push('"');
    text.push_str(path);
    text.push('"');
}

fn quote_posix(path: &str, text: &mut String) {
    text.push('\'');
    for character in path.chars() {
        if character == '\'' {
            text.push_str("'\\''");
        } else {
            text.push(character);
        }
    }
    text.push('\'');
}

fn wsl_path(path: &str) -> String {
    let bytes = path.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = path[2..].replace('\\', "/");
        return format!("/mnt/{drive}{rest}");
    }
    let lower = path.to_ascii_lowercase();
    for prefix in [r"\\wsl$\", r"\\wsl.localhost\"] {
        if lower.starts_with(prefix)
            && let Some((_distro, inner)) = path[prefix.len()..].split_once('\\')
        {
            return format!("/{}", inner.replace('\\', "/"));
        }
    }
    path.replace('\\', "/")
}

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

    #[test]
    fn dropped_windows_paths_are_quoted() {
        let paths = [
            PathBuf::from(r"C:\Proyectos\axfterm\README.md"),
            PathBuf::from(r"C:\Users\Juan Andrés\archivo con espacios.txt"),
        ];
        assert_eq!(
            format_dropped_paths(&paths, DropPathStyle::Windows),
            r#""C:\Proyectos\axfterm\README.md" "C:\Users\Juan Andrés\archivo con espacios.txt" "#
        );
    }

    #[test]
    fn dropped_posix_paths_escape_apostrophes() {
        let paths = [PathBuf::from("/home/juan/it's a file.txt")];
        assert_eq!(
            format_dropped_paths(&paths, DropPathStyle::Posix),
            "'/home/juan/it'\\''s a file.txt' "
        );
    }

    #[test]
    fn dropped_wsl_paths_use_the_mount_point() {
        let paths = [
            PathBuf::from(r"C:\Users\Juan Andrés\nota.txt"),
            PathBuf::from(r"\\wsl$\Ubuntu\home\juan\proyecto"),
        ];
        assert_eq!(
            format_dropped_paths(&paths, DropPathStyle::Wsl),
            "'/mnt/c/Users/Juan Andrés/nota.txt' '/home/juan/proyecto' "
        );
    }

    #[test]
    fn dropped_paths_with_control_characters_are_skipped() {
        let paths = [PathBuf::from("/tmp/a\nb"), PathBuf::from("/tmp/c")];
        assert_eq!(
            format_dropped_paths(&paths, DropPathStyle::Posix),
            "'/tmp/c' "
        );
    }

    #[test]
    fn drop_style_follows_the_profile_program() {
        assert_eq!(
            DropPathStyle::for_program(r"C:\Windows\System32\wsl.exe"),
            DropPathStyle::Wsl
        );
        assert_eq!(
            DropPathStyle::for_program(r"C:\Program Files\Git\bin\bash.exe"),
            DropPathStyle::Posix
        );
        assert_eq!(
            DropPathStyle::for_program(r"C:\Program Files\PowerShell\7\pwsh.exe"),
            DropPathStyle::Windows
        );
        assert_eq!(DropPathStyle::for_program("nu"), DropPathStyle::Windows);
    }
}
