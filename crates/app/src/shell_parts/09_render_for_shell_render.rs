impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self
            .theme_host
            .sync_display_scale(window.scale_factor(), cx)
        {
            cx.notify();
        }
        if self.style_revision != self.theme_host.revision {
            self.style_revision = self.theme_host.revision;
            self.refresh_terminal_styles(window, cx);
        }

        // Un drag cancelado o soltado fuera de un destino no debe dejar el
        // resaltado de drop pegado en pantalla.
        if !cx.has_active_drag() && (self.tab_drop.is_some() || self.tab_insert.is_some()) {
            self.tab_drop = None;
            self.tab_insert = None;
        }

        // Los paneles cerrados no deben dejar límites fantasma para el drag.
        self.body_bounds
            .borrow_mut()
            .retain(|pane, _| self.workspace.pane(*pane).is_some());
        if !self.close_hook_registered {
            self.close_hook_registered = true;
            let this = cx.entity().downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                if let Some(this) = this.upgrade() {
                    let _ = this.update(cx, |shell, cx| shell.request_quit(cx));
                }

                // La app cierra el proceso después de guardar.
                false
            });
        }
        if let Some(tab) = self.pending_focus.take()
            && let Some(model_tab) = self.workspace.tab(tab)
            && let Some(entry) = self.terminals.get(&model_tab.terminal_id)
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        }
        if let Some(pane) = self.zoomed
            && (!self.workspace.layout().contains(pane) || self.workspace.pane(pane).is_none())
        {
            self.zoomed = None;
        }
        if let Some(edit) = &mut self.editing
            && !edit.focused
        {
            let input = edit.input.clone();
            let handle = input.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
            input.update(cx, |input, cx| input.select_all(window, cx));
            edit.focused = true;
        }

        // El título de ventana sigue a la pestaña enfocada y marca la atención
        // pendiente de cualquier agente.
        let base_title = self
            .workspace
            .focus()
            .tab
            .map(|tab| format!("{} — Axfterm", self.tab_title(tab, cx)))
            .unwrap_or_else(|| "Axfterm".into());
        let title = if self.workspace.has_pending_attention() {
            format!("! {base_title}")
        } else {
            base_title
        };
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }
        let (background, foreground) = {
            let theme = cx.theme();
            (theme.background, theme.foreground)
        };
        let sidebar = self
            .workspace
            .sidebar()
            .visible
            .then(|| self.render_sidebar(window, cx));
        let content = if self.workspace.tab_count() == 0 {
            self.render_onboarding(window, cx)
        } else if let Some(pane) = self.zoomed {
            div()
                .size_full()
                .child(self.render_pane(pane, cx))
                .into_any_element()
        } else {
            let layout = self.workspace.layout().clone();
            self.render_layout(&layout, &[], cx)
        };
        let toast = self.render_toast(cx);
        let settings = self
            .settings
            .is_some()
            .then(|| self.render_settings(window, cx));
        let shortcuts = self.shortcuts_modal.then(|| self.render_shortcuts(cx));
        let palette = self
            .palette
            .is_some()
            .then(|| self.render_palette(window, cx));
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .key_context(WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::open_project))
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::move_tab_next_pane))
            .on_action(cx.listener(Self::split_horizontal))
            .on_action(cx.listener(Self::split_vertical))
            .on_action(cx.listener(Self::close_pane))
            .on_action(cx.listener(Self::focus_next_pane))
            .on_action(cx.listener(Self::focus_previous_pane))
            .on_action(cx.listener(Self::rename_tab))
            .on_action(cx.listener(Self::duplicate_tab))
            .on_action(cx.listener(Self::toggle_zoom))
            .on_action(cx.listener(Self::restart_tab))
            .on_action(cx.listener(Self::focus_sidebar))
            .on_action(cx.listener(Self::open_command_palette))
            .on_action(cx.listener(Self::open_search_palette))
            .on_action(cx.listener(Self::toggle_sidebar_agents))
            .on_action(cx.listener(Self::show_settings))
            .on_action(cx.listener(Self::toggle_theme_variant))
            .on_action(cx.listener(Self::next_theme))
            .on_action(cx.listener(Self::previous_theme))
            .on_action(cx.listener(Self::reload_themes))
            .on_action(cx.listener(Self::open_themes_folder))
            .on_action(cx.listener(Self::import_theme))
            .on_action(cx.listener(Self::export_theme))
            .on_action(cx.listener(Self::agent_next_attention))
            .on_action(cx.listener(Self::toggle_shortcuts))
            .on_drop::<axf_ui::gpui::ExternalPaths>(cx.listener(
                |shell, paths: &axf_ui::gpui::ExternalPaths, window, cx| {
                    for path in &paths.0 {
                        if path.is_dir() {
                            shell.open_project_path(path.clone(), window, cx);
                        }
                    }
                },
            ))
            .child(self.render_title_bar(window, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .overflow_hidden()
                    .children(sidebar)
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .w_full()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .on_drag_move(cx.listener(
                                |shell, event: &axf_ui::gpui::DragMoveEvent<TabDrag>, _, cx| {
                                    shell.update_tab_drag_target(event, cx);
                                },
                            ))
                            .child(content),
                    ),
            )
            .children(toast)
            .children(settings)
            .children(shortcuts)
            .children(palette)
    }
}
