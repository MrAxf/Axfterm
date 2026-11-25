impl Shell {
    /// Tipografía de la aplicación y de la terminal, con sliders y muestra.
    fn render_settings_typography(
        &mut self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let resolved_ui = state.fonts.ui.clone().to_string();
        let resolved_mono = state.fonts.mono.clone().to_string();
        let mono = state.fonts.mono.clone();
        let mono_size = state.mono_font_size;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let ui_family = settings.draft.ui.font_family.clone();
        let terminal_family = settings.draft.terminal.font_family.clone();
        let ui_size = settings.draft.ui.font_size;
        let ui_scale = settings.draft.ui.scale;
        let terminal_size = settings.draft.terminal.font_size;
        let line_height = settings.draft.terminal.line_height;
        let scrollback = settings.draft.terminal.scrollback_lines;
        let ui_select = self.ui_font_select.clone();
        let terminal_select = self.terminal_font_select.clone();
        let sliders = self.settings_sliders.as_ref();
        let _ = window;

        let ui_actual = font_actual_label(ui_family.as_deref(), &resolved_ui);
        let terminal_actual = font_actual_label(terminal_family.as_deref(), &resolved_mono);

        let ui_field = ui_select
            .as_ref()
            .map(|select| {
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .min_w_0()
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Fuente de la interfaz"),
                    )
                    .child(Select::new(select).small().w_full())
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child(ui_actual),
                    )
                    .into_any_element()
            })
            .unwrap_or_else(|| div().into_any_element());
        let terminal_field = terminal_select
            .as_ref()
            .map(|select| {
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .min_w_0()
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Fuente de la terminal"),
                    )
                    .child(Select::new(select).small().w_full())
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child(terminal_actual),
                    )
                    .into_any_element()
            })
            .unwrap_or_else(|| div().into_any_element());

        let size_slider = sliders
            .map(|sliders| {
                settings_slider_row(
                    cx,
                    "Tamaño de la interfaz",
                    format!("{ui_size:.0} px"),
                    &sliders.ui_font_size,
                )
            })
            .unwrap_or_else(|| div().into_any_element());
        let scale_slider = sliders
            .map(|sliders| {
                settings_slider_row(
                    cx,
                    "Escalado de la interfaz",
                    format!("{ui_scale:.2}×"),
                    &sliders.ui_scale,
                )
            })
            .unwrap_or_else(|| div().into_any_element());
        let terminal_size_slider = sliders
            .map(|sliders| {
                settings_slider_row(
                    cx,
                    "Tamaño de la terminal",
                    format!("{terminal_size:.0} px"),
                    &sliders.terminal_font_size,
                )
            })
            .unwrap_or_else(|| div().into_any_element());
        let line_height_slider = sliders
            .map(|sliders| {
                settings_slider_row(
                    cx,
                    "Altura de línea",
                    format!("{line_height:.2}×"),
                    &sliders.line_height,
                )
            })
            .unwrap_or_else(|| div().into_any_element());
        let scrollback_slider = sliders
            .map(|sliders| {
                settings_slider_row(
                    cx,
                    "Historial (terminales nuevas)",
                    format!("{scrollback} líneas"),
                    &sliders.scrollback_lines,
                )
            })
            .unwrap_or_else(|| div().into_any_element());

        let preview = div()
            .flex()
            .flex_col()
            .gap(metrics.hairline())
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .p(metrics.spacing_md())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface_raised)
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .font_family(mono.clone())
                    .text_size(mono_size)
                    .text_color(ui.text)
                    .whitespace_normal()
                    // La muestra se adapta al ancho de la tarjeta: si no cabe,
                    // envuelve en vez de tocar el borde o quedar recortada.
                    .child("❯ axfterm --help"),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .font_family(mono)
                    .text_size(mono_size)
                    .text_color(ui.muted)
                    .whitespace_normal()
                    .child("0123456789 · []{}()<> · áéíóú · ñ"),
            );

        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .w(width)
            .min_w_0()
            .child(
                div()
                    .text_size(metrics.font_size * 0.8125)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Tipografía"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xs())
                    .p(metrics.spacing_md())
                    .rounded(metrics.radius_md())
                    .border_1()
                    .border_color(ui.border)
                    .bg(ui.surface)
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child("Aplicación"),
                    )
                    .child(ui_field)
                    .child(size_slider)
                    .child(scale_slider),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xs())
                    .p(metrics.spacing_md())
                    .rounded(metrics.radius_md())
                    .border_1()
                    .border_color(ui.border)
                    .bg(ui.surface)
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child("Terminal"),
                    )
                    .child(terminal_field)
                    .child(terminal_size_slider)
                    .child(line_height_slider)
                    .child(scrollback_slider)
                    .child(preview),
            )
            .into_any_element()
    }
}

/// Fila de slider con etiqueta, valor y control accesible por teclado.
fn settings_slider_row(
    cx: &mut Context<'_, Shell>,
    label: &'static str,
    value: String,
    state: &Entity<SliderState>,
) -> AnyElement {
    let metrics = theme::metrics(cx);
    let ui = theme::state(cx).theme.ui;
    div()
        .flex()
        .flex_col()
        .gap(metrics.hairline())
        .min_w_0()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(metrics.spacing_xs())
                .child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.text)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child(value),
                ),
        )
        .child(Slider::new(state).accessibility_label(label))
        .into_any_element()
}
