use crate::wsl;
use crate::wsl_open::{
    NameList, WslProfileChoice, expand_posix, join_posix, name_list, parent_posix,
};
use axf_ui::gpui::component::IndexPath;
use axf_ui::gpui::component::list::{List, ListEvent};

impl Shell {
    /// Abre el modal de proyectos WSL.
    pub(crate) fn open_wsl_modal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.wsl_open.is_some() {
            if let Some(state) = self.wsl_open.as_ref() {
                window.focus(&state.focus, cx);
            }
            cx.notify();
            return;
        }
        if self.settings.is_some() {
            self.set_notice(
                "Cierra los ajustes antes de abrir un proyecto en WSL.",
                false,
                cx,
            );
            return;
        }
        self.close_palette(window, cx);
        self.close_shortcuts(window, cx);
        let state = WslOpenState::new(window, cx);
        let list = state.list.clone();
        let list_subscription = cx.subscribe_in(
            &list,
            window,
            |shell, _, event: &ListEvent, window, cx| match event {
                ListEvent::Confirm(index) => {
                    let secondary = shell
                        .wsl_open
                        .as_ref()
                        .is_some_and(|state| state.list.read(cx).delegate().secondary_confirm());
                    if secondary {
                        shell.open_wsl_current(window, cx);
                    } else {
                        shell.wsl_enter_index(*index, window, cx);
                    }
                }
                ListEvent::Cancel => shell.close_wsl_modal(window, cx),
                ListEvent::Select(_) => {}
            },
        );
        let path = state.path_input.clone();
        let path_subscription = cx.subscribe_in(
            &path,
            window,
            |shell, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    shell.wsl_navigate_from_input(window, cx);
                    cx.stop_propagation();
                }
            },
        );
        let mut state = state;
        state.subscriptions.push(list_subscription);
        state.subscriptions.push(path_subscription);
        let focus = state.path_input.read(cx).focus_handle(cx);
        self.wsl_open = Some(state);
        window.focus(&focus, cx);
        self.spawn_wsl_distros(cx);
        cx.notify();
    }

    pub(crate) fn spawn_wsl_distros(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(state) = self.wsl_open.as_mut() {
            state.loading_distros = true;
            state.error = None;
        }
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async { wsl::distros() })
                .await;
            let _ = this.update_in(cx, |shell, window, cx| {
                shell.apply_wsl_distros(result, window, cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn apply_wsl_distros(
        &mut self,
        result: Result<Vec<String>, wsl::WslError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let next = {
            let Some(state) = self.wsl_open.as_mut() else {
                return;
            };
            state.loading_distros = false;
            match result {
                Err(error) => {
                    state.distros.clear();
                    state.error = Some(error.to_string());
                    return;
                }
                Ok(distros) if distros.is_empty() => {
                    state.distros.clear();
                    state.error = Some(
                        "No hay distribuciones WSL instaladas. Instala una con `wsl --install`."
                            .into(),
                    );
                    return;
                }
                Ok(distros) => {
                    state.distros = distros;
                    let items: Vec<(String, String)> = state
                        .distros
                        .iter()
                        .map(|distro| (distro.clone(), distro.clone()))
                        .collect();
                    let (list, selected) = name_list(&items, state.distro.as_deref());
                    let select = cx.new(|cx| SelectState::new(list, selected, window, cx));
                    let subscription = cx.subscribe_in(
                        &select,
                        window,
                        |shell, _, event: &SelectEvent<NameList>, window, cx| {
                            if let SelectEvent::Confirm(Some(distro)) = event {
                                shell.select_wsl_distro(distro.clone(), window, cx);
                            }
                        },
                    );
                    state.distro_select = Some(select);
                    state.subscriptions.push(subscription);
                    state
                        .distro
                        .clone()
                        .or_else(|| state.distros.first().cloned())
                }
            }
        };
        if let Some(distro) = next {
            self.select_wsl_distro(distro, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn select_wsl_distro(
        &mut self,
        distro: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let profiles = self.wsl_profiles_for(&distro);
        let items: Vec<(String, String)> = profiles
            .iter()
            .map(|profile| (profile.id.to_string(), profile.name.clone()))
            .collect();
        let current = profiles.first().map(|profile| profile.id.as_str());
        let (list, selected) = name_list(&items, current);
        let select = cx.new(|cx| SelectState::new(list, selected, window, cx));
        let subscription = cx.subscribe_in(
            &select,
            window,
            |shell, _, event: &SelectEvent<NameList>, _, cx| {
                if let SelectEvent::Confirm(Some(profile)) = event
                    && let Some(state) = shell.wsl_open.as_mut()
                {
                    state.profile_id = Some(axf_config::ProfileId::new(profile.clone()));
                    cx.notify();
                }
            },
        );
        let profile_id = profiles.first().map(|profile| profile.id.clone());
        let profile_error = profiles.is_empty().then(|| {
            format!(
                "No hay un perfil WSL para `{distro}`. Añade uno con `wsl -d \"{distro}\"` \
                 en Ajustes → Perfiles."
            )
        });
        let path_input = {
            let Some(state) = self.wsl_open.as_mut() else {
                return;
            };
            state.distro = Some(distro.clone());
            state.profiles = profiles;
            state.profile_id = profile_id;
            state.profile_select = Some(select);
            state.profile_error = profile_error;
            state.path.clear();
            state.home = None;
            state.generation = state.generation.wrapping_add(1);
            state.loading_dirs = true;
            state.error = None;
            state.subscriptions.push(subscription);
            state.path_input.clone()
        };
        path_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.load_wsl_home(distro, window, cx);
        cx.notify();
    }

    fn load_wsl_home(
        &mut self,
        distro: String,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let generation = self
            .wsl_open
            .as_ref()
            .map_or(0, |state| state.generation);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { wsl::home_directory(&distro) })
                .await;
            let _ = this.update_in(cx, |shell, window, cx| {
                let Some(state) = shell.wsl_open.as_mut() else {
                    return;
                };
                if state.generation != generation {
                    return;
                }
                match result {
                    Ok(home) => {
                        state.home = Some(home.clone());
                        shell.navigate_wsl(home, window, cx);
                    }
                    Err(error) => {
                        state.loading_dirs = false;
                        state.error = Some(error.to_string());
                        let message = error.to_string();
                        state
                            .list
                            .update(cx, |list, _| list.delegate_mut().set_error(message));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn navigate_wsl(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(state) = self.wsl_open.as_ref() else {
            return;
        };
        let Some(distro) = state.distro.clone() else {
            return;
        };
        let expanded = expand_posix(&path, &state.path, state.home.as_deref());
        let generation = state.generation.wrapping_add(1);
        let input = state.path_input.clone();
        let list = state.list.clone();
        if let Some(state) = self.wsl_open.as_mut() {
            state.path = expanded.clone();
            state.generation = generation;
            state.loading_dirs = true;
            state.error = None;
        }
        input.update(cx, |input, cx| {
            input.set_value(expanded.clone(), window, cx);
        });
        list.update(cx, |list, _| list.delegate_mut().set_loading(true));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { wsl::list_directories(&distro, &expanded) })
                .await;
            let _ = this.update_in(cx, |shell, window, cx| {
                shell.apply_wsl_directories(generation, result, window, cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn apply_wsl_directories(
        &mut self,
        generation: u64,
        result: Result<Vec<String>, wsl::WslError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(state) = self.wsl_open.as_mut() else {
            return;
        };
        if state.generation != generation {
            return;
        }
        state.loading_dirs = false;
        match result {
            Ok(entries) => {
                state.error = None;
                let first = (!entries.is_empty()).then(|| IndexPath::new(0));
                state.list.update(cx, |list, cx| {
                    list.delegate_mut().set_entries(entries);
                    list.set_selected_index(first, window, cx);
                });
                let handle = state.list.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
            }
            Err(error) => {
                state.error = Some(error.to_string());
                let message = error.to_string();
                state
                    .list
                    .update(cx, |list, _| list.delegate_mut().set_error(message));
            }
        }
        cx.notify();
    }

    fn wsl_enter_index(
        &mut self,
        index: IndexPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(state) = self.wsl_open.as_ref() else {
            return;
        };
        let Some(name) = state
            .list
            .read(cx)
            .delegate()
            .entry(index.row)
            .map(str::to_owned)
        else {
            return;
        };
        let path = join_posix(&state.path, &name);
        self.navigate_wsl(path, window, cx);
    }

    fn wsl_navigate_from_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(state) = self.wsl_open.as_ref() else {
            return;
        };
        let value = state.path_input.read(cx).value().trim().to_owned();
        self.navigate_wsl(value, window, cx);
    }


    pub(crate) fn open_wsl_current(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(state) = self.wsl_open.as_ref() else {
            return;
        };
        if state.opening || state.loading_distros {
            return;
        }
        let Some(distro) = state.distro.clone() else {
            self.set_wsl_error("Elige una distribución WSL.".into(), cx);
            return;
        };
        let Some(profile) = state.profile_id.clone() else {
            self.set_wsl_error(
                "No hay un perfil WSL válido para esta distribución.".into(),
                cx,
            );
            return;
        };
        let Some(title) = self.theme_host.profile_name(&profile) else {
            self.set_wsl_error(
                format!("El perfil `{profile}` no existe en config.toml."),
                cx,
            );
            return;
        };
        let input = state.path_input.read(cx).value().trim().to_owned();
        let path = expand_posix(&input, &state.path, state.home.as_deref());
        if let Some(state) = self.wsl_open.as_mut() {
            state.opening = true;
            state.error = None;
        }
        let probe_distro = distro.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { wsl::probe_directory(&probe_distro, &path) })
                .await;
            let _ = this.update_in(cx, |shell, window, cx| {
                shell.finish_open_wsl(result, distro, profile, title, window, cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_open_wsl(
        &mut self,
        result: Result<String, wsl::WslError>,
        distro: String,
        profile: axf_config::ProfileId,
        title: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(state) = self.wsl_open.as_mut() {
            state.opening = false;
        }
        let canonical = match result {
            Ok(canonical) => canonical,
            Err(error) => {
                self.set_wsl_error(error.to_string(), cx);
                return;
            }
        };
        let opened = match self.workspace.open_project_with_host(
            PathBuf::from(&canonical),
            ProjectHost::wsl(distro.clone()),
            &profile,
            &title,
        ) {
            Ok(opened) => opened,
            Err(error) => {
                self.set_wsl_error(error.to_string(), cx);
                return;
            }
        };
        self.project_health
            .insert(opened.project, ProjectHealth::Ready);
        self.wsl_open = None;
        if let Some(tab) = opened.tab {
            self.spawn_terminal(tab.tab, tab.terminal, window, cx);
        }
        self.reveal_project(opened.project);
        self.persist(cx);
        self.restore_workspace_focus(window, cx);
        cx.notify();
    }

    fn set_wsl_error(&mut self, message: String, cx: &mut Context<'_, Self>) {
        if let Some(state) = self.wsl_open.as_mut() {
            state.opening = false;
            state.error = Some(message);
        }
        cx.notify();
    }

}
