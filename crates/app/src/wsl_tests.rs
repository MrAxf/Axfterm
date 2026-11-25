use super::*;

#[test]
fn cd_is_inserted_before_the_command_line() {
    let mut args = vec!["-d".into(), "Ubuntu".into()];
    inject_cd(&mut args, "/home/juan/app");
    assert_eq!(
        args,
        ["-d", "Ubuntu", "--cd", "/home/juan/app"]
            .map(str::to_owned)
            .to_vec()
    );

    let mut args = vec![
        "-d".into(),
        "Ubuntu".into(),
        "--".into(),
        "bash".into(),
        "-l".into(),
    ];
    inject_cd(&mut args, "/home/juan/app");
    assert_eq!(
        args,
        ["-d", "Ubuntu", "--cd", "/home/juan/app", "--", "bash", "-l"]
            .map(str::to_owned)
            .to_vec()
    );
}

#[test]
fn cd_replaces_an_existing_value() {
    let mut args = vec![
        "-d".into(),
        "Ubuntu".into(),
        "--cd".into(),
        "/viejo".into(),
        "-u".into(),
        "root".into(),
    ];
    inject_cd(&mut args, "/nuevo");
    assert_eq!(
        args,
        ["-d", "Ubuntu", "-u", "root", "--cd", "/nuevo"]
            .map(str::to_owned)
            .to_vec()
    );

    let mut args = vec!["-d".into(), "Ubuntu".into(), "--cd=/viejo".into()];
    inject_cd(&mut args, "/nuevo");
    assert_eq!(
        args,
        ["-d", "Ubuntu", "--cd", "/nuevo"]
            .map(str::to_owned)
            .to_vec()
    );
}

#[test]
fn wsl_program_detection_uses_the_file_stem() {
    assert!(is_wsl_program("wsl"));
    assert!(is_wsl_program("wsl.exe"));
    assert!(is_wsl_program(r"C:\Windows\System32\wsl.exe"));
    assert!(!is_wsl_program("pwsh"));
    assert!(!is_wsl_program("my-wsl-wrapper"));
}

#[test]
fn nul_names_keep_spaces_and_unicode() {
    let names = parse_nul_names("carpeta con espacios\0ñandú\0".as_bytes());
    assert_eq!(
        names,
        ["carpeta con espacios", "ñandú"]
            .map(str::to_owned)
            .to_vec()
    );
}

#[test]
fn entries_are_sorted_case_insensitively_and_skip_dots() {
    let entries = finish_entries(vec!["zeta".into(), "Alfa".into(), ".".into(), "..".into()]);
    assert_eq!(entries, ["Alfa", "zeta"].map(str::to_owned).to_vec());
}

#[cfg(windows)]
#[test]
#[ignore = "requiere WSL instalado en el equipo"]
fn real_wsl_conpty_spawn() {
    use axf_terminal::{PtySizeSpec, SpawnSpec, TerminalSession};
    use std::path::Path;
    use std::time::{Duration, Instant};

    let (sender, receiver) = async_channel::bounded(64);
    let spec = SpawnSpec::new(
        "wsl",
        ["-d", "Ubuntu", "--cd", "/home", "--", "/bin/pwd"]
            .map(str::to_owned)
            .to_vec(),
    );
    let mut session =
        TerminalSession::spawn(Path::new(""), PtySizeSpec::new(40, 8, 8, 16), sender, &spec)
            .expect("ConPTY arranca wsl");
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut text = Vec::new();
    let mut answered_dsr = false;
    while Instant::now() < deadline {
        match receiver.try_recv() {
            Ok(axf_terminal::engine::ParserCommand::Bytes(bytes)) => {
                text.extend(bytes);
                if !answered_dsr && text.windows(4).any(|window| window == b"\x1b[6n") {
                    answered_dsr = true;
                    if let Some(writer) = session.writer() {
                        let _ = writer.write(b"\x1b[1;1R");
                    }
                }
                if String::from_utf8_lossy(&text).contains("/home") {
                    break;
                }
            }
            Ok(axf_terminal::engine::ParserCommand::Eof)
            | Ok(axf_terminal::engine::ParserCommand::Exited(_)) => break,
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    session.kill();
    let text = String::from_utf8_lossy(&text);
    assert!(text.contains("/home"), "salida: {text:?}");
}

#[test]
#[ignore = "requiere WSL instalado en el equipo"]
fn real_wsl_roundtrip() {
    let distros = distros().expect("WSL disponible");
    let distro = distros
        .iter()
        .find(|distro| distro.as_str() == "Ubuntu")
        .or_else(|| distros.first())
        .expect("alguna distro");
    let home = home_directory(distro).expect("home de la distro");
    assert!(home.starts_with('/'));
    let canonical = probe_directory(distro, &home).expect("valida home");
    assert_eq!(canonical, home);
    let entries = list_directories(distro, &home).expect("lista home");
    assert!(entries.len() < MAX_DIRS);
    assert!(
        entries.iter().all(|entry| !entry.contains('\0')),
        "entradas con NUL: {entries:?}"
    );
    assert!(entries.len() > 1, "entradas: {entries:?}");
    assert!(matches!(
        probe_directory(distro, "/axfterm/ruta/que-no-existe"),
        Err(WslError::MissingPath(_))
    ));
}

#[test]
fn classify_maps_common_errors() {
    assert!(matches!(
        classify("No such file or directory", "Ubuntu", "/x"),
        WslError::MissingPath(_)
    ));
    assert!(matches!(
        classify("Permission denied", "Ubuntu", "/x"),
        WslError::PermissionDenied(_)
    ));
    assert!(matches!(
        classify(
            "There is no distribution with the supplied name.",
            "Ubuntu",
            "/x"
        ),
        WslError::UnknownDistro(_)
    ));
}
