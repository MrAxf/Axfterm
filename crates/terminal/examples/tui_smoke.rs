//! Smoke de TUIs reales a través de PTY + parser, sin ventana.
//!
//! `nvim` valida alt screen, input y salida limpia; `opencode` valida que una
//! TUI moderna arranca y produce frames. Si no están instalados, se omiten.
//!
//! ```powershell
//! cargo run -p axf_terminal --release --example tui_smoke
//! ```

#![expect(
    clippy::print_stdout,
    reason = "smoke de diagnóstico: su salida es el entregable"
)]

use std::time::{Duration, Instant};

use axf_terminal::engine::{ParserSetup, TerminalEvent, spawn_parser};
use axf_terminal::{GridSize, PtySizeSpec, TerminalSession};
use axf_ui::theme::TerminalPalette;

fn main() {
    run_tui("nvim", &["-u", "NONE"], b":q!\r", true);
    #[cfg(windows)]
    run_tui(
        "powershell",
        &["-NoLogo", "-NoProfile", "-Command", "opencode"],
        b"\x03",
        false,
    );
}

fn run_tui(program: &str, args: &[&str], quit: &[u8], require_exit: bool) {
    let (commands, inbox) = async_channel::bounded(64);
    let session = match TerminalSession::spawn_program(
        program,
        args,
        Some(std::path::Path::new(".")),
        PtySizeSpec::new(80, 24, 8, 16),
        commands.clone(),
    ) {
        Ok(session) => session,
        Err(error) => {
            println!("{program}: omitido ({error})");
            return;
        }
    };
    let writer = session.writer().expect("writer del PTY");
    let (frames, frame_inbox) = async_channel::bounded(2);
    let parser = spawn_parser(
        commands,
        inbox,
        frames,
        ParserSetup {
            writer: writer.clone(),
            palette: TerminalPalette::placeholder(),
            initial: GridSize::new(80, 24),
            cell_width: 8,
            cell_height: 16,
            restore: None,
            scrollback: 10_000,
        },
    )
    .expect("parser");

    let started = Instant::now();
    let mut alt_screen = false;
    let mut exit = None;
    let mut sent_quit = false;
    let deadline = started + Duration::from_secs(20);
    while Instant::now() < deadline {
        while let Ok(frame) = frame_inbox.try_recv() {
            alt_screen |= frame.mode.alt_screen;
            for event in frame.events {
                if let TerminalEvent::Exited(info) = event {
                    exit = info;
                }
            }
        }

        if !sent_quit && (alt_screen || started.elapsed() > Duration::from_secs(2)) {
            let _ = writer.write(quit);
            sent_quit = true;
        }
        if require_exit && exit.is_some() {
            break;
        }
        if !require_exit && started.elapsed() > Duration::from_secs(4) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let _ = session.try_wait();
    println!(
        "{program}: alt_screen={alt_screen} exit={:?} en {:.2} s",
        exit.map(|info| info.code),
        started.elapsed().as_secs_f64()
    );

    drop(parser);
    drop(session);
}
