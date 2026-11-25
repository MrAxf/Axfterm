impl TerminalView {
    /// Traduce una posición de ventana a una celda 0-based del grid.
    fn mouse_cell(&self, position: Point<Pixels>) -> Option<(usize, usize)> {
        let geometry = self.geometry?;
        let state = self.state.borrow();
        if state.cols == 0 || state.rows == 0 {
            return None;
        }

        let x = f32::from(position.x - geometry.origin.x) / f32::from(geometry.cell_width);
        let y = f32::from(position.y - geometry.origin.y) / f32::from(geometry.line_height);
        if x < 0.0 || y < 0.0 {
            return None;
        }

        let column = (x.floor() as usize).min(state.cols - 1);
        let row = (y.floor() as usize).min(state.rows - 1);
        Some((column, row))
    }

    fn write_mouse(&self, action: MouseAction, position: Point<Pixels>, modifiers: Modifiers) {
        let mode = self.state.borrow().mode;
        if mode.mouse_mode == MouseMode::Off {
            return;
        }
        let Some((column, row)) = self.mouse_cell(position) else {
            return;
        };
        let bytes = mouse::encode(
            action,
            column + 1,
            row + 1,
            MouseModifiers {
                shift: modifiers.shift,
                alt: modifiers.alt,
                control: modifiers.control,
            },
            mode.sgr_mouse,
            mode.utf8_mouse,
        );
        self.write(&bytes);
    }

    fn set_selection(&self, start: SelectionPoint, end: SelectionPoint, block: bool) {
        if let Some(parser) = &self.parser {
            parser.select(start, end, block);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // Un clic real cancela cualquier marca de arrastre previa.
        self.drag_hover = false;
        if event.button == MouseButton::Left {
            window.focus(&self.focus, cx);
        }

        let mode = self.state.borrow().mode;
        if event.button == MouseButton::Left && mode.mouse_mode == MouseMode::Off {
            if event.modifiers.control
                && let Some((col, row)) = self.mouse_cell(event.position)
                && let Some(url) = self.url_at(row, col)
            {
                self.pending_url = Some(url);
                cx.notify();
                return;
            }

            if let Some((col, row)) = self.mouse_cell(event.position) {
                let point = SelectionPoint { row, col };
                if event.modifiers.shift {
                    let anchor = self.selection_anchor.unwrap_or(point);
                    self.selection_anchor = Some(anchor);
                    self.selection_dragging = false;
                    self.selection_moved = true;
                    self.set_selection(anchor, point, event.modifiers.alt);
                } else {
                    self.clear_selection();
                    self.selection_anchor = Some(point);
                    self.selection_dragging = true;
                    self.selection_moved = false;
                }
                cx.notify();
            }
            return;
        }

        if let Some(button) = terminal_mouse_button(event.button) {
            self.write_mouse(MouseAction::Press(button), event.position, event.modifiers);
        }
    }

    fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // Un «drop» externo también llega como MouseUp; no debe convertirse
        // en un clic dentro de una TUI con tracking de ratón.
        if cx.has_active_drag() || self.drag_hover {
            self.drag_hover = false;
            return;
        }
        let mode = self.state.borrow().mode;
        if event.button == MouseButton::Left && mode.mouse_mode == MouseMode::Off {
            if self.selection_dragging {
                self.selection_dragging = false;
                if self.selection_moved {
                    if let Some((col, row)) = self.mouse_cell(event.position) {
                        let end = SelectionPoint { row, col };
                        let start = self.selection_anchor.unwrap_or(end);
                        self.set_selection(start, end, event.modifiers.alt);
                    }
                } else {
                    // Un clic sin arrastre limpia la selección y no deja una
                    // celda marcada.
                    self.selection_anchor = None;
                    self.selection_moved = false;
                }
            }
            return;
        }

        if let Some(button) = terminal_mouse_button(event.button) {
            self.write_mouse(
                MouseAction::Release(button),
                event.position,
                event.modifiers,
            );
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if cx.has_active_drag() {
            self.drag_hover = true;
            return;
        }
        let mode = self.state.borrow().mode;
        if mode.mouse_mode != MouseMode::Off {
            let should_send = mode.mouse_mode == MouseMode::Motion
                || (mode.mouse_mode == MouseMode::Drag && event.pressed_button.is_some());
            if should_send {
                let button = event.pressed_button.and_then(terminal_mouse_button);
                self.write_mouse(MouseAction::Motion(button), event.position, event.modifiers);
            }
            return;
        }

        if self.selection_dragging
            && event.pressed_button == Some(MouseButton::Left)
            && let Some((col, row)) = self.mouse_cell(event.position)
        {
            let end = SelectionPoint { row, col };
            let start = self.selection_anchor.unwrap_or(end);
            self.selection_moved = true;
            self.set_selection(start, end, event.modifiers.alt);
        }
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollEvent,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) {
        let mode = self.state.borrow().mode;
        let (delta_x, delta_y) = match event.delta {
            ScrollDelta::Pixels(delta) => (f32::from(delta.x), f32::from(delta.y)),
            ScrollDelta::Lines(delta) => (delta.x, delta.y),
        };
        if delta_x.abs() < f32::EPSILON && delta_y.abs() < f32::EPSILON {
            return;
        }

        let steps = match event.delta {
            ScrollDelta::Pixels(_) => (delta_y.abs() / f32::from(self.metrics.line_height))
                .ceil()
                .max(1.0) as usize,
            ScrollDelta::Lines(_) => delta_y.abs().ceil().max(1.0) as usize,
        }
        .min(10);

        // Shift+rueda es scroll local aunque la aplicación tenga tracking.
        if mode.mouse_mode != MouseMode::Off && !event.modifiers.shift {
            let action = wheel_action(delta_x, delta_y);
            for _ in 0..steps {
                self.write_mouse(action, event.position, event.modifiers);
            }
            return;
        }

        // Sin tracking, la rueda horizontal no tiene destino local.
        if delta_y.abs() < f32::EPSILON {
            return;
        }

        if mode.alternate_scroll && mode.alt_screen {
            let sequence: &[u8] = if delta_y < 0.0 {
                if mode.app_cursor {
                    b"\x1bOB"
                } else {
                    b"\x1b[B"
                }
            } else if mode.app_cursor {
                b"\x1bOA"
            } else {
                b"\x1b[A"
            };
            for _ in 0..steps {
                self.write(sequence);
            }
            return;
        }

        let delta = if delta_y > 0.0 {
            steps as i32
        } else {
            -(steps as i32)
        };
        if let Some(parser) = &self.parser {
            let _ = parser.scroll(delta);
        }
    }

    /// Aplica el tamaño que ofrece el elemento de render. Solo el parser y el
    /// PTY cambian de tamaño; el estado sigue pintando el frame anterior hasta
    /// que llega el damage completo.
    fn observe_bounds(&mut self, origin: Point<Pixels>, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        self.geometry = Some(GridGeometry {
            origin,
            cell_width: self.metrics.cell_width,
            line_height: self.metrics.line_height,
        });
        if self.requested_grid == Some((cols, rows)) {
            return;
        }
        let Some(parser) = &self.parser else {
            return;
        };

        let request = ResizeRequest {
            cols,
            rows,
            cell_width: pixels_to_u16(self.metrics.cell_width),
            cell_height: pixels_to_u16(self.metrics.line_height),
        };
        if !parser.resize(request) {
            // Cola llena: el siguiente frame volverá a intentarlo.
            return;
        }
        self.requested_grid = Some((cols, rows));

        if let Some(session) = &self.session {
            session.resize(crate::PtySizeSpec::new(
                cols,
                rows,
                request.cell_width,
                request.cell_height,
            ));
        }
    }

    fn link_at(&self, row: usize, column: usize) -> Option<String> {
        let state = self.state.borrow();
        state
            .links
            .get(row)?
            .iter()
            .find(|link| column >= link.start && column < link.end)
            .map(|link| link.uri.clone())
    }

    fn url_at(&self, row: usize, column: usize) -> Option<String> {
        if let Some(uri) = self.link_at(row, column)
            && is_web_url(&uri)
        {
            return Some(uri);
        }

        let state = self.state.borrow();
        let cells = state.cells.get(row)?;
        let mut text = String::with_capacity(cells.len());
        let mut columns = Vec::with_capacity(cells.len());
        for (index, cell) in cells.iter().enumerate() {
            if cell.attrs.wide_spacer {
                continue;
            }
            let ch = if cell.attrs.hidden { ' ' } else { cell.ch };
            text.push(ch);
            columns.push(index);
            for (combining_column, combining) in &state.combining[row] {
                if *combining_column == index {
                    text.push(*combining);
                    columns.push(index);
                }
            }
        }

        for candidate in find_urls(&text) {
            let start = columns.get(candidate.start)?;
            let end = columns.get(candidate.end.saturating_sub(1))?;
            if column >= *start && column <= *end {
                return Some(candidate.text);
            }
        }
        None
    }

    fn confirm_url(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<'_, Self>) {
        cx.stop_propagation();
        if let Some(url) = self.pending_url.take() {
            cx.open_url(&url);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn cancel_url(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<'_, Self>) {
        cx.stop_propagation();
        self.pending_url = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn dismiss_notice(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        cx.stop_propagation();
        self.notice = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }
}
