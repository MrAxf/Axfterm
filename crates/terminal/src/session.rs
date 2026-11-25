//! PTY y proceso del shell. En Windows usa ConPTY a través de `portable-pty`.
//!
//! La sesión expone `spawn`, `write`, `resize`, `kill` y `wait`. El proceso se
//! espera en un hilo propio para no bloquear nunca el hilo de UI; el código de
//! salida se publica como comando del parser y llega a la vista como evento.

include!("session_parts/01_spawnspec.rs");
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_spec_labels_the_command() {
        let spec = SpawnSpec::new("pwsh", vec!["-NoLogo".into()]);
        assert_eq!(spec.label(), "pwsh -NoLogo");
        assert_eq!(SpawnSpec::new("cmd", Vec::new()).label(), "cmd");
    }

    #[test]
    fn missing_program_reports_the_profile_command() {
        let (sender, _receiver) = async_channel::bounded(4);
        let spec = SpawnSpec::new("axfterm-programa-que-no-existe", Vec::new());
        let error =
            TerminalSession::spawn(Path::new(""), PtySizeSpec::new(20, 5, 8, 16), sender, &spec)
                .expect_err("no debe arrancar");
        let message = error.to_string();
        assert!(message.contains("axfterm-programa-que-no-existe"));
        assert!(message.contains("config.toml"));
        assert!(matches!(error, TerminalError::Spawn { .. }));
    }

    #[test]
    fn size_is_sane_and_pixels_are_saturated() {
        let size = PtySizeSpec::new(0, 0, u16::MAX, u16::MAX);
        assert_eq!(size.cols, 1);
        assert_eq!(size.rows, 1);
        let pty = size.to_pty();
        assert_eq!(pty.pixel_width, u16::MAX);
        assert_eq!(pty.pixel_height, u16::MAX);
    }

    #[test]
    fn exit_label_is_explainable() {
        assert_eq!(
            ExitInfo {
                code: Some(0),
                success: true
            }
            .label(),
            "proceso terminado (código 0)"
        );
        assert_eq!(
            ExitInfo {
                code: Some(7),
                success: false
            }
            .label(),
            "proceso terminado con error (código 7)"
        );
    }

    #[test]
    fn reader_loop_stops_when_the_channel_is_closed() {
        let (sender, receiver) = async_channel::bounded(1);
        receiver.close();
        let input = std::io::Cursor::new(vec![b'a'; 64 * 1024]);
        read_loop(Box::new(input), sender);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn cwd_falls_back_to_the_current_directory() {
        let cwd = usable_cwd(Path::new("C:/esta/ruta/no/existe/axfterm"));
        assert_eq!(
            cwd,
            std::env::current_dir().expect("el directorio actual existe")
        );
    }

    #[cfg(windows)]
    #[test]
    fn cwd_is_never_verbatim() {
        let friendly = std::env::current_dir().expect("el directorio actual existe");
        let text = friendly.display().to_string();
        // Solo tiene sentido en una ruta de unidad corta (`C:\...`).
        if text.len() > 250 || text.as_bytes().get(1) != Some(&b':') {
            return;
        }
        let verbatim = PathBuf::from(format!(r"\\?\{text}"));
        assert_eq!(usable_cwd(&verbatim), friendly);
    }

    /// Ejecuta un programa a través de sesión + parser reales.
    ///
    /// ConPTY emite `ESC[6n` al arrancar; sin el `EventListener` del parser el
    /// hijo se queda esperando la respuesta. Por eso estas pruebas usan el
    /// camino de producción en vez de una sesión aislada.
    #[cfg(windows)]
    fn run_with_parser(
        program: &str,
        args: &[&str],
        timeout: std::time::Duration,
    ) -> (String, Option<ExitInfo>) {
        let (commands, inbox) = async_channel::bounded(64);
        let session = TerminalSession::spawn_program(
            program,
            args,
            None,
            PtySizeSpec::new(40, 8, 8, 16),
            commands.clone(),
        )
        .expect("el programa de prueba arranca");
        let writer = session.writer().expect("writer disponible");
        let (frames, frame_inbox) = async_channel::bounded(4);
        let parser = crate::engine::spawn_parser(
            commands,
            inbox,
            frames,
            crate::engine::ParserSetup {
                writer,
                palette: axf_ui::theme::TerminalPalette::placeholder(),
                initial: crate::GridSize::new(40, 8),
                cell_width: 8,
                cell_height: 16,
                restore: None,
                scrollback: 10_000,
            },
        )
        .expect("el parser arranca");

        fn collect_frame(
            frame: crate::engine::ScreenFrame,
            exit: &mut Option<ExitInfo>,
            rows: &mut Vec<String>,
        ) {
            for event in &frame.events {
                if let crate::engine::TerminalEvent::Exited(info) = event {
                    *exit = *info;
                }
            }
            if frame.full {
                *rows = frame
                    .updates
                    .iter()
                    .map(|update| update.cells.iter().map(|cell| cell.ch).collect())
                    .collect();
            }
        }

        let deadline = std::time::Instant::now() + timeout;
        let mut exit = None;
        let mut rows: Vec<String> = Vec::new();

        while std::time::Instant::now() < deadline {
            while let Ok(frame) = frame_inbox.try_recv() {
                collect_frame(frame, &mut exit, &mut rows);
            }
            if exit.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Fuerza un frame completo tras la salida para leer el grid final.
        // ConPTY puede entregar un frame completo vacío justo después del
        // cierre; se conserva el frame con más texto no blanco.
        let _ = parser.scroll(0);
        let frame_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut final_rows: Vec<String> = rows;
        while std::time::Instant::now() < frame_deadline {
            let mut got_full = false;
            while let Ok(frame) = frame_inbox.try_recv() {
                if frame.full {
                    let candidate: Vec<String> = frame
                        .updates
                        .iter()
                        .map(|update| update.cells.iter().map(|cell| cell.ch).collect())
                        .collect();
                    let candidate_score = candidate
                        .iter()
                        .flat_map(|line| line.chars())
                        .filter(|ch| !ch.is_whitespace())
                        .count();
                    let current_score = final_rows
                        .iter()
                        .flat_map(|line| line.chars())
                        .filter(|ch| !ch.is_whitespace())
                        .count();
                    if candidate_score >= current_score {
                        final_rows = candidate;
                    }
                    got_full = true;
                }
            }
            if got_full {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        drop(parser);
        drop(session);
        (final_rows.join("\n"), exit)
    }

    #[cfg(windows)]
    #[test]
    fn exit_code_reaches_the_waiter() {
        let (text, exit) =
            run_with_parser("cmd", &["/C", "exit 7"], std::time::Duration::from_secs(15));
        let info = exit.expect("el parser vio el cierre");
        assert_eq!(info.code, Some(7), "texto: {text:?}");
        assert!(!info.success);
    }

    #[cfg(windows)]
    #[test]
    fn kill_ends_a_long_running_tree() {
        let (sender, receiver) = async_channel::bounded(64);
        let mut session = TerminalSession::spawn_program(
            "cmd",
            &["/C", "ping -n 30 127.0.0.1 >nul"],
            None,
            PtySizeSpec::new(20, 5, 8, 16),
            sender,
        )
        .expect("el programa largo arranca");

        session.resize(PtySizeSpec::new(40, 10, 8, 16));
        session
            .writer()
            .expect("writer disponible")
            .write(b"")
            .expect("escribir al PTY");
        session.kill();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline && session.try_wait().is_none() {
            while receiver.try_recv().is_ok() {}
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(session.try_wait().is_some(), "el árbol no terminó");
    }

    #[cfg(windows)]
    #[test]
    fn child_receives_term_environment() {
        let (text, exit) = run_with_parser(
            "cmd",
            &["/C", "echo %TERM% %COLORTERM%"],
            std::time::Duration::from_secs(15),
        );
        assert!(exit.is_some(), "texto: {text:?}");
        assert!(text.contains("xterm-256color"), "salida: {text:?}");
        assert!(text.contains("truecolor"), "salida: {text:?}");
    }
}
