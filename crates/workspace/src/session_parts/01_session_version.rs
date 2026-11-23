use crate::Workspace;
use axf_config::ProfileId;
use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
pub const SESSION_VERSION: u32 = 2;
pub const MAX_SESSION_BYTES: u64 = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionFile {
    /// Versión del esquema.
    pub version: u32,
    /// Workspace serializado.
    pub workspace: Workspace,
}
impl SessionFile {
    /// Envuelve un workspace con la versión actual.
    pub fn current(workspace: Workspace) -> Self {
        Self {
            version: SESSION_VERSION,
            workspace,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// Error de E/S.
    #[error("sesión: {0}")]
    Io(#[from] std::io::Error),
    /// JSON inválido.
    #[error("sesión inválida: {0}")]
    Json(#[from] serde_json::Error),
    /// Versión futura o desconocida.
    #[error("sesión con versión no soportada: {0}")]
    UnsupportedVersion(u32),
    /// Archivo por encima del tope.
    #[error("sesión demasiado grande: {size} bytes (máximo {max})")]
    TooLarge {
        /// Tamaño encontrado.
        size: u64,
        /// Tope permitido.
        max: u64,
    },
    /// Estructura que no se puede migrar.
    #[error("sesión sin campo version")]
    MissingVersion,
}
#[derive(Clone, Debug, Default)]
pub struct SessionLoad {
    /// Sesión válida, si la había.
    pub session: Option<SessionFile>,
    /// Aviso accionable (backup o versión no soportada).
    pub warning: Option<String>,
}
pub fn load_session(
    path: &Path,
    default_profile: &ProfileId,
) -> Result<Option<SessionFile>, SessionError> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() > MAX_SESSION_BYTES {
        return Err(SessionError::TooLarge {
            size: metadata.len(),
            max: MAX_SESSION_BYTES,
        });
    }

    let bytes = std::fs::read(path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(SessionError::MissingVersion)?;
    let version = u32::try_from(version).unwrap_or(u32::MAX);
    if version > SESSION_VERSION || version == 0 {
        return Err(SessionError::UnsupportedVersion(version));
    }

    let mut value = value;
    if version < SESSION_VERSION {
        migrate_session(&mut value, default_profile);
    }
    let mut session: SessionFile = serde_json::from_value(value)?;
    session.version = SESSION_VERSION;
    session.workspace.repair();
    Ok(Some(session))
}
fn migrate_session(value: &mut serde_json::Value, default_profile: &ProfileId) {
    if let Some(tabs) = value
        .get_mut("workspace")
        .and_then(|workspace| workspace.get_mut("tabs"))
        .and_then(serde_json::Value::as_object_mut)
    {
        for tab in tabs.values_mut() {
            let Some(tab) = tab.as_object_mut() else {
                continue;
            };
            tab.entry("profile_id")
                .or_insert_with(|| serde_json::json!(default_profile.as_str()));
            let title = tab
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            tab.entry("auto_title")
                .or_insert_with(|| serde_json::json!(title));
        }
    }
    value["version"] = serde_json::json!(SESSION_VERSION);
}
pub fn load_or_recover(path: &Path, default_profile: &ProfileId) -> SessionLoad {
    match load_session(path, default_profile) {
        Ok(session) => SessionLoad {
            session,
            warning: None,
        },
        Err(SessionError::UnsupportedVersion(version)) => SessionLoad {
            session: None,
            warning: Some(format!(
                "session.json tiene versión {version} y esta build solo entiende {SESSION_VERSION}; \
                 se arranca con un workspace limpio sin modificar el archivo."
            )),
        },
        Err(error) => {
            let backup = backup_session(path);
            let detail = match backup {
                Ok(Some(backup)) => format!(
                    "session.json no se pudo leer ({error}). Copia conservada en {}.",
                    backup.display()
                ),
                Ok(None) => format!("session.json no se pudo leer ({error})."),
                Err(backup_error) => format!(
                    "session.json no se pudo leer ({error}) y la copia falló ({backup_error})."
                ),
            };
            SessionLoad {
                session: None,
                warning: Some(detail),
            }
        }
    }
}
pub fn save_session(path: &Path, session: &SessionFile) -> Result<(), SessionError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut json = serde_json::to_vec_pretty(session)?;
    json.push(b'\n');

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temp = path.with_extension(format!("json.tmp.{}.{suffix}", std::process::id()));

    let result = (|| -> Result<(), std::io::Error> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(&json)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(error.into());
    }
    Ok(())
}
pub fn backup_session(path: &Path) -> Result<Option<PathBuf>, SessionError> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = timestamp_suffix(SystemTime::now());
    for attempt in 0..100u32 {
        let name = if attempt == 0 {
            format!("{}.bak-{stamp}", file_name(path))
        } else {
            format!("{}.bak-{stamp}-{attempt}", file_name(path))
        };
        let backup = path.with_file_name(name);
        if backup.exists() {
            continue;
        }
        std::fs::rename(path, &backup)?;
        return Ok(Some(backup));
    }
    Err(SessionError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no queda un nombre de backup libre",
    )))
}
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "session.json".into())
}
fn timestamp_suffix(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = seconds / 86_400;
    let time_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = time_of_day / 3_600;
    let minute = (time_of_day % 3_600) / 60;
    let second = time_of_day % 60;
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
}
