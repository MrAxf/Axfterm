//! Logs opt-in y crash reports locales (M9, ADR 0004).
//!
//! Sin `AXFTERM_LOG` no se instala ningún subscriber: la app no escribe logs
//! normales. Los crash reports sí se escriben siempre, en `<config>/logs`, con
//! un tope de 10 archivos. Nada sale del equipo.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing::Level;
use tracing_subscriber::fmt::writer::MakeWriter;

/// Tope del log normal; al superarlo se conserva una sola copia `.1`.
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

/// Crash reports conservados.
const MAX_CRASH_FILES: usize = 10;

const LOG_FILE: &str = "axfterm.log";
const CRASH_PREFIX: &str = "crash-";

/// Instala crash log y, si `AXFTERM_LOG` está definido, el subscriber.
///
/// Se llama antes de arrancar GPUI. Un fallo al preparar el directorio no
/// impide arrancar: simplemente no habrá logs.
pub(crate) fn init() {
    let Ok(dir) = axf_config::logs_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    prune_crashes(&dir);
    install_panic_hook(dir.clone());

    let Some(level) = level_from_env() else {
        return;
    };
    if let Err(error) = install_subscriber(&dir, level) {
        // No hay UI todavía; se deja constancia en stderr solo si existe.
        let _ = writeln!(io::stderr(), "axfterm: no se pudo iniciar el log: {error}");
    }
}

/// Nivel pedido por `AXFTERM_LOG`; `None` mantiene los logs desactivados.
fn level_from_env() -> Option<Level> {
    let value = std::env::var("AXFTERM_LOG").ok()?;
    let level = level_for(&value);
    if level.is_some()
        && !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "error" | "warn" | "warning" | "info" | "debug" | "trace"
        )
    {
        let _ = writeln!(
            io::stderr(),
            "axfterm: AXFTERM_LOG=`{value}` no es un nivel válido; se usa `info`"
        );
    }
    level
}

fn level_for(value: &str) -> Option<Level> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "off" | "none" | "0" => None,
        "error" => Some(Level::ERROR),
        "warn" | "warning" => Some(Level::WARN),
        "info" => Some(Level::INFO),
        "debug" => Some(Level::DEBUG),
        "trace" => Some(Level::TRACE),
        _ => Some(Level::INFO),
    }
}

fn install_subscriber(dir: &Path, level: Level) -> io::Result<()> {
    let path = dir.join(LOG_FILE);
    rotate_if_needed(&path, MAX_LOG_BYTES)?;
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    let writer = LogWriter {
        file: Arc::new(Mutex::new(file)),
    };
    // Si otro test o una biblioteca ya instaló un subscriber, no se pisa.
    let _ = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_ansi(false)
        .with_target(false)
        .with_writer(writer)
        .try_init();
    tracing::info!("Axfterm {} iniciado", env!("CARGO_PKG_VERSION"));
    Ok(())
}

fn rotate_if_needed(path: &Path, limit: u64) -> io::Result<()> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() <= limit {
        return Ok(());
    }
    let backup = path.with_extension("log.1");
    let _ = std::fs::remove_file(&backup);
    std::fs::rename(path, backup)
}

#[derive(Clone)]
struct LogWriter {
    file: Arc<Mutex<File>>,
}

struct LogGuard {
    file: Arc<Mutex<File>>,
}

impl Write for LogGuard {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut file = self
            .file
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        file.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file = self
            .file
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        file.flush()
    }
}

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = LogGuard;

    fn make_writer(&'a self) -> Self::Writer {
        LogGuard {
            file: Arc::clone(&self.file),
        }
    }
}

fn install_panic_hook(dir: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = crash_report(info);
        let name = format!(
            "{CRASH_PREFIX}{}-{}.log",
            axf_config::utc_stamp(),
            std::process::id()
        );
        let path = unique_path(&dir.join(name));
        let _ = axf_config::write_atomic(&path, report.as_bytes());
        prune_crashes(&dir);
        previous(info);
    }));
}

fn crash_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic sin mensaje".into());
    let location = info
        .location()
        .map(|location| {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        })
        .unwrap_or_else(|| "desconocida".into());
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("sin nombre");
    let backtrace = std::backtrace::Backtrace::force_capture();
    format!(
        "Axfterm {} — crash\n\
         momento: {}\n\
         hilo: {thread_name}\n\
         lugar: {location}\n\
         mensaje: {message}\n\n\
         backtrace:\n{backtrace}\n",
        env!("CARGO_PKG_VERSION"),
        axf_config::utc_stamp(),
    )
}

fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "crash".into());
    for attempt in 1..100 {
        let candidate = path.with_file_name(format!("{stem}-{attempt}.log"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

/// Deja los crash reports más recientes y elimina el resto.
fn prune_crashes(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut crashes: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(CRASH_PREFIX)
        })
        .map(|entry| entry.path())
        .collect();
    crashes.sort();
    let excess = crashes.len().saturating_sub(MAX_CRASH_FILES);
    for path in crashes.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_is_opt_in_and_parsed() {
        assert_eq!(level_for("off"), None);
        assert_eq!(level_for("info"), Some(Level::INFO));
        assert_eq!(level_for("TRACE"), Some(Level::TRACE));
        assert_eq!(level_for("basura"), Some(Level::INFO));
    }

    #[test]
    fn log_rotates_at_the_limit_and_keeps_one_backup() {
        let dir = std::env::temp_dir().join(format!("axfterm-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join(LOG_FILE);
        std::fs::write(&path, b"0123456789").expect("escribe");
        rotate_if_needed(&path, 5).expect("rota");
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(path.with_extension("log.1")).expect("backup"),
            "0123456789"
        );
        std::fs::write(&path, b"abc").expect("escribe");
        rotate_if_needed(&path, 5).expect("no rota");
        assert_eq!(std::fs::read_to_string(&path).expect("lee"), "abc");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn crash_pruning_keeps_the_newest() {
        let dir = std::env::temp_dir().join(format!("axfterm-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        for index in 0..(MAX_CRASH_FILES + 3) {
            std::fs::write(dir.join(format!("{CRASH_PREFIX}{index:02}.log")), b"x").expect("crash");
        }
        prune_crashes(&dir);
        let files = std::fs::read_dir(&dir).expect("lee").count();
        assert_eq!(files, MAX_CRASH_FILES);
        assert!(!dir.join("crash-00.log").exists());
        assert!(dir.join(format!("{CRASH_PREFIX}12.log")).exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
