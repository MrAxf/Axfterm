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
            .child(
                KitButton::new("settings-conflict-reload")
                    .small()
                    .label("Recargar")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.reload_settings_from_disk(cx);
                    })),
            )
            .child(
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
            )
            .child(
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
            )
            .into_any_element()
    }

    fn render_settings_appearance(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let appearance = settings.draft.appearance;
        let selected_theme = settings.draft.ui.theme.clone();
        let ui_size = settings.draft.ui.font_size;
        let ui_scale = settings.draft.ui.scale;
        let themes = self.theme_host.theme_names();
        let ui_font_select = self.ui_font_select.clone();
        let mut choices = div().flex().flex_col().gap(metrics.spacing_xxs());
        for (label, value, hint) in [
            (
                "Sistema",
                axf_config::Appearance::System,
                "Sigue el tema del sistema.",
            ),
            (
                "Claro",
                axf_config::Appearance::Light,
                "Fuerza la variante clara.",
            ),
            (
                "Oscuro",
                axf_config::Appearance::Dark,
                "Fuerza la variante oscura.",
            ),
        ] {
            let selected = appearance == value;
            let row = Self::settings_focusable(window, cx, format!("settings-appearance-{label}"))
                .flex()
                .items_center()
                .gap(metrics.spacing_sm())
                .px(metrics.spacing_sm())
                .py(metrics.spacing_xs())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    if let Some(settings) = &mut shell.settings {
                        settings.set_appearance(value);
                    }
                    shell.preview_settings(cx);
                }))
                .child(
                    Icon::new(if selected {
                        icons::CHECK
                    } else {
                        icons::CIRCLE
                    })
                    .size(IconSize::Xs)
                    .color(if selected { ui.accent } else { ui.muted }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(metrics.hairline())
                        .child(div().text_size(metrics.font_size * 0.8125).child(label))
                        .child(
                            div()
                                .text_size(metrics.font_size * 0.6875)
                                .text_color(ui.muted)
                                .child(hint),
                        ),
                );
            choices = choices.child(row);
        }
        let mut theme_list = div().flex().flex_col().gap(metrics.hairline());
        for (name, variant) in themes {
            let selected = name == selected_theme;
            let dark = variant.is_dark();
            let label = name.clone();
            let row = Self::settings_focusable(window, cx, format!("settings-theme-{name}"))
                .flex()
                .items_center()
                .gap(metrics.spacing_sm())
                .px(metrics.spacing_sm())
                .py(metrics.spacing_xs())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    if let Some(settings) = &mut shell.settings {
                        settings.set_theme(&label, dark);
                    }
                    shell.preview_settings(cx);
                }))
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
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(metrics.font_size * 0.75)
                        .child(name),
                )
                .child(
                    div()
                        .text_size(metrics.font_size * 0.6875)
                        .text_color(ui.muted)
                        .child(variant.label()),
                );
            theme_list = theme_list.child(row);
        }
        let font_field = ui_font_select.map(|select| {
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xxs())
                .child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child("Fuente de la interfaz"),
                )
                .child(Select::new(&select).small().w_full().placeholder("Sistema"))
                .into_any_element()
        });
        let body = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_lg())
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
                    .child(choices),
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
                            .child("Catálogo de temas"),
                    )
                    .child(
                        div()
                            .id("settings-theme-list")
                            .max_h(metrics.font_size * 12.0)
                            .overflow_y_scroll()
                            .child(theme_list),
                    ),
            )
            .when_some(font_field, |this, field| this.child(field))
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-ui-size",
                "Tamaño base de la UI",
                format!("{:.0} px", ui_size),
                StepField::UiFontSize,
            ))
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-ui-scale",
                "Escala global",
                format!("{:.2}×", ui_scale),
                StepField::UiScale,
            ));
        body.into_any_element()
    }

    fn render_settings_terminal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let font_select = self.terminal_font_select.clone();
        let font_size = format!("{:.0} px", settings.draft.terminal.font_size);
        let line_height = format!("{:.2}×", settings.draft.terminal.line_height);
        let mono = state.fonts.mono.clone();
        let mono_size = state.mono_font_size;
        let font_field = font_select.map(|select| {
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xxs())
                .child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child("Fuente monoespaciada"),
                )
                .child(Select::new(&select).small().w_full().placeholder("Sistema"))
                .into_any_element()
        });
        let preview = div()
            .flex()
            .flex_col()
            .gap(metrics.hairline())
            .p(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface_raised)
            .child(
                div()
                    .font_family(mono.clone())
                    .text_size(mono_size)
                    .text_color(ui.text)
                    .child("C:\\proyectos\\axfterm ❯ axfterm --help"),
            )
            .child(
                div()
                    .font_family(mono)
                    .text_size(mono_size)
                    .text_color(ui.muted)
                    .child("0123456789 · []{}()<> · áéíóú · ñ"),
            );
        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_lg())
            .when_some(font_field, |this, field| this.child(field))
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-terminal-size",
                "Tamaño de celda",
                font_size,
                StepField::TerminalFontSize,
            ))
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-line-height",
                "Altura de línea",
                line_height,
                StepField::LineHeight,
            ))
            .child(preview)
            .into_any_element()
    }
}
