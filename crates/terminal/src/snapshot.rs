//! Instantáneas del buffer de una terminal.
//!
//! El historial se guarda como una transcripción ANSI del grid (historial +
//! pantalla) para poder restaurarlo con el mismo parser que interpreta el PTY.
//! El formato es autónomo, versionado y con checksum; si algo no cuadra, la
//! app descarta la instantánea y abre una terminal nueva desde cero.
//!
//! No se guardan hipervínculos, selección, modos de ratón ni la pantalla
//! alternativa: solo el contenido visible que el usuario puede volver a leer.

use std::fmt::Write as _;

use alacritty_terminal::grid::{Dimensions, Grid, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, NamedColor};

use crate::engine::{Attrs, CellColor, SnapshotCell};

/// Versión del formato de instantánea que entiende esta build.
pub const SNAPSHOT_VERSION: u32 = 1;

/// Tope de bytes de la transcripción por terminal (ADR 0005/0008).
pub const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

const MAGIC: &str = "AXFSNAP";
const MAX_COLS: u16 = 2000;
const MAX_ROWS: u16 = 512;

/// Error al leer una instantánea.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    /// El archivo no empieza con la firma esperada.
    #[error("no es una instantánea de Axfterm")]
    Magic,
    /// Versión futura o desconocida.
    #[error("instantánea con versión no soportada: {0}")]
    UnsupportedVersion(u32),
    /// Cabecera truncada o con campos inválidos.
    #[error("cabecera de instantánea inválida")]
    Header,
    /// El cuerpo supera el límite o no coincide con la longitud declarada.
    #[error("instantánea demasiado grande o truncada")]
    TooLarge,
    /// El checksum no coincide con el cuerpo.
    #[error("checksum de instantánea incorrecto")]
    Checksum,
    /// Dimensiones fuera del rango que puede tener una ventana.
    #[error("dimensiones de instantánea fuera de rango: {cols}x{rows}")]
    Dimensions {
        /// Columnas declaradas.
        cols: u32,
        /// Filas declaradas.
        rows: u32,
    },
}

/// Buffer exportado por una terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSnapshot {
    cols: u16,
    rows: u16,
    truncated: bool,
    body: Vec<u8>,
}

impl TerminalSnapshot {
    /// Columnas del grid original.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Filas visibles del grid original.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// `true` si el tope de bytes obligó a descartar las líneas más antiguas.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// `true` si no hay contenido que restaurar.
    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }

    /// Cuerpo ANSI que se entrega al parser al restaurar.
    pub(crate) fn body(&self) -> &[u8] {
        &self.body
    }

    /// Serializa cabecera + cuerpo.
    pub fn encode(&self) -> Vec<u8> {
        let header = format!(
            "{MAGIC} {SNAPSHOT_VERSION} {} {} {} {} {:016x}\n",
            self.cols,
            self.rows,
            u8::from(self.truncated),
            self.body.len(),
            checksum(&self.body),
        );
        let mut bytes = Vec::with_capacity(header.len() + self.body.len());
        bytes.extend_from_slice(header.as_bytes());
        bytes.extend_from_slice(&self.body);
        bytes
    }

    /// Lee y valida una instantánea.
    pub fn decode(bytes: &[u8]) -> Result<Self, SnapshotError> {
        let newline = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or(SnapshotError::Header)?;
        let header = std::str::from_utf8(&bytes[..newline]).map_err(|_| SnapshotError::Header)?;
        let mut fields = header.split(' ');
        if fields.next() != Some(MAGIC) {
            return Err(SnapshotError::Magic);
        }
        let version: u32 = fields
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or(SnapshotError::Header)?;
        if version != SNAPSHOT_VERSION {
            return Err(SnapshotError::UnsupportedVersion(version));
        }
        let cols: u16 = fields
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or(SnapshotError::Header)?;
        let rows: u16 = fields
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or(SnapshotError::Header)?;
        let truncated: u8 = fields
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or(SnapshotError::Header)?;
        let declared: usize = fields
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or(SnapshotError::Header)?;
        let expected: u64 = fields
            .next()
            .and_then(|value| u64::from_str_radix(value, 16).ok())
            .ok_or(SnapshotError::Header)?;
        if fields.next().is_some() {
            return Err(SnapshotError::Header);
        }
        if cols == 0 || rows == 0 || cols > MAX_COLS || rows > MAX_ROWS {
            return Err(SnapshotError::Dimensions {
                cols: u32::from(cols),
                rows: u32::from(rows),
            });
        }
        if declared > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::TooLarge);
        }
        let body_start = newline + 1;
        let body = &bytes[body_start..];
        if body.len() != declared {
            return Err(SnapshotError::TooLarge);
        }
        if checksum(body) != expected {
            return Err(SnapshotError::Checksum);
        }
        Ok(Self {
            cols,
            rows,
            truncated: truncated != 0,
            body: body.to_vec(),
        })
    }
}

/// Exporta historial y pantalla de un grid.
///
/// Se conservan las líneas más recientes si la transcripción supera `limit`.
pub(crate) fn capture(grid: &Grid<Cell>, limit: usize) -> TerminalSnapshot {
    let cols = grid.columns();
    let rows = grid.screen_lines();
    let (Ok(cols), Ok(rows)) = (u16::try_from(cols), u16::try_from(rows)) else {
        return empty();
    };
    if cols == 0 || rows == 0 || cols > MAX_COLS || rows > MAX_ROWS {
        return empty();
    }

    let mut encoded = Vec::new();
    let mut bytes = 0usize;
    let mut truncated = false;
    let mut seen_content = false;
    let top = grid.topmost_line().0;
    let bottom = grid.bottommost_line().0;

    // De abajo arriba: primero interesan las líneas más recientes.
    for line in (top..=bottom).rev() {
        let row = &grid[Line(line)];
        let has_content = (0..usize::from(cols))
            .rev()
            .any(|column| !is_blank(&row[Column(column)]));
        if !seen_content && !has_content {
            // Líneas en blanco al final de la pantalla: no aportan.
            continue;
        }
        seen_content = true;
        let buffer = encode_row(row, usize::from(cols));
        // Reserva para el reset final que devuelve los atributos por defecto
        // antes del prompt del shell nuevo.
        if bytes.saturating_add(buffer.len()).saturating_add(4) > limit {
            truncated = true;
            break;
        }
        bytes += buffer.len();
        encoded.push(buffer);
    }

    // Tampoco interesan las líneas en blanco más antiguas del historial.
    while encoded.last().is_some_and(|row| row == b"\r\n") {
        encoded.pop();
    }
    encoded.reverse();

    let mut body = Vec::with_capacity(bytes + 4);
    for row in encoded {
        body.extend_from_slice(&row);
    }
    if !body.is_empty() {
        // El prompt del shell nuevo debe empezar con los atributos por defecto.
        body.extend_from_slice(b"\x1b[0m");
    }

    TerminalSnapshot {
        cols,
        rows,
        truncated,
        body,
    }
}

fn empty() -> TerminalSnapshot {
    TerminalSnapshot {
        cols: 0,
        rows: 0,
        truncated: false,
        body: Vec::new(),
    }
}

fn encode_row(row: &Row<Cell>, cols: usize) -> Vec<u8> {
    let last = (0..cols)
        .rev()
        .find(|column| !is_blank(&row[Column(*column)]));
    let Some(last) = last else {
        return b"\r\n".to_vec();
    };

    let mut out = b"\x1b[0m".to_vec();
    let mut style = Style::default();
    for column in 0..=last {
        let cell = &row[Column(column)];
        // El espaciador normal pertenece al carácter ancho anterior. El
        // espaciador inicial sí se conserva como espacio para no desplazar la
        // fila cuando un carácter ancho no cupo al final de la anterior.
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let next = Style::from_cell(cell);
        if next != style {
            next.write_sgr(&mut out);
            style = next;
        }
        let character = if cell.c.is_control() { ' ' } else { cell.c };
        push_char(&mut out, character);
        if let Some(zerowidth) = cell.zerowidth() {
            for character in zerowidth {
                push_char(&mut out, *character);
            }
        }
    }
    out.extend_from_slice(b"\r\n");
    out
}

fn push_char(out: &mut Vec<u8>, character: char) {
    let mut buffer = [0u8; 4];
    out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
}

fn is_blank(cell: &Cell) -> bool {
    let text = cell.c == ' ' || cell.c == '\0';
    let default_colors = matches!(cell.fg, Color::Named(NamedColor::Foreground))
        && matches!(cell.bg, Color::Named(NamedColor::Background));
    let visible = Flags::BOLD
        | Flags::DIM
        | Flags::ITALIC
        | Flags::UNDERLINE
        | Flags::INVERSE
        | Flags::STRIKEOUT
        | Flags::HIDDEN
        | Flags::WIDE_CHAR;
    let unadorned = !cell.flags.intersects(visible);
    let no_combining = cell.zerowidth().is_none_or(<[char]>::is_empty);
    text && default_colors && unadorned && no_combining
}

/// Atributos que importan al SGR; ancho y espaciador son geometría.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Style {
    fg: CellColor,
    bg: CellColor,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    reverse: bool,
    strikethrough: bool,
    hidden: bool,
}

impl Style {
    fn from_cell(cell: &Cell) -> Self {
        let attrs = attrs_of(cell);
        Self {
            fg: color_of(cell.fg),
            bg: color_of(cell.bg),
            bold: attrs.bold,
            dim: attrs.dim,
            italic: attrs.italic,
            underline: attrs.underline,
            reverse: attrs.reverse,
            strikethrough: attrs.strikethrough,
            hidden: attrs.hidden,
        }
    }

    fn write_sgr(self, out: &mut Vec<u8>) {
        let mut sequence = String::from("\x1b[0");
        if self.bold {
            sequence.push_str(";1");
        }
        if self.dim {
            sequence.push_str(";2");
        }
        if self.italic {
            sequence.push_str(";3");
        }
        if self.underline {
            sequence.push_str(";4");
        }
        if self.reverse {
            sequence.push_str(";7");
        }
        if self.hidden {
            sequence.push_str(";8");
        }
        if self.strikethrough {
            sequence.push_str(";9");
        }
        match self.fg {
            CellColor::Default => {}
            CellColor::Indexed(index) => {
                let _ = write!(sequence, ";38;5;{index}");
            }
            CellColor::Rgb(rgb) => {
                let _ = write!(
                    sequence,
                    ";38;2;{};{};{}",
                    (rgb >> 16) & 0xff,
                    (rgb >> 8) & 0xff,
                    rgb & 0xff
                );
            }
        }
        match self.bg {
            CellColor::Default => {}
            CellColor::Indexed(index) => {
                let _ = write!(sequence, ";48;5;{index}");
            }
            CellColor::Rgb(rgb) => {
                let _ = write!(
                    sequence,
                    ";48;2;{};{};{}",
                    (rgb >> 16) & 0xff,
                    (rgb >> 8) & 0xff,
                    rgb & 0xff
                );
            }
        }
        sequence.push('m');
        out.extend_from_slice(sequence.as_bytes());
    }
}

pub(crate) fn snapshot_cell(cell: &Cell) -> SnapshotCell {
    SnapshotCell {
        ch: cell.c,
        fg: color_of(cell.fg),
        bg: color_of(cell.bg),
        attrs: attrs_of(cell),
    }
}

pub(crate) fn attrs_of(cell: &Cell) -> Attrs {
    Attrs {
        bold: cell.flags.contains(Flags::BOLD),
        dim: cell.flags.contains(Flags::DIM),
        italic: cell.flags.contains(Flags::ITALIC),
        underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
        reverse: cell.flags.contains(Flags::INVERSE),
        strikethrough: cell.flags.contains(Flags::STRIKEOUT),
        hidden: cell.flags.contains(Flags::HIDDEN),
        wide_spacer: cell.flags.contains(Flags::WIDE_CHAR_SPACER),
        wide: cell.flags.contains(Flags::WIDE_CHAR),
    }
}

pub(crate) fn color_of(color: Color) -> CellColor {
    match color {
        Color::Named(named) => match named as usize {
            0..=15 => CellColor::Indexed(named as u8),
            256 | 267 | 268 => CellColor::Default,
            257 => CellColor::Default,
            258 => CellColor::Indexed(8),
            259..=266 => CellColor::Indexed((named as usize - 259) as u8),
            _ => CellColor::Default,
        },
        Color::Indexed(index) => CellColor::Indexed(index),
        Color::Spec(rgb) => {
            CellColor::Rgb(u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b))
        }
    }
}

/// FNV-1a de 64 bits sobre el cuerpo ANSI.
fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}
