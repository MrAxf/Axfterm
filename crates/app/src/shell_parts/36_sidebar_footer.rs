/// Número de sesiones de agente con proceso vivo en todo el workspace.
fn active_agent_count(workspace: &Workspace) -> usize {
    workspace
        .projects()
        .iter()
        .flat_map(|project| workspace.project_tabs(project.id))
        .filter_map(|tab| workspace.tab(tab))
        .flat_map(|tab| tab.agents.iter())
        .filter(|session| session.alive)
        .count()
}

/// Resumen corto del workspace y su versión larga para el tooltip.
fn sidebar_status_summary(projects: usize, terminals: usize, agents: usize) -> (String, String) {
    let short = format!("{projects} proy · {terminals} term · {agents} ag");
    let projects_word = if projects == 1 {
        "proyecto"
    } else {
        "proyectos"
    };
    let terminals_word = if terminals == 1 {
        "terminal"
    } else {
        "terminales"
    };
    let agents_word = if agents == 1 {
        "agente activo"
    } else {
        "agentes activos"
    };
    let full = format!(
        "{projects} {projects_word}, {terminals} {terminals_word} y {agents} {agents_word}"
    );
    (short, full)
}

impl Shell {
    /// Pie de la sidebar: accesos de agente y estado básico del workspace.
    ///
    /// Los accesos son icon-buttons para que quepan más agentes sin rediseñar
    /// la fila; el tooltip conserva la etiqueta, el atajo y el estado en el
    /// proyecto. Debajo, una línea discreta resume versión, proyectos,
    /// terminales y agentes vivos.
    fn render_sidebar_footer(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let theme_state = theme::state(cx);
        let ui = theme_state.theme.ui;
        let metrics = theme_state.metrics;
        let project = self.agent_launch_project();
        let mut actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(metrics.spacing_xs());
        for kind in AgentKind::ALL {
            let mut tooltip = format!(
                "Abrir {} en una pestaña nueva ({})",
                kind.label(),
                keys::format_chord(agent_shortcut(kind))
            );
            if let Some(state) = project.and_then(|project| self.project_agent_state(project, kind))
            {
                tooltip.push_str(&format!("\nEstado en este proyecto: {}", state.label()));
            }
            actions = actions.child(
                IconButton::new(format!("sidebar-agent-{}", kind.as_str()), agent_icon(kind))
                    .size(ControlSize::Sm)
                    .icon_size(IconSize::Md)
                    .secondary()
                    .aria_label(format!("Abrir {}", kind.label()))
                    .tooltip(tooltip)
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.launch_agent(kind, None, window, cx);
                    })),
            );
        }
        let projects = self.workspace.projects().len();
        let terminals = self.workspace.tab_count();
        let agents = active_agent_count(&self.workspace);
        let (short, full) = sidebar_status_summary(projects, terminals, agents);
        let info = div()
            .id("sidebar-footer-info")
            .flex()
            .items_center()
            .justify_between()
            .gap(metrics.spacing_xs())
            .text_size(metrics.font_size * 0.6875)
            .text_color(ui.subtle)
            .tooltip(text_tooltip(format!(
                "Axfterm v{}\n{full}",
                env!("CARGO_PKG_VERSION")
            )))
            .child(
                div()
                    .flex_shrink_0()
                    .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(short),
            );
        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_xs())
            .p(metrics.spacing_sm())
            .border_t_1()
            .border_color(ui.border)
            .child(actions)
            .child(info)
            .into_any_element()
    }
}
