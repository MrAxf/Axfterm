#[cfg(windows)]
fn discover_wsl_windows() -> Vec<DetectedProfile> {
    let Ok(distros) = wsl_distros() else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut profiles = Vec::new();
    for name in distros {
        let id = format!("wsl-{}", slugify(&name));
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

/// Distros WSL instaladas, en el orden que devuelve `wsl -l -q`.
///
/// No abre una ventana, no bloquea al llamador más de 2,5 s y devuelve un
/// error accionable si WSL no responde. La UI debe llamarlo fuera del hilo de
/// UI (ADR 0011).
#[cfg(windows)]
pub fn wsl_distros() -> Result<Vec<String>, String> {
    use std::os::windows::process::CommandExt as _;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut child = Command::new("wsl")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("no se pudo ejecutar wsl: {error}"))?;
    let deadline = Instant::now() + Duration::from_millis(2_500);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("wsl -l -q no respondió a tiempo".into());
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("wsl -l -q falló: {error}"));
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("no se pudo leer wsl -l -q: {error}"))?;
    let stderr = decode_wsl_output(&output.stderr);
    let distros = parse_wsl_distros(&decode_wsl_output(&output.stdout));
    if !output.status.success() && distros.is_empty() {
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            "wsl -l -q falló".into()
        } else {
            detail.to_owned()
        });
    }
    Ok(distros)
}

/// `wsl -l -q` fuera de Windows no existe.
#[cfg(not(windows))]
pub fn wsl_distros() -> Result<Vec<String>, String> {
    Err("WSL solo está disponible en Windows".into())
}

/// Parsea la salida de `wsl -l -q` (UTF-16 ya decodificado a texto).
pub fn parse_wsl_distros(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut distros = Vec::new();
    for line in text.lines() {
        let name = line.trim().trim_matches('\0').trim();
        if name.is_empty() || name.eq_ignore_ascii_case("Windows Subsystem for Linux") {
            continue;
        }
        if seen.insert(name.to_owned()) {
            distros.push(name.to_owned());
        }
    }
    distros
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
    fn wsl_profiles_expose_their_distro() {
        let profile = Profile::new("wsl-ubuntu", "Ubuntu", r#"wsl -d "Ubuntu 22.04""#);
        assert_eq!(profile.wsl_distro().as_deref(), Some("Ubuntu 22.04"));
        let profile = Profile::new("wsl", "WSL", r"C:\Windows\System32\wsl.exe --distribution=Debian");
        assert_eq!(profile.wsl_distro().as_deref(), Some("Debian"));
        let profile = Profile::new("wsl", "WSL", "wsl -- bash -d no");
        assert_eq!(profile.wsl_distro(), None, "después de -- no hay opciones");
        assert_eq!(
            Profile::new("pwsh", "PowerShell", "pwsh -NoLogo").wsl_distro(),
            None
        );
        assert_eq!(
            Profile::new("wsl", "WSL", "wsl").wsl_distro(),
            None,
            "sin -d la distro sería la predeterminada"
        );
    }

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
    fn wsl_distro_list_filters_noise_and_duplicates() {
        let parsed = parse_wsl_distros(
            "Ubuntu\0\r\nWindows Subsystem for Linux\0\r\nDebian\0\r\nUbuntu\0\r\n",
        );
        assert_eq!(parsed, vec!["Ubuntu", "Debian"]);
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
