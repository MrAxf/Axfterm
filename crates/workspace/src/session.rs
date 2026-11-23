//! Persistencia de la sesión (`session.json`).
//!
//! Formato versionado, escritura atómica y recuperación sin pérdida del archivo
//! original (ADR 0004). La sesión es de la app: no se recarga en caliente.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use axf_config::ProfileId;

use crate::Workspace;

/// Versión del esquema que entiende esta build.
pub const SESSION_VERSION: u32 = 2;

/// Tope de tamaño de `session.json` (ADR 0005).
pub const MAX_SESSION_BYTES: u64 = 4 * 1024 * 1024;

/// Archivo de sesión completo.
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

/// Error de carga o guardado de la sesión.
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

/// Resultado de cargar con recuperación.
#[derive(Clone, Debug, Default)]
pub struct SessionLoad {
    /// Sesión válida, si la había.
    pub session: Option<SessionFile>,
    /// Aviso accionable (backup o versión no soportada).
    pub warning: Option<String>,
}

/// Carga una sesión; `Ok(None)` si el archivo no existe.
///
/// `default_profile` rellena las pestañas de una sesión v1 al migrarla a v2.
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

/// Migra una sesión v1 → v2 rellenando `profile_id` en cada pestaña.
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

/// Carga una sesión y aplica la política de recuperación de ADR 0004.
///
/// - Corrupta o ilegible: backup fechado y arranque limpio.
/// - Versión futura: no se toca el archivo; solo aviso.
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

/// Guarda la sesión de forma atómica (temporal + `sync` + rename).
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

/// Mueve `path` a una copia fechada junto al original.
///
/// Devuelve `Ok(None)` si el archivo no existía.
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

/// `YYYYMMDD-HHMMSS` en UTC, sin dependencias de fecha.
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

/// Algoritmo civil_from_days de Howard Hinnant (UTC, proleptico gregoriano).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::SplitAxis;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("axfterm-session-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("carpeta temporal");
        dir
    }

    fn profile() -> ProfileId {
        ProfileId::new("shell")
    }

    fn sample_workspace(name: &str) -> (Workspace, PathBuf) {
        let root = temp_dir(name);
        let mut workspace = Workspace::new();
        let opened = workspace
            .open_project(&root, &profile(), "Shell")
            .expect("proyecto");
        let tab = opened.tab.expect("tab").tab;
        let pane = workspace.pane_of_tab(tab).expect("panel");
        workspace
            .split_pane(pane, SplitAxis::Vertical)
            .expect("split");
        (workspace, root)
    }

    #[test]
    fn round_trip_preserves_empty_projects_and_layout() {
        let (mut workspace, root) = sample_workspace("roundtrip");
        let empty_root = temp_dir("roundtrip-empty");
        let (empty, _) = workspace.register_project(&empty_root).expect("vacío");
        let tab_before = workspace.tab_count();
        let leaves_before = workspace.layout().leaves();
        let session = SessionFile::current(workspace);
        let path = root.join("session.json");

        save_session(&path, &session).expect("guarda");
        let loaded = load_session(&path, &profile())
            .expect("carga")
            .expect("existe");
        assert_eq!(loaded.version, SESSION_VERSION);
        assert_eq!(loaded.workspace.tab_count(), tab_before);
        assert_eq!(loaded.workspace.layout().leaves(), leaves_before);
        assert!(loaded.workspace.project(empty).is_some());
    }

    #[test]
    fn atomic_save_leaves_no_temporary_file() {
        let (workspace, root) = sample_workspace("atomic");
        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .expect("lee dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "temporales: {leftovers:?}");
    }

    #[test]
    fn v1_session_migrates_to_v2_with_the_default_profile() {
        let (workspace, root) = sample_workspace("migrate-v1");
        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("lee")).expect("json");
        value["version"] = serde_json::json!(1);
        if let Some(tabs) = value["workspace"]["tabs"].as_object_mut() {
            for tab in tabs.values_mut() {
                if let Some(object) = tab.as_object_mut() {
                    object.remove("profile_id");
                    object.remove("auto_title");
                }
            }
        }
        std::fs::write(&path, serde_json::to_vec_pretty(&value).expect("json")).expect("escribe");

        let default = ProfileId::new("pwsh");
        let loaded = load_session(&path, &default)
            .expect("carga")
            .expect("existe");
        assert_eq!(loaded.version, SESSION_VERSION);
        let tabs: Vec<_> = loaded
            .workspace
            .project_tabs(loaded.workspace.projects()[0].id);
        assert!(!tabs.is_empty());
        for tab in tabs {
            let tab = loaded.workspace.tab(tab).expect("tab");
            assert_eq!(tab.profile_id, default);
            assert!(!tab.auto_title.is_empty());
        }
    }

    #[test]
    fn future_version_is_rejected_without_touching_the_file() {
        let (workspace, root) = sample_workspace("future");
        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("lee")).expect("json");
        value["version"] = serde_json::json!(SESSION_VERSION + 1);
        std::fs::write(&path, serde_json::to_vec_pretty(&value).expect("json")).expect("escribe");

        let load = load_or_recover(&path, &profile());
        assert!(load.session.is_none());
        assert!(load.warning.is_some());
        assert!(path.exists(), "una versión futura no se mueve");
    }

    #[test]
    fn corrupt_session_is_backed_up() {
        let root = temp_dir("corrupt");
        let path = root.join("session.json");
        std::fs::write(&path, b"{ esto no es json").expect("escribe");

        let load = load_or_recover(&path, &profile());
        assert!(load.session.is_none());
        assert!(load.warning.is_some());
        assert!(!path.exists());
        let backups: Vec<_> = std::fs::read_dir(&root)
            .expect("lee dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".bak-"))
            .collect();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn oversized_session_is_rejected_before_parsing() {
        let root = temp_dir("oversized");
        let path = root.join("session.json");
        let file = std::fs::File::create(&path).expect("crea");
        file.set_len(MAX_SESSION_BYTES + 1).expect("trunca");
        let error = load_session(&path, &profile()).expect_err("debe rechazar");
        assert!(matches!(error, SessionError::TooLarge { .. }));
    }

    #[test]
    fn missing_version_is_an_error() {
        let root = temp_dir("missing-version");
        let path = root.join("session.json");
        std::fs::write(&path, br#"{"workspace":{}}"#).expect("escribe");
        let error = load_session(&path, &profile()).expect_err("debe rechazar");
        assert!(matches!(error, SessionError::MissingVersion));
    }

    #[test]
    fn timestamp_has_the_expected_shape() {
        let stamp = timestamp_suffix(UNIX_EPOCH);
        assert_eq!(stamp, "19700101-000000");
        let stamp = timestamp_suffix(UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000));
        assert_eq!(stamp, "20231114-221320");
    }

    #[test]
    fn stress_four_panes_twenty_tabs_three_projects_round_trip() {
        let mut workspace = Workspace::new();
        let mut projects = Vec::new();
        for index in 0..3 {
            let root = temp_dir(&format!("stress-{index}"));
            let opened = workspace
                .open_project(&root, &profile(), "Shell")
                .expect("proyecto");
            projects.push(opened.project);
        }
        // Un proyecto queda vacío en la sidebar (el tercero no tiene pestaña
        // propia todavía): se registra y debe sobrevivir a la sesión.
        let empty_root = temp_dir("stress-empty");
        let (empty, _) = workspace.register_project(&empty_root).expect("vacío");

        let mut pane = workspace.focus().pane;
        for axis in [
            SplitAxis::Horizontal,
            SplitAxis::Vertical,
            SplitAxis::Horizontal,
        ] {
            pane = workspace.split_pane(pane, axis).expect("split");
        }
        let leaves = workspace.layout().leaves();
        assert_eq!(leaves.len(), 4);

        while workspace.tab_count() < 20 {
            let project = projects[workspace.tab_count() % projects.len()];
            let pane = leaves[workspace.tab_count() % leaves.len()];
            workspace
                .new_tab(pane, project, &profile(), "Shell")
                .expect("pestaña");
        }
        assert_eq!(workspace.tab_count(), 20);

        // Mezcla de proyectos en un panel y movimiento entre paneles.
        let first = workspace.project_tabs(projects[0])[0];
        let target = leaves[3];
        workspace.move_tab(first, target, 0).expect("mueve");
        assert_eq!(workspace.focus().tab, Some(first));
        assert_eq!(workspace.focus().pane, target);

        let root = temp_dir("stress-session");
        let path = root.join("session.json");
        let before_leaves = workspace.layout().leaves();
        let before_tabs = workspace.tab_count();
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let loaded = load_session(&path, &profile())
            .expect("carga")
            .expect("existe")
            .workspace;
        assert_eq!(loaded.tab_count(), before_tabs);
        assert_eq!(loaded.layout().leaves(), before_leaves);
        assert_eq!(loaded.projects().len(), 4);
        assert!(loaded.project(empty).is_some());
    }

    #[test]
    fn round_trip_preserves_focus_on_a_later_pane() {
        let mut workspace = Workspace::new();
        let root = temp_dir("focus-later");
        let project = workspace
            .open_project(&root, &profile(), "Shell")
            .expect("proyecto")
            .project;
        let first = workspace.focus().pane;
        let second = workspace
            .split_pane(first, SplitAxis::Horizontal)
            .expect("split");
        let third = workspace
            .split_pane(second, SplitAxis::Vertical)
            .expect("split");
        workspace
            .new_tab(third, project, &profile(), "Shell")
            .expect("tab tercero");
        workspace.focus_pane(third).expect("foco tercero");
        let tab = workspace.focus().tab.expect("tab");

        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let loaded = load_session(&path, &profile())
            .expect("carga")
            .expect("existe")
            .workspace;
        assert_eq!(loaded.focus().pane, third);
        assert_eq!(loaded.focus().tab, Some(tab));
    }

    #[test]
    fn loaded_session_is_repaired() {
        let (workspace, root) = sample_workspace("repair-load");
        let path = root.join("session.json");
        save_session(&path, &SessionFile::current(workspace)).expect("guarda");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("lee")).expect("json");
        // Arranca un ratio imposible y un foco a un panel inexistente.
        value["workspace"]["layout"]["value"]["ratio"] = serde_json::json!(99.0);
        value["workspace"]["focus"]["pane"] = serde_json::json!(9999);
        std::fs::write(&path, serde_json::to_vec_pretty(&value).expect("json")).expect("escribe");

        let loaded = load_session(&path, &profile())
            .expect("carga")
            .expect("existe");
        assert_eq!(
            loaded.workspace.layout().ratio_at(&[]),
            Some(1.0 - crate::layout::MIN_RATIO)
        );
        let focus_pane = loaded.workspace.focus().pane;
        assert!(loaded.workspace.layout().leaves().contains(&focus_pane));
    }
}
