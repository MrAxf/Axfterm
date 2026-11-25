impl Shell {
    fn render_tab(
        &mut self,
        pane_id: PaneId,
        tab: TabId,
        index: usize,
        focused: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let active = self.workspace.pane(pane_id).and_then(|pane| pane.active) == Some(tab);
        let title = self.tab_title(tab, cx);
        let editing = self.editing.as_ref().is_some_and(|edit| edit.tab == tab);
        let aggregate = self
            .workspace
            .tab(tab)
            .and_then(|tab| tab.agent_aggregate());
        let status = aggregate
            .as_ref()
            .map(|aggregate| tab_status_from_state(aggregate.state));
        let icon = self.tab_icon(tab);
        let menu_this = cx.entity().downgrade();
        let drag_title = SharedString::from(title.clone());
        let insert = self
            .tab_insert
            .filter(|(pane, _)| *pane == pane_id)
            .map(|(_, index)| index);
        let tab_count = self
            .workspace
            .pane(pane_id)
            .map_or(0, |pane| pane.tabs.len());

        // El indicador vive dentro del chip: en su borde izquierdo o, para el
        // final de la lista, en el derecho del último. Así no aporta tamaño al
        // contenido scrollable y no inventa overflow.
        let marker_left = insert == Some(index);
        let marker_right = insert == Some(tab_count) && index + 1 == tab_count;
        let inner: AnyElement = if editing {
            if let Some(edit) = &self.editing
                && edit.tab == tab
            {
                div()
                    .flex()
                    .items_center()
                    .h_full()
                    .w_full()
                    .min_w_0()
                    .px(metrics.spacing_sm())
                    .rounded(metrics.radius_sm())
                    .border_1()
                    .border_color(ui.focus)
                    .bg(ui.surface_raised)
                    .text_color(ui.text)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(|shell, event: &KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" {
                            shell.cancel_rename(cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        TextField::new(edit.input.clone())
                            .size(ControlSize::Xs)
                            .borderless(),
                    )
                    .into_any_element()
            } else {
                div().child(title.clone()).into_any_element()
            }
        } else {
            let close_this = cx.entity().downgrade();
            TabView::new(format!("tab-{}", tab.get()), title.clone())
                .icon_source(icon.clone())
                .full_width()
                .active(active)
                .focused(focused && active)
                .status(status)
                .on_close(move |_, _, cx| {
                    if let Some(this) = close_this.upgrade() {
                        let _ = this.update(cx, |shell, cx| shell.close_tab_id(tab, cx));
                    }
                })
                .into_any_element()
        };
        div()
            .id(("tab", tab.get()))
            .relative()
            .flex()
            .items_center()
            .h_full()
            .flex_basis(metrics.tab_width())
            .min_w(metrics.tab_min_width())
            .flex_grow_0()
            .flex_shrink_1()
            .overflow_hidden()
            .cursor_pointer()
            .on_click(cx.listener(move |shell, event: &ClickEvent, window, cx| {
                if editing {
                    cx.stop_propagation();
                    return;
                }
                if event.click_count() == 2 {
                    shell.begin_rename(tab, window, cx);
                } else {
                    shell.focus_tab(tab, window, cx);
                }
            }))
            .on_drag(
                TabDrag {
                    tab,
                    title: drag_title,
                    icon,
                    status,
                },
                |drag, _, _, cx| {
                    cx.new(|_| DragGhost::tab(drag.title.clone(), drag.icon.clone(), drag.status))
                },
            )
            .on_drag_move::<axf_ui::gpui::ExternalPaths>(cx.listener(
                move |shell,
                      event: &axf_ui::gpui::DragMoveEvent<axf_ui::gpui::ExternalPaths>,
                      window,
                      cx| {
                    let viewport = shell.pane_scroll.get(&pane_id).map(|scroll| scroll.bounds());
                    let over = !shell.file_drag_blocked()
                        && file_drag_over(event.bounds, event.event.position, viewport);
                    shell.observe_file_drag(FileDragHover::Tab(tab), over, window, cx);
                },
            ))
            .on_drop::<TabDrag>(cx.listener(move |shell, drag: &TabDrag, window, cx| {
                // El indicador de inserción manda: así el drop coincide con la
                // barra que el usuario ve, incluso dentro del mismo chip.
                let insert = shell
                    .tab_insert
                    .filter(|(pane, _)| *pane == pane_id)
                    .map(|(_, index)| index)
                    .unwrap_or(index);
                shell.drop_tab(drag, pane_id, insert, window, cx);
            }))
            .context_menu(move |menu, _, _| {
                let this = menu_this.clone();
                menu.item(PopupMenuItem::new("Renombrar").on_click({
                    let this = this.clone();
                    move |_, window, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                shell.begin_rename(tab, window, cx);
                            });
                        }
                    }
                }))
                .item(PopupMenuItem::new("Duplicar").on_click({
                    let this = this.clone();
                    move |_, window, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                shell.duplicate_tab_with_window(tab, window, cx);
                            });
                        }
                    }
                }))
                .item(PopupMenuItem::new("Mover al panel siguiente").on_click({
                    let this = this.clone();
                    move |_, window, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                shell.focus_tab(tab, window, cx);
                                shell.move_focused_tab_next_pane(window, cx);
                            });
                        }
                    }
                }))
                .separator()
                .item(PopupMenuItem::new("Cerrar pestaña").on_click({
                    let this = this.clone();
                    move |_, _, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| shell.close_tab_id(tab, cx));
                        }
                    }
                }))
            })
            .child(inner)
            .when(marker_left || marker_right, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(metrics.spacing_xxs())
                        .bottom(metrics.spacing_xxs())
                        .w(metrics.tab_insert_width())
                        .rounded(metrics.hairline())
                        .bg(ui.accent)
                        .when(marker_left, |this| this.left_0())
                        .when(marker_right, |this| this.right_0()),
                )
            })
            .into_any_element()
    }

    fn scroll_tabs(&mut self, pane: PaneId, event: &ScrollEvent, cx: &mut Context<'_, Self>) {
        let Some(scroll) = self.pane_scroll.get(&pane) else {
            return;
        };
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => delta.y,
            ScrollDelta::Lines(delta) => px(delta.y * 24.0),
        };
        let offset = scroll.offset();
        let max = scroll.max_offset().x;
        let x = (offset.x - delta).min(px(0.0)).max(-max);
        if x != offset.x {
            scroll.set_offset(point(x, offset.y));
            cx.notify();
        }
    }

    /// Actualiza el destino visible del drag de una pestaña.
    fn update_tab_drag_target(
        &mut self,
        event: &axf_ui::gpui::DragMoveEvent<TabDrag>,
        cx: &mut Context<'_, Self>,
    ) {
        let position = event.event.position;
        let metrics = theme::metrics(cx);

        // Primero se desplaza la strip y después se calcula el índice: si se
        // hiciera al revés, el destino quedaría un paso por detrás del offset.
        let scrolled = self.auto_scroll_tab_drag(position, metrics);
        let edge = metrics.tab_drop_edge_ratio();
        let drop = {
            let bounds = self.body_bounds.borrow();
            bounds
                .iter()
                .find(|(_, pane_bounds)| bounds_contain(**pane_bounds, position))
                .map(|(pane, pane_bounds)| TabDropTarget {
                    pane: *pane,
                    zone: tab_drop_zone(position, *pane_bounds, edge),
                })
        };
        let insert = if drop.is_none() {
            self.tab_insert_at(position, metrics)
        } else {
            None
        };
        if scrolled || drop != self.tab_drop || insert != self.tab_insert {
            self.tab_drop = drop;
            self.tab_insert = insert;
            cx.notify();
        }
    }

    /// Índice de inserción dentro de la strip bajo el puntero.
    fn tab_insert_at(
        &self,
        position: Point<Pixels>,
        metrics: UiMetrics,
    ) -> Option<(PaneId, usize)> {
        let (pane, bounds, offset) = self
            .pane_scroll
            .iter()
            .find(|(_, scroll)| bounds_contain(scroll.bounds(), position))
            .map(|(pane, scroll)| (*pane, scroll.bounds(), scroll.offset()))?;
        let tab_count = self.workspace.pane(pane)?.tabs.len();
        let width = tab_slot_width(
            bounds.size.width,
            tab_count,
            metrics.tab_min_width(),
            metrics.tab_width(),
        );

        // `scroll.bounds()` es el viewport estable; el offset lo desplaza el
        // contenido, no el contenedor.
        let local_x = f32::from(position.x - bounds.left() - offset.x);
        let index = insert_index_at(local_x, width, tab_count);
        Some((pane, index))
    }

    /// Desplaza una sola vez la strip bajo el puntero cuando el drag está en un borde.
    ///
    /// GPUI entrega `on_drag_move` a todos los elementos que lo registran, no
    /// solo al origen del drag. Por eso este listener existe una única vez: si
    /// cada pestaña lo registrara, la velocidad del scroll crecería con el
    /// número de pestañas. Devuelve si el offset cambió.
    fn auto_scroll_tab_drag(&mut self, position: Point<Pixels>, metrics: UiMetrics) -> bool {
        let Some((pane, bounds, offset)) = self
            .pane_scroll
            .iter()
            .find(|(_, scroll)| bounds_contain(scroll.bounds(), position))
            .map(|(pane, scroll)| (*pane, scroll.bounds(), scroll.offset()))
        else {
            return false;
        };
        let Some(scroll) = self.pane_scroll.get(&pane) else {
            return false;
        };
        let x = tab_autoscroll_offset(
            offset.x,
            scroll.max_offset().x,
            position.x,
            bounds.left(),
            bounds.right(),
            metrics.drag_edge(),
            metrics.drag_nudge(),
        );
        if x == offset.x {
            return false;
        }
        scroll.set_offset(point(x, offset.y));
        true
    }

    fn drop_tab(
        &mut self,
        drag: &TabDrag,
        target: PaneId,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // El índice cacheado puede venir de un movimiento anterior; al soltar
        // manda la posición real del puntero sobre la strip.
        let index = self
            .tab_insert_at(window.mouse_position(), theme::metrics(cx))
            .filter(|(pane, _)| *pane == target)
            .map(|(_, index)| index)
            .unwrap_or(index);

        // `move_tab` quita la pestaña antes de insertarla. Si se mueve dentro
        // del mismo panel y hacia la derecha, el índice visible (el hueco de
        // la barra) pierde una posición al quitar.
        let same_pane_from = self
            .workspace
            .pane_of_tab(drag.tab)
            .filter(|source| *source == target)
            .and_then(|_| {
                self.workspace
                    .pane(target)
                    .and_then(|pane| pane.tabs.iter().position(|tab| *tab == drag.tab))
            });
        let index = adjusted_insert_index(same_pane_from, index);
        if let Err(error) = self.workspace.move_tab(drag.tab, target, index) {
            self.tab_drop = None;
            self.tab_insert = None;
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        self.finish_tab_drop(drag.tab, window, cx);
    }

    /// Suelta una pestaña en un destino con zona: centro mueve, borde divide.
    fn drop_tab_target(
        &mut self,
        drag: &TabDrag,
        target: TabDropTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(axis) = target.zone.axis() else {
            self.drop_tab(drag, target.pane, usize::MAX, window, cx);
            return;
        };

        // Un panel nuevo debe verse: el zoom temporal dejaría oculto el split.
        self.zoomed = None;
        match self
            .workspace
            .split_pane_with_tab(target.pane, axis, target.zone.side(), drag.tab)
        {
            Ok(_) => self.finish_tab_drop(drag.tab, window, cx),
            Err(error) => {
                self.tab_drop = None;
                self.tab_insert = None;
                self.set_notice(error.to_string(), true, cx);
            }
        }
    }

    fn finish_tab_drop(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.tab_drop = None;
        self.tab_insert = None;
        self.pending_focus = Some(tab);
        self.reveal_focus();
        self.ensure_terminal(tab, cx);
        if let Some(entry) = self
            .workspace
            .tab(tab)
            .and_then(|tab| self.terminals.get(&tab.terminal_id))
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        }
        self.persist(cx);
        cx.notify();
    }

    fn move_focused_tab_next_pane(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(tab) = self.workspace.focus().tab else {
            return;
        };
        let leaves = self.workspace.layout().leaves();
        if leaves.len() < 2 {
            return;
        }
        let current = self
            .workspace
            .pane_of_tab(tab)
            .and_then(|pane| leaves.iter().position(|id| *id == pane))
            .unwrap_or(0);
        let target = leaves[(current + 1) % leaves.len()];
        if let Err(error) = self.workspace.move_tab(tab, target, usize::MAX) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        self.pending_focus = Some(tab);
        self.ensure_terminal(tab, cx);
        if let Some(entry) = self
            .workspace
            .tab(tab)
            .and_then(|tab| self.terminals.get(&tab.terminal_id))
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        }
        self.persist(cx);
        cx.notify();
    }

    fn render_empty_pane(&mut self, pane: PaneId, cx: &mut Context<'_, Self>) -> AnyElement {
        let metrics = theme::metrics(cx);
        EmptyState::new(
            icons::TERMINAL,
            "Panel vacío",
            "Abre una terminal o divide el panel para empezar.",
        )
        .action(
            div()
                .flex()
                .gap(metrics.spacing_sm())
                .child(
                    Button::new(format!("empty-new-{}", pane.get()), "Nueva terminal")
                        .small()
                        .primary()
                        .icon(icons::PLUS)
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.new_tab_in_pane(pane, window, cx);
                        })),
                )
                .child(
                    Button::new(format!("empty-split-{}", pane.get()), "Dividir")
                        .small()
                        .icon(icons::SPLIT_HORIZONTAL)
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.split_pane_action(pane, SplitAxis::Horizontal, window, cx);
                        })),
                ),
        )
        .into_any_element()
    }
}
