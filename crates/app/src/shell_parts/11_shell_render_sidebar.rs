impl Shell {
    fn render_sidebar(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let width = self.workspace.sidebar().width;
        let cursor = self.sidebar_cursor;
        let sidebar_focused = self.sidebar_focus.is_focused(window);
        let groups = sidebar_view(&self.workspace, self.sidebar_section);
        let no_projects = self.workspace.projects().is_empty();
        let empty = groups.is_empty();
        let at_limit = self.workspace.tab_count() >= MAX_TABS;
        let show_shortcut = width >= f32::from(metrics.sidebar_actions_compact_width());
        let mut new_terminal = Button::new("sidebar-new-terminal", "Nueva terminal")
            .icon(icons::PLUS)
            .secondary()
            .full_width()
            .disabled(at_limit)
            .tooltip(if at_limit {
                "Límite de 32 terminales; cierra una pestaña.".to_owned()
            } else {
                format!("Nueva terminal ({})", keys::format_chord(keys::NEW_TAB))
            })
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.new_tab_for(None, None, window, cx);
            }));
        if show_shortcut {
            new_terminal = new_terminal.shortcut(keys::format_chord(keys::NEW_TAB));
        }
        let open_project_this = cx.entity().downgrade();
        let open_menu = move |menu: PopupMenu, _: &mut Window, _: &mut Context<'_, PopupMenu>| {
            let local = open_project_this.clone();
            let wsl = open_project_this.clone();
            menu.item(
                PopupMenuItem::new("Carpeta local…").on_click(move |_, _, cx| {
                    if let Some(this) = local.upgrade() {
                        let _ = this.update(cx, |shell, cx| shell.prompt_open_project(cx));
                    }
                }),
            )
            .item(
                PopupMenuItem::new("Proyecto en WSL…").on_click(move |_, window, cx| {
                    if let Some(this) = wsl.upgrade() {
                        let _ = this.update(cx, |shell, cx| {
                            shell.open_wsl_modal(window, cx);
                        });
                    }
                }),
            )
        };
        let open_project = div()
            .flex()
            .items_center()
            .child(
                IconButton::new("sidebar-open-project", icons::FOLDER_OPEN)
                    .size(ControlSize::Md)
                    .icon_size(IconSize::Md)
                    .secondary()
                    .tooltip(format!(
                        "Abrir carpeta… ({})",
                        keys::format_chord(keys::OPEN_PROJECT)
                    ))
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.prompt_open_project(cx);
                    })),
            )
            .child(
                KitButton::new("sidebar-open-project-menu")
                    .dropdown_caret(true)
                    .secondary()
                    .tooltip("Más formas de abrir un proyecto")
                    .accessibility_label("Más formas de abrir un proyecto")
                    .dropdown_menu_with_anchor(Anchor::BottomRight, open_menu),
            );
        let actions = div()
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .child(div().flex_1().min_w_0().child(new_terminal))
            .child(open_project);
        let search = Button::new("sidebar-search", "Buscar…")
            .small()
            .variant(ButtonVariant::Outline)
            .icon(icons::SEARCH)
            .full_width()
            .shortcut(keys::format_chord(keys::SEARCH_PALETTE))
            .tooltip("Buscar pestañas, paneles y proyectos")
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_palette(PaletteMode::Search, window, cx);
            }));
        let sections = div()
            .flex()
            .items_center()
            .h(metrics.control_height_md())
            .p(metrics.hairline())
            .rounded(metrics.radius_md())
            .border_1()
            .border_color(ui.border)
            .bg(ui.background)
            .child(self.render_sidebar_section_segment(0, "Todas", SidebarSection::All, cx))
            .child(self.render_sidebar_section_segment(1, "Agentes", SidebarSection::Agents, cx));
        let top = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_sm())
            .p(metrics.spacing_sm())
            .child(actions)
            .child(search)
            .child(sections);
        let mut blocks = Vec::with_capacity(groups.len());
        for group in &groups {
            blocks.push(self.render_sidebar_project(group, cursor, sidebar_focused, cx));
        }
        let sidebar_empty = (empty
            && (!no_projects || self.sidebar_section == SidebarSection::Agents))
            .then(|| self.render_sidebar_empty(cx));
        let sidebar_list_bounds = Rc::clone(&self.sidebar_list_bounds);
        let list = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .px(metrics.spacing_sm())
            .pt(metrics.spacing_xs())
            .pb(metrics.spacing_sm())
            .gap(metrics.spacing_sm())
            .on_prepaint(move |bounds, _, _| {
                *sidebar_list_bounds.borrow_mut() = Some(bounds);
            })
            .children(blocks)
            .when_some(sidebar_empty, |this, empty| this.child(empty));
        let list_section = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .mb(metrics.spacing_xs())
            .border_t_1()
            .border_b_1()
            .border_color(ui.border)
            .child(list);
        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(width))
            .flex_shrink_0()
            .border_r_1()
            .border_color(ui.border)
            .bg(ui.surface)
            .text_color(ui.text)
            .key_context(SIDEBAR_CONTEXT)
            .track_focus(&self.sidebar_focus)
            .on_key_down(cx.listener(Self::sidebar_key))
            .child(top)
            .child(list_section)
            .child(self.render_sidebar_footer(cx))
            .into_any_element()
    }

    fn render_sidebar_section_segment(
        &self,
        index: usize,
        label: &'static str,
        section: SidebarSection,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let selected = self.sidebar_section == section;
        let radius = metrics.radius_sm();
        let tooltip = (section == SidebarSection::Agents).then(|| {
            format!(
                "Sesiones de agente ({})",
                keys::format_chord(keys::TOGGLE_SIDEBAR_AGENTS)
            )
        });
        let mut segment = div()
            .id(("sidebar-section", index))
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .h_full()
            .cursor_pointer()
            .text_size(metrics.font_size * 0.75)
            .text_color(if selected { ui.text } else { ui.muted })
            .when(selected, |this| this.bg(ui.surface_raised))
            .when(!selected, move |this| {
                this.hover(move |this| this.text_color(ui.text))
            })
            .when_some(tooltip, |this, tooltip| this.tooltip(text_tooltip(tooltip)))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.set_sidebar_section(section, cx);
            }))
            .child(label);
        if index == 0 {
            segment = segment.rounded_tl(radius).rounded_bl(radius);
        } else {
            segment = segment.rounded_tr(radius).rounded_br(radius);
        }
        segment.into_any_element()
    }

    fn render_sidebar_empty(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(metrics.spacing_sm())
            .px(metrics.spacing_sm())
            .py(metrics.spacing_xl())
            .text_size(metrics.font_size * 0.75)
            .text_color(ui.muted)
            .child(Icon::new(icons::PLAYER_PLAY).size(IconSize::Lg).color(ui.muted))
            .child(
                div()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(ui.text)
                    .child("Sin agentes activos"),
            )
            .child(
                div()
                    .text_center()
                    .child("Las terminales con OpenCode o Copilot aparecerán aquí."),
            )
            .into_any_element()
    }

    fn render_sidebar_project(
        &mut self,
        group: &SidebarGroup,
        cursor: Option<SidebarRow>,
        sidebar_focused: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(project) = self.workspace.project(group.project).cloned() else {
            return div().into_any_element();
        };
        let project_id = project.id;
        let missing = self.project_missing(project_id);
        let selected = matches!(cursor, Some(SidebarRow::Project(id)) if id == project_id);
        let can_collapse = self.sidebar_section == SidebarSection::All;
        let expanded = self.workspace.expanded_projects().contains(&project_id);
        let toggle_label = if expanded { "Contraer" } else { "Expandir" };
        let chevron = can_collapse.then_some(if group.show_tabs {
            icons::CHEVRON_DOWN
        } else {
            icons::CHEVRON_RIGHT
        });
        let menu_this = cx.entity().downgrade();
        let indent = metrics.spacing_xs() + metrics.sidebar_icon_lane() + metrics.spacing_sm();

        // Con el proyecto plegado sus terminales no se ven: el encabezado
        // muestra el estado más urgente de sus agentes para que una petición
        // de atención no pase desapercibida.
        let collapsed_status = (!group.show_tabs)
            .then(|| {
                most_urgent_agent_state(group.tabs.iter().filter_map(|tab| {
                    self.workspace
                        .tab(*tab)?
                        .agent_aggregate()
                        .map(|aggregate| aggregate.state)
                }))
            })
            .flatten()
            .map(tab_status_from_state);
        let collapsed_glyph = collapsed_status.map(|status| {
            Icon::new(status.icon())
                .size(IconSize::Sm)
                .color(status.tone().color(ui))
                .spin(status.is_spinning())
        });
        let mut header = div()
            .id(("project", project_id.get()))
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .h(metrics.row_height())
            .px(metrics.spacing_xs())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(transparent_black())
            .cursor_pointer()
            .text_color(if missing { ui.danger } else { ui.muted })
            .tooltip(text_tooltip(project.display_root()))
            .child(
                div()
                    .w(metrics.sidebar_icon_lane())
                    .flex()
                    .justify_center()
                    .when_some(chevron, |this, chevron| {
                        this.child(Icon::new(chevron).size(IconSize::Sm).color(ui.muted))
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(metrics.font_size * 0.75)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(project.name.clone()),
            )
            .when(project.host.is_remote(), |this| {
                this.child(
                    div()
                        .px(metrics.spacing_xs())
                        .rounded(metrics.radius_full())
                        .bg(ui.surface_raised)
                        .text_size(metrics.font_size * 0.625)
                        .text_color(ui.subtle)
                        .child("WSL"),
                )
            })
            .when_some(collapsed_glyph, |this, glyph| this.child(glyph))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(metrics.control_height_xs())
                    .px(metrics.spacing_xs())
                    .rounded(metrics.radius_full())
                    .bg(ui.surface_raised)
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(group.tabs.len().to_string()),
            )
            .child(
                IconButton::new(format!("project-new-{}", project_id.get()), icons::PLUS)
                    .size(ControlSize::Sm)
                    .icon_size(IconSize::Sm)
                    .tooltip("Nueva terminal")
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        cx.stop_propagation();
                        shell.new_tab_for(Some(project_id), None, window, cx);
                    })),
            );
        if selected && sidebar_focused {
            header = header.bg(ui.surface_raised).border_color(ui.focus);
        } else {
            header = header.hover(move |this| this.bg(ui.surface_raised.opacity(0.55)));
        }
        let header = header
            .on_drag_move::<axf_ui::gpui::ExternalPaths>(cx.listener(
                move |shell,
                      event: &axf_ui::gpui::DragMoveEvent<axf_ui::gpui::ExternalPaths>,
                      window,
                      cx| {
                    // Solo los proyectos plegados necesitan expandirse al
                    // arrastrar archivos; los visibles ya muestran sus filas.
                    let collapsed = shell.sidebar_section == SidebarSection::All
                        && !shell.workspace.expanded_projects().contains(&project_id);
                    let over = collapsed
                        && !shell.file_drag_blocked()
                        && file_drag_over(event.bounds, event.event.position, shell.sidebar_viewport());
                    shell.observe_file_drag(
                        FileDragHover::SidebarProject(project_id),
                        over,
                        window,
                        cx,
                    );
                },
            ))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.activate_project_row(project_id, window, cx);
            }))
            .context_menu(move |menu, _, _| {
                let menu_this = menu_this.clone();
                let menu = menu.item(PopupMenuItem::new("Nueva terminal").on_click({
                    let this = menu_this.clone();
                    move |_, window, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                shell.new_tab_for(Some(project_id), None, window, cx);
                            });
                        }
                    }
                }));
                let menu = if can_collapse {
                    menu.item(PopupMenuItem::new(toggle_label).on_click({
                        let this = menu_this.clone();
                        move |_, _, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    let _ =
                                        shell.workspace.set_project_expanded(project_id, !expanded);
                                    shell.persist(cx);
                                    cx.notify();
                                });
                            }
                        }
                    }))
                } else {
                    menu
                };
                menu.separator()
                    .item(PopupMenuItem::new("Quitar proyecto").on_click({
                        let this = menu_this.clone();
                        move |_, window, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    shell.remove_project_checked(project_id, window, cx);
                                });
                            }
                        }
                    }))
            });
        let mut block = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_xxs())
            .child(header);
        if group.show_tabs {
            if missing {
                block = block.child(
                    div()
                        .pl(indent)
                        .pr(metrics.spacing_xs())
                        .py(metrics.spacing_xs())
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.danger)
                        .child("Carpeta no encontrada"),
                );
            } else if group.tabs.is_empty() {
                block = block.child(
                    div()
                        .pl(indent)
                        .pr(metrics.spacing_xs())
                        .py(metrics.spacing_xs())
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child("Sin terminales"),
                );
            }
            for tab in &group.tabs {
                block = block.child(self.render_sidebar_tab(
                    *tab,
                    project_id,
                    cursor,
                    sidebar_focused,
                    cx,
                ));
            }
        }
        block.into_any_element()
    }
}
