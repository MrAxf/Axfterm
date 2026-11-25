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
        Self::with_scrollback(cols, rows, cell_width, cell_height, SCROLLBACK_LINES)
    }

    /// Motor con un historial explícito, para probar el tope de memoria.
    pub fn with_scrollback(
        cols: usize,
        rows: usize,
        cell_width: u16,
        cell_height: u16,
        scrollback: usize,
    ) -> Self {
        let written = Arc::new(Mutex::new(Vec::new()));
        let setup = ParserSetup {
            writer: PtyWriter::new(Box::new(CaptureWriter(Arc::clone(&written)))),
            palette: TerminalPalette::placeholder(),
            initial: GridSize::new(cols, rows),
            cell_width,
            cell_height,
            restore: None,
            scrollback,
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
            scrollback: SCROLLBACK_LINES,
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
