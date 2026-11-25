impl Shell {
    fn new_tab_in_pane(&mut self, pane: PaneId, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.new_tab_in_pane_with_profile(pane, None, window, cx);
    }

    fn new_tab_in_pane_with_profile(
        &mut self,
        pane: PaneId,
        profile: Option<ProfileId>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let project = self
            .workspace
            .current_project()
            .or_else(|| self.workspace.projects().first().map(|project| project.id));
        let Some(project) = project else {
            self.prompt_open_project(cx);
            return;
        };
        let (requested, title) = match self.resolve_project_profile(project, profile) {
            Ok(profile) => profile,
            Err(error) => {
                self.set_notice(error, true, cx);
                return;
            }
        };
        match self.workspace.new_tab(pane, project, &requested, &title) {
            Ok(new_tab) => {
                self.workspace.focus_pane(pane).ok();
                self.reveal_project(project);
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn split_pane_action(
        &mut self,
        pane: PaneId,
        axis: SplitAxis,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let project = self
            .workspace
            .current_project()
            .or_else(|| self.workspace.projects().first().map(|project| project.id));
        let Some(project) = project else {
            self.prompt_open_project(cx);
            return;
        };
        let new_pane = match self.workspace.split_pane(pane, axis) {
            Ok(pane) => pane,
            Err(error) => {
                self.set_notice(error.to_string(), true, cx);
                return;
            }
        };
        let (profile, title) = match self.resolve_project_profile(project, None) {
            Ok(profile) => profile,
            Err(error) => {
                self.set_notice(error, true, cx);
                return;
            }
        };
        match self.workspace.new_tab(new_pane, project, &profile, &title) {
            Ok(new_tab) => {
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn render_tab_content(&mut self, tab: TabId, cx: &mut Context<'_, Self>) -> AnyElement {
        let Some(model_tab) = self.workspace.tab(tab).cloned() else {
            return div().into_any_element();
        };
        let metrics = theme::metrics(cx);
        let title = self.tab_title(tab, cx);
        if let Some(error) = self.spawn_errors.get(&model_tab.terminal_id).cloned() {
            let profile_missing = self.theme_host.profile(&model_tab.profile_id).is_none();
            return EmptyState::new(icons::ALERT_TRIANGLE, title, error)
                .action(
                    div()
                        .flex()
                        .gap(metrics.spacing_sm())
                        .child(
                            Button::new(format!("spawn-retry-{}", tab.get()), "Reintentar")
                                .small()
                                .primary()
                                .icon(icons::RESTART)
                                .on_click(cx.listener(move |shell, _, window, cx| {
                                    shell.restart_tab_id(tab, window, cx);
                                })),
                        )
                        .when(profile_missing, |this| {
                            this.child(
                                Button::new(
                                    format!("spawn-default-{}", tab.get()),
                                    "Usar perfil predeterminado",
                                )
                                .small()
                                .icon(icons::RESTART)
                                .on_click(cx.listener(
                                    move |shell, _, window, cx| {
                                        shell.use_default_profile(tab, window, cx);
                                    },
                                )),
                            )
                        })
                        .child(
                            Button::new(format!("spawn-close-{}", tab.get()), "Cerrar pestaña")
                                .small()
                                .ghost()
                                .on_click(cx.listener(move |shell, _, _, cx| {
                                    shell.close_tab_id(tab, cx);
                                })),
                        ),
                )
                .into_any_element();
        }
        if let Some(entry) = self.terminals.get(&model_tab.terminal_id) {
            if let Some(error) = entry.view.read(cx).error().map(str::to_owned) {
                return EmptyState::new(icons::ALERT_TRIANGLE, title, error)
                    .action(
                        div().flex().gap(metrics.spacing_sm()).child(
                            Button::new(format!("terminal-retry-{}", tab.get()), "Reintentar")
                                .small()
                                .primary()
                                .icon(icons::RESTART)
                                .on_click(cx.listener(move |shell, _, window, cx| {
                                    shell.restart_tab_id(tab, window, cx);
                                })),
                        ),
                    )
                    .into_any_element();
            }
            return div()
                .size_full()
                .child(entry.view.clone())
                .into_any_element();
        }
        if self.theme_host.profile(&model_tab.profile_id).is_none() {
            return EmptyState::new(
                icons::ALERT_TRIANGLE,
                title,
                format!(
                    "El perfil `{}` ya no existe en config.toml. Elige el perfil predeterminado o \
                     cierra la pestaña; no se arranca otro shell por sorpresa.",
                    model_tab.profile_id
                ),
            )
            .action(
                div()
                    .flex()
                    .gap(metrics.spacing_sm())
                    .child(
                        Button::new(
                            format!("missing-profile-default-{}", tab.get()),
                            "Usar perfil predeterminado",
                        )
                        .small()
                        .primary()
                        .icon(icons::RESTART)
                        .on_click(cx.listener(
                            move |shell, _, window, cx| {
                                shell.use_default_profile(tab, window, cx);
                            },
                        )),
                    )
                    .child(
                        Button::new(
                            format!("missing-profile-close-{}", tab.get()),
                            "Cerrar pestaña",
                        )
                        .small()
                        .ghost()
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.close_tab_id(tab, cx);
                        })),
                    ),
            )
            .into_any_element();
        }
        if self.pending_terminals.contains(&model_tab.terminal_id) {
            return EmptyState::new(
                icons::RESTART,
                title,
                "Restaurando la terminal y su historial…",
            )
            .into_any_element();
        }
        let missing = self.project_missing(model_tab.project_id);
        if !missing {
            return EmptyState::new(
                icons::RESTART,
                title,
                "Pestaña pendiente de restaurar. Se abrirá al activarla.",
            )
            .action(
                div()
                    .flex()
                    .gap(metrics.spacing_sm())
                    .child(
                        Button::new(format!("restore-now-{}", tab.get()), "Restaurar ahora")
                            .small()
                            .primary()
                            .icon(icons::RESTART)
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.ensure_terminal(tab, cx);
                            })),
                    )
                    .child(
                        Button::new(format!("restore-close-{}", tab.get()), "Cerrar pestaña")
                            .small()
                            .ghost()
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.close_tab_id(tab, cx);
                            })),
                    ),
            )
            .into_any_element();
        }
        let detail = self
            .project_missing_message(model_tab.project_id)
            .unwrap_or_else(|| "La carpeta del proyecto no existe.".into());
        EmptyState::new(icons::ALERT_TRIANGLE, title, detail)
        .action(
            div()
                .flex()
                .gap(metrics.spacing_sm())
                .child(
                    Button::new(
                        format!("restore-restart-{}", tab.get()),
                        "Reiniciar terminal",
                    )
                    .small()
                    .primary()
                    .icon(icons::RESTART)
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.restart_tab_id(tab, window, cx);
                    })),
                )
                .child(
                    Button::new(format!("restore-close-{}", tab.get()), "Cerrar pestaña")
                        .small()
                        .ghost()
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.close_tab_id(tab, cx);
                        })),
                ),
        )
        .into_any_element()
    }

    fn reload_themes_impl(&mut self, cx: &mut Context<'_, Self>) {
        let report = self.theme_host.reload(
            axf_config::WatchEvent {
                config: true,
                themes: true,
            },
            cx,
        );
        let applied = report.applied;
        let has_error = !report.errors.is_empty();
        let mut parts = report.errors;
        parts.extend(report.messages);
        let had = !parts.is_empty();
        if had {
            self.set_notice(parts.join(" "), has_error, cx);
        }
        if applied && !had {
            self.set_notice("Configuración y temas recargados.", false, cx);
        }
    }

    fn open_themes_folder_impl(&mut self, cx: &mut Context<'_, Self>) {
        let Some(dir) = self.theme_host.themes_dir.clone() else {
            self.set_notice("No se pudo resolver el directorio de temas.", true, cx);
            return;
        };
        if let Err(error) = std::fs::create_dir_all(&dir) {
            self.set_notice(
                format!("No se pudo crear {}: {error}", dir.display()),
                true,
                cx,
            );
            return;
        }
        cx.open_with_system(&dir);
    }

    fn import_theme_impl(&mut self, cx: &mut Context<'_, Self>) {
        let receiver = cx.prompt_for_paths(axf_ui::gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Importar tema TOML".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.first().cloned()
            {
                let result = this.update(cx, |shell, cx| {
                    let result = shell.theme_host.import_theme(&path, cx);
                    match result {
                        Ok(destination) => shell.set_notice(
                            format!("Tema importado en {}", destination.display()),
                            false,
                            cx,
                        ),
                        Err(error) => shell.set_notice(error, true, cx),
                    }
                });
                result.ok();
            }
        })
        .detach();
    }

    fn export_theme_impl(&mut self, cx: &mut Context<'_, Self>) {
        let directory = self.theme_host.config_dir.clone();
        let suggested = format!("{}.toml", sanitize_file_name(&self.theme_host.selected));
        let receiver = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = receiver.await {
                let result = this.update(cx, |shell, cx| {
                    match shell.theme_host.export_theme(&path, cx) {
                        Ok(path) => shell.set_notice(
                            format!("Tema exportado en {}", path.display()),
                            false,
                            cx,
                        ),
                        Err(error) => shell.set_notice(error, true, cx),
                    }
                });
                result.ok();
            }
        })
        .detach();
    }

    fn render_toast(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        if self.notices.is_empty() {
            return None;
        }
        let metrics = theme::metrics(cx);
        let toasts = self
            .notices
            .iter()
            .map(|notice| {
                let toast = if notice.error {
                    Toast::error(notice.text.clone())
                } else {
                    Toast::new(notice.text.clone())
                };
                match &notice.detail {
                    Some(detail) => toast.detail(detail.clone()),
                    None => toast,
                }
            })
            .collect::<Vec<_>>();
        Some(
            div()
                .absolute()
                .bottom(metrics.spacing_xl())
                .right(metrics.spacing_xl())
                .flex()
                .flex_col()
                .gap(metrics.spacing_sm())
                .children(toasts)
                .into_any_element(),
        )
    }
}
