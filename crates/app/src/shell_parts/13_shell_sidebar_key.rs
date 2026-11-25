impl Shell {
    fn sidebar_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let rows = self.sidebar_rows();
        if rows.is_empty() {
            return;
        }
        let current = self
            .sidebar_cursor
            .and_then(|cursor| rows.iter().position(|row| *row == cursor))
            .unwrap_or(0);
        let mut handled = true;
        match event.keystroke.key.as_str() {
            "down" => {
                let next = (current + 1).min(rows.len() - 1);
                self.sidebar_cursor = Some(rows[next]);
            }
            "up" => {
                let next = current.saturating_sub(1);
                self.sidebar_cursor = Some(rows[next]);
            }
            "home" => self.sidebar_cursor = Some(rows[0]),
            "end" => self.sidebar_cursor = Some(rows[rows.len() - 1]),
            "right" => match rows[current] {
                SidebarRow::Project(project) => {
                    let expanded = self.workspace.expanded_projects().contains(&project);
                    if self.sidebar_section == SidebarSection::All && !expanded {
                        let _ = self.workspace.set_project_expanded(project, true);
                        self.persist(cx);
                    } else if current + 1 < rows.len() {
                        self.sidebar_cursor = Some(rows[current + 1]);
                    }
                }
                SidebarRow::Tab(_) => {}
            },
            "left" => match rows[current] {
                SidebarRow::Project(project) => {
                    if self.sidebar_section == SidebarSection::All {
                        let _ = self.workspace.set_project_expanded(project, false);
                        self.persist(cx);
                    }
                }
                SidebarRow::Tab(tab) => {
                    if let Some(project) = self.workspace.tab(tab).map(|tab| tab.project_id)
                        && let Some(index) = rows
                            .iter()
                            .position(|row| *row == SidebarRow::Project(project))
                    {
                        self.sidebar_cursor = Some(rows[index]);
                    }
                }
            },
            "enter" => match rows[current] {
                SidebarRow::Project(project) => {
                    self.activate_project_row(project, window, cx);
                }
                SidebarRow::Tab(tab) => {
                    self.activate_sidebar_tab(tab, window, cx);
                }
            },
            "escape" => {
                window.focus(&self.focus, cx);
                if let Some(tab) = self.workspace.focus().tab
                    && let Some(entry) = self
                        .workspace
                        .tab(tab)
                        .and_then(|tab| self.terminals.get(&tab.terminal_id))
                {
                    entry.view.update(cx, |view, cx| view.focus(window, cx));
                }
            }
            _ => handled = false,
        }
        if handled {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn render_pane(&mut self, pane_id: PaneId, cx: &mut Context<'_, Self>) -> AnyElement {
        let Some(pane) = self.workspace.pane(pane_id).cloned() else {
            return div().into_any_element();
        };
        let focused = self.workspace.focus().pane == pane_id;
        let (border, background) = {
            let theme = cx.theme();
            (theme.border, theme.background)
        };
        let strip = self.render_tab_strip(pane_id, &pane, focused, cx);
        let body = match pane.active {
            Some(tab) => self.render_tab_content(tab, cx),
            None => self.render_empty_pane(pane_id, cx),
        };
        let metrics = theme::metrics(cx);
        let accent = theme::state(cx).theme.ui.accent;
        let drop_preview = self.tab_drop.filter(|target| target.pane == pane_id);
        let preview = drop_preview.map(|target| {
            let overlay = div()
                .absolute()
                .bg(accent.opacity(0.14))
                .border_1()
                .border_color(accent)
                .rounded(metrics.radius_sm())
                .on_drop::<TabDrag>(cx.listener(move |shell, drag: &TabDrag, window, cx| {
                    shell.drop_tab_target(drag, target, window, cx);
                }));
            match target.zone {
                TabDropZone::Center => overlay
                    .top(metrics.spacing_xs())
                    .left(metrics.spacing_xs())
                    .right(metrics.spacing_xs())
                    .bottom(metrics.spacing_xs()),
                TabDropZone::Left => overlay.left_0().top_0().bottom_0().w(relative(0.5)),
                TabDropZone::Right => overlay.right_0().top_0().bottom_0().w(relative(0.5)),
                TabDropZone::Top => overlay.top_0().left_0().right_0().h(relative(0.5)),
                TabDropZone::Bottom => overlay.bottom_0().left_0().right_0().h(relative(0.5)),
            }
            .into_any_element()
        });
        let body_bounds = Rc::clone(&self.body_bounds);
        div()
            .id(("pane", pane_id.get()))
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(background)
            .border_1()
            // Se conserva el grosor para no alterar el layout ni el reflow del
            // PTY al cambiar el foco; solo cambia el color.
            .border_color(if focused { background } else { border })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _, _, cx| {
                    if shell.workspace.focus().pane != pane_id {
                        let _ = shell.workspace.focus_pane(pane_id);
                        cx.notify();
                    }
                }),
            )
            .child(strip)
            .child(
                div()
                    .id(("pane-body", pane_id.get()))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .on_prepaint(move |bounds, _, _| {
                        body_bounds.borrow_mut().insert(pane_id, bounds);
                    })
                    .child(body)
                    .children(preview)
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.focus_pane_click(pane_id, window, cx);
                    }))
                    .on_drop::<TabDrag>(cx.listener(move |shell, drag: &TabDrag, window, cx| {
                        let target = shell.tab_drop.filter(|target| target.pane == pane_id);
                        if let Some(target) = target {
                            shell.drop_tab_target(drag, target, window, cx);
                        } else {
                            shell.drop_tab(drag, pane_id, usize::MAX, window, cx);
                        }
                    })),
            )
            .into_any_element()
    }

    fn focus_pane_click(&mut self, pane: PaneId, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Err(error) = self.workspace.focus_pane(pane) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        self.reveal_focus();
        if let Some(tab) = self.workspace.focus().tab {
            let _ = self
                .workspace
                .acknowledge_tab_agents(tab, SystemTime::now());
            self.ensure_terminal(tab, cx);
            if let Some(entry) = self
                .workspace
                .tab(tab)
                .and_then(|tab| self.terminals.get(&tab.terminal_id))
            {
                entry.view.update(cx, |view, cx| view.focus(window, cx));
            } else {
                window.focus(&self.focus, cx);
            }
            self.pending_focus = Some(tab);
        }
        self.persist(cx);
        cx.notify();
    }

    fn duplicate_tab_with_window(
        &mut self,
        tab: TabId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let target = self.terminal_target(tab, cx);
        match self.workspace.duplicate_tab(tab) {
            Ok(new_tab) => {
                self.spawn_terminal_with_target(
                    new_tab.tab,
                    new_tab.terminal,
                    target,
                    SpawnOptions {
                        restore: None,
                        focus: true,
                    },
                    window,
                    cx,
                );
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }
}
