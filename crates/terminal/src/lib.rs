//! Motor de terminal de Axfterm: PTY, parser VT, codificación de input y
//! `TerminalView`.
//!
//! Frontera de tipos (ADR 0001): `alacritty_terminal` no sale de este crate; la
//! UI consume `ScreenFrame` y `TerminalView`.

use alacritty_terminal::grid::Dimensions;

pub mod engine;
pub mod ime;
pub mod input;
pub mod mouse;
pub mod paste;
pub mod paths;
pub mod session;
pub mod snapshot;
pub mod view;

pub use engine::{
    CellColor, CursorShape, CursorSnapshot, EngineBench, HyperlinkSpan, ModeFlags, MouseMode,
    ProgramState, ProgramStatus, ScreenFrame, SelectionPoint, SelectionSpan, SnapshotCell,
    SnapshotRequest, TerminalEvent,
};
pub use ime::ImeState;
pub use session::{ExitInfo, PtySession, PtySizeSpec, PtyWriter, SpawnSpec, TerminalSession};
pub use snapshot::{MAX_SNAPSHOT_BYTES, SNAPSHOT_VERSION, SnapshotError, TerminalSnapshot};
pub use view::{
    CopySelection, PasteSelection, SendBacktab, SendTab, TERMINAL_CONTEXT, TerminalSignal,
    TerminalStyle, TerminalView,
};

/// Error de arranque o de hilo del terminal.
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    /// El PTY no pudo arrancar el programa del perfil.
    #[error("no se pudo iniciar `{program}`: {message}. Revisa el perfil en config.toml.")]
    Spawn {
        /// Ejecutable pedido.
        program: String,
        /// Causa del sistema operativo.
        message: String,
    },
    /// Error del PTY o del shell.
    #[error("PTY: {0}")]
    Pty(String),
    /// Error al crear un hilo.
    #[error("hilo: {0}")]
    Thread(String),
}

/// Dimensiones que entiende `alacritty_terminal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    columns: usize,
    screen_lines: usize,
}

impl GridSize {
    /// Crea un tamaño saneado (mínimo 1x1).
    pub fn new(columns: usize, screen_lines: usize) -> Self {
        Self {
            columns: columns.max(1),
            screen_lines: screen_lines.max(1),
        }
    }
}

impl Dimensions for GridSize {
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
