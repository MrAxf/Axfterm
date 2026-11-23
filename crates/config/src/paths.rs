//! Rutas nativas de configuración y sesión (ADR 0004).
//!
//! Windows primero; macOS y Linux quedan tras `cfg` y se mantienen portables.
//! `AXFTERM_CONFIG_DIR` existe para tests y diagnóstico, no como modo portable
//! de producto.

use std::path::PathBuf;

/// Error al resolver el directorio de configuración.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// La variable de entorno existe pero es una ruta relativa o vacía.
    #[error("AXFTERM_CONFIG_DIR debe ser una ruta absoluta: {0}")]
    InvalidOverride(String),
    /// La plataforma no expone el directorio base esperado.
    #[error("no se pudo resolver el directorio de configuración ({0} no está definido)")]
    MissingBase(&'static str),
    /// No se pudo crear el directorio.
    #[error("no se pudo crear el directorio de configuración: {0}")]
    Io(String),
}

/// Directorio de configuración de Axfterm.
pub fn config_dir() -> Result<PathBuf, PathError> {
    if let Some(value) = std::env::var_os("AXFTERM_CONFIG_DIR") {
        let path = PathBuf::from(&value);
        if !path.is_absolute() || value.is_empty() {
            return Err(PathError::InvalidOverride(
                value.to_string_lossy().into_owned(),
            ));
        }
        return Ok(path);
    }

    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("APPDATA").ok_or(PathError::MissingBase("APPDATA"))?;
        Ok(PathBuf::from(base).join("axfterm"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or(PathError::MissingBase("HOME"))?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("axfterm"))
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let xdg = PathBuf::from(xdg);
            if xdg.is_absolute() {
                return Ok(xdg.join("axfterm"));
            }
        }
        let home = std::env::var_os("HOME").ok_or(PathError::MissingBase("HOME"))?;
        Ok(PathBuf::from(home).join(".config").join("axfterm"))
    }
}

/// Directorio de temas de usuario (`<config>/themes`).
pub fn themes_dir() -> Result<PathBuf, PathError> {
    Ok(config_dir()?.join("themes"))
}

/// Ruta de `config.toml`.
pub fn config_path() -> Result<PathBuf, PathError> {
    Ok(config_dir()?.join("config.toml"))
}

/// Ruta de `session.json`.
pub fn session_path() -> Result<PathBuf, PathError> {
    Ok(config_dir()?.join("session.json"))
}

/// Directorio de instantáneas de terminal (`<config>/scrollback`).
pub fn scrollback_dir() -> Result<PathBuf, PathError> {
    Ok(config_dir()?.join("scrollback"))
}

/// Crea el directorio de configuración si hace falta.
pub fn ensure_config_dir() -> Result<PathBuf, PathError> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir).map_err(|error| PathError::Io(error.to_string()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Las variables de entorno son globales al proceso; serializa los tests
    // que las tocan.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn override_must_be_absolute() {
        let _guard = ENV_LOCK.lock().expect("lock");
        // SAFETY: test serializado; nadie más lee esta variable en el crate.
        unsafe { std::env::set_var("AXFTERM_CONFIG_DIR", "relativo/axfterm") };
        let error = config_dir().expect_err("debe rechazar ruta relativa");
        assert!(matches!(error, PathError::InvalidOverride(_)));
        unsafe { std::env::remove_var("AXFTERM_CONFIG_DIR") };
    }

    #[test]
    fn session_and_themes_live_under_the_override() {
        let _guard = ENV_LOCK.lock().expect("lock");
        let base = std::env::temp_dir().join("axfterm-paths-test");
        // SAFETY: test serializado.
        unsafe { std::env::set_var("AXFTERM_CONFIG_DIR", &base) };
        assert_eq!(themes_dir().expect("themes"), base.join("themes"));
        assert_eq!(config_path().expect("config"), base.join("config.toml"));
        assert_eq!(session_path().expect("session"), base.join("session.json"));
        assert_eq!(ensure_config_dir().expect("crea"), base);
        assert!(base.is_dir());
        unsafe { std::env::remove_var("AXFTERM_CONFIG_DIR") };
        let _ = std::fs::remove_dir_all(base);
    }
}
