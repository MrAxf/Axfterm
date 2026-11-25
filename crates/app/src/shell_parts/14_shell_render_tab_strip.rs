impl Shell {
    fn render_tab_strip(
        &mut self,
        pane_id: PaneId,
        pane: &axf_workspace::Pane,
        focused: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let metrics = theme::metrics(cx);
        let (border, tab_bar) = {
            let theme = cx.theme();
            (theme.border, theme.tab_bar)
        };
        let scroll = self.pane_scroll.entry(pane_id).or_default().clone();
        let tabs = pane.tabs.clone();
        let active = pane.active;
        let focus_changed = focused && self.last_focused_pane != Some(pane_id);
        if focus_changed {
            self.last_focused_pane = Some(pane_id);
        }
        if let Some(active) = active {
            let needs_reveal = self.revealed.get(&pane_id) != Some(&active) || focus_changed;
            if needs_reveal && let Some(index) = pane.active_index() {
                if scroll.bounds_for_item(index).is_some() {
                    scroll.scroll_to_item(index);
                    self.revealed.insert(pane_id, active);
                } else {
                    // La pestaña aún no se ha medido: pide otro frame para
                    // revelarla en cuanto el layout la conozca.
                    cx.notify();
                }
            }
        }
        let overflow = scroll.max_offset().x > px(0.0);
        let scroll_step = metrics.tab_scroll_step();
        let shell_entity = cx.entity().entity_id();
        let this = cx.entity().downgrade();
        let mut tab_elements = Vec::with_capacity(tabs.len());
        for (index, tab) in tabs.iter().copied().enumerate() {
            tab_elements.push(self.render_tab(pane_id, tab, index, focused, cx));
        }
        let left_scroll = scroll.clone();
        let right_scroll = scroll.clone();
        let pane_menu_this = this.clone();
        let default_profile = self.theme_host.settings.terminal.default_profile.clone();
        let at_limit = self.workspace.tab_count() >= MAX_TABS;
        let local_project = pane
            .active
            .and_then(|tab| self.workspace.tab(tab))
            .and_then(|tab| self.workspace.project(tab.project_id))
            .is_none_or(|project| !project.host.is_remote());
        let profiles: Vec<(ProfileId, String)> = if local_project {
            self.theme_host
                .profiles()
                .iter()
                .map(|profile| (profile.id.clone(), profile.name.clone()))
                .collect()
        } else {
            Vec::new()
        };
        let mut profile_items: Vec<(ProfileId, String, bool, ProfileIconSource)> =
            Vec::with_capacity(profiles.len());
        for (id, name) in profiles {
            let icon = self.theme_host.profile_icon(&id);
            profile_items.push((id.clone(), name, id == default_profile, icon));
        }
        div()
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .px(metrics.spacing_sm())
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .h(metrics.tab_height())
            .flex_shrink_0()
            .border_b_1()
            .border_color(border)
            .bg(tab_bar)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _, window, cx| {
                    shell.focus_pane_click(pane_id, window, cx);
                }),
            )
            .when(overflow, |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            IconButton::new(
                                format!("tabs-left-{}", pane_id.get()),
                                icons::CHEVRON_LEFT,
                            )
                            .size(ControlSize::Sm)
                            .icon_size(IconSize::Custom(metrics.tab_icon()))
                            .tooltip("Desplazar pestañas")
                            .on_click({
                                let scroll = left_scroll.clone();
                                let entity = shell_entity;
                                move |_, _, cx| {
                                    let offset = scroll.offset();
                                    let max = scroll.max_offset().x;
                                    let x = tab_scroll_offset(offset.x, max, scroll_step);
                                    if x != offset.x {
                                        scroll.set_offset(point(x, offset.y));
                                        cx.notify(entity);
                                    }
                                }
                            }),
                        ),
                )
            })
            .child(
                div()
                    .id(("tabs-scroll", pane_id.get()))
                    .relative()
                    .flex()
                    .flex_grow_0()
                    .flex_shrink_1()
                    .min_w_0()
                    .h_full()
                    .overflow_x_scroll()
                    .track_scroll(&scroll)
                    .on_scroll(cx.listener(move |shell, event: &ScrollEvent, _, cx| {
                        shell.scroll_tabs(pane_id, event, cx);
                    }))
                    .on_drop::<TabDrag>(cx.listener(move |shell, drag: &TabDrag, window, cx| {
                        let index = shell
                            .tab_insert
                            .filter(|(pane, _)| *pane == pane_id)
                            .map(|(_, index)| index)
                            .unwrap_or(usize::MAX);
                        shell.drop_tab(drag, pane_id, index, window, cx);
                    }))
                    .children(tab_elements),
            )
            .when(overflow, |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            IconButton::new(
                                format!("tabs-right-{}", pane_id.get()),
                                icons::CHEVRON_RIGHT,
                            )
                            .size(ControlSize::Sm)
                            .icon_size(IconSize::Custom(metrics.tab_icon()))
                            .tooltip("Desplazar pestañas")
                            .on_click({
                                let scroll = right_scroll.clone();
                                let entity = shell_entity;
                                move |_, _, cx| {
                                    let offset = scroll.offset();
                                    let max = scroll.max_offset().x;
                                    let x = tab_scroll_offset(offset.x, max, -scroll_step);
                                    if x != offset.x {
                                        scroll.set_offset(point(x, offset.y));
                                        cx.notify(entity);
                                    }
                                }
                            }),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_xs())
                    .flex_shrink_0()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        IconButton::new(format!("tabs-new-{}", pane_id.get()), icons::PLUS)
                            .size(ControlSize::Sm)
                            .icon_size(IconSize::Custom(metrics.tab_icon()))
                            .tooltip("Nueva terminal")
                            .on_click(cx.listener(move |shell, _, window, cx| {
                                shell.new_tab_in_pane(pane_id, window, cx);
                            })),
                    )
                    .child(
                        KitButton::new(("pane-menu", pane_id.get()))
                            .small()
                            .ghost()
                            .dropdown_caret(true)
                            .dropdown_menu(move |menu, window, cx| {
                                let this = pane_menu_this.clone();
                                let profile_menu = {
                                    let items = profile_items.clone();
                                    let this = this.clone();
                                    PopupMenu::build(window, cx, move |mut submenu, _, _| {
                                        for (id, name, is_default, icon) in items {
                                            let this = this.clone();
                                            submenu = submenu.item(
                                                PopupMenuItem::element(move |_, cx| {
                                                    let metrics = theme::metrics(cx);
                                                    let color = theme::state(cx).theme.ui.text;
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(metrics.spacing_xs())
                                                        .child(
                                                            ProfileIcon::new(icon.clone())
                                                                .size(IconSize::Sm)
                                                                .color(color),
                                                        )
                                                        .child(name.clone())
                                                        .into_any_element()
                                                })
                                                .checked(is_default)
                                                .on_click(move |_, window, cx| {
                                                    if let Some(this) = this.upgrade() {
                                                        let _ = this.update(cx, |shell, cx| {
                                                            shell.new_tab_in_pane_with_profile(
                                                                pane_id,
                                                                Some(id.clone()),
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                            );
                                        }
                                        submenu
                                    })
                                };
                                let open_agent = |kind: AgentKind| -> PopupMenuItem {
                                    let this = this.clone();
                                    PopupMenuItem::element(move |_, cx| {
                                        let metrics = theme::metrics(cx);
                                        let theme_state = theme::state(cx);
                                        let ui = theme_state.theme.ui;
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(metrics.spacing_xs())
                                            .child(
                                                Icon::new(agent_icon(kind))
                                                    .size(IconSize::Sm)
                                                    .color(ui.text),
                                            )
                                            .child(kind.label())
                                            .child(
                                                div()
                                                    .ml_auto()
                                                    .text_size(metrics.font_size * 0.6875)
                                                    .text_color(ui.muted)
                                                    .child(keys::format_chord(agent_shortcut(kind))),
                                            )
                                            .into_any_element()
                                    })
                                    .disabled(at_limit)
                                    .on_click(move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.launch_agent(kind, Some(pane_id), window, cx);
                                            });
                                        }
                                    })
                                };
                                menu.item(PopupMenuItem::new("Nueva terminal").on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.new_tab_in_pane(pane_id, window, cx);
                                            });
                                        }
                                    }
                                }))
                                .when(!profile_items.is_empty(), |menu| {
                                    menu.item(PopupMenuItem::submenu("Con perfil", profile_menu))
                                })
                                .item(open_agent(AgentKind::OpenCode))
                                .item(open_agent(AgentKind::Copilot))
                                .separator()
                                .item(PopupMenuItem::new("Dividir a la derecha").on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.split_pane_action(
                                                    pane_id,
                                                    SplitAxis::Horizontal,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                }))
                                .item(PopupMenuItem::new("Dividir abajo").on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.split_pane_action(
                                                    pane_id,
                                                    SplitAxis::Vertical,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                }))
                                .item(PopupMenuItem::new("Zoom temporal").on_click({
                                    let this = this.clone();
                                    move |_, _, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.zoomed = if shell.zoomed == Some(pane_id) {
                                                    None
                                                } else {
                                                    Some(pane_id)
                                                };
                                                cx.notify();
                                            });
                                        }
                                    }
                                }))
                                .separator()
                                .item(PopupMenuItem::new("Ajustes…").on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.open_settings(window, cx);
                                            });
                                        }
                                    }
                                }))
                                .item(
                                    PopupMenuItem::new("Cerrar panel").on_click({
                                        let this = this.clone();
                                        move |_, window, cx| {
                                            if let Some(this) = this.upgrade() {
                                                let _ = this.update(cx, |shell, cx| {
                                                    if shell.pane_has_live_processes(pane_id, cx) {
                                                        shell.confirm(
                                                            window,
                                                            "Cerrar panel",
                                                            "Hay procesos vivos en este panel.",
                                                            PendingConfirm::ClosePane(pane_id),
                                                            cx,
                                                        );
                                                    } else {
                                                        shell.close_pane_impl(pane_id, cx);
                                                    }
                                                });
                                            }
                                        }
                                    }),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
}
