impl TerminalView {
    /// Arranca el PTY y el parser. Si falla, la vista muestra el error en vez
    /// de tumbar la ventana.
    pub fn new(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        style: TerminalStyle,
        cwd: PathBuf,
        spec: &crate::SpawnSpec,
        restore: Option<crate::TerminalSnapshot>,
        focus: bool,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        if focus {
            window.focus(&focus_handle, cx);
        }
        let metrics = Metrics::new(window, &style);
        let state = Rc::new(RefCell::new(RenderState::new()));
        let debug = DebugRender::from_env();

        match Self::start(&metrics, &state, cwd, spec, restore, cx) {
            Ok((session, parser, pump)) => {
                let writer = session.writer();
                let pid = session.pid();
                let (focus_in, focus_out) =
                    register_focus_listeners(window, cx, &focus_handle, &state, writer);
                Self {
                    focus: focus_handle,
                    session: Some(session),
                    parser: Some(parser),
                    state,
                    metrics,
                    style,
                    geometry: None,
                    requested_grid: None,
                    error: None,
                    notice: None,
                    pending_url: None,
                    title: None,
                    cwd: None,
                    pid,
                    signals: Vec::new(),
                    debug,
                    ime: ImeState::default(),
                    selection_anchor: None,
                    selection_dragging: false,
                    selection_moved: false,
                    _focus_in: focus_in,
                    _focus_out: focus_out,
                    _pump: Some(pump),
                }
            }
            Err(error) => Self {
                focus: focus_handle,
                session: None,
                parser: None,
                state,
                metrics,
                style,
                geometry: None,
                requested_grid: None,
                error: Some(error.to_string()),
                notice: None,
                pending_url: None,
                title: None,
                cwd: None,
                pid: None,
                signals: Vec::new(),
                debug,
                ime: ImeState::default(),
                selection_anchor: None,
                selection_dragging: false,
                selection_moved: false,
                _focus_in: None,
                _focus_out: None,
                _pump: None,
            },
        }
    }

    /// Título anunciado por la shell (`OSC 0/2`), si lo hay.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Error de arranque o del parser, para que la app ofrezca acciones.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Programa que arrancó el PTY (`pwsh`, `cmd`, etc.), si hay sesión.
    pub fn program(&self) -> Option<&str> {
        self.session.as_ref().map(TerminalSession::program)
    }

    /// Aplica un tema, fuente o escala nuevos sin recrear el PTY.
    pub fn update_style(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        style: TerminalStyle,
    ) {
        let same = self.style.font_family == style.font_family
            && self.style.font_size == style.font_size
            && self.style.fallbacks == style.fallbacks
            && self.style.line_height == style.line_height
            && self.style.palette == style.palette
            && self.style.background == style.background
            && self.style.border == style.border
            && self.style.padding == style.padding;
        if same {
            return;
        }
        self.metrics = Metrics::new(window, &style);
        self.style = style;
        self.state.borrow_mut().invalidate_caches();
        self.geometry = None;
        self.requested_grid = None;
        cx.notify();
    }

    /// Último cwd anunciado por la shell (`OSC 7`).
    pub fn cwd(&self) -> Option<&PathBuf> {
        self.cwd.as_ref()
    }

    /// `true` mientras el proceso del PTY sigue vivo.
    pub fn is_running(&self) -> bool {
        self.session.is_some() && !self.state.borrow().ended
    }

    /// PID raíz del PTY, si el sistema lo expuso.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Instante del último bloque de salida recibido.
    pub fn last_output_at(&self) -> Option<Instant> {
        self.state.borrow().last_output
    }

    /// Estado de salida, si el proceso ya terminó.
    pub fn exit_info(&self) -> Option<ExitInfo> {
        self.state.borrow().exited
    }

    /// Retira y devuelve las señales pendientes para los agentes.
    pub fn take_signals(&mut self) -> Vec<TerminalSignal> {
        std::mem::take(&mut self.signals)
    }

    /// Código de salida, si el proceso ya terminó.
    pub fn exit_code(&self) -> Option<u32> {
        self.state.borrow().exited.and_then(|info| info.code)
    }

    /// Envía una línea de comando al PTY terminado en retorno de carro.
    pub fn send_command(&self, command: &str) {
        let command = command.trim();
        if command.is_empty() || !self.is_running() {
            return;
        }
        let mut bytes = Vec::with_capacity(command.len() + 1);
        bytes.extend_from_slice(command.as_bytes());
        bytes.push(b'\r');
        self.write(&bytes);
    }

    /// Lleva el foco de teclado a la terminal.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.focus.focus(window, cx);
    }

    /// Pide una instantánea del buffer para guardarla al cerrar.
    pub fn request_snapshot(&self) -> Option<SnapshotRequest> {
        self.parser.as_ref().and_then(ParserHandle::snapshot)
    }

    fn start(
        metrics: &Metrics,
        state: &Rc<RefCell<RenderState>>,
        cwd: PathBuf,
        spec: &crate::SpawnSpec,
        restore: Option<crate::TerminalSnapshot>,
        cx: &mut Context<'_, Self>,
    ) -> Result<(TerminalSession, ParserHandle, Task<()>), TerminalError> {
        let (commands, inbox) = async_channel::bounded(64);
        let initial = GridSize::new(80, 24);
        let cell_width = pixels_to_u16(metrics.cell_width);
        let cell_height = pixels_to_u16(metrics.line_height);

        let session = TerminalSession::spawn(
            &cwd,
            crate::PtySizeSpec::new(80, 24, cell_width, cell_height),
            commands.clone(),
            spec,
        )?;
        let writer = session
            .writer()
            .ok_or_else(|| TerminalError::Pty("el PTY no expuso un escritor".into()))?;
        let (frames, frame_inbox) = async_channel::bounded(2);
        let parser = crate::engine::spawn_parser(
            commands,
            inbox,
            frames,
            crate::engine::ParserSetup {
                writer,
                palette: metrics.palette,
                initial,
                cell_width,
                cell_height,
                restore,
            },
        )?;

        let state = Rc::clone(state);
        let pump = cx.spawn(async move |this, cx| {
            while let Ok(mut frame) = frame_inbox.recv().await {
                let events = std::mem::take(&mut frame.events);
                let bytes = frame.bytes;
                {
                    let mut state = state.borrow_mut();
                    if bytes > 0 {
                        state.last_output = Some(Instant::now());
                    }
                    state.apply(frame);
                }
                let updated = this.update_in(cx, |view, window, cx| {
                    view.handle_events(events, window, cx);
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        });

        Ok((session, parser, pump))
    }

    fn write(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(writer) = self.session.as_ref().and_then(TerminalSession::writer) {
            let _ = writer.write(bytes);
        }
    }

    fn scroll_to_bottom(&self) {
        if let Some(parser) = &self.parser {
            parser.scroll_to_bottom();
        }
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_dragging = false;
        self.selection_moved = false;
        if self.state.borrow().selection.is_some()
            && let Some(parser) = &self.parser
        {
            parser.clear_selection();
        }
    }

    fn handle_events(
        &mut self,
        events: Vec<TerminalEvent>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        for event in events {
            match event {
                TerminalEvent::Title(title) => {
                    // El título es propiedad de la pestaña; la app decide el
                    // título de la ventana a partir de la pestaña enfocada.
                    self.title = title.filter(|title| !title.is_empty());
                }
                TerminalEvent::Cwd(path) => self.cwd = Some(path),
                TerminalEvent::Bell => {
                    self.signals.push(TerminalSignal::Attention(None));
                    self.state.borrow_mut().bell_flash = true;
                    self.schedule_bell_clear(cx);
                }
                TerminalEvent::Attention(message) => {
                    self.signals.push(TerminalSignal::Attention(message));
                }
                TerminalEvent::ProgramStatus(status) => {
                    self.signals.push(TerminalSignal::ProgramStatus(status));
                }
                TerminalEvent::MouseCursor(shape) => {
                    window.set_window_cursor_style(cursor_style_from_osc(&shape));
                }
                TerminalEvent::Clipboard(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
                TerminalEvent::Exited(info) => {
                    self.signals.push(TerminalSignal::Exited(info));
                    let mut state = self.state.borrow_mut();
                    state.ended = true;
                    // EOF puede llegar después del código real; no borrarlo.
                    if info.is_some() || state.exited.is_none() {
                        state.exited = info;
                    }
                }
                TerminalEvent::Error(message) => self.error = Some(message),
            }
        }
    }

    fn schedule_bell_clear(&self, cx: &mut Context<'_, Self>) {
        let timer = cx.background_executor().timer(Duration::from_millis(140));
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |view, cx| {
                view.state.borrow_mut().bell_flash = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if event.keystroke.modifiers.platform {
            return;
        }

        // Diagnóstico: F12 vuelca el grid cuando `AXFTERM_TRACE_CELLS` está
        // activo. No se reenvía al PTY para no alterar la TUI observada.
        if self.debug.trace_cells
            && event.keystroke.key == "f12"
            && !event.keystroke.modifiers.modified()
        {
            if let Some(parser) = &self.parser {
                parser.dump();
            }
            return;
        }

        // Fallback si el host no registró los keybindings de copiar/pegar.
        // En la app, la acción se resuelve antes y este bloque no se ejecuta.
        if event.keystroke.modifiers.control && event.keystroke.modifiers.shift {
            match event.keystroke.key.as_str() {
                "v" => {
                    self.paste_from_clipboard(cx);
                    return;
                }
                "c" => {
                    if let Some(parser) = &self.parser {
                        parser.copy_selection();
                    }
                    return;
                }
                _ => {}
            }
        }

        // Los caracteres imprimibles llegan por el manejador de texto (IME):
        // si además los escribiéramos aquí, cada tecla se duplicaría. Las
        // teclas de control y las secuencias van por `input::encode`.
        if is_text_input(&event.keystroke) {
            return;
        }

        let mode = self.state.borrow().mode;
        let bytes = input::encode(&event.keystroke, event.prefer_character_input, mode);
        if bytes.is_empty() {
            return;
        }
        cx.stop_propagation();
        self.scroll_to_bottom();
        self.clear_selection();
        self.write(&bytes);
        cx.notify();
    }

    fn copy_action(&mut self, _: &CopySelection, _: &mut Window, _: &mut Context<'_, Self>) {
        if let Some(parser) = &self.parser {
            parser.copy_selection();
        }
    }

    fn paste_action(&mut self, _: &PasteSelection, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.paste_from_clipboard(cx);
    }

    fn tab_action(&mut self, _: &SendTab, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.scroll_to_bottom();
        self.clear_selection();
        self.write(b"\t");
        cx.notify();
    }

    fn backtab_action(&mut self, _: &SendBacktab, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.scroll_to_bottom();
        self.clear_selection();
        self.write(b"\x1b[Z");
        cx.notify();
    }

    fn paste_from_clipboard(&mut self, cx: &mut Context<'_, Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let Some(text) = item.text() else {
            return;
        };
        self.paste_text(&text, cx);
    }

    fn paste_text(&mut self, text: &str, cx: &mut Context<'_, Self>) {
        if text.len() > MAX_PASTE_BYTES {
            self.notice = Some(format!(
                "Pegado rechazado: {} bytes (máximo 1 MiB). Usa un archivo.",
                text.len()
            ));
            cx.notify();
            return;
        }
        let bracketed = self.state.borrow().mode.bracketed_paste;
        let bytes = normalize_paste(text, bracketed);
        self.scroll_to_bottom();
        self.clear_selection();
        self.write(&bytes);
        cx.notify();
    }
}
