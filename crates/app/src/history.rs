//! Instantáneas de terminal en disco.
//!
//! Un archivo por terminal, fuera de `session.json` para no acercarse a su
//! límite. El nombre incluye un `epoch` de la ejecución: si la sesión se
//! recupera desde cero, los archivos de la vida anterior no se restauran por
//! error en una pestaña nueva con un ID reutilizado.

use std::collections::HashSet;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use axf_terminal::{MAX_SNAPSHOT_BYTES, SnapshotError, TerminalSnapshot};
use axf_workspace::TerminalId;

const EPOCH_FILE: &str = "epoch";
const SNAPSHOT_SUFFIX: &str = ".snap";

/// Error al leer o escribir el historial.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HistoryError {
    /// Error de E/S.
    #[error("historial: {0}")]
    Io(#[from] std::io::Error),
    /// Instantánea inválida.
    #[error("historial: {0}")]
    Snapshot(#[from] SnapshotError),
    /// Archivo por encima del tope.
    #[error("historial demasiado grande")]
    TooLarge,
}

impl HistoryError {
    /// `true` si el archivo no se podrá leer nunca y conviene borrarlo. Una
    /// versión futura se conserva por si otra build sabe entenderla.
    pub(crate) fn discardable(&self) -> bool {
        match self {
            Self::TooLarge => true,
            Self::Snapshot(error) => !matches!(error, SnapshotError::UnsupportedVersion(_)),
            Self::Io(_) => false,
        }
    }
}

/// Almacén de instantáneas de la ejecución actual.
#[derive(Clone, Debug)]
pub(crate) struct HistoryStore {
    dir: Option<PathBuf>,
    epoch: String,
}

impl HistoryStore {
    /// Resuelve el directorio y prepara un `epoch` nuevo cuando la sesión no
    /// se pudo restaurar (arranque limpio o recuperación tras corrupción).
    pub(crate) fn from_config(fresh_session: bool) -> Self {
        let dir = match axf_config::scrollback_dir() {
            Ok(dir) => dir,
            Err(error) => {
                tracing::warn!("no se pudo resolver el historial: {error}");
                return Self::disabled();
            }
        };
        if let Err(error) = std::fs::create_dir_all(&dir) {
            tracing::warn!("no se pudo crear {}: {error}", dir.display());
            return Self::disabled();
        }

        let existing = read_epoch(&dir);
        let epoch = if fresh_session {
            None
        } else {
            existing.clone()
        };
        let epoch = match epoch {
            Some(epoch) => epoch,
            None => {
                let epoch = new_epoch();
                if let Err(error) =
                    axf_config::write_atomic(&dir.join(EPOCH_FILE), epoch.as_bytes())
                {
                    tracing::warn!("no se pudo escribir el epoch del historial: {error}");
                    return Self::disabled();
                }
                epoch
            }
        };
        Self {
            dir: Some(dir),
            epoch,
        }
    }

    fn disabled() -> Self {
        Self {
            dir: None,
            epoch: String::new(),
        }
    }

    /// Almacén explícito para tests.
    #[cfg(test)]
    fn at(dir: PathBuf, epoch: &str) -> Self {
        std::fs::create_dir_all(&dir).expect("crea el directorio de test");
        Self {
            dir: Some(dir),
            epoch: epoch.to_owned(),
        }
    }

    /// Carga la instantánea de una terminal, si existe.
    pub(crate) fn load(
        &self,
        terminal: TerminalId,
    ) -> Result<Option<TerminalSnapshot>, HistoryError> {
        let Some(dir) = &self.dir else {
            return Ok(None);
        };
        let path = self.path(dir, terminal);
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let max = u64::try_from(MAX_SNAPSHOT_BYTES.saturating_add(4096)).unwrap_or(u64::MAX);
        if metadata.len() > max {
            return Err(HistoryError::TooLarge);
        }
        let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
        File::open(&path)?.take(max).read_to_end(&mut bytes)?;
        Ok(Some(TerminalSnapshot::decode(&bytes)?))
    }

    /// Guarda una instantánea; si está vacía, retira la anterior.
    pub(crate) fn save(
        &self,
        terminal: TerminalId,
        snapshot: &TerminalSnapshot,
    ) -> Result<(), HistoryError> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        if snapshot.is_empty() {
            self.remove(terminal);
            return Ok(());
        }
        axf_config::write_atomic(&self.path(dir, terminal), &snapshot.encode())?;
        Ok(())
    }

    /// Elimina la instantánea de una terminal cerrada.
    pub(crate) fn remove(&self, terminal: TerminalId) {
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_file(self.path(dir, terminal));
        }
    }

    /// Borra archivos de pestañas que ya no existen y de ejecuciones
    /// anteriores. `keep` son todas las terminales de la sesión, estén o no
    /// arrancadas: una pestaña sin visitar conserva su historial.
    pub(crate) fn prune(&self, keep: &HashSet<TerminalId>) {
        let Some(dir) = &self.dir else {
            return;
        };
        let prefix = format!("{}-", self.epoch);
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!("no se pudo leer {}: {error}", dir.display());
                return;
            }
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == EPOCH_FILE {
                continue;
            }
            if !name.ends_with(SNAPSHOT_SUFFIX) {
                continue;
            }
            let keep_file = name
                .strip_prefix(&prefix)
                .and_then(|rest| rest.strip_suffix(SNAPSHOT_SUFFIX))
                .and_then(|raw| raw.parse::<u64>().ok())
                .is_some_and(|id| keep.contains(&TerminalId::new(id)));
            if !keep_file {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    fn path(&self, dir: &Path, terminal: TerminalId) -> PathBuf {
        dir.join(format!(
            "{}-{}{}",
            self.epoch,
            terminal.get(),
            SNAPSHOT_SUFFIX
        ))
    }

    #[cfg(test)]
    fn epoch(&self) -> &str {
        &self.epoch
    }

    #[cfg(test)]
    fn file(&self, terminal: TerminalId) -> PathBuf {
        self.path(self.dir.as_deref().expect("directorio de test"), terminal)
    }
}

fn read_epoch(dir: &Path) -> Option<String> {
    let value = std::fs::read_to_string(dir.join(EPOCH_FILE)).ok()?;
    let value = value.trim();
    (!value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'))
    .then(|| value.to_owned())
}

fn new_epoch() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{:x}-{:x}-{:x}",
        now.as_secs(),
        now.subsec_nanos(),
        std::process::id()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("axfterm-history-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("carpeta temporal");
        dir
    }

    fn snapshot() -> TerminalSnapshot {
        let mut engine = axf_terminal::EngineBench::new(20, 4, 8, 16);
        engine.advance(b"hola\r\n\x1b[31mmundo\x1b[0m");
        engine.snapshot()
    }

    #[test]
    fn roundtrip_preserves_the_snapshot() {
        let dir = temp_dir("roundtrip");
        let store = HistoryStore::at(dir, "abc");
        let terminal = TerminalId::new(7);
        let snapshot = snapshot();
        assert!(!snapshot.is_empty());

        store.save(terminal, &snapshot).expect("guarda");
        let loaded = store
            .load(terminal)
            .expect("carga")
            .expect("existe historial");
        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn save_replaces_and_removes_old_history() {
        let dir = temp_dir("remove");
        let store = HistoryStore::at(dir, "abc");
        let terminal = TerminalId::new(3);
        store.save(terminal, &snapshot()).expect("guarda");

        let empty = axf_terminal::EngineBench::new(20, 4, 8, 16).snapshot();
        assert!(empty.is_empty());
        store.save(terminal, &empty).expect("retira");
        assert!(store.load(terminal).expect("consulta").is_none());
    }

    #[test]
    fn prune_keeps_current_tabs_and_drops_the_rest() {
        let dir = temp_dir("prune");
        let store = HistoryStore::at(dir.clone(), "abc");
        let kept = TerminalId::new(1);
        let closed = TerminalId::new(2);
        store.save(kept, &snapshot()).expect("guarda 1");
        store.save(closed, &snapshot()).expect("guarda 2");
        std::fs::write(dir.join("otro-9.snap"), b"viejo").expect("epoch antiguo");

        let mut keep = HashSet::new();
        keep.insert(kept);
        store.prune(&keep);

        assert!(store.file(kept).exists());
        assert!(!store.file(closed).exists());
        assert!(!dir.join("otro-9.snap").exists());
    }

    #[test]
    fn oversized_files_are_rejected_before_reading() {
        let dir = temp_dir("oversized");
        let store = HistoryStore::at(dir, "abc");
        let terminal = TerminalId::new(4);
        let file = std::fs::File::create(store.file(terminal)).expect("crea");
        file.set_len(u64::try_from(MAX_SNAPSHOT_BYTES + 8192).expect("cabe"))
            .expect("trunca");
        let error = store.load(terminal).expect_err("debe rechazar");
        assert!(matches!(error, HistoryError::TooLarge));
    }

    #[test]
    fn epoch_is_safe_for_file_names() {
        let epoch = new_epoch();
        assert!(!epoch.is_empty());
        assert!(
            epoch
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        );
    }

    #[test]
    fn store_is_disabled_without_a_directory() {
        let store = HistoryStore::disabled();
        assert!(store.load(TerminalId::new(1)).expect("sin error").is_none());
        assert_eq!(store.epoch(), "");
    }
}
