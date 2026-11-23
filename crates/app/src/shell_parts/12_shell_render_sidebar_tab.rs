impl Shell {
    fn render_sidebar_tab(
        &mut self,
        tab: TabId,
        project: ProjectId,
        cursor: Option<SidebarRow>,
        sidebar_focused: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(model_tab) = self.workspace.tab(tab).cloned() else {
            return div().into_any_element();
        };
        let title = self.tab_title(tab, cx);
        let focused = self.workspace.focus().tab == Some(tab);
        let selected = cursor == Some(SidebarRow::Tab(tab));
        let missing = self
            .workspace
            .project(project)
            .is_some_and(|project| !project.root.is_dir());
        let aggregate = self
            .workspace
            .tab(tab)
            .and_then(|tab| tab.agent_aggregate());
        let agent_status = aggregate
            .as_ref()
            .map(|aggregate| tab_status_from_state(aggregate.state));
        let icon = self.tab_icon(tab);
        let profile = self
            .profile_title(&model_tab.profile_id)
            .unwrap_or_else(|| model_tab.profile_id.to_string());
        let live_cwd = self
            .terminals
            .get(&model_tab.terminal_id)
            .and_then(|entry| entry.view.read(cx).cwd().cloned());
        let project_root = self
            .workspace
            .project(project)
            .map(|project| project.root.clone());
        let context = match (project_root.as_deref(), live_cwd.as_deref()) {
            (Some(root), Some(cwd)) => context_path(root, cwd),
            _ => None,
        };
        let running = self
            .terminals
            .get(&model_tab.terminal_id)
            .is_some_and(|entry| entry.view.read(cx).is_running());
        let pending = self.pending_terminals.contains(&model_tab.terminal_id);
        let spawn_error = self.spawn_errors.contains_key(&model_tab.terminal_id);
        let state_label = if pending {
            "arrancando"
        } else if running {
            "en ejecución"
        } else {
            "detenido"
        };
        let mut meta_parts = Vec::new();
        if spawn_error {
            if profile != title {
                meta_parts.push(profile.clone());
            }
            meta_parts.push("error de arranque".to_owned());
        } else if let Some(aggregate) = &aggregate {
            let session = &aggregate.dominant;
            let status = tab_status_from_state(session.state);
            meta_parts.push(session.kind.label().to_owned());
            if let Some(context) = context {
                meta_parts.push(context);
            }
            if let Some(progress) = session.progress {
                meta_parts.push(format!("{progress}%"));
            }
            if let Some(message) = &session.message {
                meta_parts.push(message.clone());
            } else {
                meta_parts.push(status.label().to_ascii_lowercase());
            }
        } else {
            if profile != title {
                meta_parts.push(profile.clone());
            }
            if let Some(context) = context {
                meta_parts.push(context);
            } else {
                meta_parts.push(state_label.to_owned());
            }
        }
        let meta = meta_parts.join(" · ");
        let status = if spawn_error {
            Some((
                icons::ALERT_TRIANGLE,
                ui.danger,
                "Error de arranque".to_owned(),
            ))
        } else if aggregate.is_some() {
            // El agente se muestra como glifo en el hueco de estado (a la
            // derecha), así que el icono de la terminal queda sin superponer.
            None
        } else if pending {
            Some((icons::LOADER, ui.info, "Arrancando".to_owned()))
        } else if !running {
            Some((icons::STOP, ui.muted, "Detenido".to_owned()))
        } else {
            None
        };
        let title_color = if missing { ui.danger } else { ui.text };
        let meta_color = if missing || spawn_error {
            ui.danger
        } else {
            ui.muted
        };
        let icon_color = if focused || (selected && sidebar_focused) {
            ui.accent
        } else if aggregate.is_some() {
            ui.text
        } else {
            ui.muted
        };
        let row_tooltip = aggregate.as_ref().map(|aggregate| {
            let session = &aggregate.dominant;
            let status = tab_status_from_state(session.state);
            let now = SystemTime::now();
            let detail = session.message.as_deref().unwrap_or_else(|| status.label());
            format!(
                "{} · {} · {}\n{}",
                session.kind.label(),
                status.label(),
                format_duration(session.duration(now)),
                detail
            )
        });
        let menu_this = cx.entity().downgrade();
        let hover_group = format!("sidebar-tab-{}", tab.get());
        let mut row = div()
            .id(("sidebar-tab", tab.get()))
            .group(hover_group.clone())
            .relative()
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .h(metrics.sidebar_tab_height())
            .px(metrics.spacing_xs())
            .rounded(metrics.radius_md())
            .border_1()
            .border_color(transparent_black())
            .cursor_pointer()
            .when_some(row_tooltip, |this, tooltip| {
                this.tooltip(text_tooltip(tooltip))
            })
            .child(
                div()
                    .w(metrics.sidebar_icon_lane())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(metrics.sidebar_tab_icon())
                            .child(
                                ProfileIcon::new(icon)
                                    .size(IconSize::Custom(metrics.sidebar_tab_icon()))
                                    .color(icon_color),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.875)
                            .text_color(title_color)
                            .child(title),
                    )
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(meta_color)
                            .child(meta),
                    ),
            );
        let mut slot = div()
            .w(metrics.control_height_sm())
            .h(metrics.control_height_sm())
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0();
        if let Some(agent) = agent_status {
            slot = slot.child(
                div()
                    .id(format!("sidebar-agent-status-{}", tab.get()))
                    .group_hover(hover_group.clone(), |this| this.opacity(0.0))
                    .tooltip(text_tooltip(agent.label()))
                    .child(
                        Icon::new(agent.icon())
                            .size(IconSize::Md)
                            .color(agent.tone().color(ui))
                            .spin(agent.is_spinning()),
                    ),
            );
        }
        if let Some((status_icon, status_color, status_label)) = status {
            slot = slot.child(
                div()
                    .id(format!("sidebar-status-{}", tab.get()))
                    .group_hover(hover_group.clone(), |this| this.opacity(0.0))
                    .tooltip(text_tooltip(status_label))
                    .child(
                        Icon::new(status_icon)
                            .size(IconSize::Md)
                            .color(status_color),
                    ),
            );
        }
        slot = slot.child(
            div()
                .opacity(0.0)
                .group_hover(hover_group, |this| this.opacity(1.0))
                .child(
                    IconButton::new(format!("sidebar-close-{}", tab.get()), icons::CLOSE)
                        .size(ControlSize::Xs)
                        .icon_size(IconSize::Sm)
                        .tooltip("Cerrar terminal")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            cx.stop_propagation();
                            shell.close_tab_id(tab, cx);
                        })),
                ),
        );
        row = row.child(slot);
        if focused {
            row = row.bg(ui.surface_raised).child(
                div()
                    .absolute()
                    .left_0()
                    .top(metrics.spacing_xs())
                    .bottom(metrics.spacing_xs())
                    .w(metrics.tab_indicator())
                    .rounded(metrics.radius_full())
                    .bg(ui.accent),
            );
        }
        if selected && sidebar_focused {
            row = row.bg(ui.surface_raised).border_color(ui.focus);
        }
        if !focused && !(selected && sidebar_focused) {
            row = row.hover(move |this| this.bg(ui.surface_raised.opacity(0.55)));
        }
        let has_agent = aggregate.is_some();
        let row = row
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.activate_sidebar_tab(tab, window, cx);
            }))
            .context_menu(move |menu, _, _| {
                let menu = menu
                    .item(PopupMenuItem::new("Renombrar").on_click({
                        let this = menu_this.clone();
                        move |_, window, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    shell.begin_rename(tab, window, cx);
                                });
                            }
                        }
                    }))
                    .item(PopupMenuItem::new("Duplicar").on_click({
                        let this = menu_this.clone();
                        move |_, window, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    shell.duplicate_tab_with_window(tab, window, cx);
                                });
                            }
                        }
                    }));
                let menu = if has_agent {
                    menu.separator()
                        .item(PopupMenuItem::new("Reconocer agente").on_click({
                            let this = menu_this.clone();
                            move |_, _, cx| {
                                if let Some(this) = this.upgrade() {
                                    let _ = this.update(cx, |shell, cx| {
                                        if shell
                                            .workspace
                                            .acknowledge_tab_agents(tab, SystemTime::now())
                                        {
                                            shell.persist(cx);
                                            cx.notify();
                                        }
                                    });
                                }
                            }
                        }))
                        .item(PopupMenuItem::new("Marcar atención").on_click({
                            let this = menu_this.clone();
                            move |_, _, cx| {
                                if let Some(this) = this.upgrade() {
                                    let _ = this.update(cx, |shell, cx| {
                                        let now = SystemTime::now();
                                        shell.workspace.update_tab_agents(tab, |sessions| {
                                            if let Some(session) = sessions.first_mut() {
                                                session.apply_attention(
                                                    Some(AttentionKind::Permission),
                                                    Some("Marcado manualmente".into()),
                                                    now,
                                                );
                                            }
                                        });
                                        cx.notify();
                                    });
                                }
                            }
                        }))
                } else {
                    menu
                };
                menu.separator()
                    .item(PopupMenuItem::new("Cerrar pestaña").on_click({
                        let this = menu_this.clone();
                        move |_, _, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| shell.close_tab_id(tab, cx));
                            }
                        }
                    }))
            });
        row.into_any_element()
    }

    fn activate_project_row(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.sidebar_focus, cx);
        self.sidebar_cursor = Some(SidebarRow::Project(project));
        if self.sidebar_section == SidebarSection::All {
            let expanded = self.workspace.expanded_projects().contains(&project);
            let _ = self.workspace.set_project_expanded(project, !expanded);
            self.persist(cx);
        }
        cx.notify();
    }

    fn activate_sidebar_tab(
        &mut self,
        tab: TabId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.sidebar_focus, cx);
        self.sidebar_cursor = Some(SidebarRow::Tab(tab));
        self.focus_tab(tab, window, cx);
    }
}
