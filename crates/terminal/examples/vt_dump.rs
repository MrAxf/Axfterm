//! Spike S3 — motor VT sobre `alacritty_terminal`.
//!
//! Alimenta un `Term` con secuencias de escape y comprueba lo que el render de
//! M1 va a necesitar: atributos SGR (16/256/truecolor), alt screen, cursor,
//! caracteres anchos, resize/reflow, scrollback, damage y las respuestas que el
//! terminal debe escribir de vuelta al PTY (DSR, tamaño de área, color).
//!
//! ```powershell
//! cargo run -p axf_terminal --example vt_dump
//! ```
//!
//! Falla con exit ≠ 0 si alguna comprobación no cuadra.

#![expect(
    clippy::print_stdout,
    reason = "spike de validación: su salida es el entregable"
)]

use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};

const ROWS: usize = 12;
const COLS: usize = 80;

/// Dimensiones mínimas que pide `Term`.
#[derive(Clone, Copy, Debug)]
struct Size {
    columns: usize,
    screen_lines: usize,
}

impl Size {
    fn new(columns: usize, screen_lines: usize) -> Self {
        Self {
            columns,
            screen_lines,
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

/// Listener que conserva lo que el terminal quiere devolver al PTY y los
/// eventos que la UI tendría que atender. `Term` exige `Send`, de ahí el `Arc`.
#[derive(Clone, Default)]
struct Recorder {
    writes: Arc<Mutex<Vec<String>>>,
    events: Arc<Mutex<Vec<String>>>,
}

impl Recorder {
    fn take_writes(&self) -> Vec<String> {
        std::mem::take(&mut *self.writes.lock().expect("lock de writes"))
    }

    fn take_events(&self) -> Vec<String> {
        std::mem::take(&mut *self.events.lock().expect("lock de events"))
    }
}

impl EventListener for Recorder {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => self.writes.lock().expect("lock de writes").push(text),
            Event::TextAreaSizeRequest(format) => {
                let reply = format(alacritty_terminal::event::WindowSize {
                    num_lines: ROWS as u16,
                    num_cols: COLS as u16,
                    cell_width: 8,
                    cell_height: 16,
                });
                self.events
                    .lock()
                    .expect("lock de events")
                    .push(format!("TextAreaSizeRequest -> {reply:?}"));
            }
            Event::ColorRequest(index, format) => {
                let rgb = Rgb {
                    r: 0x1e,
                    g: 0x21,
                    b: 0x2a,
                };
                let reply = format(rgb);
                self.events
                    .lock()
                    .expect("lock de events")
                    .push(format!("ColorRequest({index}) -> {reply:?}"));
            }
            other => self
                .events
                .lock()
                .expect("lock de events")
                .push(format!("{other:?}")),
        }
    }
}

struct Harness {
    term: Term<Recorder>,
    parser: Processor,
    recorder: Recorder,
}

impl Harness {
    fn new(rows: usize, cols: usize) -> Self {
        let recorder = Recorder::default();
        let term = Term::new(
            alacritty_terminal::term::Config {
                scrolling_history: 1_000,
                ..Default::default()
            },
            &Size::new(cols, rows),
            recorder.clone(),
        );
        Self {
            term,
            parser: Processor::new(),
            recorder,
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    fn cell(&self, line: usize, column: usize) -> &alacritty_terminal::term::cell::Cell {
        &self.term.grid()[Line(line as i32)][Column(column)]
    }

    fn damage_lines(&mut self) -> (bool, Vec<usize>) {
        let damage = self.term.damage();
        let mut full = false;
        let mut lines = Vec::new();
        match damage {
            TermDamage::Full => full = true,
            TermDamage::Partial(iter) => {
                for bounds in iter {
                    lines.push(bounds.line);
                }
            }
        }
        self.term.reset_damage();
        (full, lines)
    }

    fn dump(&self) {
        let rows = self.term.grid().screen_lines();
        let columns = self.term.grid().columns();
        for line in 0..rows {
            let mut text = String::new();
            let mut marks = Vec::new();
            for column in 0..columns {
                let cell = self.cell(line, column);
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                text.push(cell.c);
                if cell.fg != Color::Named(NamedColor::Foreground)
                    || cell.bg != Color::Named(NamedColor::Background)
                    || !cell.flags.is_empty()
                {
                    marks.push(format!(
                        "{column}:fg={:?} bg={:?} flags={:?}",
                        cell.fg, cell.bg, cell.flags
                    ));
                }
            }
            println!("  {line:>2} |{}", text.trim_end());
            if !marks.is_empty() {
                println!("     ^ {}", marks.join(" | "));
            }
        }
        let content = self.term.renderable_content();
        println!(
            "     cursor={:?} shape={:?} mode={:?}",
            content.cursor.point, content.cursor.shape, content.mode
        );
    }
}

struct Checks {
    failures: usize,
}

impl Checks {
    fn check(&mut self, name: &str, ok: bool) {
        if ok {
            println!("  [OK]   {name}");
        } else {
            println!("  [FAIL] {name}");
            self.failures += 1;
        }
    }
}

fn main() -> ExitCode {
    let mut checks = Checks { failures: 0 };

    scenario("SGR 16/256/truecolor", &mut checks, sgr);
    scenario("alt screen", &mut checks, alt_screen);
    scenario("cursor y DECTCEM", &mut checks, cursor);
    scenario("caracteres anchos", &mut checks, wide_chars);
    scenario("resize y damage", &mut checks, resize_and_damage);
    scenario("scrollback", &mut checks, scrollback);
    scenario("respuestas al PTY", &mut checks, pty_replies);
    scenario("bracketed paste", &mut checks, bracketed_paste);

    if checks.failures == 0 {
        println!("RESULTADO S3: VT OK (8 escenarios, 0 fallos)");
        ExitCode::SUCCESS
    } else {
        println!("RESULTADO S3: {} comprobaciones fallidas", checks.failures);
        ExitCode::FAILURE
    }
}

fn scenario(name: &str, checks: &mut Checks, body: impl FnOnce(&mut Harness, &mut Checks)) {
    println!("\n== {name} ==");
    let mut harness = Harness::new(ROWS, COLS);
    body(&mut harness, checks);
    harness.dump();
}

fn sgr(harness: &mut Harness, checks: &mut Checks) {
    harness.feed(
        b"\x1b[31mR\x1b[0m\x1b[1;32mG\x1b[0m\x1b[38;5;196mI\x1b[0m\x1b[38;2;10;20;30mT\x1b[0m",
    );

    checks.check(
        "SGR 31 -> Named(Red)",
        harness.cell(0, 0).fg == Color::Named(NamedColor::Red),
    );
    let green = harness.cell(0, 1);
    checks.check(
        "SGR 1;32 -> Named(Green) + BOLD",
        green.fg == Color::Named(NamedColor::Green) && green.flags.contains(Flags::BOLD),
    );
    checks.check(
        "SGR 38;5;196 -> Indexed(196)",
        harness.cell(0, 2).fg == Color::Indexed(196),
    );
    checks.check(
        "SGR 38;2;10;20;30 -> Spec(rgb)",
        harness.cell(0, 3).fg
            == Color::Spec(Rgb {
                r: 10,
                g: 20,
                b: 30,
            }),
    );
    // El reset debe devolver la celda al default, sin arrastrar atributos.
    let reset = harness.cell(0, 4);
    checks.check(
        "SGR 0 restaura default",
        reset.fg == Color::Named(NamedColor::Foreground) && reset.flags.is_empty(),
    );
}

fn alt_screen(harness: &mut Harness, checks: &mut Checks) {
    harness.feed(b"MAIN");
    checks.check("grid principal visible", harness.cell(0, 0).c == 'M');

    harness.feed(b"\x1b[?1049h");
    checks.check(
        "1049h activa ALT_SCREEN",
        harness.mode().contains(TermMode::ALT_SCREEN),
    );
    checks.check("alt grid arranca vacío", harness.cell(0, 0).c == ' ');

    // 1049 conserva la posición del cursor del grid principal; ir a casa antes
    // de escribir para no comprobar una celda distinta.
    harness.feed(b"\x1b[H");
    harness.feed(b"ALT");
    checks.check("alt grid escribe", harness.cell(0, 0).c == 'A');

    harness.feed(b"\x1b[?1049l");
    checks.check(
        "1049l desactiva ALT_SCREEN",
        !harness.mode().contains(TermMode::ALT_SCREEN),
    );
    checks.check(
        "grid principal restaurado",
        harness.cell(0, 0).c == 'M' && harness.cell(0, 3).c == 'N',
    );
}

fn cursor(harness: &mut Harness, checks: &mut Checks) {
    harness.feed(b"\x1b[5;10H");
    let cursor = harness.term.renderable_content().cursor.point;
    checks.check(
        "CUP 5;10 -> Line(4), Column(9)",
        cursor == Point::new(Line(4), Column(9)),
    );

    harness.feed(b"\x1b[?25l");
    checks.check(
        "DECTCEM off quita SHOW_CURSOR",
        !harness.mode().contains(TermMode::SHOW_CURSOR),
    );
    harness.feed(b"\x1b[?25h");
    checks.check(
        "DECTCEM on restaura SHOW_CURSOR",
        harness.mode().contains(TermMode::SHOW_CURSOR),
    );
}

fn wide_chars(harness: &mut Harness, checks: &mut Checks) {
    harness.feed("日本語".as_bytes());

    let cells = (0..3)
        .map(|i| harness.cell(0, i * 2))
        .map(|cell| (cell.c, cell.flags.contains(Flags::WIDE_CHAR)))
        .collect::<Vec<_>>();
    checks.check(
        "3 ideogramas marcados WIDE_CHAR",
        cells == vec![('日', true), ('本', true), ('語', true)],
    );
    let spacers_ok = (0..3).all(|i| {
        harness
            .cell(0, i * 2 + 1)
            .flags
            .contains(Flags::WIDE_CHAR_SPACER)
    });
    checks.check("columnas contiguas marcadas spacer", spacers_ok);
    checks.check(
        "cursor avanza 6 columnas",
        harness.term.renderable_content().cursor.point.column == Column(6),
    );
}

fn resize_and_damage(harness: &mut Harness, checks: &mut Checks) {
    let _ = harness.damage_lines();
    harness.feed(b"hola");
    let (full, lines) = harness.damage_lines();
    checks.check(
        "damage incremental marca la línea escrita",
        !full && lines == vec![0],
    );

    harness.term.resize(Size::new(40, 8));
    checks.check(
        "resize actualiza dimensiones",
        harness.term.grid().screen_lines() == 8 && harness.term.grid().columns() == 40,
    );
    let (full, _) = harness.damage_lines();
    checks.check("resize marca damage completo", full);

    // Reflow: una línea más ancha que el nuevo ancho debe envolver.
    harness.term.resize(Size::new(10, 4));
    harness.feed(b"\x1b[2J\x1b[H");
    harness.feed(b"1234567890ABCDEF");
    let wrapped = harness.cell(0, 9).c == '0'
        && harness.cell(1, 0).c == 'A'
        && harness.cell(0, 9).flags.contains(Flags::WRAPLINE);
    checks.check("reflow envuelve a la línea siguiente", wrapped);
}

fn scrollback(harness: &mut Harness, checks: &mut Checks) {
    for index in 0..20 {
        harness.feed(format!("linea {index}\r\n").as_bytes());
    }
    let history = harness.term.grid().history_size();
    checks.check("scrollback acumula historia", history >= 8);

    // Limpiar el damage inicial para que la comprobación de scroll no lo herede.
    let _ = harness.damage_lines();
    harness.term.scroll_display(Scroll::Delta(5));
    checks.check(
        "scroll_display mueve el viewport",
        harness.term.grid().display_offset() == 5,
    );
    let (full, _) = harness.damage_lines();
    checks.check("scroll marca damage completo", full);

    harness.term.scroll_display(Scroll::Bottom);
    checks.check(
        "Scroll::Bottom vuelve al presente",
        harness.term.grid().display_offset() == 0,
    );
}

fn pty_replies(harness: &mut Harness, checks: &mut Checks) {
    harness.feed(b"\x1b[3;7H\x1b[6n");
    let writes = harness.recorder.take_writes();
    checks.check(
        "DSR 6n responde con la posición del cursor",
        writes.iter().any(|write| write.contains("\x1b[3;7R")),
    );

    // CSI 18t responde por PtyWrite; CSI 14t pide tamaño en píxeles por evento.
    harness.feed(b"\x1b[18t");
    let writes = harness.recorder.take_writes();
    checks.check(
        "CSI 18t responde con el tamaño en caracteres",
        writes.iter().any(|write| write.contains("\x1b[8;12;80t")),
    );

    harness.feed(b"\x1b[14t");
    let events = harness.recorder.take_events();
    checks.check(
        "CSI 14t pide el tamaño en píxeles",
        events
            .iter()
            .any(|event| event.starts_with("TextAreaSizeRequest")),
    );

    harness.feed(b"\x1b]10;?\x07");
    let events = harness.recorder.take_events();
    checks.check(
        "OSC 10 pide el color de foreground",
        events
            .iter()
            .any(|event| event.starts_with("ColorRequest(256")),
    );
}

fn bracketed_paste(harness: &mut Harness, checks: &mut Checks) {
    harness.feed(b"\x1b[?2004h");
    checks.check(
        "2004h activa BRACKETED_PASTE",
        harness.mode().contains(TermMode::BRACKETED_PASTE),
    );
    harness.feed(b"\x1b[?2004l");
    checks.check(
        "2004l desactiva BRACKETED_PASTE",
        !harness.mode().contains(TermMode::BRACKETED_PASTE),
    );
}
