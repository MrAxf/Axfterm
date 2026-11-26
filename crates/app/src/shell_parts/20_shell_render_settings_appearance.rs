impl Shell {
    fn render_settings_conflict(
        &mut self,
        message: String,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        div()
            .mt(metrics.spacing_sm())
            .flex()
            .flex_wrap()
            .items_center()
            .gap(metrics.spacing_sm())
            .p(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(ui.warning)
            .bg(ui.surface_raised)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.text)
                    .child(message),
            )
            .child(with_hand(
                KitButton::new("settings-conflict-reload")
                    .small()
                    .label("Recargar")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.reload_settings_from_disk(cx);
                    })),
            ))
            .child(with_hand(
                KitButton::new("settings-conflict-keep")
                    .small()
                    .ghost()
                    .label("Seguir editando")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.keep_editing();
                        }
                        cx.notify();
                    })),
            ))
            .child(with_hand(
                KitButton::new("settings-conflict-overwrite")
                    .small()
                    .danger()
                    .label("Sobrescribir y guardar")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.allow_overwrite();
                        }
                        shell.save_settings(cx);
                    })),
            ))
            .into_any_element()
    }

    fn render_settings_appearance(
        &mut self,
        body_width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mono = state.fonts.mono.clone();
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let appearance = settings.draft.appearance;
        let selected_theme = settings.draft.ui.theme.clone();
        let themes = self.theme_host.catalog.themes.clone();
        let two_columns = f32::from(body_width) >= f32::from(metrics.font_size) * 44.0;
        let gap = metrics.spacing_lg();
        let column_width = if two_columns {
            px((f32::from(body_width) - f32::from(gap)) * 0.5)
        } else {
            body_width
        };
        let appearance_index = match appearance {
            axf_config::Appearance::System => 0,
            axf_config::Appearance::Light => 1,
            axf_config::Appearance::Dark => 2,
        };
        let choices = SegmentedControl::new("settings-appearance")
            .options(["Sistema", "Claro", "Oscuro"])
            .selected(appearance_index)
            .size(ControlSize::Sm)
            .on_select(cx.listener(|shell, index: &usize, _, cx| {
                let appearance = match index {
                    0 => axf_config::Appearance::System,
                    1 => axf_config::Appearance::Light,
                    2 => axf_config::Appearance::Dark,
                    _ => return,
                };
                if let Some(settings) = &mut shell.settings {
                    settings.set_appearance(appearance);
                }
                shell.preview_settings(cx);
            }));
        let mut theme_list = div()
            .id("settings-theme-list")
            .flex()
            .flex_wrap()
            .gap(metrics.spacing_sm());
        for candidate in themes {
            let name = candidate.meta.name.clone();
            let dark = candidate.variant().is_dark();
            let selected = name == selected_theme;
            let card_width = theme_card_width(metrics, column_width);
            let swatch = div()
                .flex()
                .flex_shrink_0()
                .gap(metrics.hairline())
                .w(metrics.font_size * 1.6)
                .h(metrics.font_size * 1.0)
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .rounded(metrics.radius_sm())
                        .bg(candidate.ui.accent),
                )
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .rounded(metrics.radius_sm())
                        .bg(candidate.terminal.ansi(2)),
                )
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .rounded(metrics.radius_sm())
                        .bg(candidate.terminal.ansi(4)),
                );
            let preview = div()
                .flex()
                .items_center()
                .gap(metrics.spacing_xs())
                .p(metrics.spacing_xs())
                .rounded(metrics.radius_sm())
                .bg(candidate.terminal.background)
                .font_family(mono.clone())
                .text_size(metrics.font_size * 0.6875)
                .child(
                    div()
                        .text_color(candidate.terminal.ansi(2))
                        .child("›"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_color(candidate.terminal.foreground)
                        .child("cargo check"),
                )
                .child(
                    div()
                        .text_color(candidate.terminal.ansi(4))
                        .child("OK"),
                );
            let click_name = name.clone();
            let card_label = format!("{name} · {}", candidate.variant().label());
            let id: ElementId = format!("settings-theme-{name}").into();
            let focus = window
                .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
                .read(cx)
                .clone();
            let focused = focus.is_focused(window);
            let card = div()
                .id(id)
                .track_focus(&focus)
                .tooltip(text_tooltip(name.clone()))
                .role(Role::Button)
                .aria_label(card_label)
                .flex()
                .flex_col()
                .gap(metrics.spacing_xs())
                .w(card_width)
                .min_w_0()
                .p(metrics.spacing_md())
                .rounded(metrics.radius_md())
                .border_1()
                .border_color(if focused {
                    ui.focus
                } else if selected {
                    ui.accent
                } else {
                    ui.border
                })
                .bg(if selected {
                    ui.selection
                } else {
                    ui.surface
                })
                .cursor_pointer()
                .when(!selected, move |this| {
                    this.hover(move |this| this.bg(ui.surface_hover))
                        .active(move |this| this.bg(ui.surface_active))
                })
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.select_settings_theme(&click_name, dark, cx);
                }))
                .on_key_down(cx.listener({
                    let key_name = name.clone();
                    move |shell, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            shell.select_settings_theme(&key_name, dark, cx);
                            cx.stop_propagation();
                        }
                    }
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(metrics.spacing_xs())
                        .min_w_0()
                        .child(swatch)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(metrics.font_size * 0.75)
                                .text_color(ui.text)
                                .child(name),
                        )
                        .child(
                            Icon::new(if selected {
                                icons::CHECK
                            } else if dark {
                                icons::MOON
                            } else {
                                icons::SUN
                            })
                            .size(IconSize::Xs)
                            .color(if selected { ui.accent } else { ui.muted }),
                        ),
                )
                .child(preview);
            theme_list = theme_list.child(card);
        }
        let theme_group = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_sm())
            .w(column_width)
            .min_w_0()
            .child(
                div()
                    .text_size(metrics.font_size * 0.8125)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Tema"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Modo de color"),
                    )
                    .child(choices)
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child("Sistema sigue la apariencia del dispositivo."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .min_w_0()
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Catálogo de temas"),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child(format!("Actual: {selected_theme}")),
                    ),
            )
            .child(theme_list);
        let typography = self.render_settings_typography(column_width, window, cx);
        div()
            .flex()
            .flex_wrap()
            .items_start()
            .w(body_width)
            .max_w(body_width)
            .min_w_0()
            .gap(gap)
            .child(theme_group)
            .child(typography)
            .into_any_element()
    }

    fn select_settings_theme(&mut self, name: &str, dark: bool, cx: &mut Context<'_, Self>) {
        if let Some(settings) = &mut self.settings {
            settings.set_theme(name, dark);
        }
        self.preview_settings(cx);
    }
}

/// Ancho de tarjeta de tema: dos por fila si caben con holgura.
fn theme_card_width(metrics: UiMetrics, available: Pixels) -> Pixels {
    let half = px(f32::from(available - metrics.spacing_sm()) * 0.5);
    if half >= metrics.font_size * 10.0 {
        half
    } else {
        available
    }
}
