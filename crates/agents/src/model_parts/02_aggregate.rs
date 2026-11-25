pub fn aggregate(sessions: &[AgentSession]) -> Option<AgentAggregate> {
    let dominant = sessions
        .iter()
        .max_by(|left, right| {
            left.state
                .priority()
                .cmp(&right.state.priority())
                .then(left.last_activity.cmp(&right.last_activity))
                .then(left.id.cmp(&right.id))
        })?
        .clone();
    let mut breakdown = Vec::new();
    for state in [
        AgentState::NeedsAttention,
        AgentState::Working,
        AgentState::Error,
        AgentState::Finished,
        AgentState::Idle,
        AgentState::Stopped,
    ] {
        let count = sessions
            .iter()
            .filter(|session| session.state == state)
            .count();
        if count > 0 {
            breakdown.push((state, count));
        }
    }
    Some(AgentAggregate {
        total: sessions.len(),
        state: dominant.state,
        dominant,
        breakdown,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn session(id: u64, kind: AgentKind, pid: u32, seconds: u64) -> AgentSession {
        AgentSession::new(id, kind, pid, at(seconds))
    }

    #[test]
    fn priority_follows_the_adr() {
        let ordered = [
            AgentState::Stopped,
            AgentState::Idle,
            AgentState::Finished,
            AgentState::Error,
            AgentState::Working,
            AgentState::NeedsAttention,
        ];
        let mut priorities: Vec<u8> = ordered.iter().map(|state| state.priority()).collect();
        let sorted = {
            let mut copy = priorities.clone();
            copy.sort_unstable();
            copy
        };
        assert_eq!(priorities, sorted);
        priorities.dedup();
        assert_eq!(priorities.len(), ordered.len());
        assert!(AgentState::NeedsAttention.priority() > AgentState::Working.priority());
        assert!(AgentState::Working.priority() > AgentState::Error.priority());
        assert!(AgentState::Error.priority() > AgentState::Finished.priority());
        assert!(AgentState::Finished.priority() > AgentState::Idle.priority());
        assert!(AgentState::Idle.priority() > AgentState::Stopped.priority());
    }

    #[test]
    fn protocol_states_and_kinds_map_without_guessing() {
        assert_eq!(
            AgentState::from_protocol("blocked"),
            Some(AgentState::NeedsAttention)
        );
        assert_eq!(
            AgentState::from_protocol("done"),
            Some(AgentState::Finished)
        );
        assert_eq!(AgentState::from_protocol("clear"), None);
        assert_eq!(AgentState::from_protocol("desconocido"), None);
        assert_eq!(
            AttentionKind::from_protocol("permission"),
            Some(AttentionKind::Permission)
        );
        assert_eq!(AttentionKind::from_protocol("otro"), None);
        assert_eq!(AgentKind::from_app("opencode2"), Some(AgentKind::OpenCode));
        assert_eq!(AgentKind::from_app("copilot"), Some(AgentKind::Copilot));
        assert_eq!(AgentKind::from_app("make"), None);
        assert_eq!(AgentKind::from_name("opencode"), Some(AgentKind::OpenCode));
        assert_eq!(AgentKind::from_name("copilot"), Some(AgentKind::Copilot));
        assert_eq!(AgentKind::OpenCode.label(), "OpenCode");
        assert_eq!(AgentKind::Copilot.label(), "Copilot");
        assert_eq!(AgentKind::OpenCode.resume_command(), "opencode --continue");
        assert_eq!(AgentKind::Copilot.resume_command(), "copilot --continue");
    }

    #[test]
    fn a_report_replaces_the_whole_record() {
        let mut session = session(1, AgentKind::Copilot, 10, 1);
        let mut report = AgentReport::new(AgentState::Working);
        report.app = Some("opencode".into());
        report.message = Some("Compilando".into());
        report.progress = Some(40);
        let transition = session.apply_report(&report, at(2));
        assert!(!transition.notify);
        assert_eq!(transition.to, AgentState::Working);
        assert_eq!(session.progress, Some(40));
        assert_eq!(session.message.as_deref(), Some("Compilando"));

        let done = AgentReport::new(AgentState::Finished);
        let transition = session.apply_report(&done, at(3));
        assert!(transition.notify);
        assert_eq!(transition.from, AgentState::Working);
        assert_eq!(transition.to, AgentState::Finished);

        let idle = AgentReport::new(AgentState::Idle);
        session.apply_report(&idle, at(3));
        assert_eq!(session.app, None);
        assert_eq!(session.message, None);
        assert_eq!(session.progress, None);
    }

    #[test]
    fn opencode_idle_after_working_finishes_once() {
        let mut session = session(1, AgentKind::OpenCode, 10, 1);
        session.apply_report(&AgentReport::new(AgentState::Working), at(2));

        let transition = session.apply_report(&AgentReport::new(AgentState::Idle), at(3));
        assert_eq!(transition.from, AgentState::Working);
        assert_eq!(transition.to, AgentState::Finished);
        assert!(transition.notify);
        assert_eq!(session.state, AgentState::Finished);

        // OpenCode repite `idle` en el registro raíz y `clear` en el de la
        // sesión; ninguno debe borrar el resultado pendiente.
        let transition = session.apply_report(&AgentReport::new(AgentState::Idle), at(4));
        assert!(!transition.notify);
        assert_eq!(session.state, AgentState::Finished);

        let transition = session.apply_report(&AgentReport::clear(), at(5));
        assert!(!transition.notify);
        assert_eq!(session.state, AgentState::Finished);

        // Un turno nuevo vuelve a trabajar con normalidad.
        session.apply_report(&AgentReport::new(AgentState::Working), at(6));
        assert_eq!(session.state, AgentState::Working);
    }

    #[test]
    fn opencode_clear_after_working_finishes() {
        let mut session = session(1, AgentKind::OpenCode, 10, 1);
        session.apply_report(&AgentReport::new(AgentState::Working), at(2));

        let transition = session.apply_report(&AgentReport::clear(), at(3));
        assert_eq!(transition.from, AgentState::Working);
        assert_eq!(transition.to, AgentState::Finished);
        assert!(transition.notify);
        assert_eq!(session.state, AgentState::Finished);
    }

    #[test]
    fn opencode_idle_without_a_working_report_does_not_finish() {
        let mut session = session(1, AgentKind::OpenCode, 10, 1);
        // La salida inicial de la TUI basta para marcar Working en el escaneo,
        // pero no es un turno de trabajo reportado por OSC.
        session.apply_output_activity(at(2));
        assert_eq!(session.state, AgentState::Working);

        let transition = session.apply_report(&AgentReport::new(AgentState::Idle), at(3));
        assert_eq!(transition.to, AgentState::Idle);
        assert!(!transition.notify);
        assert_eq!(session.state, AgentState::Idle);
        assert!(session.reported);
    }

    #[test]
    fn copilot_idle_after_working_stays_idle() {
        let mut session = session(1, AgentKind::Copilot, 11, 1);
        session.apply_report(&AgentReport::new(AgentState::Working), at(2));

        let transition = session.apply_report(&AgentReport::new(AgentState::Idle), at(3));
        assert_eq!(transition.to, AgentState::Idle);
        assert!(!transition.notify);
        assert_eq!(session.state, AgentState::Idle);
    }

    #[test]
    fn blocked_reports_keep_kind_and_progress() {
        let mut session = session(1, AgentKind::Copilot, 11, 1);
        let mut report = AgentReport::new(AgentState::NeedsAttention);
        report.attention = Some(AttentionKind::Question);
        report.progress = Some(10);
        report.message = Some("¿Sobrescribir?".into());
        session.apply_report(&report, at(2));
        assert_eq!(session.attention, Some(AttentionKind::Question));
        assert_eq!(session.progress, Some(10));
        assert!(session.needs_attention());
    }

    #[test]
    fn done_and_error_survive_the_process_but_working_does_not() {
        let mut done = session(1, AgentKind::OpenCode, 10, 1);
        done.apply_report(&AgentReport::new(AgentState::Finished), at(2));
        let transition = done.process_gone(Some(true), at(3));
        assert_eq!(done.state, AgentState::Finished);
        assert!(!transition.notify);
        assert!(!done.alive);

        let mut working = session(2, AgentKind::OpenCode, 10, 1);
        working.apply_report(&AgentReport::new(AgentState::Working), at(2));
        working.process_gone(None, at(3));
        assert_eq!(working.state, AgentState::Finished);

        let mut idle = session(3, AgentKind::OpenCode, 10, 1);
        idle.process_gone(None, at(3));
        assert_eq!(idle.state, AgentState::Stopped);
    }

    #[test]
    fn shell_exit_uses_the_exit_code_as_fallback() {
        let mut session = session(1, AgentKind::OpenCode, 10, 1);
        session.apply_report(&AgentReport::new(AgentState::Working), at(2));
        let transition = session.shell_exited(false, at(3));
        assert_eq!(session.state, AgentState::Error);
        assert!(transition.notify);

        let mut success = AgentSession::new(2, AgentKind::OpenCode, 12, at(1));
        success.shell_exited(true, at(4));
        assert_eq!(success.state, AgentState::Finished);
    }

    #[test]
    fn attention_latches_until_acknowledged() {
        let mut session = session(1, AgentKind::OpenCode, 10, 1);
        session.apply_attention(None, Some("campana".into()), at(2));
        assert_eq!(session.state, AgentState::NeedsAttention);
        session.apply_output_activity(at(3));
        assert_eq!(session.state, AgentState::NeedsAttention);
        assert!(!session.is_stale_on_focus());
        assert!(session.acknowledge(at(4)));
        assert_eq!(session.state, AgentState::Idle);
        assert_eq!(session.message, None);
        assert!(!session.acknowledge(at(5)));
    }

    #[test]
    fn focus_removes_sessions_whose_process_is_gone() {
        let mut live = session(1, AgentKind::OpenCode, 10, 1);
        live.alive = true;
        let mut gone = session(2, AgentKind::OpenCode, 11, 1);
        gone.process_gone(None, at(2));
        assert!(!gone.state.is_terminal_alert() && gone.is_stale_on_focus());

        let mut settled = session(3, AgentKind::OpenCode, 12, 1);
        settled.apply_report(&AgentReport::new(AgentState::Finished), at(2));
        settled.process_gone(Some(true), at(3));
        // Terminado sin proceso se retira al enfocar, no antes.
        assert!(!settled.is_stale_on_focus());
        settled.acknowledge(at(4));
        assert!(!settled.alive);
        assert!(settled.is_stale_on_focus());
    }

    #[test]
    fn aggregate_picks_the_highest_priority_and_breaks_down_states() {
        let mut working = session(1, AgentKind::OpenCode, 10, 1);
        working.apply_report(&AgentReport::new(AgentState::Working), at(2));
        let mut attention = session(2, AgentKind::Copilot, 11, 1);
        attention.apply_attention(Some(AttentionKind::Permission), None, at(3));
        let aggregate = aggregate(&[working, attention]).expect("agregado");
        assert_eq!(aggregate.total, 2);
        assert_eq!(aggregate.state, AgentState::NeedsAttention);
        assert_eq!(aggregate.dominant.kind, AgentKind::Copilot);
        assert_eq!(
            aggregate.breakdown,
            vec![(AgentState::NeedsAttention, 1), (AgentState::Working, 1),]
        );
    }

    #[test]
    fn aggregate_ties_go_to_the_most_recent_session() {
        let first = session(1, AgentKind::OpenCode, 10, 1);
        let second = session(2, AgentKind::OpenCode, 11, 2);
        let aggregate = aggregate(&[first, second]).expect("agregado");
        assert_eq!(aggregate.dominant.id, 2);
    }
}
