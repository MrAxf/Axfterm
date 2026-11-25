use crate::engine::{
    CellColor, CursorShape, CursorSnapshot, HyperlinkSpan, ModeFlags, MouseMode, ParserHandle,
    ProgramStatus, ResizeRequest, ScreenFrame, SelectionPoint, SelectionSpan, SnapshotCell,
    SnapshotRequest, TerminalEvent,
};
use crate::ime::ImeState;
use crate::input;
use crate::mouse::{self, MouseAction, MouseButton as TerminalMouseButton, MouseModifiers};
use crate::paste::normalize_paste;
use crate::session::{ExitInfo, PtyWriter, TerminalSession};
use crate::{GridSize, TerminalError};
use axf_ui::gpui;
use axf_ui::gpui::component::menu::ContextMenuExt as _;
use axf_ui::gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, ExternalPaths, FocusHandle, Font, FontFallbacks, FontId,
    GlobalElementId, Hsla,
    InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, Keystroke, LayoutId,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, Render, ScrollDelta, ScrollEvent, ShapedLine, SharedString, Size, StrikethroughStyle,
    Style, Styled, Subscription, Task, TextAlign, TextRun, TextSystem, UTF16Selection,
    UnderlineStyle, Window, div, fill, font, point, px, size,
};
use axf_ui::theme::TerminalPalette;
use std::cell::RefCell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
axf_ui::gpui::actions!(
    axfterm_terminal,
    [CopySelection, PasteSelection, SendTab, SendBacktab]
);
pub const TERMINAL_CONTEXT: &str = "Terminal";
const MAX_PASTE_BYTES: usize = 1024 * 1024;
#[derive(Clone, Debug)]
pub struct TerminalStyle {
    /// Familia monoespaciada.
    pub font_family: SharedString,
    /// Cadena de fallback (Nerd Font, emoji, CJK).
    pub fallbacks: Vec<SharedString>,
    /// Tamaño de fuente.
    pub font_size: Pixels,
    /// Altura de línea explícita; `None` usa la natural del glifo.
    pub line_height: Option<Pixels>,
    /// Paleta ANSI y colores base.
    pub palette: TerminalPalette,
    /// Fondo del área de terminal.
    pub background: Hsla,
    /// Borde del área de terminal; `None` lo oculta.
    pub border: Option<Hsla>,
    /// Padding interior derivado del tamaño de fuente.
    pub padding: Pixels,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalSignal {
    /// Reporte OSC 7501.
    ProgramStatus(ProgramStatus),
    /// Notificación OSC 9/777/1337 o campana.
    Attention(Option<String>),
    /// El proceso terminó.
    Exited(Option<ExitInfo>),
}
#[derive(Clone, Copy, Debug, Default)]
struct DebugRender {
    trace_cells: bool,
    no_underline: bool,
    no_bg: bool,
    bg_overlap: bool,
    row_lines: bool,
}
impl DebugRender {
    fn from_env() -> Self {
        Self {
            trace_cells: env_flag("AXFTERM_TRACE_CELLS"),
            no_underline: env_flag("AXFTERM_DEBUG_NO_UNDERLINE"),
            no_bg: env_flag("AXFTERM_DEBUG_NO_BG"),
            bg_overlap: env_flag("AXFTERM_DEBUG_BG_OVERLAP"),
            row_lines: env_flag("AXFTERM_DEBUG_ROW_LINES"),
        }
    }
}
fn env_flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value != "0")
}
fn is_monospaced(text_system: &TextSystem, font_id: FontId, font_size: Pixels) -> bool {
    let wide = text_system
        .advance(font_id, font_size, 'M')
        .map(|size| size.width);
    let narrow = text_system
        .advance(font_id, font_size, 'i')
        .map(|size| size.width);
    match (wide, narrow) {
        (Ok(wide), Ok(narrow)) => (f32::from(wide) - f32::from(narrow)).abs() < 0.5,
        _ => false,
    }
}
#[derive(Clone)]
struct Metrics {
    font: Font,
    font_size: Pixels,
    cell_width: Pixels,
    line_height: Pixels,
    palette: TerminalPalette,
}
#[derive(Clone, Copy, Debug)]
struct GridGeometry {
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
}
impl Metrics {
    fn new(window: &mut Window, style: &TerminalStyle) -> Self {
        let text_system = window.text_system();
        let mut font = font(&style.font_family);
        if !style.fallbacks.is_empty() {
            font.fallbacks = Some(FontFallbacks::from_fonts(
                style
                    .fallbacks
                    .iter()
                    .map(|family| family.to_string())
                    .collect(),
            ));
        }
        let mut font_id = text_system.resolve_font(&font);
        let mut cell_width = text_system
            .advance(font_id, style.font_size, 'M')
            .map(|advance| advance.width)
            .unwrap_or_else(|_| style.font_size * 0.6);

        // La terminal exige una familia monoespaciada. Si el tema entrega una
        // proporcional (o una que no resuelve), se cae a una instalada.
        if !is_monospaced(text_system, font_id, style.font_size) {
            let installed = text_system.all_font_names();
            for candidate in ["Cascadia Mono", "Consolas", "Courier New"] {
                if !installed.iter().any(|name| name == candidate) {
                    continue;
                }
                let candidate_font = axf_ui::gpui::font(candidate);
                let candidate_id = text_system.resolve_font(&candidate_font);
                if !is_monospaced(text_system, candidate_id, style.font_size) {
                    continue;
                }
                if let Ok(advance) = text_system.advance(candidate_id, style.font_size, 'M') {
                    font = candidate_font;
                    font_id = candidate_id;
                    cell_width = advance.width;
                    break;
                }
            }
        }

        // La altura natural (ascent + descent) es la que hace que los glifos de
        // bloque y las cajas de una TUI queden contiguos. Multiplicar el tamaño
        // de fuente dejaba una línea de 1 px entre filas.
        let ascent = text_system.ascent(font_id, style.font_size);
        let descent = text_system.descent(font_id, style.font_size);
        // GPUI entrega el descent con signo negativo en Windows. Restarlo da
        // exactamente la altura del glifo de bloque (█), así que las cajas y el
        // logo de una TUI quedan contiguos sin recortar el texto.
        let natural = if descent < px(0.0) {
            ascent - descent
        } else {
            ascent + descent
        };
        let natural = if natural > px(1.0) {
            natural
        } else {
            style.font_size * 1.2
        };
        let line_height = style
            .line_height
            .filter(|height| *height > px(0.0))
            .map(|height| height.max(natural))
            .unwrap_or(natural);

        if let Some(path) = std::env::var_os("AXFTERM_TRACE_FONT")
            && let Ok(mut file) = std::fs::File::create(path)
        {
            use std::io::Write as _;
            let block = text_system
                .typographic_bounds(font_id, style.font_size, '█')
                .map(|bounds| bounds.size);
            let _ = writeln!(
                file,
                "family={:?} size={:?} ascent={:?} descent={:?} line={:?} block={:?} cell={:?}",
                style.font_family, style.font_size, ascent, descent, line_height, block, cell_width
            );
        }

        Self {
            font,
            font_size: style.font_size,
            cell_width,
            line_height,
            palette: style.palette,
        }
    }
}
struct RenderState {
    cols: usize,
    rows: usize,
    cells: Vec<Vec<SnapshotCell>>,
    combining: Vec<Vec<(usize, char)>>,
    links: Vec<Vec<HyperlinkSpan>>,
    cache: Vec<Option<ShapedLine>>,
    cursor: Option<CursorSnapshot>,
    mode: ModeFlags,
    display_offset: usize,
    history_size: usize,
    selection: Option<SelectionSpan>,
    ended: bool,
    exited: Option<ExitInfo>,
    bell_flash: bool,
    last_output: Option<Instant>,
    scratch_text: String,
    scratch_runs: Vec<TextRun>,
}
impl RenderState {
    fn new() -> Self {
        let cols = 80;
        let rows = 24;
        Self {
            cols,
            rows,
            cells: vec![vec![SnapshotCell::default(); cols]; rows],
            combining: vec![Vec::new(); rows],
            links: vec![Vec::new(); rows],
            cache: vec![None; rows],
            cursor: None,
            mode: ModeFlags::default(),
            display_offset: 0,
            history_size: 0,
            selection: None,
            ended: false,
            exited: None,
            bell_flash: false,
            last_output: None,
            scratch_text: String::new(),
            scratch_runs: Vec::new(),
        }
    }

    fn invalidate_caches(&mut self) {
        self.cache = vec![None; self.rows];
        self.scratch_text.clear();
        self.scratch_runs.clear();
    }

    fn apply(&mut self, frame: ScreenFrame) {
        let resized = frame.cols != self.cols || frame.rows != self.rows;
        let scrolled = frame.display_offset != self.display_offset;
        if frame.full || resized || scrolled {
            self.cols = frame.cols.max(1);
            self.rows = frame.rows.max(1);
            self.cells = vec![vec![SnapshotCell::default(); self.cols]; self.rows];
            self.combining = vec![Vec::new(); self.rows];
            self.links = vec![Vec::new(); self.rows];
            self.cache = vec![None; self.rows];
            self.scratch_text.clear();
            self.scratch_runs.clear();
        }

        for update in frame.updates {
            if update.line >= self.rows {
                continue;
            }
            let mut cells = update.cells;
            cells.resize(self.cols, SnapshotCell::default());
            cells.truncate(self.cols);
            self.cells[update.line] = cells;
            self.combining[update.line] = update.combining;
            self.links[update.line] = update.links;
            self.cache[update.line] = None;
        }

        self.cursor = frame.cursor;
        self.mode = frame.mode;
        self.display_offset = frame.display_offset;
        self.history_size = frame.history_size;
        self.selection = frame.selection;
    }
}
pub struct TerminalView {
    focus: FocusHandle,
    session: Option<TerminalSession>,
    parser: Option<ParserHandle>,
    state: Rc<RefCell<RenderState>>,
    metrics: Metrics,
    style: TerminalStyle,
    geometry: Option<GridGeometry>,
    requested_grid: Option<(usize, usize)>,
    error: Option<String>,
    notice: Option<String>,
    pending_url: Option<String>,
    title: Option<String>,
    cwd: Option<PathBuf>,
    pid: Option<u32>,
    signals: Vec<TerminalSignal>,
    debug: DebugRender,
    ime: ImeState,
    selection_anchor: Option<SelectionPoint>,
    selection_dragging: bool,
    selection_moved: bool,
    /// Un arrastre (archivos o pestaña) está sobre la terminal.
    drag_hover: bool,
    _focus_in: Option<Subscription>,
    _focus_out: Option<Subscription>,
    _pump: Option<Task<()>>,
}
