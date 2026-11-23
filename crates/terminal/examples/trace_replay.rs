//! Replay de una traza cruda del PTY para diagnosticar el render.
//!
//! Uso:
//!
//! ```powershell
//! cargo run -p axf_terminal --example trace_replay -- "$env:TEMP\axf-pty.vt" 145 44
//! ```
//!
//! Imprime el texto del grid, las celdas con flags o colores no-default y los
//! colores dinámicos pedidos por la aplicación (OSC 4/10/11/12). Sirve para
//! saber si un artefacto visual viene de subrayados, fondos o glifos.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "herramienta de diagnóstico: su salida es el entregable"
)]

use std::process::ExitCode;

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor};

#[derive(Clone, Copy, Debug)]
struct Size {
    columns: usize,
    screen_lines: usize,
}

impl Size {
    fn new(columns: usize, screen_lines: usize) -> Self {
        Self {
            columns: columns.max(1),
            screen_lines: screen_lines.max(1),
        }
    }
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("uso: trace_replay <archivo.vt> [columnas] [filas]");
        return ExitCode::FAILURE;
    };
    let columns = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(145);
    let rows = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(44);

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("no se pudo leer {path}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let size = Size::new(columns, rows);
    let mut term = Term::new(
        Config {
            scrolling_history: 10_000,
            ..Default::default()
        },
        &size,
        VoidListener,
    );
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, &bytes);

    let content = term.renderable_content();
    println!(
        "traza: {} bytes, grid {}x{}",
        bytes.len(),
        term.columns(),
        term.screen_lines()
    );
    println!(
        "defaults dinámicos: fg={:?} bg={:?} cursor={:?}",
        content.colors[NamedColor::Foreground],
        content.colors[NamedColor::Background],
        content.colors[NamedColor::Cursor],
    );
    for index in 0..16 {
        let named = match index {
            0 => NamedColor::Black,
            1 => NamedColor::Red,
            2 => NamedColor::Green,
            3 => NamedColor::Yellow,
            4 => NamedColor::Blue,
            5 => NamedColor::Magenta,
            6 => NamedColor::Cyan,
            7 => NamedColor::White,
            8 => NamedColor::BrightBlack,
            9 => NamedColor::BrightRed,
            10 => NamedColor::BrightGreen,
            11 => NamedColor::BrightYellow,
            12 => NamedColor::BrightBlue,
            13 => NamedColor::BrightMagenta,
            14 => NamedColor::BrightCyan,
            _ => NamedColor::BrightWhite,
        };
        if let Some(rgb) = content.colors[named] {
            println!("  ansi[{index}] = {rgb:?}");
        }
    }

    let mut underline = 0usize;
    let mut strikethrough = 0usize;
    let mut reverse = 0usize;
    let mut spans = 0usize;
    let grid = term.grid();

    for line in 0..rows {
        let row = &grid[Line(line as i32)];
        let mut text = String::with_capacity(columns);
        let mut notes = Vec::new();

        for column in 0..columns {
            let cell = &row[Column(column)];
            if !cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                text.push(if cell.flags.contains(Flags::HIDDEN) {
                    ' '
                } else {
                    cell.c
                });
            }
            if cell.flags.contains(Flags::ALL_UNDERLINES) {
                underline += 1;
            }
            if cell.flags.contains(Flags::STRIKEOUT) {
                strikethrough += 1;
            }
            if cell.flags.contains(Flags::INVERSE) {
                reverse += 1;
            }
            if cell.flags.contains(Flags::WIDE_CHAR) {
                spans += 1;
            }

            let custom_color = !matches!(cell.fg, Color::Named(NamedColor::Foreground))
                || !matches!(cell.bg, Color::Named(NamedColor::Background));
            if custom_color || !cell.flags.is_empty() {
                notes.push(format!(
                    "{column}:{:?} fg={:?} bg={:?} flags={:?}",
                    cell.c, cell.fg, cell.bg, cell.flags
                ));
            }
        }

        println!("row {line:02}: {text}");
        if !notes.is_empty() {
            println!("  {}", notes.join(" | "));
        }
    }

    println!(
        "resumen: underline={underline} strike={strikethrough} reverse={reverse} wide={spans}"
    );
    ExitCode::SUCCESS
}
