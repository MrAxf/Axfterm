//! Flood real: un proceso escribe 1 MiB por el PTY y el parser lo consume.
//!
//! Mide el camino completo PTY -> canal acotado -> parser -> frames. Sirve para
//! comprobar que el output masivo no bloquea el hilo de UI (aquí, el bucle
//! principal) y que el damage se coalesce.
//!
//! ```powershell
//! cargo run -p axf_terminal --release --example flood
//! ```

#![expect(
    clippy::print_stdout,
    reason = "bench de diagnóstico: su salida es el entregable"
)]

use std::time::{Duration, Instant};

use axf_terminal::engine::{ParserSetup, TerminalEvent, spawn_parser};
use axf_terminal::{GridSize, PtySizeSpec, TerminalSession};
use axf_ui::theme::TerminalPalette;

const TARGET_BYTES: usize = 1024 * 1024;

fn main() {
    let (program, args) = flood_command();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (commands, inbox) = async_channel::bounded(64);
    let session = TerminalSession::spawn_program(
        program,
        &args,
        None,
        PtySizeSpec::new(80, 24, 8, 16),
        commands.clone(),
    )
    .expect("el proceso de flood arranca");
    let writer = session.writer().expect("writer del PTY");
    let (frames, frame_inbox) = async_channel::bounded(2);
    let parser = spawn_parser(
        commands,
        inbox,
        frames,
        ParserSetup {
            writer,
            palette: TerminalPalette::placeholder(),
            initial: GridSize::new(80, 24),
            cell_width: 8,
            cell_height: 16,
            restore: None,
        },
    )
    .expect("el parser arranca");

    let started = Instant::now();
    let deadline = started + Duration::from_secs(60);
    let mut frame_count = 0usize;
    let mut exited = false;
    while Instant::now() < deadline {
        while let Ok(frame) = frame_inbox.try_recv() {
            frame_count += 1;
            for event in frame.events {
                if matches!(event, TerminalEvent::Exited(_)) {
                    exited = true;
                }
            }
        }
        if exited {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let elapsed = started.elapsed();
    let mib = TARGET_BYTES as f64 / (1024.0 * 1024.0);

    println!(
        "flood: {mib:.2} MiB en {:.3} s ({:.2} MiB/s, {frame_count} frames)",
        elapsed.as_secs_f64(),
        mib / elapsed.as_secs_f64().max(f64::EPSILON)
    );
    if !exited {
        println!("aviso: el proceso no terminó antes del timeout");
    }

    drop(parser);
    drop(session);
}

#[cfg(windows)]
fn flood_command() -> (&'static str, Vec<String>) {
    (
        "powershell",
        vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-Command".into(),
            format!(
                "$b=[byte[]]::new({TARGET_BYTES}); [Array]::Fill($b, [byte]121); \
                 $o=[Console]::OpenStandardOutput(); $o.Write($b,0,$b.Length); $o.Flush()"
            ),
        ],
    )
}

#[cfg(not(windows))]
fn flood_command() -> (&'static str, Vec<String>) {
    (
        "sh",
        vec![
            "-c".into(),
            format!(
                "dd if=/dev/zero bs=1024 count={} 2>/dev/null | tr '\\0' y",
                TARGET_BYTES / 1024
            ),
        ],
    )
}
