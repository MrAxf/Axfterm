impl Shell {
    /// Abre la ruta indicada con la aplicación del sistema.
    fn settings_open_path(&mut self, path: Option<PathBuf>, cx: &mut Context<'_, Self>) {
        match path {
            Some(path) => cx.open_with_system(&path),
            None => self.set_notice("La ruta no está disponible.", true, cx),
        }
    }

    // --- Panel de ajustes: render --------------------------------------
    /// Barra lateral de secciones del panel.
    fn render_settings_rail(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let section = self
            .settings
            .as_ref()
            .map(|settings| settings.section)
            .unwrap_or(SettingsSection::Appearance);
        let mut rows = div().flex().flex_col().gap(metrics.spacing_xxs()).min_w_0();
        for candidate in SettingsSection::ALL {
            let candidate = *candidate;
            let selected = candidate == section;
            let row = Self::settings_focusable(
                window,
                cx,
                format!("settings-section-{}", candidate.label()),
            )
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .min_w_0()
            .h(metrics.control_height_md())
            .px(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .when(selected, |this| this.bg(ui.selection))
            .when(!selected, |this| {
                this.hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
            })
            .on_click(cx.listener(move |shell, _, _, cx| {
                if let Some(settings) = &mut shell.settings {
                    settings.set_section(candidate);
                }
                shell.settings_scroll = ScrollHandle::new();
                cx.notify();
            }))
            .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if let Some(settings) = &mut shell.settings {
                        settings.set_section(candidate);
                    }
                    shell.settings_scroll = ScrollHandle::new();
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                Icon::new(candidate.icon())
                    .size(IconSize::Sm)
                    .color(if selected { ui.accent } else { ui.muted }),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(ui.text)
                    .child(candidate.label()),
            );
            rows = rows.child(row);
        }
        let rail_width = metrics.font_size * 11.5 + metrics.spacing_sm() * 2.0;
        div()
            .w(rail_width)
            .max_w(rail_width)
            .min_w_0()
            .flex_shrink_0()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .p(metrics.spacing_sm())
                    .border_r_1()
                    .border_color(ui.border)
                    .child(rows),
            )
            .into_any_element()
    }

    /// Cabecera con el título de la sección, la ayuda y los avisos.
    fn render_settings_header(
        &mut self,
        cx: &mut Context<'_, Self>,
        section: SettingsSection,
        notice: Option<(String, bool)>,
        conflict: Option<String>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut header = div()
            .flex()
            .flex_col()
            .w_full()
            .gap(metrics.spacing_xxs())
            .px(metrics.spacing_lg())
            .pt(metrics.spacing_lg())
            .pb(metrics.spacing_sm())
            .child(
                div()
                    .text_size(metrics.font_size * 0.95)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(section.label()),
            )
            .child(
                div()
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.muted)
                    .child(section.description()),
            )
            .when_some(notice, |this, (text, error)| {
                this.child(
                    div()
                        .pt(metrics.spacing_xs())
                        .text_size(metrics.font_size * 0.75)
                        .text_color(if error { ui.danger } else { ui.muted })
                        .child(text),
                )
            });
        if let Some(message) = conflict {
            header = header.child(self.render_settings_conflict(message, cx));
        }
        header.into_any_element()
    }

    /// Contenido de la sección activa.
    fn render_settings_body(
        &mut self,
        section: SettingsSection,
        body_width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        match section {
            SettingsSection::Appearance => self.render_settings_appearance(body_width, window, cx),
            SettingsSection::Profiles => self.render_settings_profiles(body_width, window, cx),
            SettingsSection::Shortcuts => self.render_settings_shortcuts(window, cx),
            SettingsSection::Agents => self.render_settings_agents(window, cx),
        }
    }

    fn render_settings(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let metrics = theme::state(cx).metrics;
        self.ensure_settings_sliders(cx);
        self.sync_settings_sliders(window, cx);
        self.ensure_agent_profile_selects(window, cx);
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let section = settings.section;
        let dirty = settings.is_dirty();
        let notice = settings.visible_notice().cloned();
        let conflict = settings.conflict_message().map(str::to_owned);
        let errors = settings.errors.clone();
        let rail = self.render_settings_rail(window, cx);
        let header = self.render_settings_header(cx, section, notice, conflict);
        let footer = self.render_settings_footer(cx, dirty, &errors);
        let height = (window.viewport_size().height - metrics.font_size * 20.0)
            .max(metrics.font_size * 16.0);
        let max_width = window.viewport_size().width - metrics.font_size * 4.0;
        let min_width = (metrics.font_size * 40.0).min(max_width);
        let card_width = (metrics.font_size * 62.0).min(max_width).max(min_width);
        let rail_width = metrics.font_size * 11.5 + metrics.spacing_sm() * 2.0;
        let body_width = (card_width - rail_width - metrics.spacing_lg() * 4.0).max(px(0.0));
        let body = self.render_settings_body(section, body_width, window, cx);
        let scroll = self.settings_scroll.clone();
        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(header)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(
                        div()
                            .id("settings-content")
                            .size_full()
                            .overflow_x_hidden()
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .px(metrics.spacing_lg())
                            .pb(metrics.spacing_sm())
                            .child(
                                div()
                                    .w(body_width)
                                    .max_w(body_width)
                                    .min_w_0()
                                    .child(body),
                            ),
                    )
                    // La barra vive fuera del área desplazable: colgada del
                    // div con `overflow_y_scroll` subía con el contenido.
                    .vertical_scrollbar(&scroll),
            )
            .child(footer);
        let card_body = div()
            .flex()
            .w_full()
            .min_w_0()
            .h(height)
            .min_h_0()
            .overflow_hidden()
            .child(rail)
            .child(content);
        let modal = Modal::new("settings-modal", "Ajustes")
            .width(card_width)
            .align_top()
            .body(card_body)
            .on_close(cx.listener(|shell, _, window, cx| {
                shell.close_settings(window, cx);
            }))
            .into_any_element();
        let layer = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context(SETTINGS_CONTEXT)
            .track_focus(&self.settings_focus)
            .tab_group()
            .on_action(cx.listener(Self::settings_cancel))
            .on_action(cx.listener(Self::settings_save))
            .on_action(cx.listener(Self::settings_next_section))
            .on_action(cx.listener(Self::settings_previous_section))
            .child(modal);
        let extra = self
            .render_settings_profile_modal(cx)
            .or_else(|| self.render_settings_defaults_confirm(cx))
            .or_else(|| self.render_settings_delete_confirm(cx));
        let layer = if let Some(extra) = extra {
            layer.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .focus_trap("settings-modal-trap", &self.settings_profile_focus)
                    .child(extra),
            )
        } else {
            layer
        };
        layer
            .focus_trap("settings-trap", &self.settings_focus)
            .into_any_element()
    }

}
