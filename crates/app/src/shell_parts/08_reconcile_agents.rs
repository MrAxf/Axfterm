fn reconcile_agents(
    sessions: &mut Vec<AgentSession>,
    detected: &[DetectedAgent],
    running: bool,
    exit: Option<axf_terminal::ExitInfo>,
    recent_output: bool,
    now: SystemTime,
    next_id: &mut AgentSessionId,
) -> (bool, AgentTransitions) {
    let before = sessions.clone();
    let mut transitions = Vec::new();
    if !running {
        let success = exit.is_some_and(|info| info.success);
        for session in sessions.iter_mut() {
            let transition = session.shell_exited(success, now);
            if transition.notify {
                transitions.push((session.id, transition));
            }
        }
        return (before != *sessions, transitions);
    }
    for session in sessions.iter_mut() {
        session.alive = false;
    }
    if detected.is_empty() {
        sessions.retain(|session| session.pid != 0);
    } else if let Some(agent) = detected.first() {
        sessions.truncate(1);
        if let Some(session) = sessions.first_mut() {
            if session.kind != agent.kind {
                let id = *next_id;
                *next_id = next_id.wrapping_add(1);
                let mut replacement = AgentSession::new(id, agent.kind, agent.pid, now);
                replacement.alive = true;
                if recent_output {
                    replacement.state = AgentState::Working;
                }
                *session = replacement;
            } else {
                session.pid = agent.pid;
                session.alive = true;
            }
        } else {
            let id = *next_id;
            *next_id = next_id.wrapping_add(1);
            let mut session = AgentSession::new(id, agent.kind, agent.pid, now);
            session.alive = true;
            if recent_output {
                session.state = AgentState::Working;
            }
            sessions.push(session);
        }
    }
    for session in sessions.iter_mut() {
        if session.pid == 0 {
            continue;
        }
        let transition = if !session.alive {
            session.process_gone(None, now)
        } else if !session.reported {
            if recent_output {
                session.apply_output_activity(now)
            } else {
                session.apply_quiescence(now)
            }
        } else {
            continue;
        };
        if transition.notify {
            transitions.push((session.id, transition));
        }
    }
    (before != *sessions, transitions)
}
fn agent_report_from_status(status: &ProgramStatus) -> AgentReport {
    let state = match status.state {
        ProgramState::Idle => AgentState::Idle,
        ProgramState::Working => AgentState::Working,
        ProgramState::Blocked => AgentState::NeedsAttention,
        ProgramState::Done => AgentState::Finished,
        ProgramState::Error => AgentState::Error,
        ProgramState::Clear => AgentState::Idle,
    };
    let mut report = if status.state == ProgramState::Clear {
        AgentReport::clear()
    } else {
        AgentReport::new(state)
    };
    report.attention = status
        .kind
        .as_deref()
        .and_then(AttentionKind::from_protocol);
    report.progress = status.progress;
    report.app = status.app.clone();
    report.title = status.title.clone();
    report.message = status.message.clone();
    report.id = status.id.clone();
    report
}
fn terminal_style(cx: &mut Context<'_, Shell>) -> TerminalStyle {
    let state = theme::state(cx);
    TerminalStyle {
        font_family: state.fonts.mono.clone(),
        fallbacks: state.fonts.mono_fallbacks.clone(),
        font_size: state.mono_font_size,
        line_height: (state.terminal_line_height > px(0.0)).then_some(state.terminal_line_height),
        palette: state.theme.terminal,
        background: state.theme.ui.background,
        // El borde del panel lo dibuja el shell según el foco; la terminal solo
        // usa este color para el flash de campana.
        border: None,
        // Todo tamaño deriva del tamaño base de la fuente (AGENTS.md).
        padding: state.metrics.panel_padding(),
    }
}
fn tab_status_from_state(state: AgentState) -> TabStatus {
    match state {
        AgentState::NeedsAttention => TabStatus::Attention,
        AgentState::Working => TabStatus::Working,
        AgentState::Error => TabStatus::Error,
        AgentState::Finished => TabStatus::Finished,
        AgentState::Idle => TabStatus::Idle,
        AgentState::Stopped => TabStatus::Stopped,
    }
}
fn agent_icon(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::OpenCode => icons::BRAND_OPENCODE,
        AgentKind::Copilot => icons::BRAND_COPILOT,
    }
}
fn most_urgent_agent_state(states: impl IntoIterator<Item = AgentState>) -> Option<AgentState> {
    states.into_iter().max_by_key(|state| state.priority())
}
fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {:02}m", seconds / 3_600, (seconds % 3_600) / 60)
    }
}
fn terminal_tab_icon(program: Option<&str>) -> &'static str {
    let Some(program) = program else {
        return icons::TERMINAL;
    };
    let name = program_stem(program).to_ascii_lowercase();
    match name.as_str() {
        "pwsh" | "powershell" => icons::BRAND_POWERSHELL,
        _ => icons::TERMINAL,
    }
}
fn program_stem(program: &str) -> &str {
    std::path::Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
}
fn title_is_shell_path(title: &str, program: Option<&str>) -> bool {
    let Some(program) = program else {
        return false;
    };
    let segment = title
        .trim()
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .trim();
    program_stem(segment).eq_ignore_ascii_case(program_stem(program))
}
fn resolve_tab_title(
    tab: &axf_workspace::Tab,
    live_title: Option<&str>,
    program: Option<&str>,
    project_name: Option<&str>,
) -> String {
    if tab.custom_title && !tab.title.is_empty() {
        return tab.title.clone();
    }
    if let Some(title) = live_title
        && !title.trim().is_empty()
        && !title_is_shell_path(title, program)
    {
        return title.to_owned();
    }
    if !tab.auto_title.is_empty() {
        return tab.auto_title.clone();
    }
    if !tab.title.is_empty() {
        return tab.title.clone();
    }
    project_name.unwrap_or("Terminal").to_owned()
}
fn sanitize_file_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "tema".into()
    } else {
        trimmed.to_string()
    }
}
fn tab_scroll_offset(current: Pixels, max: Pixels, delta: Pixels) -> Pixels {
    (current + delta).min(px(0.0)).max(-max)
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct TitleBarTitle {
    max_width: Pixels,
    padding_left: Pixels,
    padding_right: Pixels,
}
fn title_bar_title_geometry(
    viewport_width: Pixels,
    sidebar_width: Pixels,
    controls_width: Pixels,
    max_width: Pixels,
    padding: Pixels,
) -> TitleBarTitle {
    let free_width = viewport_width - sidebar_width - controls_width;
    let center = viewport_width * 0.5;
    let left_room = center - sidebar_width - padding;
    let right_room = viewport_width - controls_width - center - padding;
    if left_room > px(0.0) && right_room > px(0.0) {
        let room = left_room.min(right_room);
        let shift = sidebar_width - controls_width;
        let max_width = (room * 2.0).min(max_width);
        if shift >= px(0.0) {
            TitleBarTitle {
                max_width,
                padding_left: px(0.0),
                padding_right: shift,
            }
        } else {
            TitleBarTitle {
                max_width,
                padding_left: -shift,
                padding_right: px(0.0),
            }
        }
    } else {
        TitleBarTitle {
            max_width: (free_width - padding * 2.0).max(px(0.0)).min(max_width),
            padding_left: px(0.0),
            padding_right: px(0.0),
        }
    }
}
fn window_controls_width(metrics: UiMetrics) -> Pixels {
    #[cfg(target_os = "windows")]
    {
        metrics.titlebar_control_width() * 3.0
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = metrics;
        px(0.0)
    }
}
fn split_key(path: &[Branch]) -> String {
    let mut key = String::with_capacity(path.len());
    for branch in path {
        key.push(match branch {
            Branch::First => '0',
            Branch::Second => '1',
        });
    }
    key
}
fn window_controls(metrics: UiMetrics, ui: theme::UiTokens, maximized: bool) -> AnyElement {
    #[cfg(target_os = "windows")]
    {
        let maximize = if maximized {
            // `copy` (dos cuadrados superpuestos) lee como restaurar.
            icons::DUPLICATE
        } else {
            icons::WINDOW_MAXIMIZE
        };
        div()
            .flex()
            .items_center()
            .h_full()
            .flex_shrink_0()
            .child(window_control(
                metrics,
                ui,
                "title-minimize",
                icons::WINDOW_MINIMIZE,
                WindowControlArea::Min,
                false,
            ))
            .child(window_control(
                metrics,
                ui,
                "title-maximize",
                maximize,
                WindowControlArea::Max,
                false,
            ))
            .child(window_control(
                metrics,
                ui,
                "title-close",
                icons::CLOSE,
                WindowControlArea::Close,
                true,
            ))
            .into_any_element()
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (metrics, ui, maximized);
        div().flex_shrink_0().into_any_element()
    }
}
#[cfg(target_os = "windows")]
fn window_control(
    metrics: UiMetrics,
    ui: theme::UiTokens,
    id: &'static str,
    icon: &'static str,
    area: WindowControlArea,
    destructive: bool,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(metrics.titlebar_control_width())
        .h_full()
        .flex_shrink_0()
        .window_control_area(area)
        .hover(move |this| {
            if destructive {
                // Tinte de peligro sin cambiar el icono: el SVG no hereda el
                // color del padre, y así funciona igual en temas claros y
                // oscuros.
                this.bg(ui.danger.opacity(0.2))
            } else {
                this.bg(ui.surface_raised)
            }
        })
        .child(
            svg()
                .path(icon)
                .size(metrics.titlebar_control_icon())
                .text_color(ui.muted),
        )
        .into_any_element()
}
