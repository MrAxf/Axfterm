impl Shell {
    /// Barra de título propia.
    ///
    /// La marca y la franja central son zonas de arrastre nativas; el toggle
    /// de la sidebar queda fuera de ellas para que su clic no mueva la
    /// ventana. Los controles usan `WindowControlArea`, así que minimizar,
    /// maximizar, cerrar, el doble clic y los snap layouts siguen siendo los
    /// del sistema.
    fn render_title_bar(&mut self, window: &Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let sidebar = self.workspace.sidebar();
        let sidebar_visible = sidebar.visible;
        let sidebar_width = sidebar.width;
        let title_label = self.workspace.focus().tab.and_then(|tab_id| {
            let project_id = self.workspace.tab(tab_id)?.project_id;
            let project = self.workspace.project(project_id)?;
            let terminal = self.tab_title(tab_id, cx);
            Some(format!("{}/{}", project.name, terminal))
        });
        let title = title_bar_title_geometry(
            window.viewport_size().width,
            px(sidebar_width),
            window_controls_width(metrics),
            metrics.titlebar_title_max(),
            metrics.titlebar_padding(),
        );

        // La marca vive en `crates/ui/assets/brand` y se tiñe con el tema. El
        // .ico que embebe el ejecutable comparte el mismo dibujo (M9).
        let brand = div()
            .id("title-brand")
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(metrics.titlebar_gap())
            .h_full()
            .window_control_area(WindowControlArea::Drag)
            // El arrastre llega como caption nativo; sin esto, el texto podría
            // iniciar una selección que sigue al puntero tras soltar.
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                axf_ui::gpui::base::GlobalState::suppress_text_selection(cx);
            })
            .child(
                Icon::new(icons::AXFTERM)
                    .size(IconSize::Custom(metrics.titlebar_icon()))
                    .color(ui.accent),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(metrics.titlebar_font_size())
                    .text_color(ui.text)
                    .child(APP_NAME),
            );
        let toggle = IconButton::new("title-sidebar", icons::SIDEBAR)
            .size_px(metrics.titlebar_button())
            .icon_size(IconSize::Custom(metrics.titlebar_button_icon()))
            .tooltip(if sidebar_visible {
                "Ocultar barra lateral (Ctrl+B)"
            } else {
                "Mostrar barra lateral (Ctrl+B)"
            })
            .on_click(cx.listener(|shell, _, _, cx| {
                let visible = shell.workspace.sidebar().visible;
                shell.set_sidebar_visible(!visible, cx);
            }));

        // Entrada provisional a ajustes: hasta M7 abre el modal de tema y
        // fuentes. Va pegado al toggle y dentro de la columna de la sidebar,
        // así que no invade la separación con el área central.
        let settings = IconButton::new("title-settings", icons::SETTINGS)
            .size_px(metrics.titlebar_button())
            .icon_size(IconSize::Custom(metrics.titlebar_button_icon()))
            .tooltip(format!("Ajustes ({})", keys::format_chord(keys::SETTINGS)))
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_settings(window, cx);
            }));

        // La sidebar y el toggle comparten siempre el mismo borde: si la
        // sidebar se oculta, el icono no se mueve de sitio.
        let left = div()
            .flex()
            .items_center()
            .gap(metrics.spacing_xs())
            .h_full()
            .flex_shrink_0()
            .px(metrics.titlebar_padding())
            .w(px(sidebar_width))
            .child(brand)
            .child(settings)
            .child(toggle);
        let drag = div()
            .id("title-drag")
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .h_full()
            .min_w_0()
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                axf_ui::gpui::base::GlobalState::suppress_text_selection(cx);
            })
            // El título se centra en la ventana, no en el hueco: la franja de
            // arrastre compensa la diferencia entre la columna de la sidebar
            // y los controles de ventana.
            .when(title.padding_left > px(0.0), |this| {
                this.pl(title.padding_left)
            })
            .when(title.padding_right > px(0.0), |this| {
                this.pr(title.padding_right)
            })
            .when_some(title_label, |this, label| {
                this.child(
                    div()
                        .min_w_0()
                        .max_w(title.max_width)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(metrics.titlebar_font_size())
                        .text_color(ui.muted)
                        .child(label),
                )
            });
        let controls = window_controls(metrics, ui, window.is_maximized());
        div()
            .flex()
            .items_center()
            .h(metrics.titlebar_height())
            .w_full()
            .flex_shrink_0()
            .bg(ui.surface)
            .border_b_1()
            .border_color(ui.border)
            .child(left)
            .child(drag)
            .child(controls)
            .into_any_element()
    }

    fn render_onboarding(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let metrics = theme::metrics(cx);
        let empty_project = self.workspace.projects().is_empty();
        let message = if empty_project {
            "Sin proyectos todavía"
        } else {
            "Sin terminales abiertas"
        };
        let detail = if empty_project {
            "Registra una carpeta para agrupar sus terminales en la sidebar."
        } else {
            "Abre una terminal en el proyecto actual o registra otra carpeta."
        };
        let open_this = cx.entity().downgrade();
        EmptyState::new(icons::FOLDER, message, detail)
            .action(
                div()
                    .flex()
                    .gap(metrics.spacing_sm())
                    .child(
                        DropdownButton::new("onboarding-open")
                            .primary()
                            .button(
                                KitButton::new("onboarding-open-local")
                                    .label("Abrir carpeta…")
                                    .on_click(cx.listener(|shell, _, window, cx| {
                                        shell.open_project_path_dialog(window, cx);
                                    })),
                            )
                            .dropdown_menu_with_anchor(
                                Anchor::BottomLeft,
                                move |menu, _, _| {
                                    let local = open_this.clone();
                                    let wsl = open_this.clone();
                                    menu.item(
                                        PopupMenuItem::new("Carpeta local…").on_click(
                                            move |_, window, cx| {
                                                if let Some(this) = local.upgrade() {
                                                    let _ = this.update(cx, |shell, cx| {
                                                        shell.open_project_path_dialog(
                                                            window, cx,
                                                        );
                                                    });
                                                }
                                            },
                                        ),
                                    )
                                    .item(
                                        PopupMenuItem::new("Proyecto en WSL…").on_click(
                                            move |_, window, cx| {
                                                if let Some(this) = wsl.upgrade() {
                                                    let _ = this.update(cx, |shell, cx| {
                                                        shell.open_wsl_modal(window, cx);
                                                    });
                                                }
                                            },
                                        ),
                                    )
                                },
                            )
                            .menu_accessibility_label("Más formas de abrir un proyecto"),
                    )
                    .when(!empty_project, |this| {
                        this.child(
                            Button::new("onboarding-new", "Nueva terminal")
                                .icon(icons::PLUS)
                                .on_click(cx.listener(|shell, _, window, cx| {
                                    shell.new_tab_for(None, None, window, cx);
                                })),
                        )
                    }),
            )
            .into_any_element()
    }

    fn open_project_path_dialog(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        self.prompt_open_project(cx);
    }

    fn render_layout(
        &mut self,
        node: &PaneTree,
        path: &[Branch],
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        match node {
            PaneTree::Leaf(pane) => self.render_pane(*pane, cx),
            PaneTree::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let axis = *axis;
                let ratio = *ratio;
                let key = split_key(path);
                let mut first_path = path.to_vec();
                first_path.push(Branch::First);
                let mut second_path = path.to_vec();
                second_path.push(Branch::Second);
                let first_el = self.render_layout(first, &first_path, cx);
                let second_el = self.render_layout(second, &second_path, cx);
                let bounds = Rc::clone(&self.split_bounds);
                let prepaint_key = key.clone();
                let divider_key = key.clone();
                let divider_path = path.to_vec();
                let first_wrap = div()
                    .map(|this| match axis {
                        SplitAxis::Horizontal => this.w(relative(ratio)).flex_shrink_0(),
                        SplitAxis::Vertical => this.h(relative(ratio)).flex_shrink_0(),
                    })
                    .min_w_0()
                    .min_h_0()
                    .child(first_el);
                let second_wrap = div().flex_1().min_w_0().min_h_0().child(second_el);
                let splitter = UiMetrics::new(cx.theme().font_size).splitter();
                let divider = div()
                    .id(format!("split-{divider_key}"))
                    .flex_shrink_0()
                    .map(|this| match axis {
                        SplitAxis::Horizontal => this.w(splitter).h_full(),
                        SplitAxis::Vertical => this.h(splitter).w_full(),
                    })
                    .bg(cx.theme().border)
                    .hover(|this| this.bg(cx.theme().primary))
                    .cursor_pointer()
                    .on_drag(
                        SplitDrag {
                            path: divider_path.clone(),
                            axis,
                            key: divider_key.clone(),
                        },
                        |_, _, _, cx| cx.new(|_| DragGhost::new("Redimensionar")),
                    )
                    .on_drag_move(cx.listener(
                        move |shell, event: &axf_ui::gpui::DragMoveEvent<SplitDrag>, _, cx| {
                            let bounds = shell
                                .split_bounds
                                .borrow()
                                .get(&event.drag(cx).key)
                                .copied();
                            if let Some(bounds) = bounds {
                                let ratio = match event.drag(cx).axis {
                                    SplitAxis::Horizontal => {
                                        let width: f32 = bounds.size.width.into();
                                        if width > 0.0 {
                                            (event.event.position.x - bounds.left()) / px(width)
                                        } else {
                                            0.5
                                        }
                                    }
                                    SplitAxis::Vertical => {
                                        let height: f32 = bounds.size.height.into();
                                        if height > 0.0 {
                                            (event.event.position.y - bounds.top()) / px(height)
                                        } else {
                                            0.5
                                        }
                                    }
                                };
                                if shell.workspace.resize_split(&event.drag(cx).path, ratio) {
                                    cx.notify();
                                }
                            }
                        },
                    ))
                    .on_click(cx.listener({
                        let path = path.to_vec();
                        move |shell, event: &ClickEvent, _, cx| {
                            if event.click_count() == 2 && shell.workspace.resize_split(&path, 0.5)
                            {
                                shell.persist(cx);
                                cx.notify();
                            }
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|shell, _, _, cx| shell.persist(cx)),
                    );
                let container = div()
                    .flex()
                    .size_full()
                    .map(|this| match axis {
                        SplitAxis::Horizontal => this.flex_row(),
                        SplitAxis::Vertical => this.flex_col(),
                    })
                    .on_prepaint(move |bounds_value, _, _| {
                        bounds
                            .borrow_mut()
                            .insert(prepaint_key.clone(), bounds_value);
                    })
                    .child(first_wrap)
                    .child(divider)
                    .child(second_wrap);
                container.into_any_element()
            }
        }
    }

    fn sidebar_rows(&self) -> Vec<SidebarRow> {
        sidebar_rows_from_groups(&sidebar_view(&self.workspace, self.sidebar_section))
    }
}
