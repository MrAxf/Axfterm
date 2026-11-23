//! Motor VT: hilo parser que posee `Term` y publica filas dañadas a la UI.
//!
//! El hilo de UI nunca toca `alacritty_terminal`. El parser recibe bytes y
//! comandos por un canal acotado, aplica el damage y envía un `ScreenFrame` con
//! las filas que cambiaron. Los tipos de alacritty quedan confinados aquí
//! (ADR 0001).

use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionRange, SelectionType};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::{Config, Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::{
    Color, CursorShape as AlacrittyCursor, NamedColor, Processor, Rgb,
};

use axf_ui::theme::TerminalPalette;

use crate::session::{ExitInfo, PtyWriter};
use crate::snapshot::{self, TerminalSnapshot};
use crate::{GridSize, TerminalError};

/// Historial por defecto de v0.1 (ADR 0005).
const SCROLLBACK_LINES: usize = 10_000;
/// Bytes máximos que el parser procesa antes de publicar un frame.
const BATCH_LIMIT: usize = 256 * 1024;
/// Tope del buffer de un OSC en curso (diagnóstico defensivo).
const OSC_LIMIT: usize = 8 * 1024;

/// Eventos que el parser entrega a la UI además del grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Título de ventana pedido por la aplicación (`OSC 0/2`).
    Title(Option<String>),
    /// Directorio de trabajo (`OSC 7`).
    Cwd(PathBuf),
    /// Campana (`BEL`).
    Bell,
    /// Notificación de la aplicación (`OSC 9/777/1337`).
    Attention(Option<String>),
    /// Estado reportado por un programa con OSC 7501.
    ProgramStatus(ProgramStatus),
    /// Forma del puntero pedida por la aplicación (`OSC 22`).
    MouseCursor(String),
    /// Texto que debe ir al portapapeles.
    Clipboard(String),
    /// El proceso terminó; `None` si el PTY cerró sin código.
    Exited(Option<ExitInfo>),
    /// Error recuperable del motor que merece mostrarse.
    Error(String),
}

/// Estado de un programa según OSC 7501.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramState {
    /// En reposo.
    Idle,
    /// Trabajando.
    Working,
    /// Bloqueado esperando al usuario.
    Blocked,
    /// Terminó correctamente.
    Done,
    /// Falló.
    Error,
    /// Retira el registro.
    Clear,
}

/// Reporte normalizado de OSC 7501.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramStatus {
    /// Estado reportado.
    pub state: ProgramState,
    /// Motivo de `blocked`, si es uno conocido.
    pub kind: Option<String>,
    /// Progreso 0–100.
    pub progress: Option<u8>,
    /// Nombre estable del programa.
    pub app: Option<String>,
    /// Etiqueta corta.
    pub title: Option<String>,
    /// Detalle de una línea.
    pub message: Option<String>,
    /// Identificador de registro, si lo trae.
    pub id: Option<String>,
}

/// Comandos que recibe el hilo parser.
#[derive(Debug)]
pub enum ParserCommand {
    /// Bytes de salida del PTY.
    Bytes(Vec<u8>),
    /// Cambio de tamaño del grid y de la celda en píxeles.
    Resize(ResizeRequest),
    /// Desplazamiento relativo del viewport (`+` hacia el historial).
    Scroll(i32),
    /// Volver al final del scrollback.
    ScrollToBottom,
    /// Definir una selección en coordenadas del viewport.
    Select {
        /// Inicio (fila, columna) 0-based.
        start: SelectionPoint,
        /// Fin (fila, columna) 0-based.
        end: SelectionPoint,
        /// Selección rectangular.
        block: bool,
    },
    /// Quitar la selección.
    ClearSelection,
    /// Copiar la selección actual al portapapeles.
    CopySelection,
    /// Foco entró o salió (DECSET 1004).
    Focus(bool),
    /// El PTY llegó a EOF (el proceso terminó o el canal se rompió).
    Eof,
    /// El proceso terminó con este estado.
    Exited(ExitInfo),
    /// Exporta el buffer actual; `None` si no hay nada que restaurar.
    Snapshot(async_channel::Sender<Option<TerminalSnapshot>>),
    /// Volcado puntual del grid para diagnóstico.
    Dump,
    /// Cierre ordenado del parser.
    Close,
}

/// Punto de selección en coordenadas del viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionPoint {
    /// Fila visible (0 = arriba).
    pub row: usize,
    /// Columna visible.
    pub col: usize,
}

/// Tamaño al que hay que llevar el grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResizeRequest {
    /// Columnas visibles.
    pub cols: usize,
    /// Filas visibles.
    pub rows: usize,
    /// Ancho de celda en píxeles (para `CSI 14t`).
    pub cell_width: u16,
    /// Alto de celda en píxeles (para `CSI 14t`).
    pub cell_height: u16,
}

/// Color de una celda, sin acoplarse a los tipos de alacritty.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CellColor {
    /// Foreground o background por defecto, según el canal.
    #[default]
    Default,
    /// Color de la paleta de 256 entradas.
    Indexed(u8),
    /// Color RGB empaquetado como `0xRRGGBB`.
    Rgb(u32),
}

/// Atributos SGR que afectan al render.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attrs {
    /// Negrita.
    pub bold: bool,
    /// Atenuado.
    pub dim: bool,
    /// Cursiva.
    pub italic: bool,
    /// Subrayado (cualquier variante).
    pub underline: bool,
    /// Colores invertidos.
    pub reverse: bool,
    /// Tachado.
    pub strikethrough: bool,
    /// Texto oculto (se pinta como espacio).
    pub hidden: bool,
    /// Celda que continúa un carácter ancho; no se pinta texto.
    pub wide_spacer: bool,
    /// Carácter ancho (ocupa dos columnas).
    pub wide: bool,
}

/// Celda que viaja del parser al render.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SnapshotCell {
    /// Carácter visible.
    pub ch: char,
    /// Color de primer plano.
    pub fg: CellColor,
    /// Color de fondo.
    pub bg: CellColor,
    /// Atributos SGR.
    pub attrs: Attrs,
}

impl Default for SnapshotCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: CellColor::Default,
            bg: CellColor::Default,
            attrs: Attrs::default(),
        }
    }
}

/// Tramo de hipervínculo dentro de una fila.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HyperlinkSpan {
    /// Columna inicial (inclusive).
    pub start: usize,
    /// Columna final (exclusive).
    pub end: usize,
    /// URI tal como la entregó la aplicación.
    pub uri: String,
}

/// Fila actualizada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowUpdate {
    /// Índice de fila en el viewport (0 = arriba).
    pub line: usize,
    /// Celdas de la fila.
    pub cells: Vec<SnapshotCell>,
    /// Combinaciones Unicode que se añaden después del carácter base.
    pub combining: Vec<(usize, char)>,
    /// Hipervínculos de la fila.
    pub links: Vec<HyperlinkSpan>,
}

/// Forma del cursor que entiende el render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    /// Bloque.
    Block,
    /// Barra vertical.
    Beam,
    /// Subrayado.
    Underline,
    /// Bloque hueco.
    HollowBlock,
}

/// Posición y forma del cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorSnapshot {
    /// Fila del viewport.
    pub line: usize,
    /// Columna.
    pub column: usize,
    /// Forma.
    pub shape: CursorShape,
}

/// Selección visible en coordenadas del viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionSpan {
    /// Fila inicial.
    pub start_line: usize,
    /// Columna inicial (inclusive).
    pub start_col: usize,
    /// Fila final.
    pub end_line: usize,
    /// Columna final (inclusive).
    pub end_col: usize,
    /// Selección rectangular.
    pub block: bool,
}

/// Nivel de tracking de ratón solicitado por la aplicación.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseMode {
    /// Sin tracking; el clic pertenece a Axfterm.
    #[default]
    Off,
    /// Solo pulsación y liberación (DECSET 1000).
    Click,
    /// Pulsación, liberación y arrastre con botón (DECSET 1002).
    Drag,
    /// Todo movimiento (DECSET 1003).
    Motion,
}

/// Modos del terminal que necesita la entrada y el render.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModeFlags {
    /// La aplicación pide bracketed paste.
    pub bracketed_paste: bool,
    /// Flechas en modo aplicación (`ESC O A`).
    pub app_cursor: bool,
    /// Pantalla alternativa activa.
    pub alt_screen: bool,
    /// Cursor visible.
    pub show_cursor: bool,
    /// La aplicación pide reportar foco (DECSET 1004).
    pub focus_in_out: bool,
    /// Tracking de ratón solicitado.
    pub mouse_mode: MouseMode,
    /// Codificación SGR 1006 (`ESC [ < ... M`).
    pub sgr_mouse: bool,
    /// Coordenadas de ratón en UTF-8 (X10 extendido).
    pub utf8_mouse: bool,
    /// La rueda reenvía flechas en pantalla alternativa (DECSET 1007).
    pub alternate_scroll: bool,
}

/// Frame publicado por el parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenFrame {
    /// Columnas del grid.
    pub cols: usize,
    /// Filas del grid.
    pub rows: usize,
    /// Bytes de salida procesados en este lote (actividad para agentes).
    pub bytes: usize,
    /// Si es cierto, `updates` contiene todas las filas y la cache se invalida.
    pub full: bool,
    /// Filas cambiadas.
    pub updates: Vec<RowUpdate>,
    /// Cursor visible, si lo hay.
    pub cursor: Option<CursorSnapshot>,
    /// Modos activos.
    pub mode: ModeFlags,
    /// Líneas desplazadas hacia el historial.
    pub display_offset: usize,
    /// Líneas totales en el historial.
    pub history_size: usize,
    /// Selección visible.
    pub selection: Option<SelectionSpan>,
    /// Eventos ocurridos en este lote.
    pub events: Vec<TerminalEvent>,
}

/// Handle del hilo parser.
pub struct ParserHandle {
    commands: async_channel::Sender<ParserCommand>,
    thread: Option<thread::JoinHandle<()>>,
}

/// Petición de instantánea que se resuelve en el hilo parser.
pub struct SnapshotRequest {
    receiver: async_channel::Receiver<Option<TerminalSnapshot>>,
}

impl SnapshotRequest {
    /// Espera la instantánea; `None` si el parser desapareció o no había nada
    /// que guardar.
    pub async fn wait(self) -> Option<TerminalSnapshot> {
        self.receiver.recv().await.ok().flatten()
    }
}

impl std::fmt::Debug for SnapshotRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SnapshotRequest")
            .finish_non_exhaustive()
    }
}

impl ParserHandle {
    /// Pide una instantánea sin bloquear la UI. Devuelve `None` si el canal
    /// está saturado o el parser ya cerró; en ese caso se conserva la
    /// instantánea anterior, si existía.
    pub fn snapshot(&self) -> Option<SnapshotRequest> {
        let (sender, receiver) = async_channel::bounded(1);
        self.commands
            .try_send(ParserCommand::Snapshot(sender))
            .ok()?;
        Some(SnapshotRequest { receiver })
    }
    /// Pide un resize al parser. Devuelve `false` si la cola está llena y hay
    /// que reintentar en el siguiente frame (no bloquea la UI).
    pub fn resize(&self, request: ResizeRequest) -> bool {
        self.commands
            .try_send(ParserCommand::Resize(request))
            .is_ok()
    }

    /// Desplaza el viewport (`+` hacia el historial).
    pub fn scroll(&self, delta: i32) -> bool {
        self.commands.try_send(ParserCommand::Scroll(delta)).is_ok()
    }

    /// Vuelve al final del scrollback.
    pub fn scroll_to_bottom(&self) -> bool {
        self.commands
            .try_send(ParserCommand::ScrollToBottom)
            .is_ok()
    }

    /// Define la selección actual.
    pub fn select(&self, start: SelectionPoint, end: SelectionPoint, block: bool) -> bool {
        self.commands
            .try_send(ParserCommand::Select { start, end, block })
            .is_ok()
    }

    /// Quita la selección.
    pub fn clear_selection(&self) -> bool {
        self.commands
            .try_send(ParserCommand::ClearSelection)
            .is_ok()
    }

    /// Copia la selección; el texto llega como `TerminalEvent::Clipboard`.
    pub fn copy_selection(&self) -> bool {
        self.commands.try_send(ParserCommand::CopySelection).is_ok()
    }

    /// Informa al PTY del cambio de foco (solo si DECSET 1004 está activo).
    pub fn focus(&self, focused: bool) -> bool {
        self.commands
            .try_send(ParserCommand::Focus(focused))
            .is_ok()
    }

    /// Pide un volcado del grid si el diagnóstico está activo.
    pub fn dump(&self) {
        let _ = self.commands.try_send(ParserCommand::Dump);
    }

    /// Cierra el parser. No hace `join`: el hilo termina al vaciar el canal.
    pub fn close(&mut self) {
        let _ = self.commands.try_send(ParserCommand::Close);
        self.thread.take();
    }
}

impl std::fmt::Debug for ParserHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ParserHandle")
            .finish_non_exhaustive()
    }
}

/// Estado que comparten el listener y el parser para responder al PTY.
#[derive(Clone, Copy, Debug)]
struct WindowMetrics {
    cols: usize,
    rows: usize,
    cell_width: u16,
    cell_height: u16,
}

/// Datos que el parser necesita para arrancar.
#[derive(Debug)]
pub struct ParserSetup {
    /// Escritor compartido para respuestas al PTY.
    pub writer: PtyWriter,
    /// Paleta para `OSC 10/11/12`.
    pub palette: TerminalPalette,
    /// Tamaño inicial del grid.
    pub initial: GridSize,
    /// Ancho de celda en píxeles.
    pub cell_width: u16,
    /// Alto de celda en píxeles.
    pub cell_height: u16,
    /// Buffer previo que se reproduce antes de leer el PTY.
    pub restore: Option<TerminalSnapshot>,
}

/// Núcleo del motor VT. Vive en el hilo parser; los tests lo usan en línea.
struct TerminalCore {
    term: Term<Listener>,
    parser: Processor,
    scanner: OscScanner,
    events: Rc<RefCell<Vec<TerminalEvent>>>,
    metrics: Arc<Mutex<WindowMetrics>>,
    writer: PtyWriter,
}

impl TerminalCore {
    fn new(setup: ParserSetup) -> Self {
        let ParserSetup {
            writer,
            palette,
            initial,
            cell_width,
            cell_height,
            restore,
        } = setup;
        let metrics = Arc::new(Mutex::new(WindowMetrics {
            cols: initial.columns(),
            rows: initial.screen_lines(),
            cell_width,
            cell_height,
        }));
        let events = Rc::new(RefCell::new(Vec::new()));
        let listener = Listener {
            writer: writer.clone(),
            metrics: Arc::clone(&metrics),
            palette,
            events: Rc::clone(&events),
        };
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            ..Default::default()
        };
        let mut parser = Processor::new();
        let term = match restore {
            Some(restore) if !restore.is_empty() && restore.cols() > 0 && restore.rows() > 0 => {
                let saved = GridSize::new(usize::from(restore.cols()), usize::from(restore.rows()));
                let mut term = Term::new(config, &saved, listener);
                // Se reproduce antes de leer el PTY: el historial queda debajo
                // de la salida del shell nuevo.
                parser.advance(&mut term, restore.body());
                term.scroll_display(Scroll::Bottom);
                term
            }
            _ => Term::new(config, &initial, listener),
        };

        Self {
            term,
            parser,
            scanner: OscScanner::default(),
            events,
            metrics,
            writer,
        }
    }

    /// Exporta historial y pantalla como instantánea ANSI.
    ///
    /// Si una TUI a pantalla completa (`ALT_SCREEN`) está abierta, se sale a la
    /// pantalla principal antes de capturar y se restaura si hace falta, para
    /// no dejar el cuadro congelado de la TUI al reabrir la terminal.
    fn snapshot(&mut self) -> TerminalSnapshot {
        let was_alt = self.term.mode().contains(TermMode::ALT_SCREEN);
        if was_alt {
            self.term.swap_alt();
        }
        let snapshot = snapshot::capture(self.term.grid(), snapshot::MAX_SNAPSHOT_BYTES);
        if was_alt {
            self.term.swap_alt();
        }
        snapshot
    }

    fn advance(&mut self, bytes: &[u8]) {
        let scan = self.scanner.advance(bytes);
        for reply in scan.replies {
            let _ = self.writer.write(&reply);
        }
        self.events.borrow_mut().extend(scan.events);
        self.parser.advance(&mut self.term, bytes);
    }

    fn resize(&mut self, request: ResizeRequest) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.cols = request.cols;
            metrics.rows = request.rows;
            metrics.cell_width = request.cell_width;
            metrics.cell_height = request.cell_height;
        }
        self.term.resize(GridSize::new(request.cols, request.rows));
    }

    fn scroll(&mut self, delta: i32) {
        self.term.scroll_display(Scroll::Delta(delta));
    }

    fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    fn select(&mut self, start: SelectionPoint, end: SelectionPoint, block: bool) {
        let cols = self.term.columns();
        let rows = self.term.screen_lines();
        if cols == 0 || rows == 0 {
            return;
        }
        let offset = self.term.grid().display_offset() as i32;
        let mut start_point = Point::new(
            Line(start.row.min(rows - 1) as i32 - offset),
            Column(start.col.min(cols - 1)),
        );
        let mut end_point = Point::new(
            Line(end.row.min(rows - 1) as i32 - offset),
            Column(end.col.min(cols - 1)),
        );
        if start_point > end_point {
            std::mem::swap(&mut start_point, &mut end_point);
        }
        let ty = if block {
            SelectionType::Block
        } else {
            SelectionType::Simple
        };
        let mut selection = Selection::new(ty, start_point, Side::Left);
        selection.update(end_point, Side::Right);
        self.term.selection = Some(selection);
    }

    fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    fn copy_selection(&mut self) {
        if let Some(text) = self.term.selection_to_string() {
            self.events
                .borrow_mut()
                .push(TerminalEvent::Clipboard(text));
        }
    }

    fn focus(&mut self, focused: bool) {
        if !self.term.mode().contains(TermMode::FOCUS_IN_OUT) {
            return;
        }
        let sequence: &[u8] = if focused { b"\x1b[I" } else { b"\x1b[O" };
        let _ = self.writer.write(sequence);
    }

    fn push_event(&self, event: TerminalEvent) {
        self.events.borrow_mut().push(event);
    }

    fn frame(&mut self, force_full: bool, bytes: usize) -> ScreenFrame {
        let rows = self.term.screen_lines();
        let cols = self.term.columns();

        let display_offset = self.term.grid().display_offset();
        let history_size = self.term.grid().history_size();
        // Con historial visible el damage parcial no basta: hay que releer el
        // viewport completo. Es barato comparado con el flujo normal.
        let force_full = force_full || display_offset > 0;

        let (full, lines) = if force_full {
            (true, (0..rows).collect::<Vec<_>>())
        } else {
            let damage = self.term.damage();
            match damage {
                TermDamage::Full => (true, (0..rows).collect::<Vec<_>>()),
                TermDamage::Partial(iterator) => {
                    let mut damaged = vec![false; rows];
                    for bounds in iterator {
                        if bounds.line < rows {
                            damaged[bounds.line] = true;
                        }
                    }
                    (
                        false,
                        damaged
                            .iter()
                            .enumerate()
                            .filter_map(|(line, damaged)| damaged.then_some(line))
                            .collect(),
                    )
                }
            }
        };
        self.term.reset_damage();

        let (mode, cursor, selection) = {
            let content = self.term.renderable_content();
            let mouse_mode = if content.mode.contains(TermMode::MOUSE_MOTION) {
                MouseMode::Motion
            } else if content.mode.contains(TermMode::MOUSE_DRAG) {
                MouseMode::Drag
            } else if content.mode.contains(TermMode::MOUSE_REPORT_CLICK) {
                MouseMode::Click
            } else {
                MouseMode::Off
            };
            let mode = ModeFlags {
                bracketed_paste: content.mode.contains(TermMode::BRACKETED_PASTE),
                app_cursor: content.mode.contains(TermMode::APP_CURSOR),
                alt_screen: content.mode.contains(TermMode::ALT_SCREEN),
                show_cursor: content.mode.contains(TermMode::SHOW_CURSOR),
                focus_in_out: content.mode.contains(TermMode::FOCUS_IN_OUT),
                mouse_mode,
                sgr_mouse: content.mode.contains(TermMode::SGR_MOUSE),
                utf8_mouse: content.mode.contains(TermMode::UTF8_MOUSE),
                alternate_scroll: content.mode.contains(TermMode::ALTERNATE_SCROLL),
            };
            let cursor = match content.cursor.shape {
                AlacrittyCursor::Hidden => None,
                shape => {
                    let line = content.cursor.point.line.0 + display_offset as i32;
                    let column = content.cursor.point.column.0;
                    if line >= 0 && (line as usize) < rows && column < cols {
                        Some(CursorSnapshot {
                            line: line as usize,
                            column,
                            shape: match shape {
                                AlacrittyCursor::Underline => CursorShape::Underline,
                                AlacrittyCursor::Beam => CursorShape::Beam,
                                AlacrittyCursor::HollowBlock => CursorShape::HollowBlock,
                                _ => CursorShape::Block,
                            },
                        })
                    } else {
                        None
                    }
                }
            };
            let selection = selection_span(content.selection, rows, cols, display_offset);
            (mode, cursor, selection)
        };

        let grid = self.term.grid();
        let updates = lines
            .into_iter()
            .map(|line| {
                let grid_line = Line(line as i32 - display_offset as i32);
                let row = &grid[grid_line];
                let cells = (0..cols)
                    .map(|column| snapshot::snapshot_cell(&row[Column(column)]))
                    .collect();
                RowUpdate {
                    line,
                    cells,
                    combining: collect_combining(row, cols),
                    links: collect_links(row, cols),
                }
            })
            .collect();

        let events = std::mem::take(&mut *self.events.borrow_mut());

        ScreenFrame {
            cols,
            rows,
            bytes,
            full,
            updates,
            cursor,
            mode,
            display_offset,
            history_size,
            selection,
            events,
        }
    }
}

fn selection_span(
    selection: Option<SelectionRange>,
    rows: usize,
    cols: usize,
    display_offset: usize,
) -> Option<SelectionSpan> {
    let selection = selection?;
    if rows == 0 || cols == 0 {
        return None;
    }
    let offset = display_offset as i32;
    let start_line = selection.start.line.0 + offset;
    let end_line = selection.end.line.0 + offset;
    if end_line < 0 || start_line >= rows as i32 {
        return None;
    }

    let start_clamped_top = start_line < 0;
    let end_clamped_bottom = end_line >= rows as i32;
    let start_line = start_line.clamp(0, rows as i32 - 1) as usize;
    let end_line = end_line.clamp(0, rows as i32 - 1) as usize;
    let start_col = if start_clamped_top {
        0
    } else {
        selection.start.column.0.min(cols - 1)
    };
    let end_col = if end_clamped_bottom {
        cols - 1
    } else {
        selection.end.column.0.min(cols - 1)
    };

    Some(SelectionSpan {
        start_line,
        start_col,
        end_line,
        end_col,
        block: selection.is_block,
    })
}

fn collect_links(row: &alacritty_terminal::grid::Row<Cell>, cols: usize) -> Vec<HyperlinkSpan> {
    let mut links: Vec<HyperlinkSpan> = Vec::new();
    for column in 0..cols {
        let Some(link) = row[Column(column)].hyperlink() else {
            continue;
        };
        let uri = link.uri();
        match links.last_mut() {
            Some(last) if last.end == column && last.uri.as_str() == uri => last.end += 1,
            _ => links.push(HyperlinkSpan {
                start: column,
                end: column + 1,
                uri: uri.to_owned(),
            }),
        }
    }
    links
}

fn collect_combining(row: &alacritty_terminal::grid::Row<Cell>, cols: usize) -> Vec<(usize, char)> {
    let mut combining = Vec::new();
    for column in 0..cols {
        if let Some(chars) = row[Column(column)].zerowidth() {
            for ch in chars {
                combining.push((column, *ch));
            }
        }
    }
    combining
}

/// Arranca el hilo parser.
pub fn spawn_parser(
    commands: async_channel::Sender<ParserCommand>,
    inbox: async_channel::Receiver<ParserCommand>,
    frames: async_channel::Sender<ScreenFrame>,
    setup: ParserSetup,
) -> Result<ParserHandle, TerminalError> {
    let handle = thread::Builder::new()
        .name("axf-terminal-parser".into())
        .spawn(move || {
            let mut core = TerminalCore::new(setup);
            let mut force_full = true;

            // Diagnóstico opcional: no altera el render normal.
            let mut pty_trace = trace_file("AXFTERM_TRACE_PTY", "axf-pty.vt");
            let mut cell_trace = trace_file("AXFTERM_TRACE_CELLS", "axf-cells.txt");
            let mut dumps = 0usize;

            'outer: while let Ok(first) = inbox.recv_blocking() {
                let mut batch = vec![first];
                let mut bytes_budget = 0usize;
                while bytes_budget < BATCH_LIMIT {
                    match inbox.try_recv() {
                        Ok(command) => {
                            if let ParserCommand::Bytes(bytes) = &command {
                                bytes_budget += bytes.len();
                            }
                            batch.push(command);
                        }
                        Err(async_channel::TryRecvError::Empty) => break,
                        Err(async_channel::TryRecvError::Closed) => break 'outer,
                    }
                }

                let mut batch_bytes = 0usize;
                let mut close = false;
                for command in batch {
                    match command {
                        ParserCommand::Bytes(bytes) => {
                            batch_bytes += bytes.len();
                            if let Some(file) = pty_trace.as_mut() {
                                let _ = file.write_all(&bytes);
                            }
                            core.advance(&bytes);
                        }
                        ParserCommand::Resize(request) => {
                            core.resize(request);
                            force_full = true;
                        }
                        ParserCommand::Scroll(delta) => {
                            core.scroll(delta);
                            force_full = true;
                        }
                        ParserCommand::ScrollToBottom => {
                            core.scroll_to_bottom();
                            force_full = true;
                        }
                        ParserCommand::Select { start, end, block } => {
                            core.select(start, end, block);
                        }
                        ParserCommand::ClearSelection => core.clear_selection(),
                        ParserCommand::CopySelection => core.copy_selection(),
                        ParserCommand::Focus(focused) => core.focus(focused),
                        ParserCommand::Eof => {
                            core.push_event(TerminalEvent::Exited(None));
                        }
                        ParserCommand::Exited(info) => {
                            core.push_event(TerminalEvent::Exited(Some(info)));
                        }
                        ParserCommand::Snapshot(reply) => {
                            let snapshot = core.snapshot();
                            let snapshot = (!snapshot.is_empty()).then_some(snapshot);
                            let _ = reply.try_send(snapshot);
                        }
                        ParserCommand::Dump => {
                            if let Some(file) = cell_trace.as_mut() {
                                let _ = dump_snapshot(&core.term, file, dumps);
                                dumps += 1;
                            }
                        }
                        ParserCommand::Close => {
                            close = true;
                            break;
                        }
                    }
                }
                if close {
                    break;
                }

                let frame = core.frame(force_full, batch_bytes);
                force_full = false;
                if frames.send_blocking(frame).is_err() {
                    break;
                }
            }
        })
        .map_err(|error| TerminalError::Thread(error.to_string()))?;

    Ok(ParserHandle {
        commands,
        thread: Some(handle),
    })
}

/// Abre el archivo de traza indicado por una variable de entorno.
///
/// El valor `1` usa un nombre por defecto en el directorio temporal. Cualquier
/// otro valor se interpreta como ruta.
fn trace_file(variable: &str, default_name: &str) -> Option<File> {
    let value = std::env::var_os(variable)?;
    let path = if value == "1" {
        std::env::temp_dir().join(default_name)
    } else {
        PathBuf::from(value)
    };
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .ok()
}

/// Escribe un volcado legible del grid y de los colores dinámicos.
fn dump_snapshot(term: &Term<Listener>, file: &mut File, sequence: usize) -> std::io::Result<()> {
    let rows = term.screen_lines();
    let cols = term.columns();
    let content = term.renderable_content();
    writeln!(
        file,
        "=== snapshot {sequence} {cols}x{rows} ===\ndefaults fg={:?} bg={:?} cursor={:?}\nmode={:?}",
        content.colors[NamedColor::Foreground],
        content.colors[NamedColor::Background],
        content.colors[NamedColor::Cursor],
        content.mode,
    )?;

    let grid = term.grid();
    for line in 0..rows {
        let row = &grid[Line(line as i32)];
        let mut text = String::with_capacity(cols);
        let mut notes = Vec::new();

        for column in 0..cols {
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

            let custom_color = !matches!(cell.fg, Color::Named(NamedColor::Foreground))
                || !matches!(cell.bg, Color::Named(NamedColor::Background));
            if custom_color || !cell.flags.is_empty() || cell.hyperlink().is_some() {
                notes.push(format!(
                    "{column}:{:?} fg={:?} bg={:?} flags={:?}",
                    cell.c, cell.fg, cell.bg, cell.flags
                ));
            }
        }

        writeln!(file, "row {line:02}: {text}")?;
        if !notes.is_empty() {
            writeln!(file, "  {}", notes.join(" | "))?;
        }
    }

    writeln!(file)?;
    file.flush()
}

/// Listener que devuelve al PTY lo que el motor VT pide.
#[derive(Clone)]
struct Listener {
    writer: PtyWriter,
    metrics: Arc<Mutex<WindowMetrics>>,
    palette: TerminalPalette,
    events: Rc<RefCell<Vec<TerminalEvent>>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                let _ = self.writer.write(text.as_bytes());
            }
            Event::TextAreaSizeRequest(format) => {
                let metrics = match self.metrics.lock() {
                    Ok(metrics) => *metrics,
                    Err(poisoned) => *poisoned.into_inner(),
                };
                let reply = format(WindowSize {
                    num_lines: metrics.rows.min(usize::from(u16::MAX)) as u16,
                    num_cols: metrics.cols.min(usize::from(u16::MAX)) as u16,
                    cell_width: metrics.cell_width,
                    cell_height: metrics.cell_height,
                });
                let _ = self.writer.write(reply.as_bytes());
            }
            Event::ColorRequest(index, format) => {
                let rgb = self.palette.named(index).to_rgb();
                let reply = format(Rgb {
                    r: (rgb.r.clamp(0.0, 1.0) * 255.0).round() as u8,
                    g: (rgb.g.clamp(0.0, 1.0) * 255.0).round() as u8,
                    b: (rgb.b.clamp(0.0, 1.0) * 255.0).round() as u8,
                });
                let _ = self.writer.write(reply.as_bytes());
            }
            Event::Title(title) => self
                .events
                .borrow_mut()
                .push(TerminalEvent::Title(Some(title))),
            Event::ResetTitle => self.events.borrow_mut().push(TerminalEvent::Title(None)),
            Event::Bell => self.events.borrow_mut().push(TerminalEvent::Bell),
            _ => {}
        }
    }
}

/// Salida de un avance del escáner: eventos y respuestas para el PTY.
#[derive(Default)]
struct ScanOutput {
    events: Vec<TerminalEvent>,
    replies: Vec<Vec<u8>>,
}

/// Escáner incremental de OSC que vte no interpreta (por ejemplo OSC 7).
///
/// Mantiene estado entre lecturas para que una secuencia partida en dos chunks
/// no se pierda. Solo emite los códigos que Axfterm necesita.
#[derive(Default)]
struct OscScanner {
    state: OscState,
    body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum OscState {
    #[default]
    Ground,
    Esc,
    Osc,
    OscEsc,
}

/// Respuesta a `OSC 7501 ; ?` con los estados y kinds soportados.
const PROGRAM_STATUS_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";
/// Límite de la secuencia completa de OSC 7501 (ADR 0009 / propuesta 0.4).
const PROGRAM_STATUS_LIMIT: usize = 4096;
/// Límite de una clave de OSC 7501.
const PROGRAM_KEY_LIMIT: usize = 16;

impl OscScanner {
    fn advance(&mut self, bytes: &[u8]) -> ScanOutput {
        let mut output = ScanOutput::default();
        for &byte in bytes {
            match self.state {
                OscState::Ground => {
                    if byte == 0x1b {
                        self.state = OscState::Esc;
                    }
                }
                OscState::Esc => {
                    if byte == b']' {
                        self.body.clear();
                        self.state = OscState::Osc;
                    } else if byte != 0x1b {
                        self.state = OscState::Ground;
                    }
                }
                OscState::Osc => match byte {
                    0x07 => {
                        if let Some(event) = self.finish(&mut output) {
                            output.events.push(event);
                        }
                        self.state = OscState::Ground;
                    }
                    0x1b => self.state = OscState::OscEsc,
                    _ => {
                        if self.body.len() < OSC_LIMIT {
                            self.body.push(byte);
                        }
                    }
                },
                OscState::OscEsc => {
                    if byte == b'\\' {
                        if let Some(event) = self.finish(&mut output) {
                            output.events.push(event);
                        }
                        self.state = OscState::Ground;
                    } else {
                        // Un ESC que no forma ST aborta el OSC; si empieza otra
                        // secuencia, la procesamos desde Ground. Así una OSC
                        // truncada no se traga la siguiente OSC válida.
                        self.body.clear();
                        self.state = if byte == 0x1b {
                            OscState::Esc
                        } else {
                            OscState::Ground
                        };
                    }
                }
            }
        }
        output
    }

    fn finish(&mut self, output: &mut ScanOutput) -> Option<TerminalEvent> {
        let body = std::mem::take(&mut self.body);
        let (code, payload) = split_osc(&body)?;
        match code {
            7 => file_uri_to_path(payload).map(TerminalEvent::Cwd),
            9 | 777 | 1337 => notification_text(code, payload)
                .map(|text| TerminalEvent::Attention((!text.is_empty()).then_some(text))),
            22 => Some(TerminalEvent::MouseCursor(payload.to_owned())),
            7501 => {
                if is_program_status_query(payload) {
                    output.replies.push(PROGRAM_STATUS_REPLY.to_vec());
                    return None;
                }
                parse_program_status(payload).map(TerminalEvent::ProgramStatus)
            }
            _ => None,
        }
    }
}

fn is_program_status_query(payload: &str) -> bool {
    let payload = payload.trim();
    payload == "?" || payload.starts_with("?:")
}

/// Parsea `OSC 7501`; un reporte inválido se descarta entero.
fn parse_program_status(payload: &str) -> Option<ProgramStatus> {
    let payload = payload.trim();
    if payload.is_empty() || payload.len() > PROGRAM_STATUS_LIMIT {
        return None;
    }
    let mut status = ProgramStatus {
        state: ProgramState::Idle,
        kind: None,
        progress: None,
        app: None,
        title: None,
        message: None,
        id: None,
    };
    let mut state: Option<ProgramState> = None;

    for pair in payload.split(':') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.len() > PROGRAM_KEY_LIMIT {
            return None;
        }
        match key {
            "state" => {
                state = match value {
                    "idle" => Some(ProgramState::Idle),
                    "working" => Some(ProgramState::Working),
                    "blocked" => Some(ProgramState::Blocked),
                    "done" => Some(ProgramState::Done),
                    "error" => Some(ProgramState::Error),
                    "clear" => Some(ProgramState::Clear),
                    _ => None,
                };
            }
            "kind" => {
                status.kind = match value {
                    "permission" | "question" | "auth" => Some(value.to_owned()),
                    _ => None,
                };
            }
            "progress" => {
                status.progress = value.parse::<u8>().ok().filter(|value| *value <= 100);
            }
            "app" => status.app = valid_app(value),
            "title" => match decode_text(value, 256, 192) {
                Ok(text) => status.title = text,
                Err(()) => return None,
            },
            "msg" => match decode_text(value, 2732, 2048) {
                Ok(text) => status.message = text,
                Err(()) => return None,
            },
            "id" => match parse_id(value) {
                Ok(id) => status.id = id,
                Err(()) => return None,
            },
            _ => {}
        }
    }

    let state = state?;
    status.state = state;
    Some(status)
}

/// `app`: hasta 32 caracteres de `A-Z a-z 0-9 _ . + -`; otro valor cuenta como
/// ninguno (no invalida el reporte).
fn valid_app(value: &str) -> Option<String> {
    if value.is_empty() || value.len() > 32 {
        return None;
    }
    value
        .chars()
        .all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '+' | '-')
        })
        .then(|| value.to_owned())
}

/// `id`: ruta de hasta 8 segmentos de 32 caracteres; un `id` inválido descarta
/// el reporte entero.
fn parse_id(value: &str) -> Result<Option<String>, ()> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 128 {
        return Err(());
    }
    let segments: Vec<&str> = value.split('/').collect();
    if segments.len() > 8 {
        return Err(());
    }
    for segment in &segments {
        if segment.is_empty() || segment.len() > 32 {
            return Err(());
        }
        let valid = segment.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | '.' | ',' | '+' | '=' | '-')
        });
        if !valid {
            return Err(());
        }
    }
    Ok(Some(value.to_owned()))
}

/// Decodifica base64 y rechaza texto con caracteres de control.
fn decode_text(
    value: &str,
    encoded_limit: usize,
    decoded_limit: usize,
) -> Result<Option<String>, ()> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > encoded_limit {
        return Err(());
    }
    let decoded = base64_decode(value).ok_or(())?;
    if decoded.len() > decoded_limit {
        return Err(());
    }
    let text = String::from_utf8(decoded).map_err(|_| ())?;
    if text.chars().any(is_forbidden_text_character) {
        return Err(());
    }
    Ok((!text.is_empty()).then_some(text))
}

fn is_forbidden_text_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{202a}'
                | '\u{202b}'
                | '\u{202c}'
                | '\u{202d}'
                | '\u{202e}'
                | '\u{2066}'
                | '\u{2067}'
                | '\u{2068}'
                | '\u{2069}'
        )
}

/// Base64 estándar con relleno opcional.
fn base64_decode(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0u8;
    let mut padded = false;
    for &byte in bytes {
        if byte == b'=' {
            padded = true;
            continue;
        }
        if padded {
            return None;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}

/// Texto de una notificación OSC 9/777/1337, sin controles.
fn notification_text(code: u32, payload: &str) -> Option<String> {
    let text = match code {
        9 => {
            // OSC 9;4 es progreso de ConEmu, no una notificación.
            if payload.trim_start().starts_with("4;") {
                return None;
            }
            payload
        }
        777 => {
            let mut parts = payload.split(';');
            let _kind = parts.next();
            let title = parts.next().unwrap_or_default();
            let body = parts.next().unwrap_or_default();
            if body.is_empty() { title } else { body }
        }
        1337 => {
            let payload = payload
                .strip_prefix("Notify=")
                .or_else(|| payload.strip_prefix("Notification="))
                .unwrap_or_default();
            payload.rsplit_once(';').map_or(payload, |(_, body)| body)
        }
        _ => return None,
    };
    let text: String = text
        .chars()
        .filter(|character| !is_forbidden_text_character(*character))
        .take(2048)
        .collect();
    Some(text.trim().to_owned())
}

fn split_osc(body: &[u8]) -> Option<(u32, &str)> {
    let separator = body.iter().position(|&byte| byte == b';')?;
    let code = std::str::from_utf8(&body[..separator]).ok()?.parse().ok()?;
    let payload = std::str::from_utf8(&body[separator + 1..]).ok()?;
    Some((code, payload))
}

/// Convierte `file://` en una ruta del sistema.
fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let decoded = percent_decode(rest);

    #[cfg(windows)]
    {
        let decoded = decoded.strip_prefix("localhost").unwrap_or(&decoded);
        let decoded = decoded.strip_prefix('/').unwrap_or(decoded);
        // `file:///C:/...` llega como `/C:/...`; `file://server/share` como
        // `server/share` y se convierte en UNC.
        let looks_like_drive = decoded.as_bytes().get(1).is_some_and(|&byte| byte == b':');
        if looks_like_drive {
            return Some(PathBuf::from(decoded.replace('/', "\\")));
        }
        Some(PathBuf::from(format!(r"\\{}", decoded.replace('/', "\\"))))
    }
    #[cfg(not(windows))]
    {
        Some(PathBuf::from(decoded))
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            decoded.push(high * 16 + low);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Escritor de diagnóstico que guarda lo que el motor responde al PTY.
#[derive(Clone, Default)]
struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        match self.0.lock() {
            Ok(mut captured) => captured.extend_from_slice(buffer),
            Err(poisoned) => poisoned.into_inner().extend_from_slice(buffer),
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Utilidad de diagnóstico para tests y benchmarks del motor.
///
/// No forma parte de la API de la UI; existe para medir y fijar casos VT sin
/// levantar una ventana.
#[doc(hidden)]
pub struct EngineBench {
    core: TerminalCore,
    written: Arc<Mutex<Vec<u8>>>,
}

impl std::fmt::Debug for EngineBench {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EngineBench")
            .finish_non_exhaustive()
    }
}

impl EngineBench {
    /// Crea un motor con un escritor que guarda las respuestas.
    pub fn new(cols: usize, rows: usize, cell_width: u16, cell_height: u16) -> Self {
        let written = Arc::new(Mutex::new(Vec::new()));
        let setup = ParserSetup {
            writer: PtyWriter::new(Box::new(CaptureWriter(Arc::clone(&written)))),
            palette: TerminalPalette::placeholder(),
            initial: GridSize::new(cols, rows),
            cell_width,
            cell_height,
            restore: None,
        };
        Self {
            core: TerminalCore::new(setup),
            written,
        }
    }

    /// Motor que arranca con una instantánea restaurada.
    pub fn from_snapshot(
        snapshot: TerminalSnapshot,
        cols: usize,
        rows: usize,
        cell_width: u16,
        cell_height: u16,
    ) -> Self {
        let written = Arc::new(Mutex::new(Vec::new()));
        let setup = ParserSetup {
            writer: PtyWriter::new(Box::new(CaptureWriter(Arc::clone(&written)))),
            palette: TerminalPalette::placeholder(),
            initial: GridSize::new(cols, rows),
            cell_width,
            cell_height,
            restore: Some(snapshot),
        };
        Self {
            core: TerminalCore::new(setup),
            written,
        }
    }

    /// Bytes que el motor escribió de vuelta (respuestas DSR, OSC 7501, etc.).
    pub fn written(&self) -> Vec<u8> {
        match self.written.lock() {
            Ok(captured) => captured.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Exporta el estado actual para probar el roundtrip.
    pub fn snapshot(&mut self) -> TerminalSnapshot {
        self.core.snapshot()
    }

    /// Exporta con un tope de bytes explícito para probar el recorte.
    pub fn snapshot_with_limit(&self, limit: usize) -> TerminalSnapshot {
        snapshot::capture(self.core.term.grid(), limit)
    }

    /// Alimenta bytes al parser.
    pub fn advance(&mut self, bytes: &[u8]) {
        self.core.advance(bytes);
    }

    /// Cambia de tamaño.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.core.resize(ResizeRequest {
            cols,
            rows,
            cell_width: 8,
            cell_height: 16,
        });
    }

    /// Desplaza el viewport.
    pub fn scroll(&mut self, delta: i32) {
        self.core.scroll(delta);
    }

    /// Selecciona y devuelve el texto copiado.
    pub fn select_and_copy(
        &mut self,
        start: SelectionPoint,
        end: SelectionPoint,
        block: bool,
    ) -> Option<String> {
        self.core.select(start, end, block);
        self.core.term.selection_to_string()
    }

    /// Construye un frame completo.
    pub fn frame(&mut self) -> ScreenFrame {
        self.core.frame(true, 0)
    }
}
