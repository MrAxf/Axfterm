/// Estado visual de una acción de fila.
#[derive(Clone, Copy)]
struct RowActionStyle {
    enabled: bool,
    danger: bool,
}

impl RowActionStyle {
    const NORMAL: Self = Self {
        enabled: true,
        danger: false,
    };
    const DANGER: Self = Self {
        enabled: true,
        danger: true,
    };
    const DISABLED: Self = Self {
        enabled: false,
        danger: false,
    };
}

impl Shell {
    /// Tarjetas de perfiles con sus acciones dentro de cada fila.
    fn render_profile_rows(
        &mut self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mono = state.fonts.mono.clone();
        let profiles = self
            .settings
            .as_ref()
            .map(|settings| settings.profiles().to_vec())
            .unwrap_or_default();
        let default_profile = self
            .settings
            .as_ref()
            .map(|settings| settings.draft.terminal.default_profile.clone());
        let icons: Vec<ProfileIconSource> = profiles
            .iter()
            .map(|profile| self.theme_host.profile_icon(&profile.id))
            .collect();
        let count = profiles.len();
        let mut list = div()
            .flex()
            .flex_col()
            .w(width)
            .max_w(width)
            .min_w_0()
            .gap(metrics.spacing_sm());
        for (index, profile) in profiles.iter().enumerate() {
            let id = profile.id.clone();
            let is_default = default_profile.as_ref() == Some(&profile.id);
            let shell_entity = cx.entity();
            let edit_id = id.clone();
            let duplicate_id = id.clone();
            let up_id = id.clone();
            let down_id = id.clone();
            let default_id = id.clone();
            let delete_id = id.clone();
            let actions = div()
                .flex()
                .items_center()
                .gap(metrics.spacing_xxs())
                .flex_shrink_0()
                .child(Self::profile_action(
                    window,
                    cx,
                    format!("settings-profile-edit-{id}"),
                    icons::RENAME,
                    "Editar perfil",
                    RowActionStyle::NORMAL,
                    {
                        let shell = shell_entity.clone();
                        move |window: &mut Window, cx: &mut App| {
                            shell.update(cx, |shell, cx| {
                                shell.open_profile_editor(edit_id.clone(), window, cx);
                            });
                        }
                    },
                ))
                .child(Self::profile_action(
                    window,
                    cx,
                    format!("settings-profile-duplicate-{id}"),
                    icons::DUPLICATE,
                    "Duplicar perfil",
                    RowActionStyle::NORMAL,
                    {
                        let shell = shell_entity.clone();
                        move |_window: &mut Window, cx: &mut App| {
                            shell.update(cx, |shell, cx| {
                                let result = shell
                                    .settings
                                    .as_mut()
                                    .map(|settings| settings.duplicate_profile(&duplicate_id));
                                if let Some(Err(error)) = result {
                                    shell.set_notice(error, true, cx);
                                }
                                cx.notify();
                            });
                        }
                    },
                ))
                .child(Self::profile_action(
                    window,
                    cx,
                    format!("settings-profile-up-{id}"),
                    icons::CHEVRON_UP,
                    "Subir",
                    if index > 0 { RowActionStyle::NORMAL } else { RowActionStyle::DISABLED },
                    {
                        let shell = shell_entity.clone();
                        move |_window: &mut Window, cx: &mut App| {
                            shell.update(cx, |shell, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.move_profile(&up_id, -1);
                                }
                                cx.notify();
                            });
                        }
                    },
                ))
                .child(Self::profile_action(
                    window,
                    cx,
                    format!("settings-profile-down-{id}"),
                    icons::CHEVRON_DOWN,
                    "Bajar",
                    if index + 1 < count {
                        RowActionStyle::NORMAL
                    } else {
                        RowActionStyle::DISABLED
                    },
                    {
                        let shell = shell_entity.clone();
                        move |_window: &mut Window, cx: &mut App| {
                            shell.update(cx, |shell, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.move_profile(&down_id, 1);
                                }
                                cx.notify();
                            });
                        }
                    },
                ))
                .child(Self::profile_action(
                    window,
                    cx,
                    format!("settings-profile-default-{id}"),
                    icons::CHECK,
                    "Usar como predeterminado",
                    if is_default {
                        RowActionStyle::DISABLED
                    } else {
                        RowActionStyle::NORMAL
                    },
                    {
                        let shell = shell_entity.clone();
                        move |_window: &mut Window, cx: &mut App| {
                            shell.update(cx, |shell, cx| {
                                if let Some(settings) = &mut shell.settings
                                    && let Err(error) = settings.set_default_profile(&default_id)
                                {
                                    settings.set_notice(error, true);
                                }
                                cx.notify();
                            });
                        }
                    },
                ))
                .child(Self::profile_action(
                    window,
                    cx,
                    format!("settings-profile-delete-{id}"),
                    icons::CLOSE,
                    "Eliminar perfil",
                    RowActionStyle::DANGER,
                    {
                        let shell = shell_entity.clone();
                        move |_window: &mut Window, cx: &mut App| {
                            shell.update(cx, |shell, cx| {
                                shell.request_delete_profile(&delete_id, cx);
                            });
                        }
                    },
                ));
            let row = div()
                .flex()
                .items_center()
                .flex_wrap()
                .gap(metrics.spacing_sm())
                .p(metrics.spacing_md())
                .rounded(metrics.radius_md())
                .border_1()
                .border_color(if is_default { ui.accent } else { ui.border })
                .bg(ui.surface)
                .w(width)
                .max_w(width)
                .min_w_0()
                .child(
                    ProfileIcon::new(icons[index].clone())
                        .size(IconSize::Sm)
                        .color(ui.text),
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
                                .gap(metrics.spacing_xs())
                                .min_w_0()
                                .child(
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(metrics.font_size * 0.8125)
                                        .child(profile.name.clone()),
                                )
                                .when(is_default, |this| {
                                    this.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(metrics.spacing_xxs())
                                            .px(metrics.spacing_xs())
                                            .rounded(metrics.radius_sm())
                                            .bg(ui.accent.opacity(0.16))
                                            .text_size(metrics.font_size * 0.625)
                                            .text_color(ui.accent)
                                            .child(
                                                Icon::new(icons::CHECK)
                                                    .size(IconSize::Xs)
                                                    .color(ui.accent),
                                            )
                                            .child("Predeterminado"),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .font_family(mono.clone())
                                .text_size(metrics.font_size * 0.6875)
                                .text_color(ui.muted)
                                .child(profile.command.clone()),
                        ),
                )
                .child(actions);
            list = list.child(row);
        }
        list.into_any_element()
    }

    /// Acción de icono dentro de una fila, alcanzable con Tab y Enter.
    fn profile_action(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        id: impl Into<ElementId>,
        icon: &'static str,
        tooltip: &'static str,
        style: RowActionStyle,
        activate: impl Fn(&mut Window, &mut App) + 'static,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let color = if !style.enabled {
            ui.muted
        } else if style.danger {
            ui.danger
        } else {
            ui.text
        };
        let hover = if style.danger {
            axf_ui::components::Tone::Danger.hover(ui)
        } else {
            ui.surface_hover
        };
        let pressed = if style.danger {
            axf_ui::components::Tone::Danger.active(ui)
        } else {
            ui.surface_active
        };
        let activate = Rc::new(activate);
        let key: ElementId = id.into();
        let mut element = if style.enabled {
            Self::settings_focusable(window, cx, key)
        } else {
            div().id(key)
        }
        .flex()
        .items_center()
        .justify_center()
        .size(metrics.control_height_sm())
        .rounded(metrics.radius_sm())
        .tooltip(text_tooltip(tooltip))
        .child(Icon::new(icon).size(IconSize::Md).color(color));
        if style.enabled {
            let click = activate.clone();
            let key = activate;
            element = element
                .cursor_pointer()
                .hover(move |this| this.bg(hover))
                .active(move |this| this.bg(pressed))
                .on_click(move |_, window, cx| click(window, cx))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        key(window, cx);
                        cx.stop_propagation();
                    }
                });
        }
        element.into_any_element()
    }

    /// Botones para añadir perfiles o detectarlos.
    fn render_profile_toolbar(&mut self, width: Pixels, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let metrics = state.metrics;
        let running = matches!(
            self.settings.as_ref().map(|settings| &settings.detect),
            Some(DetectState::Running)
        );
        div()
            .flex()
            .flex_wrap()
            .w(width)
            .max_w(width)
            .min_w_0()
            .gap(metrics.spacing_xs())
            .child(with_hand(
                KitButton::new("settings-profile-add")
                    .small()
                    .primary()
                    .label("Nuevo perfil")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.open_new_profile(window, cx);
                    })),
            ))
            .child(with_hand(
                KitButton::new("settings-profile-detect")
                    .small()
                    .ghost()
                    .label("Detectar terminales")
                    .disabled(running)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.start_detect(cx);
                    })),
            ))
            .into_any_element()
    }

    /// Resultado del descubrimiento local de terminales.
    fn render_profile_detect(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut detect = div().flex().flex_col().gap(metrics.spacing_xs());
        if let Some(settings) = self.settings.as_ref() {
            match &settings.detect {
                DetectState::Running => {
                    detect = detect.child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Buscando terminales en PATH…"),
                    );
                }
                DetectState::Ready(found) => {
                    let mut found_list = div()
                        .flex()
                        .flex_col()
                        .gap(metrics.hairline())
                        .p(metrics.spacing_sm())
                        .rounded(metrics.radius_sm())
                        .border_1()
                        .border_color(ui.border)
                        .bg(ui.surface);
                    for profile in found {
                        found_list = found_list.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(metrics.spacing_sm())
                                .text_size(metrics.font_size * 0.75)
                                .child(
                                    Icon::new(icons::TERMINAL)
                                        .size(IconSize::Xs)
                                        .color(ui.muted),
                                )
                                .child(profile.name.clone())
                                .child(
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(metrics.font_size * 0.6875)
                                        .text_color(ui.muted)
                                        .child(profile.command.clone()),
                                ),
                        );
                    }
                    detect = detect.child(found_list).child(
                        div()
                            .flex()
                            .gap(metrics.spacing_xs())
                            .child(with_hand(
                                KitButton::new("settings-profile-detect-add")
                                    .small()
                                    .primary()
                                    .label(format!("Añadir {}", found.len()))
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        shell.merge_detected(cx);
                                    })),
                            ))
                            .child(with_hand(
                                KitButton::new("settings-profile-detect-discard")
                                    .small()
                                    .ghost()
                                    .label("Descartar")
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        if let Some(settings) = &mut shell.settings {
                                            settings.detect = DetectState::Idle;
                                        }
                                        cx.notify();
                                    })),
                            )),
                    );
                }
                DetectState::Idle => {}
            }
        }
        detect.into_any_element()
    }
}
