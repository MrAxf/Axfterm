impl Shell {
    fn render_palette(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(palette) = self.palette.as_ref() else {
            return div().into_any_element();
        };
        let input = palette.input.clone();
        let rows: Vec<(usize, PaletteItem, Vec<usize>)> = palette
            .rows
            .iter()
            .enumerate()
            .map(|(row, entry)| {
                let item = palette.item_at(entry).clone();
                (row, item, entry.indices.clone())
            })
            .collect();
        let selected = palette.selected;
        let scroll = palette.scroll.clone();
        let mode = palette.mode;
        let filter = palette.filter;
        let empty = rows.is_empty();
        let list_max_h = metrics
            .palette_list_max_height(window.viewport_size().height, mode == PaletteMode::Search);
        let body = if empty {
            let hint = if mode == PaletteMode::Search && filter.is_some() {
                "Pulsa Ctrl+1 para ver todo o prueba con otra palabra."
            } else {
                "Prueba con otra palabra."
            };
            div()
                .flex()
                .items_center()
                .justify_center()
                .min_h((metrics.font_size * 10.0).min(list_max_h))
                .py(metrics.spacing_lg())
                .child(EmptyState::new(icons::SEARCH, palette.empty_text(), hint).compact())
                .into_any_element()
        } else {
            let mut list = div()
                .id("palette-list")
                .flex()
                .flex_col()
                .gap(metrics.spacing_xs())
                .max_h(list_max_h)
                .track_scroll(&scroll)
                .overflow_y_scroll();
            for (row, item, indices) in rows {
                let active = row == selected;
                let color = if active { ui.accent } else { ui.text };
                let icon = match &item.icon {
                    PaletteIcon::Svg(path) => Icon::new(path)
                        .size(IconSize::Md)
                        .color(color)
                        .into_any_element(),
                    PaletteIcon::Profile(source) => ProfileIcon::new(source.clone())
                        .size(IconSize::Md)
                        .color(color)
                        .into_any_element(),
                };
                let spans: Vec<AnyElement> = palette::highlight(&item.label, &indices)
                    .into_iter()
                    .map(|span| {
                        div()
                            .when(span.matched && active, |this| {
                                this.font_weight(FontWeight::SEMIBOLD)
                            })
                            .when(span.matched && !active, |this| this.text_color(ui.accent))
                            .child(span.text)
                            .into_any_element()
                    })
                    .collect();
                let shortcut = item
                    .shortcut
                    .clone()
                    .map(|shortcut| Kbd::new(shortcut).into_any_element());
                list = list.child(
                    div()
                        .id(("palette-row", row))
                        .flex()
                        .items_center()
                        .gap(metrics.spacing_sm())
                        .flex_shrink_0()
                        .min_h(metrics.control_height_md())
                        .px(metrics.palette_row_padding_x())
                        .py(metrics.palette_row_padding())
                        .rounded(metrics.radius_sm())
                        .when(active, |this| this.bg(ui.selection))
                        .when(!active, move |this| {
                            this.hover(move |this| this.bg(ui.surface_hover))
                                .active(move |this| this.bg(ui.surface_active))
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.palette_click(row, window, cx);
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(metrics.palette_icon_lane())
                                .flex_shrink_0()
                                .child(icon),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(metrics.hairline())
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .text_size(metrics.palette_label_size())
                                        .line_height(metrics.font_size * 1.05)
                                        .children(spans),
                                )
                                .when_some(item.detail.clone(), |this, detail| {
                                    this.child(
                                        div()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .text_size(metrics.palette_detail_size())
                                            .line_height(metrics.font_size * 0.9)
                                            .text_color(if active { ui.text } else { ui.muted })
                                            .child(detail),
                                    )
                                }),
                        )
                        .when(mode == PaletteMode::Search, |this| {
                            this.child(
                                div()
                                    .w(metrics.font_size * 5.5)
                                    .flex_shrink_0()
                                    .text_right()
                                    .text_size(metrics.palette_meta_size())
                                    .text_color(if active { ui.text } else { ui.muted })
                                    .child(item.kind.label()),
                            )
                        })
                        .children(shortcut),
                );
            }
            div()
                .relative()
                .w_full()
                .child(list)
                // Fuera del área desplazable: si colgara del div con
                // `overflow_y_scroll`, el desplazamiento la arrastraría con el
                // contenido.
                .vertical_scrollbar(&scroll)
                .into_any_element()
        };
        let mode_control = SegmentedControl::new("palette-mode")
            .options([
                SegmentOption::new("Comandos").tooltip("Cambiar a comandos (Ctrl+Tab)"),
                SegmentOption::new("Búsqueda").tooltip("Cambiar a búsqueda (Ctrl+Tab)"),
            ])
            .selected(if mode == PaletteMode::Search { 1 } else { 0 })
            .size(ControlSize::Sm)
            .on_select(cx.listener(|shell, index: &usize, window, cx| {
                let mode = if *index == 1 {
                    PaletteMode::Search
                } else {
                    PaletteMode::Commands
                };
                shell.palette_set_mode(mode, window, cx);
            }));
        let mut filter_options = vec![SegmentOption::new("Todo").tooltip("Todo (Ctrl+1)")];
        for (index, kind) in PaletteKind::SEARCH_FILTERS.into_iter().enumerate() {
            filter_options.push(
                SegmentOption::new(kind.filter_label())
                    .tooltip(format!("{} (Ctrl+{})", kind.filter_label(), index + 2)),
            );
        }
        let selected_filter = filter
            .and_then(|filter| {
                PaletteKind::SEARCH_FILTERS
                    .iter()
                    .position(|kind| *kind == filter)
            })
            .map_or(0, |index| index + 1);
        let filter_control = SegmentedControl::new("palette-filter")
            .options(filter_options)
            .selected(selected_filter)
            .size(ControlSize::Sm)
            .on_select(cx.listener(|shell, index: &usize, window, cx| {
                let filter = index
                    .checked_sub(1)
                    .and_then(|index| PaletteKind::SEARCH_FILTERS.get(index).copied());
                shell.palette_set_filter(filter, window, cx);
            }));
        let header = div()
            .p(metrics.spacing_sm())
            .border_b_1()
            .border_color(ui.border)
            .flex()
            .flex_col()
            .gap(metrics.spacing_sm())
            .child(
                div().w_full().min_w_0().child(
                    TextField::new(input)
                        .size(ControlSize::Sm)
                        .prefix_icon(icons::SEARCH)
                        .cleanable(true)
                        .aria_label("Buscar en Axfterm"),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .child(mode_control)
                    .when(mode == PaletteMode::Search, |this| {
                        this.child(filter_control)
                    }),
            );
        let panel = div()
            .w_full()
            .max_w(metrics.palette_width())
            .mx_auto()
            .mt(metrics.titlebar_height() + metrics.spacing_xl())
            .rounded(metrics.radius_lg())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface)
            .shadow_lg()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(header)
            .child(div().p(metrics.spacing_sm()).child(body))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .border_t_1()
                    .border_color(ui.border)
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(metrics.spacing_xs())
                            .child(Kbd::new("↑↓"))
                            .child("navegar")
                            .child(Kbd::new("Enter"))
                            .child("ejecutar"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(metrics.spacing_xs())
                            .child(Kbd::new("Ctrl+Tab"))
                            .child("modo")
                            .when(mode == PaletteMode::Search, |this| {
                                this.child(Kbd::new("Ctrl+1–6")).child("filtrar")
                            })
                            .child(Kbd::new("Esc"))
                            .child("cerrar"),
                    ),
            );
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context(PALETTE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::palette_next))
            .on_action(cx.listener(Self::palette_previous))
            .on_action(cx.listener(Self::palette_accept))
            .on_action(cx.listener(Self::palette_cancel))
            .on_action(cx.listener(Self::palette_switch_mode))
            .on_action(cx.listener(Self::palette_filter_all))
            .on_action(cx.listener(Self::palette_filter_tabs))
            .on_action(cx.listener(Self::palette_filter_panes))
            .on_action(cx.listener(Self::palette_filter_projects))
            .on_action(cx.listener(Self::palette_filter_profiles))
            .on_action(cx.listener(Self::palette_filter_agents))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, window, cx| {
                    shell.close_palette(window, cx);
                }),
            )
            // Aísla la terminal de debajo: sin esto, la rueda y el resto de
            // eventos de ratón seguían llegando al PTY.
            .occlude()
            .bg(ui.overlay)
            .child(panel)
            .into_any_element()
    }

    fn render_shortcuts(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut body = div().flex().flex_col().gap(metrics.spacing_sm());
        for category in crate::actions::ActionCategory::ALL {
            let actions: Vec<&crate::actions::ActionMeta> = crate::actions::all()
                .filter(|action| action.category == *category && action.enabled)
                .collect();
            if actions.is_empty() {
                continue;
            }
            let mut rows = div().flex().flex_col().gap(metrics.hairline()).child(
                div()
                    .pt(metrics.spacing_xs())
                    .text_size(metrics.font_size * 0.6875)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ui.muted)
                    .child(category.label()),
            );
            for action in actions {
                let chord = self
                    .keymap
                    .chords_for(action.id)
                    .unwrap_or_else(|| "—".to_owned());
                rows = rows.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(metrics.spacing_sm())
                        .py(metrics.spacing_xxs())
                        .child(
                            div()
                                .min_w_0()
                                .text_size(metrics.font_size * 0.75)
                                .child(action.label),
                        )
                        .child(Kbd::new(chord)),
                );
            }
            body = body.child(rows);
        }
        body = body.child(
            div()
                .pt(metrics.spacing_sm())
                .text_size(metrics.font_size * 0.6875)
                .text_color(ui.muted)
                .child(
                    "Comandos (Ctrl+Shift+K) ejecuta acciones; búsqueda (Ctrl+P) salta a \
                     pestañas, paneles, proyectos, perfiles y agentes en atención. Dentro de \
                     la paleta, Ctrl+Tab cambia de modo y Ctrl+1–6 filtra por tipo. Los ajustes \
                     están en Ctrl+Alt+T. Cambia los atajos en la sección [keymap] de config.toml.",
                ),
        );
        let footer = div().flex().justify_end().child(
            Button::new("shortcuts-close", "Cerrar")
                .small()
                .ghost()
                .on_click(cx.listener(|shell, _, window, cx| {
                    shell.close_shortcuts(window, cx);
                })),
        );
        let modal = Modal::new("shortcuts-modal", "Atajos de teclado")
            .description("Generado desde el registro de acciones")
            .body(
                div()
                    .max_h(metrics.font_size * 24.0)
                    .overflow_y_scrollbar()
                    .child(body),
            )
            .footer(footer)
            .on_close(cx.listener(|shell, _, window, cx| {
                shell.close_shortcuts(window, cx);
            }))
            .into_any_element();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context(SHORTCUTS_CONTEXT)
            .track_focus(&self.shortcuts_focus)
            .on_action(cx.listener(Self::palette_cancel))
            .child(modal)
            .into_any_element()
    }

    // --- Panel de ajustes: estado auxiliar ------------------------------
    /// Fila enfocable del panel: un único `FocusHandle` persistente por id.
    fn settings_focusable(
        window: &mut Window,
        cx: &mut axf_ui::gpui::App,
        id: impl Into<ElementId>,
    ) -> Stateful<Div> {
        let key: ElementId = id.into();
        let handle = window
            .use_keyed_state(key.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let ui = theme::state(cx).theme.ui;
        div()
            .id(key)
            .min_w_0()
            .track_focus(&handle)
            .border_1()
            .border_color(if handle.is_focused(window) {
                ui.focus
            } else {
                ui.border.opacity(0.0)
            })
    }
}
