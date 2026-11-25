//! Adaptador WSL: procesos, rutas y argumentos.
//!
//! Todo se ejecuta como argv, sin shell, con `CREATE_NO_WINDOW`, timeout y
//! fuera del hilo de UI (ADR 0011). Este módulo no conoce GPUI ni el workspace;
//! solo produce rutas y listas que la app convierte en `Project`/`SpawnSpec`.

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Tiempo máximo para una operación WSL corriente (distro ya arrancada).
const TIMEOUT: Duration = Duration::from_secs(5);
/// Tiempo máximo para el primer arranque de una distro en frío.
const COLD_TIMEOUT: Duration = Duration::from_secs(12);
/// Tope defensivo para una carpeta con una salida patológica.
const MAX_DIRS: usize = 20_000;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Error accionable de una operación WSL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WslError {
    /// `wsl.exe` no existe o WSL no está instalado.
    NotAvailable(String),
    /// El proceso no respondió a tiempo.
    Timeout,
    /// La distribución no está registrada.
    UnknownDistro(String),
    /// La ruta no existe en la distribución.
    MissingPath(String),
    /// La ruta existe pero no se puede leer.
    PermissionDenied(String),
    /// La salida no es la esperada.
    InvalidPath(String),
    /// Cualquier otro fallo, con detalle.
    CommandFailed(String),
}

impl std::fmt::Display for WslError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable(detail) => write!(formatter, "WSL no está disponible: {detail}"),
            Self::Timeout => formatter.write_str("WSL no respondió a tiempo"),
            Self::UnknownDistro(distro) => {
                write!(
                    formatter,
                    "la distribución `{distro}` no existe o no está registrada"
                )
            }
            Self::MissingPath(path) => {
                write!(
                    formatter,
                    "la carpeta `{path}` no existe en la distribución"
                )
            }
            Self::PermissionDenied(path) => {
                write!(formatter, "no hay permiso para leer `{path}`")
            }
            Self::InvalidPath(detail) => formatter.write_str(detail),
            Self::CommandFailed(detail) => formatter.write_str(detail),
        }
    }
}

/// Distros WSL instaladas.
pub(crate) fn distros() -> Result<Vec<String>, WslError> {
    axf_config::wsl_distros().map_err(|detail| {
        let lower = detail.to_ascii_lowercase();
        if lower.contains("no se pudo ejecutar") || lower.contains("no está disponible") {
            WslError::NotAvailable(detail)
        } else {
            WslError::CommandFailed(detail)
        }
    })
}

/// Valida una carpeta y devuelve su ruta POSIX canónica.
///
/// Primero intenta `realpath -e`, que resuelve symlinks y distingue «no
/// existe» de «sin permiso». Si `realpath` no está en la distro, usa
/// `test -d` + `pwd -P`. No se fía del código de `pwd` a solas: con un `--cd`
/// inválido WSL imprime el error y puede ejecutar `pwd` desde `/` con código 0.
pub(crate) fn probe_directory(distro: &str, path: &str) -> Result<String, WslError> {
    let mut command = Command::new("wsl");
    command.args(["-d", distro, "--", "/usr/bin/realpath", "-e", "--", path]);
    let output = run(command, COLD_TIMEOUT)?;
    let canonical = decode(&output.stdout);
    if output.status.success() {
        let canonical = canonical.trim();
        if canonical.starts_with('/') && !canonical.is_empty() {
            return Ok(canonical.to_owned());
        }
        return Err(WslError::InvalidPath(format!(
            "`{path}` se resolvió a una ruta no absoluta: `{canonical}`"
        )));
    }
    let stderr = decode(&output.stderr);
    if output.status.code() == Some(127) || stderr.contains("realpath: not found") {
        return probe_with_test(distro, path);
    }
    Err(classify(&stderr, distro, path))
}

fn probe_with_test(distro: &str, path: &str) -> Result<String, WslError> {
    let mut command = Command::new("wsl");
    command.args(["-d", distro, "--", "/usr/bin/test", "-d", path]);
    let output = run(command, TIMEOUT)?;
    if !output.status.success() {
        return Err(classify(&decode(&output.stderr), distro, path));
    }

    let mut command = Command::new("wsl");
    command.args(["-d", distro, "--cd", path, "--", "/bin/pwd", "-P"]);
    let output = run(command, TIMEOUT)?;
    let canonical = decode(&output.stdout);
    let canonical = canonical.trim();
    if output.status.success() && canonical.starts_with('/') {
        return Ok(canonical.to_owned());
    }
    Err(classify(&decode(&output.stderr), distro, path))
}

/// Home del usuario de la distro.
pub(crate) fn home_directory(distro: &str) -> Result<String, WslError> {
    let mut command = Command::new("wsl");
    command.args(["-d", distro, "--cd", "~", "--", "/bin/pwd", "-P"]);
    let output = run(command, COLD_TIMEOUT)?;
    let home = decode(&output.stdout);
    let home = home.trim();
    if output.status.success() && home.starts_with('/') {
        return Ok(home.to_owned());
    }
    let mut command = Command::new("wsl");
    command.args(["-d", distro, "--", "/usr/bin/printenv", "HOME"]);
    let output = run(command, TIMEOUT)?;
    let home = decode(&output.stdout);
    let home = home.trim();
    if output.status.success() && home.starts_with('/') {
        return Ok(home.to_owned());
    }
    Err(classify(&decode(&output.stderr), distro, "~"))
}

/// Subcarpetas de `path`, ordenadas y sin `.`/`..`.
///
/// `find -printf` da nombres separados por NUL y soporta espacios, UTF-8 y
/// saltos de línea. Si la distro no trae ese `find`, se cae a `ls -1p`.
pub(crate) fn list_directories(distro: &str, path: &str) -> Result<Vec<String>, WslError> {
    let mut command = Command::new("wsl");
    command.args([
        "-d",
        distro,
        "--",
        "/usr/bin/find",
        "-L",
        path,
        "-mindepth",
        "1",
        "-maxdepth",
        "1",
        "-type",
        "d",
        "-printf",
        // `wsl.exe` consume una barra invertida al reenviar argv: se envían
        // dos para que `find` reciba `\0` y separe por NUL.
        "%f\\\\0",
    ]);
    let output = run(command, TIMEOUT)?;
    if output.status.success() {
        return Ok(finish_entries(parse_nul_names(&output.stdout)));
    }
    let stderr = decode(&output.stderr);
    if output.status.code() == Some(127)
        || stderr.contains("unknown predicate")
        || stderr.contains("find: not found")
    {
        return list_with_ls(distro, path);
    }
    Err(classify(&stderr, distro, path))
}

fn list_with_ls(distro: &str, path: &str) -> Result<Vec<String>, WslError> {
    let mut command = Command::new("wsl");
    command.args(["-d", distro, "--", "/bin/ls", "-1p", "--", path]);
    let output = run(command, TIMEOUT)?;
    if !output.status.success() {
        return Err(classify(&decode(&output.stderr), distro, path));
    }
    let text = decode(&output.stdout);
    let mut entries = Vec::new();
    for line in text.lines() {
        let Some(name) = line.strip_suffix('/') else {
            continue;
        };
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        entries.push(name.to_owned());
    }
    Ok(finish_entries(entries))
}

fn parse_nul_names(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

fn finish_entries(mut entries: Vec<String>) -> Vec<String> {
    entries.retain(|name| name != "." && name != "..");
    entries.sort_by(|left, right| {
        left.to_lowercase()
            .cmp(&right.to_lowercase())
            .then_with(|| left.cmp(right))
    });
    entries.truncate(MAX_DIRS);
    entries
}

/// Inserta `--cd <path>` en los argumentos de un perfil `wsl`.
///
/// Sustituye cualquier `--cd` previo y se coloca antes de la línea de comandos
/// (`--`, `-e`, `--exec`), que es donde WSL acepta opciones.
pub(crate) fn inject_cd(args: &mut Vec<String>, path: &str) {
    let mut cleaned = Vec::with_capacity(args.len() + 2);
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--cd" {
            index += 2;
            continue;
        }
        if args[index].starts_with("--cd=") {
            index += 1;
            continue;
        }
        cleaned.push(args[index].clone());
        index += 1;
    }
    let position = cleaned
        .iter()
        .position(|argument| argument == "--" || argument == "-e" || argument == "--exec")
        .unwrap_or(cleaned.len());
    cleaned.splice(position..position, ["--cd".to_owned(), path.to_owned()]);
    *args = cleaned;
}

/// `true` si el ejecutable resuelto es `wsl`/`wsl.exe`.
pub(crate) fn is_wsl_program(program: &str) -> bool {
    let name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    name == "wsl" || name == "wsl.exe"
}

fn run(mut command: Command, timeout: Duration) -> Result<Output, WslError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|error| WslError::NotAvailable(error.to_string()))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(WslError::Timeout);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(WslError::CommandFailed(error.to_string()));
            }
        }
    }
    child
        .wait_with_output()
        .map_err(|error| WslError::CommandFailed(error.to_string()))
}

fn classify(stderr: &str, distro: &str, path: &str) -> WslError {
    let detail = stderr.trim();
    let lower = detail.to_ascii_lowercase();
    if lower.contains("no distribution with the supplied name")
        || lower.contains("distribution not found")
        || lower.contains("la distribución no existe")
    {
        return WslError::UnknownDistro(distro.to_owned());
    }
    if lower.contains("permission denied") || lower.contains("operation not permitted") {
        return WslError::PermissionDenied(path.to_owned());
    }
    if lower.contains("no such file") || lower.contains("cannot access") {
        return WslError::MissingPath(path.to_owned());
    }
    if lower.contains("not installed") || lower.contains("not recognized") {
        return WslError::NotAvailable(detail.to_owned());
    }
    if detail.is_empty() {
        WslError::CommandFailed(format!("no se pudo leer `{path}` en `{distro}`"))
    } else {
        WslError::CommandFailed(format!("{detail} ({path})"))
    }
}

fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes);
    let zeroes = bytes
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|byte| **byte == 0)
        .count();
    let half = bytes.len().div_ceil(2).max(1);
    if bytes.len() >= 2 && zeroes * 2 >= half {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(test)]
#[path = "wsl_tests.rs"]
mod tests;
