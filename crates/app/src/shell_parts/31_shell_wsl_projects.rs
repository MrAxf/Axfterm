impl Shell {
    fn wsl_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let keystroke = &event.keystroke;
        let key = keystroke.key.to_ascii_lowercase();
        if key == "escape" {
            self.close_wsl_modal(window, cx);
            cx.stop_propagation();
            return;
        }
        if keystroke.modifiers.control && key == "l" {
            if let Some(state) = self.wsl_open.as_ref() {
                let input = state.path_input.clone();
                let handle = input.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
                input.update(cx, |input, cx| input.select_all(window, cx));
            }
            cx.stop_propagation();
            return;
        }
        if keystroke.modifiers.control && key == "enter" {
            self.open_wsl_current(window, cx);
            cx.stop_propagation();
            return;
        }
        if keystroke.modifiers.alt && key == "up" {
            let path = self
                .wsl_open
                .as_ref()
                .map(|state| parent_posix(&state.path))
                .unwrap_or_else(|| "/".to_owned());
            self.navigate_wsl(path, window, cx);
            cx.stop_propagation();
            return;
        }
        if keystroke.modifiers.alt && key == "home" {
            let home = self
                .wsl_open
                .as_ref()
                .and_then(|state| state.home.clone());
            if let Some(home) = home {
                self.navigate_wsl(home, window, cx);
            }
            cx.stop_propagation();
        }
    }

    pub(crate) fn close_wsl_modal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.wsl_open.take().is_some() {
            self.restore_workspace_focus(window, cx);
            cx.notify();
        }
    }

    /// Fixture de Studio para revisar el modal sin depender de WSL.
    #[cfg(feature = "studio")]
    pub(crate) fn studio_wsl_fixture(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        let mut state = WslOpenState::new(window, cx);
        let distros = vec!["Ubuntu".to_owned(), "Debian".to_owned()];
        let items: Vec<(String, String)> = distros
            .iter()
            .map(|distro| (distro.clone(), distro.clone()))
            .collect();
        let (distro_list, distro_selected) = name_list(&items, Some("Ubuntu"));
        state.distro_select = Some(cx.new(|cx| {
            SelectState::new(distro_list, distro_selected, window, cx)
        }));
        state.distros = distros;
        state.distro = Some("Ubuntu".into());

        let profiles = vec![WslProfileChoice {
            id: axf_config::ProfileId::new("wsl-ubuntu"),
            name: "Ubuntu (WSL)".into(),
        }];
        let items: Vec<(String, String)> = profiles
            .iter()
            .map(|profile| (profile.id.to_string(), profile.name.clone()))
            .collect();
        let (profile_list, profile_selected) = name_list(&items, Some("wsl-ubuntu"));
        state.profile_select = Some(cx.new(|cx| {
            SelectState::new(profile_list, profile_selected, window, cx)
        }));
        state.profiles = profiles;
        state.profile_id = Some(axf_config::ProfileId::new("wsl-ubuntu"));
        state.home = Some("/home/juan".into());
        state.path = "/home/juan/codigo/axfterm".into();
        state.loading_distros = false;
        state.loading_dirs = false;
        let path = state.path.clone();
        let input = state.path_input.clone();
        input.update(cx, |input, cx| {
            input.set_value(path, window, cx);
        });
        state.list.update(cx, |list, cx| {
            list.delegate_mut().set_entries(vec![
                "app".into(),
                "docs".into(),
                "scripts".into(),
                "target".into(),
            ]);
            list.set_selected_index(Some(IndexPath::new(0)), window, cx);
        });
        let focus = state.list.read(cx).focus_handle(cx);
        self.wsl_open = Some(state);
        window.focus(&focus, cx);
        cx.notify();
        Ok(())
    }

    /// Fixture de Studio: abre un proyecto WSL real sin pasar por el modal.
    #[cfg(feature = "studio")]
    pub(crate) fn studio_open_wsl_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), String> {
        let distro = "Ubuntu";
        let path = "/home";
        let profile = self
            .wsl_profiles_for(distro)
            .into_iter()
            .next()
            .map(|profile| profile.id)
            .ok_or_else(|| format!("no hay perfil WSL para {distro}"))?;
        let title = self
            .theme_host
            .profile_name(&profile)
            .ok_or_else(|| format!("el perfil {profile} no existe"))?;
        let opened = self
            .workspace
            .open_project_with_host(
                PathBuf::from(path),
                ProjectHost::wsl(distro),
                &profile,
                &title,
            )
            .map_err(|error| error.to_string())?;
        self.project_health
            .insert(opened.project, ProjectHealth::Ready);
        if let Some(tab) = opened.tab {
            self.spawn_terminal(tab.tab, tab.terminal, window, cx);
        }
        self.reveal_project(opened.project);
        self.persist(cx);
        cx.notify();
        Ok(())
    }

    fn wsl_profiles_for(&self, distro: &str) -> Vec<WslProfileChoice> {
        self.theme_host
            .profiles()
            .iter()
            .filter(|profile| profile.wsl_distro().as_deref() == Some(distro))
            .map(|profile| WslProfileChoice {
                id: profile.id.clone(),
                name: profile.name.clone(),
            })
            .collect()
    }

    /// Perfil por defecto de un proyecto, validado contra su host.
    pub(crate) fn resolve_project_profile(
        &self,
        project: ProjectId,
        requested: Option<ProfileId>,
    ) -> Result<(ProfileId, String), String> {
        let Some(project) = self.workspace.project(project) else {
            return Err("El proyecto no existe.".into());
        };
        let candidate = requested
            .or_else(|| project.default_profile.clone())
            .or_else(|| self.theme_host.default_profile().map(|profile| profile.id.clone()))
            .ok_or_else(|| "No hay un perfil de terminal disponible.".to_owned())?;
        let profile = self
            .theme_host
            .profile(&candidate)
            .ok_or_else(|| format!("El perfil `{candidate}` no existe en config.toml."))?;
        if let Some(distro) = project.host.wsl_distro() {
            match profile.wsl_distro() {
                Some(profile_distro) if profile_distro == distro => {}
                Some(profile_distro) => {
                    return Err(format!(
                        "El perfil `{}` arranca en {profile_distro}, pero el proyecto vive en {distro}.",
                        profile.name
                    ));
                }
                None => {
                    return Err(format!(
                        "El perfil `{}` no es de WSL; este proyecto necesita `wsl -d \"{distro}\"`.",
                        profile.name
                    ));
                }
            }
        }
        Ok((candidate, profile.name.clone()))
    }

    fn project_target(&self, project: ProjectId) -> Option<SpawnTarget> {
        self.workspace.project(project).map(|project| SpawnTarget {
            host: project.host.clone(),
            path: project.root.clone(),
        })
    }

    pub(crate) fn spawn_project_health_checks(&mut self, cx: &mut Context<'_, Self>) {
        let projects: Vec<(ProjectId, String, String)> = self
            .workspace
            .projects()
            .iter()
            .filter_map(|project| {
                project.host.wsl_distro().map(|distro| {
                    (
                        project.id,
                        distro.to_owned(),
                        project.root.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect();
        for (project, distro, path) in projects {
            self.project_health
                .insert(project, ProjectHealth::Checking);
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { wsl::probe_directory(&distro, &path) })
                    .await;
                let _ = this.update(cx, |shell, cx| {
                    if shell.workspace.project(project).is_none() {
                        return;
                    }
                    let health = match result {
                        Ok(_) => ProjectHealth::Ready,
                        Err(error) => ProjectHealth::Missing(error.to_string()),
                    };
                    shell.project_health.insert(project, health);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub(crate) fn project_missing(&self, project: ProjectId) -> bool {
        self.project_missing_message(project).is_some()
    }

    pub(crate) fn project_missing_message(&self, project: ProjectId) -> Option<String> {
        let project = self.workspace.project(project)?;
        match &project.host {
            ProjectHost::Local => (!project.root.is_dir())
                .then(|| format!("La carpeta del proyecto no existe: {}", project.root.display())),
            ProjectHost::Wsl { .. } => self
                .project_health
                .get(&project.id)
                .and_then(ProjectHealth::missing)
                .map(str::to_owned),
        }
    }
}
