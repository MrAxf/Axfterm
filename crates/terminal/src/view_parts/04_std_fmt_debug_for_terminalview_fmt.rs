impl std::fmt::Debug for TerminalView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalView")
            .field("error", &self.error)
            .field("requested_grid", &self.requested_grid)
            .field("cwd", &self.cwd)
            .finish_non_exhaustive()
    }
}
impl Drop for TerminalView {
    fn drop(&mut self) {
        if let Some(mut parser) = self.parser.take() {
            parser.close();
        }
        if let Some(mut session) = self.session.take() {
            session.close();
        }
    }
}
fn register_focus_listeners(
    window: &mut Window,
    cx: &mut Context<'_, TerminalView>,
    focus: &FocusHandle,
    state: &Rc<RefCell<RenderState>>,
    writer: Option<PtyWriter>,
) -> (Option<Subscription>, Option<Subscription>) {
    let Some(writer) = writer else {
        return (None, None);
    };

    let focus_in_state = Rc::clone(state);
    let focus_in_writer = writer.clone();
    let focus_in = window.on_focus_in(focus, cx, move |_, _| {
        if focus_in_state.borrow().mode.focus_in_out {
            let _ = focus_in_writer.write(b"\x1b[I");
        }
    });

    let focus_out_state = Rc::clone(state);
    let focus_out_writer = writer;
    let focus_out = window.on_focus_out(focus, cx, move |_, _, _| {
        if focus_out_state.borrow().mode.focus_in_out {
            let _ = focus_out_writer.write(b"\x1b[O");
        }
    });

    (Some(focus_in), Some(focus_out))
}
impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = self.metrics.palette;
        let background = self.style.background;
        let border = self.style.border;
        let padding = self.style.padding;
        let has_session = self.session.is_some();
        let error = self.error.clone();
        let notice = self.notice.clone();
        let pending_url = self.pending_url.clone();
        let view = cx.entity();
        let preedit = self.ime.preedit().to_owned();
        let (ended, exit_info, bell_flash) = {
            let state = self.state.borrow();
            (state.ended, state.exited, state.bell_flash)
        };

        let body = match error {
            Some(error) => div()
                .flex_1()
                .p_4()
                .text_color(palette.foreground)
                .child(error)
                .into_any_element(),
            None => div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(TerminalElement {
                    view: view.clone(),
                    state: Rc::clone(&self.state),
                    metrics: self.metrics.clone(),
                    debug: self.debug,
                    preedit,
                })
                .into_any_element(),
        };

        let border_color = if bell_flash {
            palette.cursor
        } else {
            border.unwrap_or(background)
        };

        let exit_overlay = ended.then(|| {
            let accent = match exit_info {
                Some(info) if info.success => palette.ansi(2),
                Some(_) => palette.ansi(1),
                None => palette.ansi(8),
            };
            let label = exit_info
                .map(ExitInfo::label)
                .unwrap_or_else(|| "proceso terminado".into());
            div()
                .absolute()
                .bottom_2()
                .left_2()
                .right_2()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .border_1()
                .border_color(accent)
                .bg(background.opacity(0.94))
                .text_color(palette.foreground)
                .child(label)
        });

        let url_overlay = pending_url.map(|url| {
            div()
                .absolute()
                .bottom_2()
                .left_2()
                .right_2()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .border_1()
                .border_color(palette.cursor)
                .bg(background.opacity(0.96))
                .text_color(palette.foreground)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().flex_1().overflow_hidden().child(url))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .bg(palette.ansi(2))
                        .text_color(background)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::confirm_url))
                        .child("Abrir"),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .bg(palette.ansi(8))
                        .text_color(palette.foreground)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::cancel_url))
                        .child("Cancelar"),
                )
        });

        let notice_overlay = notice.map(|notice| {
            div()
                .absolute()
                .top_2()
                .left_2()
                .right_2()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .border_1()
                .border_color(palette.ansi(3))
                .bg(background.opacity(0.96))
                .text_color(palette.foreground)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().flex_1().overflow_hidden().child(notice))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .bg(palette.ansi(8))
                        .text_color(palette.foreground)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::dismiss_notice))
                        .child("Cerrar"),
                )
        });

        div()
            .flex()
            .flex_col()
            .relative()
            .size_full()
            .bg(background)
            .text_color(palette.foreground)
            .p(padding)
            .border_1()
            .border_color(border_color)
            .key_context(TERMINAL_CONTEXT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_drop::<ExternalPaths>(cx.listener(Self::on_file_drop))
            .on_action(cx.listener(Self::copy_action))
            .on_action(cx.listener(Self::paste_action))
            .on_action(cx.listener(Self::tab_action))
            .on_action(cx.listener(Self::backtab_action))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll(cx.listener(Self::on_scroll_wheel))
            .children(has_session.then(|| ImeAnchor {
                focus: self.focus.clone(),
                view,
            }))
            .child(body)
            .children(exit_overlay)
            .children(url_overlay)
            .children(notice_overlay)
            .context_menu(|menu, _, _| {
                menu.menu("Copiar", Box::new(CopySelection))
                    .separator()
                    .menu("Pegar", Box::new(PasteSelection))
            })
    }
}
impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<String> {
        self.ime.text_for_range(range)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<UTF16Selection> {
        self.ime.marked_range().map(|range| UTF16Selection {
            range,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Range<usize>> {
        self.ime.marked_range()
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        self.ime.clear();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let bytes = self.ime.commit(text);
        if !bytes.is_empty() {
            self.scroll_to_bottom();
            self.clear_selection();
            self.write(&bytes);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.ime.set_preedit(new_text);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Bounds<Pixels>> {
        let Some(geometry) = self.geometry else {
            return Some(element_bounds);
        };
        let state = self.state.borrow();
        let cursor = state.cursor.or_else(|| {
            state.rows.checked_sub(1).map(|line| CursorSnapshot {
                line,
                column: 0,
                shape: CursorShape::Block,
            })
        })?;
        let origin = point(
            geometry.origin.x + geometry.cell_width * cursor.column,
            geometry.origin.y + geometry.line_height * cursor.line,
        );
        Some(Bounds::new(
            origin,
            size(geometry.cell_width, geometry.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        None
    }

    fn paste(&mut self, item: ClipboardItem, _window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(text) = item.text() else {
            return;
        };
        self.paste_text(&text, cx);
    }
}
struct ImeAnchor {
    focus: FocusHandle,
    view: Entity<TerminalView>,
}
impl Element for ImeAnchor {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
        let layout = window.request_layout(Style::default(), std::iter::empty(), cx);
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.handle_input(
            &self.focus,
            ElementInputHandler::new(window.bounds(), self.view.clone()),
            cx,
        );
    }
}
impl IntoElement for ImeAnchor {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
struct TerminalElement {
    view: Entity<TerminalView>,
    state: Rc<RefCell<RenderState>>,
    metrics: Metrics,
    debug: DebugRender,
    preedit: String,
}
struct PrepaintState {
    lines: Vec<(usize, ShapedLine)>,
    preedit: Option<(ShapedLine, Point<Pixels>)>,
    rows: usize,
    cols: usize,
    origin: Point<Pixels>,
}
