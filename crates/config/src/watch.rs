//! Hot reload de `config.toml` y `themes/` (ADR 0004).
//!
//! `notify` corre en su propio hilo. Un hilo de debounce absorbe ráfagas de los
//! editores y publica un único evento por canal acotado; la UI nunca se
//! bloquea esperando al sistema de archivos.

use std::path::Path;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

/// Ventana de silencio para absorber escrituras de editores.
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// Qué cambió desde el último evento.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WatchEvent {
    /// `config.toml` cambió (o se creó/eliminó).
    pub config: bool,
    /// Algún `themes/*.toml` cambió.
    pub themes: bool,
}

impl WatchEvent {
    /// Une dos eventos para no perder cambios durante el debounce.
    pub fn merge(self, other: Self) -> Self {
        Self {
            config: self.config || other.config,
            themes: self.themes || other.themes,
        }
    }
}

/// Error al registrar el watch.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    /// `notify` no pudo crear el watcher.
    #[error("no se pudo crear el watcher de configuración: {0}")]
    Notify(String),
    /// No se pudo observar una ruta.
    #[error("no se pudo observar {path}: {message}")]
    Watch {
        /// Ruta.
        path: String,
        /// Detalle.
        message: String,
    },
}

/// Watcher vivo mientras la ventana existe.
pub struct ConfigWatcher {
    /// Eventos ya debounced.
    pub events: Receiver<WatchEvent>,
    // El orden importa: al soltar el watcher se cierra el canal del hilo de
    // debounce.
    _watcher: RecommendedWatcher,
}

impl std::fmt::Debug for ConfigWatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConfigWatcher")
            .finish_non_exhaustive()
    }
}

/// Observa `config_path` y `themes_dir`.
///
/// Crea `themes/` si no existe para poder observar altas de archivos.
pub fn watch(config_path: &Path, themes_dir: &Path) -> Result<ConfigWatcher, WatchError> {
    if let Err(error) = std::fs::create_dir_all(themes_dir) {
        return Err(WatchError::Watch {
            path: themes_dir.display().to_string(),
            message: error.to_string(),
        });
    }

    let config_name = config_path.to_path_buf();
    let themes_root = themes_dir.to_path_buf();
    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<WatchEvent>();

    let handler_config = config_name.clone();
    let handler_themes = themes_root.clone();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let Ok(event) = result else {
            return;
        };
        let mut change = WatchEvent::default();
        for path in &event.paths {
            if same_file(path, &handler_config) {
                change.config = true;
            } else if is_theme_file(path, &handler_themes) {
                change.themes = true;
            }
        }
        if change != WatchEvent::default() {
            let _ = raw_tx.send(change);
        }
    })
    .map_err(|error| WatchError::Notify(error.to_string()))?;

    if let Some(parent) = config_path.parent()
        && let Err(error) = watcher.watch(parent, RecursiveMode::NonRecursive)
    {
        return Err(WatchError::Watch {
            path: parent.display().to_string(),
            message: error.to_string(),
        });
    }
    if let Err(error) = watcher.watch(themes_dir, RecursiveMode::Recursive) {
        return Err(WatchError::Watch {
            path: themes_dir.display().to_string(),
            message: error.to_string(),
        });
    }

    let (tx, rx) = async_channel::bounded(1);
    std::thread::Builder::new()
        .name("axf-config-debounce".into())
        .spawn(move || debounce_loop(raw_rx, tx))
        .map_err(|error| WatchError::Notify(format!("no se pudo crear el hilo: {error}")))?;

    Ok(ConfigWatcher {
        events: rx,
        _watcher: watcher,
    })
}

fn debounce_loop(raw: std::sync::mpsc::Receiver<WatchEvent>, tx: Sender<WatchEvent>) {
    loop {
        let Ok(mut pending) = raw.recv() else {
            return;
        };
        loop {
            match raw.recv_timeout(DEBOUNCE) {
                Ok(next) => pending = pending.merge(next),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = tx.send_blocking(pending);
                    return;
                }
            }
        }
        if tx.send_blocking(pending).is_err() {
            return;
        }
    }
}

fn same_file(path: &Path, target: &Path) -> bool {
    if path == target {
        return true;
    }
    match (path.canonicalize(), target.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn is_theme_file(path: &Path, root: &Path) -> bool {
    if !path.starts_with(root) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.') || name.ends_with('~') {
        return false;
    }
    if name.contains(".tmp") || name.contains(".bak-") {
        return false;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_merge_without_losing_flags() {
        let a = WatchEvent {
            config: true,
            themes: false,
        };
        let b = WatchEvent {
            config: false,
            themes: true,
        };
        assert_eq!(
            a.merge(b),
            WatchEvent {
                config: true,
                themes: true
            }
        );
    }

    #[test]
    fn theme_temp_files_are_ignored() {
        let root = std::env::temp_dir().join("axfterm-watch-root");
        assert!(is_theme_file(&root.join("one.toml"), &root));
        assert!(!is_theme_file(&root.join(".one.toml.swp"), &root));
        assert!(!is_theme_file(&root.join("one.toml.tmp.1"), &root));
        assert!(!is_theme_file(
            &root.join("one.toml.bak-20260101-000000"),
            &root
        ));
        assert!(!is_theme_file(&root.join("notas.txt"), &root));
    }

    #[test]
    fn watcher_reports_a_config_edit() {
        let root = std::env::temp_dir().join(format!(
            "axfterm-watch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let themes = root.join("themes");
        let config = root.join("config.toml");
        std::fs::create_dir_all(&root).expect("root");
        std::fs::write(&config, "schema_version = 1\n").expect("config");
        let watcher = watch(&config, &themes).expect("watch");

        std::fs::write(&config, "schema_version = 1\nappearance = \"dark\"\n")
            .expect("edita config");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let event = loop {
            match watcher.events.try_recv() {
                Ok(event) => break event,
                Err(async_channel::TryRecvError::Empty) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => panic!("sin evento de config: {error}"),
            }
        };
        assert!(event.config);
        let _ = std::fs::remove_dir_all(root);
    }
}
