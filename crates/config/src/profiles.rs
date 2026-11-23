//! Perfiles de terminal, tokenizer de `command` y descubrimiento local.
//!
//! El crate no conoce GPUI ni abre PTYs: solo describe qué programa arrancar.
//! La app traduce [`CommandLine`] a `axf_terminal::SpawnSpec` y resuelve los
//! iconos. Las reglas del tokenizer están en ADR 0006 y `docs/profiles.md`.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Límite de perfiles de v0.1 (ADR 0005).
pub const MAX_PROFILES: usize = 32;
/// Longitud máxima del nombre visible de un perfil.
pub const MAX_PROFILE_NAME_CHARS: usize = 64;
/// Longitud máxima de la línea de comandos.
pub const MAX_PROFILE_COMMAND_CHARS: usize = 1024;
/// Tope de una imagen de icono (ADR 0005).
pub const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;

/// Identificador estable de un perfil.
///
/// Es un slug: minúsculas, dígitos, `-` y `_`, sin espacios. El nombre visible
/// se puede cambiar sin romper la sesión (ADR 0006).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(String);

impl ProfileId {
    /// Construye un id a partir de su valor textual.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Valor textual.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Perfil de respaldo cuando aún no hay configuración válida.
    pub fn fallback() -> Self {
        Self::new(fallback_profile().id.as_str())
    }

    /// `true` si el id respeta el formato de slug.
    pub fn is_valid_slug(&self) -> bool {
        let mut characters = self.0.chars();
        match characters.next() {
            Some(first) if first.is_ascii_alphanumeric() => {}
            _ => return false,
        }
        self.0.len() <= MAX_PROFILE_NAME_CHARS
            && characters.all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '-'
                    || character == '_'
            })
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for ProfileId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ProfileId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Perfil declarado en `config.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Identificador estable.
    pub id: ProfileId,
    /// Nombre visible (título inicial de la pestaña).
    pub name: String,
    /// Una línea: ejecutable + argumentos.
    pub command: String,
    /// Ruta relativa al directorio de config o absoluta. Si falta, Windows usa
    /// el icono del ejecutable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<PathBuf>,
}

impl Profile {
    /// Crea un perfil completo.
    pub fn new(
        id: impl Into<ProfileId>,
        name: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            command: command.into(),
            icon: None,
        }
    }

    /// Icono explícito.
    pub fn with_icon(mut self, icon: impl Into<PathBuf>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Tokeniza `command` en ejecutable + argv.
    pub fn command_line(&self) -> Result<CommandLine, CommandError> {
        tokenize(&self.command)
    }

    /// Stem del ejecutable para fallback de icono y diagnósticos.
    pub fn program_stem(&self) -> String {
        tokenize(&self.command)
            .map(|line| {
                Path::new(&line.program)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(&line.program)
                    .to_owned()
            })
            .unwrap_or_default()
    }
}

/// Línea de comandos tokenizada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLine {
    /// Ejecutable (primer token).
    pub program: String,
    /// Argumentos en orden.
    pub args: Vec<String>,
}

/// Error del tokenizer.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// La línea está vacía.
    #[error("el comando no puede estar vacío")]
    Empty,
    /// Falta el ejecutable.
    #[error("el comando no declara un ejecutable")]
    MissingProgram,
    /// Comilla sin cerrar.
    #[error("comilla {quote} sin cerrar")]
    UnterminatedQuote {
        /// Carácter de la comilla.
        quote: char,
    },
}

/// Tokeniza una línea de comandos con las reglas de ADR 0006.
///
/// - Espacios separan tokens.
/// - `"..."` y `'...'` agrupan; `\"` y `\'` escapan dentro de su comilla.
/// - Las barras invertidas son literales para no romper rutas Windows.
/// - No hay variables de entorno ni operadores de shell.
pub fn tokenize(command: &str) -> Result<CommandLine, CommandError> {
    if command.trim().is_empty() {
        return Err(CommandError::Empty);
    }
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut characters = command.chars().peekable();

    while let Some(character) = characters.next() {
        match quote {
            Some(active) => {
                if character == '\\' && characters.peek() == Some(&active) {
                    current.push(active);
                    characters.next();
                } else if character == active {
                    quote = None;
                } else {
                    current.push(character);
                }
            }
            None => match character {
                '"' | '\'' => {
                    quote = Some(character);
                    started = true;
                }
                character if character.is_whitespace() => {
                    if started {
                        tokens.push(std::mem::take(&mut current));
                        started = false;
                    }
                }
                character => {
                    current.push(character);
                    started = true;
                }
            },
        }
    }

    if let Some(quote) = quote {
        return Err(CommandError::UnterminatedQuote { quote });
    }
    if started {
        tokens.push(current);
    }

    if tokens.is_empty() {
        return Err(CommandError::MissingProgram);
    }
    let mut tokens = tokens.into_iter();
    let program = tokens.next().ok_or(CommandError::MissingProgram)?;
    if program.trim().is_empty() {
        return Err(CommandError::MissingProgram);
    }
    Ok(CommandLine {
        program,
        args: tokens.collect(),
    })
}

/// Perfil detectado y, si se conoce, ruta absoluta del ejecutable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedProfile {
    /// Perfil listo para escribir en config.
    pub profile: Profile,
    /// Ejecutable resuelto, para extraer su icono.
    pub executable: Option<PathBuf>,
}

/// Resultado del descubrimiento local.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Discovery {
    /// Perfiles en orden de preferencia.
    pub profiles: Vec<DetectedProfile>,
}

impl Discovery {
    /// Id del perfil por defecto (el primero).
    pub fn default_profile(&self) -> ProfileId {
        self.profiles
            .first()
            .map(|detected| detected.profile.id.clone())
            .unwrap_or_else(ProfileId::fallback)
    }

    /// Perfiles sin los metadatos de ejecutable.
    pub fn profiles_only(&self) -> Vec<Profile> {
        self.profiles
            .iter()
            .map(|detected| detected.profile.clone())
            .collect()
    }
}

/// Descubre shells nativos en PATH y rutas conocidas.
///
/// Windows consulta WSL aparte con [`discover_wsl`] para no bloquear el
/// arranque. Nunca falla: si no encuentra nada devuelve el respaldo de la
/// plataforma.
pub fn discover() -> Discovery {
    let mut found = Vec::new();
    #[cfg(windows)]
    {
        discover_windows(&mut found);
    }
    #[cfg(not(windows))]
    {
        discover_unix(&mut found);
    }
    if found.is_empty() {
        found.push(fallback_detected());
    }
    let mut seen = std::collections::HashSet::new();
    found.retain(|detected| {
        seen.insert(detected.profile.id.as_str().to_owned())
            && !detected.profile.command.trim().is_empty()
    });
    Discovery { profiles: found }
}

/// Perfil de respaldo por plataforma.
pub fn fallback_profile() -> Profile {
    fallback_detected().profile
}

fn fallback_detected() -> DetectedProfile {
    #[cfg(windows)]
    {
        DetectedProfile {
            profile: Profile::new("cmd", "Símbolo del sistema", "cmd /Q"),
            executable: resolve_executable("cmd"),
        }
    }
    #[cfg(not(windows))]
    {
        DetectedProfile {
            profile: Profile::new("sh", "sh", "/bin/sh"),
            executable: Some(PathBuf::from("/bin/sh")),
        }
    }
}

/// Resuelve un ejecutable en PATH (con extensiones en Windows).
pub fn resolve_executable(program: &str) -> Option<PathBuf> {
    let path = Path::new(program);
    if path.is_absolute() || program.contains(['/', '\\']) {
        return path.is_file().then(|| path.to_path_buf());
    }

    let directories = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    for directory in directories {
        #[cfg(windows)]
        {
            let extensions = ["", ".exe", ".cmd", ".bat", ".com"];
            for extension in extensions {
                let candidate = directory.join(format!("{program}{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        #[cfg(not(windows))]
        {
            let candidate = directory.join(program);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Comillas dobles si el ejecutable contiene espacios o comillas.
fn quote_program(program: &Path) -> String {
    let text = program.display().to_string();
    if text.contains([' ', '\t', '"']) {
        format!("\"{}\"", text.replace('"', "\\\""))
    } else {
        text
    }
}

fn detected(id: &str, name: &str, executable: PathBuf, args: &[&str]) -> DetectedProfile {
    let command = if args.is_empty() {
        quote_program(&executable)
    } else {
        format!("{} {}", quote_program(&executable), args.join(" "))
    };
    DetectedProfile {
        profile: Profile::new(id, name, command),
        executable: Some(executable),
    }
}

#[cfg(windows)]
fn discover_windows(found: &mut Vec<DetectedProfile>) {
    if let Some(pwsh) = resolve_executable("pwsh").or_else(|| {
        std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .map(|base| base.join("PowerShell").join("7").join("pwsh.exe"))
            .filter(|path| path.is_file())
    }) {
        found.push(detected("pwsh", "PowerShell 7", pwsh, &["-NoLogo"]));
    }
    if let Some(powershell) = resolve_executable("powershell").or_else(|| {
        std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|base| {
                base.join("System32")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe")
            })
            .filter(|path| path.is_file())
    }) {
        found.push(detected(
            "powershell",
            "Windows PowerShell",
            powershell,
            &["-NoLogo"],
        ));
    }
    if let Some(cmd) = resolve_executable("cmd") {
        found.push(detected("cmd", "Símbolo del sistema", cmd, &["/Q"]));
    }
    if let Some(nu) = resolve_executable("nu") {
        found.push(detected("nu", "Nu", nu, &[]));
    }
    if let Some(bash) = git_bash() {
        found.push(detected("git-bash", "Git Bash", bash, &["--login", "-i"]));
    }
}

#[cfg(windows)]
fn git_bash() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(
            PathBuf::from(program_files)
                .join("Git")
                .join("bin")
                .join("bash.exe"),
        );
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local)
                .join("Programs")
                .join("Git")
                .join("bin")
                .join("bash.exe"),
        );
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .or_else(|| resolve_executable("bash"))
}

#[cfg(not(windows))]
fn discover_unix(found: &mut Vec<DetectedProfile>) {
    if let Some(shell) = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        let id = shell
            .file_name()
            .and_then(|name| name.to_str())
            .map(slugify)
            .unwrap_or_else(|| "shell".into());
        let name = shell
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Shell")
            .to_owned();
        found.push(detected(&id, &name, shell, &[]));
    }
    for (program, id, name) in [
        ("zsh", "zsh", "Zsh"),
        ("bash", "bash", "Bash"),
        ("fish", "fish", "Fish"),
        ("nu", "nu", "Nu"),
        ("sh", "sh", "sh"),
    ] {
        if let Some(path) = resolve_executable(program)
            && !found
                .iter()
                .any(|detected| detected.profile.id.as_str() == id)
        {
            found.push(detected(id, name, path, &[]));
        }
    }
}

/// Detecta distros WSL en segundo plano. Windows únicamente.
///
/// Ejecuta `wsl -l -q` con timeout, decodifica UTF-16 y evita duplicados. Un
/// fallo o una máquina sin WSL devuelve una lista vacía.
pub fn discover_wsl() -> Vec<DetectedProfile> {
    #[cfg(windows)]
    {
        discover_wsl_windows()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
fn discover_wsl_windows() -> Vec<DetectedProfile> {
    use std::os::windows::process::CommandExt as _;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut child = match Command::new("wsl")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return Vec::new(),
    };
    let deadline = Instant::now() + Duration::from_millis(1500);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
        }
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };
    let text = decode_wsl_output(&output.stdout);
    let mut seen = std::collections::HashSet::new();
    let mut profiles = Vec::new();
    for line in text.lines() {
        let name = line.trim().trim_matches('\0').trim();
        if name.is_empty() || name.eq_ignore_ascii_case("Windows Subsystem for Linux") {
            continue;
        }
        let id = format!("wsl-{}", slugify(name));
        if !seen.insert(id.clone()) {
            continue;
        }
        profiles.push(DetectedProfile {
            profile: Profile::new(
                id,
                format!("{name} (WSL)"),
                format!("wsl -d \"{}\"", name.replace('"', "\\\"")),
            ),
            executable: resolve_executable("wsl"),
        });
    }
    profiles
}

#[cfg(windows)]
fn decode_wsl_output(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes);
    let even_zeroes = bytes
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|byte| **byte == 0)
        .count();
    let half = bytes.len().div_ceil(2).max(1);
    if bytes.len() >= 2 && even_zeroes * 2 >= half {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "perfil" } else { slug };
    let mut bounded = slug.to_owned();
    if bounded.len() > MAX_PROFILE_NAME_CHARS {
        bounded.truncate(MAX_PROFILE_NAME_CHARS);
        while bounded.ends_with('-') {
            bounded.pop();
        }
    }
    bounded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn tokenizer_handles_windows_paths_with_spaces() {
        let line = tokenize(r#""C:\Program Files\PowerShell\7\pwsh.exe" -NoLogo"#)
            .expect("comando válido");
        assert_eq!(line.program, r"C:\Program Files\PowerShell\7\pwsh.exe");
        assert_eq!(line.args, vec!["-NoLogo"]);
    }

    #[test]
    fn tokenizer_keeps_backslashes_literal() {
        let line = tokenize(r"C:\tools\shell.exe --path C:\Users\name").expect("válido");
        assert_eq!(line.program, r"C:\tools\shell.exe");
        assert_eq!(line.args, vec!["--path", r"C:\Users\name"]);
    }

    #[test]
    fn tokenizer_escapes_only_its_own_quote() {
        let line = tokenize(r#"cmd "a\"b" 'c\'d' "e\f""#).expect("válido");
        assert_eq!(line.program, "cmd");
        assert_eq!(line.args, vec!["a\"b", "c'd", r"e\f"]);
    }

    #[test]
    fn tokenizer_preserves_empty_arguments() {
        let line = tokenize(r#"cmd "" "" x"#).expect("válido");
        assert_eq!(line.args, vec!["", "", "x"]);
    }

    #[test]
    fn tokenizer_passes_shell_operators_as_arguments() {
        let line = tokenize("cmd /C echo a && echo b | more > out").expect("válido");
        assert_eq!(line.program, "cmd");
        assert_eq!(
            line.args,
            vec![
                "/C", "echo", "a", "&&", "echo", "b", "|", "more", ">", "out"
            ]
        );
    }

    #[test]
    fn invalid_commands_are_rejected() {
        assert_eq!(tokenize("   "), Err(CommandError::Empty));
        assert!(matches!(
            tokenize(r#"cmd "sin cerrar"#),
            Err(CommandError::UnterminatedQuote { quote: '"' })
        ));
        assert!(matches!(
            tokenize(r#"cmd 'sin cerrar"#),
            Err(CommandError::UnterminatedQuote { quote: '\'' })
        ));
    }

    #[test]
    fn slugs_are_stable_and_bounded() {
        assert_eq!(slugify("Ubuntu 22.04 LTS"), "ubuntu-22-04-lts");
        assert_eq!(slugify("___"), "perfil");
        assert!(ProfileId::new("git-bash").is_valid_slug());
        assert!(!ProfileId::new("").is_valid_slug());
        assert!(!ProfileId::new("Con espacios").is_valid_slug());
        assert!(!ProfileId::new("-inicio").is_valid_slug());
    }

    #[test]
    fn discovery_uses_path_and_returns_a_default() {
        let _guard = ENV_LOCK.lock().expect("lock");
        let dir = std::env::temp_dir().join(format!(
            "axfterm-discovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        #[cfg(windows)]
        let executable = dir.join("nu.exe");
        #[cfg(not(windows))]
        let executable = dir.join("nu");
        std::fs::write(&executable, b"").expect("exe falso");
        let previous = std::env::var_os("PATH");
        // SAFETY: test serializado por ENV_LOCK.
        unsafe { std::env::set_var("PATH", &dir) };
        let discovery = discover();
        if let Some(previous) = previous {
            // SAFETY: test serializado por ENV_LOCK.
            unsafe { std::env::set_var("PATH", previous) };
        } else {
            // SAFETY: test serializado por ENV_LOCK.
            unsafe { std::env::remove_var("PATH") };
        }
        assert!(!discovery.profiles.is_empty());
        assert!(
            discovery
                .profiles
                .iter()
                .any(|detected| { detected.profile.id.as_str() == "nu" })
        );
        let default = discovery.default_profile();
        assert!(
            discovery
                .profiles
                .iter()
                .any(|detected| detected.profile.id == default)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn pwsh_discovery_keeps_user_profile_enabled() {
        let _guard = ENV_LOCK.lock().expect("lock");
        let dir = std::env::temp_dir().join(format!(
            "axfterm-pwsh-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let executable = dir.join("pwsh.exe");
        std::fs::write(&executable, b"").expect("exe falso");
        let previous = std::env::var_os("PATH");
        // SAFETY: test serializado por ENV_LOCK.
        unsafe { std::env::set_var("PATH", &dir) };
        let discovery = discover();
        if let Some(previous) = previous {
            // SAFETY: test serializado por ENV_LOCK.
            unsafe { std::env::set_var("PATH", previous) };
        } else {
            // SAFETY: test serializado por ENV_LOCK.
            unsafe { std::env::remove_var("PATH") };
        }
        let _ = std::fs::remove_dir_all(dir);

        let pwsh = discovery
            .profiles
            .iter()
            .find(|detected| detected.profile.id.as_str() == "pwsh")
            .expect("pwsh detectado");
        assert!(!pwsh.profile.command.contains("-NoProfile"));
        assert_eq!(
            pwsh.profile.command,
            format!("{} -NoLogo", quote_program(&executable))
        );
    }

    #[test]
    fn wsl_output_decodes_utf16_and_utf8() {
        #[cfg(windows)]
        {
            let utf16 = "Ubuntu\r\nDebian\r\n"
                .encode_utf16()
                .flat_map(|unit| unit.to_le_bytes())
                .collect::<Vec<_>>();
            assert!(decode_wsl_output(&utf16).contains("Ubuntu"));
            assert!(decode_wsl_output(b"Ubuntu\nDebian\n").contains("Debian"));
        }
    }
}
