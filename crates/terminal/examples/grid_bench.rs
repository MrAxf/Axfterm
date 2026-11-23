//! Spike S4 — render de un grid de terminal con GPUI.
//!
//! Implementa un `Element` real (request_layout/prepaint/paint) que pinta un
//! grid monoespaciado con quads de fondo y shaping cacheado por fila, y mide el
//! coste de CPU de prepaint y paint. No mide FPS: GPUI está limitado por el
//! vsync y por el monitor, así que el presupuesto de 8/16 ms se evalúa como
//! tiempo de CPU por frame.
//!
//! ```powershell
//! cargo run -p axf_terminal --example grid_bench -- 200 50 120
//! cargo run -p axf_terminal --example grid_bench -- 80 24 120
//! ```
//!
//! Argumentos: `cols rows frames_por_fase` (por defecto 200x50, 120 por fase).
//! El ejemplo hace dos fases: daño completo (re-shape de todas las filas) y daño
//! incremental (solo la fila modificada), y sale con el resumen.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "spike de medición: su salida es el entregable"
)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use axf_ui::gpui::component::theme::{Theme, ThemeMode};
use axf_ui::gpui::{
    App, AppContext, Bounds, Context, Element, ElementId, Font, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Render, ShapedLine, Size,
    Style, Styled, TextAlign, TextRun, Window, div, fill, font, point, px, size,
};

const FONT_SCALE: f32 = 1.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    FullDamage,
    Incremental,
    Done,
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    ch: char,
    fg_accent: bool,
    bg_accent: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg_accent: false,
            bg_accent: false,
        }
    }
}

#[derive(Clone)]
struct Metrics {
    font: Font,
    font_size: Pixels,
    cell_width: Pixels,
    line_height: Pixels,
    foreground: Hsla,
    foreground_accent: Hsla,
    background_accent: Hsla,
}

struct BenchState {
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,
    /// Cache por índice de fila; se invalida con el damage del parser.
    cache: Vec<Option<ShapedLine>>,
    dirty: Vec<usize>,
    full: bool,
    frames_per_phase: usize,
    phase: Phase,
    prepaint_ms: Vec<f64>,
    paint_ms: Vec<f64>,
    cache_hits: usize,
    cache_misses: usize,
}

impl BenchState {
    fn new(cols: usize, rows: usize, frames_per_phase: usize) -> Self {
        let mut cells = vec![Cell::default(); cols * rows];
        // Contenido determinista: ASCII, con runs de color cada 13 columnas y
        // fondo en algunas filas. Ejercita el agrupado de quads.
        for row in 0..rows {
            for col in 0..cols {
                let seed = (row * 31 + col * 17) % 94;
                let ch = char::from(b'!' + seed as u8);
                cells[row * cols + col] = Cell {
                    ch,
                    fg_accent: col % 13 == 0,
                    bg_accent: row % 9 == 0 && col % 5 == 0,
                };
            }
        }
        Self {
            cols,
            rows,
            cells,
            cache: vec![None; rows],
            dirty: (0..rows).collect(),
            full: true,
            frames_per_phase,
            phase: Phase::FullDamage,
            prepaint_ms: Vec::with_capacity(frames_per_phase * 2),
            paint_ms: Vec::with_capacity(frames_per_phase * 2),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    fn total_frames(&self) -> usize {
        self.prepaint_ms.len()
    }

    fn advance(&mut self) {
        let done = self.total_frames();
        self.phase = if done >= self.frames_per_phase * 2 {
            Phase::Done
        } else if done >= self.frames_per_phase {
            Phase::Incremental
        } else {
            Phase::FullDamage
        };
        self.full = self.phase == Phase::FullDamage;

        match self.phase {
            Phase::FullDamage => {
                self.dirty.clear();
            }
            Phase::Incremental => {
                self.dirty.clear();
                // Un cambio por frame: una celda de una fila al azar.
                let row = done % self.rows;
                let col = (done * 7) % self.cols;
                self.cells[row * self.cols + col] = Cell {
                    ch: if done % 2 == 0 { '#' } else { '@' },
                    fg_accent: true,
                    bg_accent: false,
                };
                self.dirty.push(row);
            }
            Phase::Done => {}
        }
    }

    fn row_runs(&self, row: usize, metrics: &Metrics) -> (String, Vec<TextRun>) {
        let mut text = String::with_capacity(self.cols);
        let mut runs = Vec::new();
        let mut current: Option<(bool, usize)> = None;
        for col in 0..self.cols {
            let cell = self.cells[row * self.cols + col];
            text.push(cell.ch);
            match current {
                Some((accent, len)) if accent == cell.fg_accent => {
                    current = Some((accent, len + 1));
                }
                Some((accent, len)) => {
                    runs.push(TextRun {
                        len,
                        font: metrics.font.clone(),
                        color: if accent {
                            metrics.foreground_accent
                        } else {
                            metrics.foreground
                        },
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    });
                    current = Some((cell.fg_accent, 1));
                }
                None => current = Some((cell.fg_accent, 1)),
            }
        }
        if let Some((accent, len)) = current {
            runs.push(TextRun {
                len,
                font: metrics.font.clone(),
                color: if accent {
                    metrics.foreground_accent
                } else {
                    metrics.foreground
                },
                background_color: None,
                underline: None,
                strikethrough: None,
            });
        }
        (text, runs)
    }
}

struct GridBench {
    metrics: Metrics,
    state: Rc<RefCell<BenchState>>,
}

impl GridBench {
    fn new(
        cols: usize,
        rows: usize,
        frames_per_phase: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let theme = Theme::global(cx);
        let font_size = theme.mono_font_size;
        let font = font(&theme.mono_font_family);
        let cell_width = window
            .text_system()
            .advance(window.text_system().resolve_font(&font), font_size, 'M')
            .map(|advance| advance.width)
            .unwrap_or_else(|_| font_size * 0.6);

        let metrics = Metrics {
            font,
            font_size,
            cell_width,
            line_height: font_size * FONT_SCALE,
            foreground: theme.foreground,
            foreground_accent: theme.accent,
            background_accent: theme.sidebar,
        };

        let state = Rc::new(RefCell::new(BenchState::new(cols, rows, frames_per_phase)));

        cx.on_next_frame(window, |this, window, cx| this.tick(window, cx));

        Self { metrics, state }
    }

    fn tick(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let (phase, frames) = {
            let mut state = self.state.borrow_mut();
            if state.phase == Phase::Done {
                drop(state);
                self.report(cx);
                cx.quit();
                return;
            }
            state.advance();
            if state.phase == Phase::Done {
                drop(state);
                self.report(cx);
                cx.quit();
                return;
            }
            (state.phase, state.total_frames())
        };
        if frames % 30 == 0 {
            println!("fase {phase:?} frame {frames}");
        }
        cx.notify();
        cx.on_next_frame(window, |this, window, cx| this.tick(window, cx));
    }

    fn report(&self, _cx: &mut Context<'_, Self>) {
        let state = self.state.borrow();
        let half = state.frames_per_phase;
        let full_pre = &state.prepaint_ms[..half.min(state.prepaint_ms.len())];
        let full_paint = &state.paint_ms[..half.min(state.paint_ms.len())];
        let inc_pre = &state.prepaint_ms[half.min(state.prepaint_ms.len())..];
        let inc_paint = &state.paint_ms[half.min(state.paint_ms.len())..];

        let full_total: Vec<f64> = full_pre
            .iter()
            .zip(full_paint)
            .map(|(a, b)| a + b)
            .collect();
        let inc_total: Vec<f64> = inc_pre.iter().zip(inc_paint).map(|(a, b)| a + b).collect();

        println!(
            "\nRESULTADO S4 grid={}x{} frames/fase={}",
            state.cols, state.rows, half
        );
        println!(
            "  completo : prepaint p50={:.3} ms p95={:.3} ms | paint p50={:.3} ms p95={:.3} ms | total p50={:.3} p95={:.3}",
            percentile(full_pre, 0.50),
            percentile(full_pre, 0.95),
            percentile(full_paint, 0.50),
            percentile(full_paint, 0.95),
            percentile(&full_total, 0.50),
            percentile(&full_total, 0.95)
        );
        println!(
            "  incremental: prepaint p50={:.3} ms p95={:.3} ms | paint p50={:.3} ms p95={:.3} ms | total p50={:.3} p95={:.3}",
            percentile(inc_pre, 0.50),
            percentile(inc_pre, 0.95),
            percentile(inc_paint, 0.50),
            percentile(inc_paint, 0.95),
            percentile(&inc_total, 0.50),
            percentile(&inc_total, 0.95)
        );
        println!(
            "  cache shaping: {} hits / {} misses ({} filas x {} columnas)",
            state.cache_hits, state.cache_misses, state.rows, state.cols
        );
        let budget = if state.rows * state.cols > 4000 {
            16.0
        } else {
            8.0
        };
        let verdict = if percentile(&inc_total, 0.95) < budget {
            "OK"
        } else {
            "EXCEDE"
        };
        println!("  presupuesto {budget:.0} ms (p95 incremental) -> {verdict}");
    }
}

impl Render for GridBench {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div().size_full().child(GridElement {
            state: Rc::clone(&self.state),
            metrics: self.metrics.clone(),
        })
    }
}

struct GridElement {
    state: Rc<RefCell<BenchState>>,
    metrics: Metrics,
}

impl Element for GridElement {
    type RequestLayoutState = ();
    type PrepaintState = Vec<(usize, ShapedLine)>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout = window.request_layout(
            Style {
                size: Size::full(),
                ..Default::default()
            },
            std::iter::empty(),
            cx,
        );
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let started = Instant::now();
        let mut state = self.state.borrow_mut();
        let metrics = self.metrics.clone();

        let mut shaped = Vec::with_capacity(state.rows);
        for row in 0..state.rows {
            // En daño completo se shapea siempre (peor caso). En incremental solo
            // las filas que el VT marcó como dañadas.
            let must_shape = state.full || state.dirty.contains(&row);
            if !must_shape {
                let cached = state.cache.get(row).and_then(Option::clone);
                if let Some(cached) = cached {
                    state.cache_hits += 1;
                    shaped.push((row, cached));
                    continue;
                }
            }

            let (text, runs) = state.row_runs(row, &metrics);
            let line = window
                .text_system()
                .shape_line(text.into(), metrics.font_size, &runs, None);
            if row < state.cache.len() {
                state.cache[row] = Some(line.clone());
            }
            state.cache_misses += 1;
            shaped.push((row, line));
        }

        state
            .prepaint_ms
            .push(started.elapsed().as_secs_f64() * 1e3);
        shaped
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let started = Instant::now();
        {
            let state = self.state.borrow();
            let metrics = &self.metrics;
            let origin = bounds.origin;
            let cell_w = metrics.cell_width;
            let line_h = metrics.line_height;

            // Fondos: quads agrupados por runs horizontales.
            for row in 0..state.rows {
                let mut run_start = 0usize;
                let mut current = state.cells[row * state.cols].bg_accent;
                for col in 1..=state.cols {
                    let next = col < state.cols
                        && state.cells[row * state.cols + col].bg_accent == current;
                    if !next {
                        if current {
                            let x = origin.x + cell_w * run_start;
                            let y = origin.y + line_h * row;
                            let quad = fill(
                                Bounds::new(point(x, y), size(cell_w * (col - run_start), line_h)),
                                metrics.background_accent,
                            );
                            window.paint_quad(quad);
                        }
                        run_start = col;
                        if col < state.cols {
                            current = state.cells[row * state.cols + col].bg_accent;
                        }
                    }
                }
            }
        }

        // Texto: una línea shapeada por fila.
        let origin = bounds.origin;
        let line_h = self.metrics.line_height;
        for (row, line) in prepaint.iter() {
            let line_origin = point(origin.x, origin.y + line_h * *row);
            let _ = line.paint(line_origin, line_h, TextAlign::Left, None, window, cx);
        }

        let elapsed = started.elapsed().as_secs_f64() * 1e3;
        self.state.borrow_mut().paint_ms.push(elapsed);
    }
}

impl IntoElement for GridElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn percentile(samples: &[f64], percentile: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut values = samples.to_vec();
    values.sort_by(|a, b| a.total_cmp(b));
    let index = ((values.len() - 1) as f64 * percentile).round() as usize;
    values[index]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cols = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(200);
    let rows = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(50);
    let frames = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(120);

    axf_ui::gpui::application().run(move |cx| {
        axf_ui::gpui::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);

        let options = axf_ui::gpui::WindowOptions {
            window_bounds: Some(axf_ui::gpui::WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.0), px(800.0)),
                cx,
            ))),
            titlebar: Some(axf_ui::gpui::TitlebarOptions {
                title: Some("Axfterm — spike render".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = axf_ui::gpui::open_window(options, cx, move |window, cx| {
            cx.new(|cx| GridBench::new(cols, rows, frames, window, cx))
        });
        if let Err(error) = result {
            eprintln!("grid_bench: no se pudo abrir la ventana: {error}");
        }
    });
}
