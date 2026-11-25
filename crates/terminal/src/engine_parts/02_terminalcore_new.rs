impl TerminalCore {
    fn new(setup: ParserSetup) -> Self {
        let ParserSetup {
            writer,
            palette,
            initial,
            cell_width,
            cell_height,
            restore,
            scrollback,
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
        // La restauración puede traer un ancho distinto al inicial; el tope de
        // memoria se calcula con el ancho real del primer grid.
        let restore_cols = restore
            .as_ref()
            .filter(|restore| !restore.is_empty() && restore.cols() > 0)
            .map(|restore| usize::from(restore.cols()))
            .unwrap_or_else(|| initial.columns());
        let config = Config {
            scrolling_history: crate::engine::effective_scrollback(scrollback, restore_cols),
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
            requested_scrollback: scrollback,
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
        // El tope de 32 MiB depende del ancho: al ensanchar se recorta el
        // historial antiguo para no superarlo.
        let limit = crate::engine::effective_scrollback(self.requested_scrollback, request.cols);
        self.term.grid_mut().update_history(limit);
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
