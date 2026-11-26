// Fixture de Studio con proyectos registrados y sin terminales.

impl Shell {
    /// Proyectos registrados sin terminales: la bienvenida con sus dos acciones.
    fn studio_seed_projects(&mut self, cx: &mut Context<'_, Self>) -> Result<(), String> {
        let root = self
            .studio
            .as_ref()
            .map(|studio| {
                studio
                    .shots_dir
                    .parent()
                    .unwrap_or(&studio.shots_dir)
                    .join("projects-empty")
            })
            .unwrap_or_else(|| PathBuf::from("target/studio/projects-empty"));
        let profile = self.theme_host.settings.terminal.default_profile.clone();
        let Some(title) = self.profile_title(&profile) else {
            return Err("no hay un perfil por defecto válido para el fixture".into());
        };
        let mut tabs = Vec::new();
        for name in ["atlas", "lumen"] {
            let directory = root.join(name);
            std::fs::create_dir_all(&directory)
                .map_err(|error| format!("no se pudo crear {}: {error}", directory.display()))?;
            let readme = directory.join("README.md");
            if !readme.exists() {
                let _ = std::fs::write(
                    &readme,
                    format!("# {name}\n\nProyecto de demostración para Axfterm Studio.\n"),
                );
            }
            let opened = self
                .workspace
                .open_project(&directory, &profile, &title)
                .map_err(|error| error.to_string())?;
            if let Some(tab) = opened.tab {
                tabs.push(tab.tab);
            }
        }
        for tab in tabs {
            let _ = self.workspace.close_tab(tab);
        }
        if let Some(studio) = self.studio.as_mut() {
            studio.fixture = Some("projects".into());
            studio.agents_frozen = false;
        }
        self.set_sidebar_section(SidebarSection::All, cx);
        cx.notify();
        Ok(())
    }
}

