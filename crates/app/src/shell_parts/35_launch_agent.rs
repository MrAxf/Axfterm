/// Atajo con el que se abre un agente en una pestaña nueva.
fn agent_shortcut(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::OpenCode => keys::OPEN_OPENCODE,
        AgentKind::Copilot => keys::OPEN_COPILOT,
    }
}

/// Perfil resuelto para abrir un agente, con el aviso de por qué no se usó el
/// preferido cuando aplica.
struct AgentLaunchPlan {
    profile: ProfileId,
    name: String,
    warning: Option<String>,
}

/// Comprueba que un perfil puede arrancar en el host de un proyecto.
///
/// Para un proyecto local se exige un perfil que no abra WSL: si no, el agente
/// arrancaría fuera de la carpeta del proyecto. Para WSL la distro debe
/// coincidir. El perfil del proyecto elegido a mano no pasa por aquí; es una
/// decisión explícita que el spawn valida con sus propios mensajes.
fn profile_fits_project_host(profile: &Profile, host: &ProjectHost) -> Result<(), String> {
    match host {
        ProjectHost::Local => match profile.wsl_distro() {
            Some(distro) => Err(format!(
                "El perfil preferido «{}» arranca en {distro} y el proyecto es local; \
                 no abriría en su carpeta.",
                profile.name
            )),
            None => Ok(()),
        },
        ProjectHost::Wsl { distro } => match profile.wsl_distro() {
            Some(profile_distro) if &profile_distro == distro => Ok(()),
            Some(profile_distro) => Err(format!(
                "El perfil preferido «{}» arranca en {profile_distro}, pero el proyecto \
                 vive en {distro}.",
                profile.name
            )),
            None => Err(format!(
                "El perfil preferido «{}» no es de WSL, pero el proyecto vive en {distro}.",
                profile.name
            )),
        },
    }
}

/// Elige el perfil de un lanzamiento: preferido, proyecto y global.
///
/// `fits` valida existencia y host para el preferido; `exists` solo la
/// existencia para los perfiles que el usuario eligió en el proyecto o en la
/// configuración global. Un preferido inválido cae al siguiente perfil con un
/// motivo; un perfil de proyecto inexistente es un error.
fn choose_agent_profile(
    preferred: Option<&ProfileId>,
    project_profile: Option<&ProfileId>,
    global_profile: &ProfileId,
    fits: &impl Fn(&ProfileId) -> Result<(), String>,
    exists: &impl Fn(&ProfileId) -> Result<(), String>,
) -> Result<(ProfileId, Option<String>), String> {
    if let Some(preferred) = preferred {
        match fits(preferred) {
            Ok(()) => return Ok((preferred.clone(), None)),
            Err(reason) => {
                let fallback = project_profile
                    .cloned()
                    .unwrap_or_else(|| global_profile.clone());
                if &fallback == preferred {
                    return Err(reason);
                }
                exists(&fallback)?;
                return Ok((fallback, Some(reason)));
            }
        }
    }
    let fallback = project_profile
        .cloned()
        .unwrap_or_else(|| global_profile.clone());
    exists(&fallback)?;
    Ok((fallback, None))
}

impl Shell {
    fn open_opencode(&mut self, _: &crate::OpenOpenCode, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.launch_agent(AgentKind::OpenCode, None, window, cx);
    }

    fn open_copilot(&mut self, _: &crate::OpenCopilot, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.launch_agent(AgentKind::Copilot, None, window, cx);
    }

    /// Proyecto de destino: el de la pestaña enfocada, el cursor de la sidebar
    /// o el primero, con el mismo orden que «Nueva terminal».
    fn agent_launch_project(&self) -> Option<ProjectId> {
        self.workspace
            .current_project()
            .or_else(|| self.sidebar_cursor_project())
            .or_else(|| self.workspace.projects().first().map(|project| project.id))
    }

    /// Estado más urgente de un agente dentro de un proyecto, para el tooltip
    /// del pie. Es informativo: no cambia lo que hace el clic.
    fn project_agent_state(&self, project: ProjectId, kind: AgentKind) -> Option<AgentState> {
        most_urgent_agent_state(
            self.workspace
                .project_tabs(project)
                .into_iter()
                .filter_map(|tab| self.workspace.tab(tab))
                .flat_map(|tab| tab.agents.iter())
                .filter(|session| session.kind == kind)
                .map(|session| session.state),
        )
    }

    fn plan_agent_launch(
        &self,
        project: ProjectId,
        kind: AgentKind,
    ) -> Result<AgentLaunchPlan, String> {
        let Some(model_project) = self.workspace.project(project) else {
            return Err("El proyecto ya no existe.".into());
        };
        let preferred = self
            .theme_host
            .settings
            .agents
            .preferred_profiles
            .get(kind.as_str());
        let project_profile = model_project.default_profile.as_ref();
        let global_profile = &self.theme_host.settings.terminal.default_profile;
        let fits = |id: &ProfileId| -> Result<(), String> {
            let profile = self
                .theme_host
                .profile(id)
                .ok_or_else(|| format!("El perfil preferido `{id}` no existe en config.toml."))?;
            profile_fits_project_host(profile, &model_project.host)
        };
        let exists = |id: &ProfileId| -> Result<(), String> {
            self.theme_host
                .profile(id)
                .map(|_| ())
                .ok_or_else(|| format!("El perfil `{id}` no existe en config.toml."))
        };
        let (profile, warning) = choose_agent_profile(
            preferred,
            project_profile,
            global_profile,
            &fits,
            &exists,
        )?;
        let name = self
            .theme_host
            .profile_name(&profile)
            .ok_or_else(|| format!("El perfil `{profile}` no existe en config.toml."))?;
        Ok(AgentLaunchPlan {
            profile,
            name,
            warning,
        })
    }

    /// Abre una pestaña nueva con el perfil resuelto y escribe el comando del
    /// agente cuando el PTY está listo.
    fn launch_agent(
        &mut self,
        kind: AgentKind,
        pane: Option<PaneId>,
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
        let Some(project) = self.agent_launch_project() else {
            self.set_notice("Abre un proyecto antes de lanzar un agente.", false, cx);
            self.prompt_open_project(cx);
            return;
        };
        let pane = pane.unwrap_or_else(|| self.workspace.focus().pane);
        if self.workspace.pane(pane).is_none() {
            self.set_notice("El panel ya no existe.", true, cx);
            return;
        }
        let plan = match self.plan_agent_launch(project, kind) {
            Ok(plan) => plan,
            Err(error) => {
                self.set_notice(error, true, cx);
                return;
            }
        };
        if let Some(warning) = plan.warning {
            self.set_notice(format!("{warning} Se usa «{}»", plan.name), false, cx);
        }
        match self
            .workspace
            .new_tab(pane, project, &plan.profile, &plan.name)
        {
            Ok(new_tab) => {
                self.reveal_project(project);
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.queue_agent_launch_command(new_tab.tab, kind, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    /// Envía el comando del agente al shell de la pestaña recién arrancada.
    ///
    /// Se espera un instante para que el shell publique su prompt, igual que la
    /// reanudación. Si el spawn falló no hay terminal y la vista de error ya es
    /// visible; si el shell muere antes, `send_command` no escribe nada.
    fn queue_agent_launch_command(
        &mut self,
        tab: TabId,
        kind: AgentKind,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(model_tab) = self.workspace.tab(tab) else {
            return;
        };
        let Some(entry) = self.terminals.get(&model_tab.terminal_id) else {
            return;
        };
        let command = agent_launch_command(
            self.theme_host
                .profile(&model_tab.profile_id)
                .and_then(|profile| profile.agent_command(kind.as_str())),
            kind,
        );
        let view = entry.view.clone();
        cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let _ = view.update(cx, |view, _| view.send_command(&command));
        })
        .detach();
    }
}

#[cfg(test)]
mod launch_tests {
    use super::*;
    use std::collections::HashSet;

    fn profile(id: &str, command: &str) -> Profile {
        Profile::new(id, id.to_uppercase(), command)
    }

    fn check_fits(profile: &Profile, host: ProjectHost) -> Result<(), String> {
        profile_fits_project_host(profile, &host)
    }

    #[test]
    fn local_projects_reject_wsl_preferences_and_accept_local_ones() {
        let local = profile("pwsh", "pwsh");
        let wsl = profile("ubuntu", r#"wsl -d "Ubuntu""#);
        assert!(check_fits(&local, ProjectHost::Local).is_ok());
        let error = check_fits(&wsl, ProjectHost::Local).expect_err("WSL no abre local");
        assert!(error.contains("proyecto es local"));
    }

    #[test]
    fn wsl_projects_require_the_same_distro() {
        let host = ProjectHost::wsl("Ubuntu");
        let same = profile("ubuntu", r#"wsl -d "Ubuntu""#);
        let other = profile("debian", r#"wsl -d "Debian""#);
        let local = profile("pwsh", "pwsh");
        assert!(check_fits(&same, host.clone()).is_ok());
        assert!(check_fits(&other, host.clone()).is_err());
        assert!(check_fits(&local, host).is_err());
    }

    #[test]
    fn preferred_profile_wins_only_when_it_fits() {
        let preferred = ProfileId::new("preferred");
        let project = ProfileId::new("project");
        let global = ProfileId::new("global");
        let fits = |_: &ProfileId| Ok(());
        let exists = |_: &ProfileId| Ok(());
        let (chosen, warning) = choose_agent_profile(
            Some(&preferred),
            Some(&project),
            &global,
            &fits,
            &exists,
        )
        .expect("elige preferido");
        assert_eq!(chosen, preferred);
        assert!(warning.is_none());
    }

    #[test]
    fn invalid_preference_falls_back_and_explains_why() {
        let preferred = ProfileId::new("preferred");
        let project = ProfileId::new("project");
        let global = ProfileId::new("global");
        let fits = |id: &ProfileId| {
            (id != &preferred)
                .then_some(())
                .ok_or_else(|| "no encaja".to_owned())
        };
        let exists = |_: &ProfileId| Ok(());
        let (chosen, warning) = choose_agent_profile(
            Some(&preferred),
            Some(&project),
            &global,
            &fits,
            &exists,
        )
        .expect("cae al proyecto");
        assert_eq!(chosen, project);
        assert_eq!(warning.as_deref(), Some("no encaja"));
    }

    #[test]
    fn fallback_uses_global_and_rejects_a_missing_project_profile() {
        let preferred = ProfileId::new("preferred");
        let project = ProfileId::new("project");
        let global = ProfileId::new("global");
        let fits = |_: &ProfileId| Err("no encaja".to_owned());
        let available: HashSet<ProfileId> = [preferred.clone(), global.clone()].into();
        let exists = |id: &ProfileId| {
            available
                .contains(id)
                .then_some(())
                .ok_or_else(|| "no existe".to_owned())
        };
        let (chosen, warning) =
            choose_agent_profile(Some(&preferred), None, &global, &fits, &exists)
                .expect("cae al global");
        assert_eq!(chosen, global);
        assert!(warning.is_some());

        let error = choose_agent_profile(Some(&preferred), Some(&project), &global, &fits, &exists)
            .expect_err("perfil de proyecto inexistente");
        assert!(error.contains("no existe"));

        let (chosen, warning) = choose_agent_profile(None, None, &global, &fits, &exists)
            .expect("sin preferido usa el global");
        assert_eq!(chosen, global);
        assert!(warning.is_none());
    }

    #[test]
    fn default_profile_wins_without_a_preference() {
        let project = ProfileId::new("project");
        let global = ProfileId::new("global");
        let fits = |_: &ProfileId| Ok(());
        let exists = |_: &ProfileId| Ok(());
        let (chosen, warning) = choose_agent_profile(None, Some(&project), &global, &fits, &exists)
            .expect("usa el del proyecto");
        assert_eq!(chosen, project);
        assert!(warning.is_none());
    }
}
