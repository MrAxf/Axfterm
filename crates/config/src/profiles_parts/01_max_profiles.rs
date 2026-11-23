use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
pub const MAX_PROFILES: usize = 32;
pub const MAX_PROFILE_NAME_CHARS: usize = 64;
pub const MAX_PROFILE_COMMAND_CHARS: usize = 1024;
pub const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLine {
    /// Ejecutable (primer token).
    pub program: String,
    /// Argumentos en orden.
    pub args: Vec<String>,
}
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedProfile {
    /// Perfil listo para escribir en config.
    pub profile: Profile,
    /// Ejecutable resuelto, para extraer su icono.
    pub executable: Option<PathBuf>,
}
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
