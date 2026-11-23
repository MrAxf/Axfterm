//! Iconos de perfil: ruta manual y extracción del icono del ejecutable.
//!
//! El modelo de config solo guarda una ruta. Este módulo es el adaptador de
//! plataforma (ADR 0006): en Windows extrae el icono del `.exe` con la API de
//! shell y GDI, lo convierte a PNG con alpha y lo cachea en
//! `<config>/icons/cache/`. En macOS/Linux devuelve `None` y la UI usa el
//! fallback Tabler.

use std::path::{Path, PathBuf};

/// Tope de una imagen de icono de perfil (ADR 0005).
pub const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;

/// Tope de archivos de la caché de iconos extraídos.
pub const MAX_CACHE_FILES: usize = 64;

/// Error al resolver un icono. La config sigue siendo válida: la UI cae al
/// icono genérico y puede mostrar el aviso.
#[derive(Debug, thiserror::Error)]
pub enum IconError {
    /// La ruta manual no existe o no es un archivo.
    #[error("el icono `{path}` no existe")]
    Missing {
        /// Ruta pedida.
        path: String,
    },
    /// La imagen supera el tope de 2 MiB.
    #[error("el icono `{path}` supera el límite de 2 MiB")]
    TooLarge {
        /// Ruta pedida.
        path: String,
    },
    /// La extracción del ejecutable falló.
    #[error("no se pudo extraer el icono de `{path}`: {message}")]
    Extract {
        /// Ejecutable.
        path: String,
        /// Causa.
        message: String,
    },
    /// La caché no se pudo escribir.
    #[error("no se pudo escribir la caché de iconos: {0}")]
    Cache(String),
}

/// Caché acotada de iconos extraídos.
#[derive(Clone, Debug)]
pub struct IconCache {
    root: PathBuf,
}

impl IconCache {
    /// Crea la caché en `<config>/icons/cache/`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Directorio de la caché.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resuelve el icono de un perfil.
    ///
    /// `manual` es la ruta ya resuelta contra el directorio de config.
    /// `executable` es la ruta absoluta del programa, si se conoce.
    pub fn resolve(
        &self,
        manual: Option<&Path>,
        executable: Option<&Path>,
        base: &Path,
    ) -> Result<Option<PathBuf>, IconError> {
        if let Some(manual) = manual {
            let path = if manual.is_absolute() {
                manual.to_path_buf()
            } else {
                base.join(manual)
            };
            return validate_manual(&path).map(Some);
        }
        let Some(executable) = executable else {
            return Ok(None);
        };
        #[cfg(windows)]
        {
            self.extract_cached(executable).map(Some)
        }
        #[cfg(not(windows))]
        {
            let _ = executable;
            Ok(None)
        }
    }

    #[cfg(windows)]
    fn extract_cached(&self, executable: &Path) -> Result<PathBuf, IconError> {
        let metadata = std::fs::metadata(executable).map_err(|error| IconError::Extract {
            path: executable.display().to_string(),
            message: error.to_string(),
        })?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let key = cache_key(executable, modified, metadata.len());
        let destination = self.root.join(format!("{key}.png"));
        if destination.is_file() {
            // Un toque mantiene vivos los iconos en uso al limpiar la caché.
            let _ = filetime_touch(&destination);
            return Ok(destination);
        }
        std::fs::create_dir_all(&self.root).map_err(|error| IconError::Cache(error.to_string()))?;
        let temp = self
            .root
            .join(format!("{key}.tmp.{}.png", std::process::id()));
        extract_executable_icon(executable, &temp).map_err(|error| match error {
            IconError::Extract { message, .. } => IconError::Extract {
                path: executable.display().to_string(),
                message,
            },
            other => other,
        })?;
        std::fs::rename(&temp, &destination).map_err(|error| {
            let _ = std::fs::remove_file(&temp);
            IconError::Cache(error.to_string())
        })?;
        prune_cache(&self.root, MAX_CACHE_FILES);
        Ok(destination)
    }
}

fn validate_manual(path: &Path) -> Result<PathBuf, IconError> {
    let metadata = std::fs::metadata(path).map_err(|_| IconError::Missing {
        path: path.display().to_string(),
    })?;
    if !metadata.is_file() {
        return Err(IconError::Missing {
            path: path.display().to_string(),
        });
    }
    if metadata.len() > MAX_ICON_BYTES {
        return Err(IconError::TooLarge {
            path: path.display().to_string(),
        });
    }
    Ok(path.to_path_buf())
}

/// Clave estable de caché: ejecutable + mtime + tamaño.
pub fn cache_key(executable: &Path, modified_nanos: u128, len: u64) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let seed = format!(
        "{}\0{}\0{}",
        executable.to_string_lossy().to_lowercase(),
        modified_nanos,
        len
    );
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Borra los archivos más antiguos hasta dejar como máximo `max`.
pub fn prune_cache(root: &Path, max: usize) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((modified, path))
        })
        .collect();
    if files.len() <= max {
        return;
    }
    files.sort_by_key(|(modified, _)| *modified);
    let remove = files.len() - max;
    for (_, path) in files.into_iter().take(remove) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(windows)]
fn filetime_touch(path: &Path) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.set_modified(std::time::SystemTime::now())
}

/// Extrae el icono grande de un ejecutable Windows y lo guarda como PNG RGBA.
#[cfg(windows)]
pub fn extract_executable_icon(executable: &Path, destination: &Path) -> Result<(), IconError> {
    use std::os::windows::ffi::OsStrExt as _;

    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, GdiFlush, GetDC, ReleaseDC, SelectObject,
    };
    use windows_sys::Win32::UI::Shell::ExtractIconExW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx, HICON};

    const ICON_SIZE: i32 = 64;

    let wide: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut large: HICON = std::ptr::null_mut();
    let mut small: HICON = std::ptr::null_mut();
    // SAFETY: punteros válidos a HICON y cadena UTF-16 terminada en NUL.
    let count = unsafe { ExtractIconExW(wide.as_ptr(), 0, &mut large, &mut small, 1) };
    if count == 0 || (large.is_null() && small.is_null()) {
        if !large.is_null() {
            // SAFETY: el HICON lo devolvió `ExtractIconExW` y no se usa después.
            unsafe { DestroyIcon(large) };
        }
        if !small.is_null() {
            // SAFETY: idem.
            unsafe { DestroyIcon(small) };
        }
        return Err(IconError::Extract {
            path: executable.display().to_string(),
            message: "el ejecutable no expone iconos".into(),
        });
    }
    let icon = if large.is_null() { small } else { large };

    let result = (|| -> Result<Vec<u8>, IconError> {
        // SAFETY: DC de pantalla; los recursos se liberan al final del bloque.
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            if screen.is_null() {
                return Err(IconError::Cache("GetDC devolvió NULL".into()));
            }
            let memory = CreateCompatibleDC(screen);
            if memory.is_null() {
                ReleaseDC(std::ptr::null_mut(), screen);
                return Err(IconError::Cache("CreateCompatibleDC devolvió NULL".into()));
            }

            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ICON_SIZE,
                // Altura negativa: filas de arriba abajo.
                biHeight: -ICON_SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                memory,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(memory);
                ReleaseDC(std::ptr::null_mut(), screen);
                return Err(IconError::Cache("CreateDIBSection falló".into()));
            }
            let previous = SelectObject(memory, bitmap);
            GdiFlush();
            DrawIconEx(
                memory,
                0,
                0,
                icon,
                ICON_SIZE,
                ICON_SIZE,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            );
            GdiFlush();

            let length = (ICON_SIZE * ICON_SIZE * 4) as usize;
            // SAFETY: `CreateDIBSection` reserva exactamente ese tamaño BGRA.
            let raw = std::slice::from_raw_parts(bits as *const u8, length);
            let mut rgba = Vec::with_capacity(length);
            let mut any_alpha = false;
            for pixel in raw.chunks_exact(4) {
                any_alpha |= pixel[3] != 0;
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
            if !any_alpha {
                // Algunos iconos antiguos no traen canal alpha: opacos es mejor
                // que un cuadrado invisible.
                for pixel in rgba.chunks_exact_mut(4) {
                    pixel[3] = 255;
                }
            }

            SelectObject(memory, previous);
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(std::ptr::null_mut(), screen);
            Ok(rgba)
        }
    })();

    // SAFETY: ambos HICON vienen de `ExtractIconExW` y ya no se usan.
    unsafe {
        if !large.is_null() {
            DestroyIcon(large);
        }
        if !small.is_null() {
            DestroyIcon(small);
        }
    }

    let rgba = result?;
    write_png(destination, ICON_SIZE as u32, &rgba)
}

#[cfg(windows)]
fn write_png(destination: &Path, size: u32, rgba: &[u8]) -> Result<(), IconError> {
    use std::io::BufWriter;
    let file =
        std::fs::File::create(destination).map_err(|error| IconError::Cache(error.to_string()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| IconError::Cache(error.to_string()))?;
    writer
        .write_image_data(rgba)
        .map_err(|error| IconError::Cache(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_depends_on_path_mtime_and_size() {
        let path = Path::new(r"C:\Program Files\PowerShell\7\pwsh.exe");
        let a = cache_key(path, 1, 10);
        let b = cache_key(path, 1, 10);
        let c = cache_key(path, 2, 10);
        let d = cache_key(path, 1, 11);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn missing_and_oversized_manual_icons_fall_back() {
        let dir = std::env::temp_dir().join(format!(
            "axfterm-icon-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let cache = IconCache::new(dir.join("cache"));
        let missing = cache
            .resolve(Some(Path::new("no-existe.png")), None, &dir)
            .expect_err("debe faltar");
        assert!(matches!(missing, IconError::Missing { .. }));

        let big = dir.join("big.png");
        let file = std::fs::File::create(&big).expect("crea");
        file.set_len(MAX_ICON_BYTES + 1).expect("trunca");
        let too_large = cache
            .resolve(Some(&big), None, &dir)
            .expect_err("debe rechazar");
        assert!(matches!(too_large, IconError::TooLarge { .. }));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn manual_icon_is_resolved_relative_to_config() {
        let dir = std::env::temp_dir().join(format!(
            "axfterm-icon-rel-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let icon = dir.join("icon.png");
        std::fs::write(&icon, b"png").expect("escribe");
        let cache = IconCache::new(dir.join("cache"));
        let resolved = cache
            .resolve(Some(Path::new("icon.png")), None, &dir)
            .expect("icono válido")
            .expect("existe");
        assert_eq!(resolved, icon);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn extracts_a_png_with_alpha_from_cmd_exe() {
        let executable = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|base| base.join("System32").join("cmd.exe"))
            .expect("SystemRoot");
        assert!(executable.is_file(), "cmd.exe debe existir");
        let dir = std::env::temp_dir().join(format!(
            "axfterm-icon-extract-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let destination = dir.join("cmd.png");
        extract_executable_icon(&executable, &destination).expect("extrae");
        let bytes = std::fs::read(&destination).expect("lee png");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(bytes.len() > 100);

        let cache = IconCache::new(dir.join("cache"));
        let cached = cache
            .resolve(None, Some(&executable), &dir)
            .expect("cache")
            .expect("icono");
        assert!(cached.is_file());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pruning_keeps_the_newest_files() {
        let dir = std::env::temp_dir().join(format!(
            "axfterm-icon-prune-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        for index in 0..6 {
            let path = dir.join(format!("{index}.png"));
            std::fs::write(&path, b"x").expect("escribe");
            let time = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(index as u64 + 1);
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .expect("abre");
            file.set_modified(time).expect("mtime");
        }
        prune_cache(&dir, 3);
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .expect("lee")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["3.png", "4.png", "5.png"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
