use crate::session::{ExitInfo, PtyWriter};
use crate::snapshot::{self, TerminalSnapshot};
use crate::{GridSize, TerminalError};
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
use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
const SCROLLBACK_LINES: usize = 10_000;
const MAX_SCROLLBACK_BYTES: usize = 32 * 1024 * 1024;
const BATCH_LIMIT: usize = 256 * 1024;
const OSC_LIMIT: usize = 8 * 1024;

/// Líneas de historial efectivas para `cols` columnas, respetando el tope de
/// 32 MiB por terminal (ADR 0005). El resto de la memoria del motor (grid,
/// parser, cachés de render) no entra en este cálculo.
pub fn effective_scrollback(requested: usize, cols: usize) -> usize {
    if requested == 0 || cols == 0 {
        return 0;
    }
    let per_line = cols
        .saturating_mul(std::mem::size_of::<Cell>())
        .max(1);
    requested.min(MAX_SCROLLBACK_BYTES / per_line)
}
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionPoint {
    /// Fila visible (0 = arriba).
    pub row: usize,
    /// Columna visible.
    pub col: usize,
}
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HyperlinkSpan {
    /// Columna inicial (inclusive).
    pub start: usize,
    /// Columna final (exclusive).
    pub end: usize,
    /// URI tal como la entregó la aplicación.
    pub uri: String,
}
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorSnapshot {
    /// Fila del viewport.
    pub line: usize,
    /// Columna.
    pub column: usize,
    /// Forma.
    pub shape: CursorShape,
}
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
pub struct ParserHandle {
    commands: async_channel::Sender<ParserCommand>,
    thread: Option<thread::JoinHandle<()>>,
}
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
#[derive(Clone, Copy, Debug)]
struct WindowMetrics {
    cols: usize,
    rows: usize,
    cell_width: u16,
    cell_height: u16,
}
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
    /// Líneas de historial pedidas por la configuración.
    pub scrollback: usize,
}
struct TerminalCore {
    term: Term<Listener>,
    parser: Processor,
    scanner: OscScanner,
    events: Rc<RefCell<Vec<TerminalEvent>>>,
    metrics: Arc<Mutex<WindowMetrics>>,
    writer: PtyWriter,
    requested_scrollback: usize,
}
