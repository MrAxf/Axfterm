//! Identificadores estables del workspace.
//!
//! Los IDs son newtypes sobre `u64`, serializables y únicos durante la sesión.
//! El contador se persiste para que restaurar una sesión no reutilice IDs
//! (ADR 0003).

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            /// Construye el ID a partir de su valor estable.
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            /// Valor numérico del ID.
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self::new(value)
            }
        }
    };
}

id_type!(ProjectId, "Identificador de un proyecto de la sidebar.");
id_type!(PaneId, "Identificador de un panel del árbol de layout.");
id_type!(TabId, "Identificador de una pestaña.");
id_type!(
    TerminalId,
    "Identificador de la terminal (PTY) de una pestaña."
);

/// Contadores monótonos para emitir IDs nuevos sin reutilizarlos.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdCounters {
    next_project: u64,
    next_pane: u64,
    next_tab: u64,
    next_terminal: u64,
}

impl IdCounters {
    /// Contadores listos para un workspace nuevo (empiezan en 1).
    pub fn new() -> Self {
        Self {
            next_project: 1,
            next_pane: 1,
            next_tab: 1,
            next_terminal: 1,
        }
    }

    /// Reserva un `ProjectId`.
    pub fn take_project(&mut self) -> ProjectId {
        let id = ProjectId::new(self.next_project.max(1));
        self.next_project = id.get().saturating_add(1);
        id
    }

    /// Reserva un `PaneId`.
    pub fn take_pane(&mut self) -> PaneId {
        let id = PaneId::new(self.next_pane.max(1));
        self.next_pane = id.get().saturating_add(1);
        id
    }

    /// Reserva un `TabId`.
    pub fn take_tab(&mut self) -> TabId {
        let id = TabId::new(self.next_tab.max(1));
        self.next_tab = id.get().saturating_add(1);
        id
    }

    /// Reserva un `TerminalId`.
    pub fn take_terminal(&mut self) -> TerminalId {
        let id = TerminalId::new(self.next_terminal.max(1));
        self.next_terminal = id.get().saturating_add(1);
        id
    }

    /// Ajusta los contadores para que no colisionen con IDs ya cargados.
    pub fn reconcile(
        &mut self,
        projects: impl IntoIterator<Item = ProjectId>,
        panes: impl IntoIterator<Item = PaneId>,
        tabs: impl IntoIterator<Item = TabId>,
        terminals: impl IntoIterator<Item = TerminalId>,
    ) {
        for id in projects {
            self.next_project = self.next_project.max(id.get().saturating_add(1));
        }
        for id in panes {
            self.next_pane = self.next_pane.max(id.get().saturating_add(1));
        }
        for id in tabs {
            self.next_tab = self.next_tab.max(id.get().saturating_add(1));
        }
        for id in terminals {
            self.next_terminal = self.next_terminal.max(id.get().saturating_add(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_emit_unique_ids_and_reconcile() {
        let mut counters = IdCounters::new();
        assert_eq!(counters.take_project().get(), 1);
        assert_eq!(counters.take_project().get(), 2);
        assert_eq!(counters.take_pane().get(), 1);
        assert_eq!(counters.take_tab().get(), 1);
        assert_eq!(counters.take_terminal().get(), 1);

        counters.reconcile(
            [ProjectId::new(10)],
            [PaneId::new(9)],
            [TabId::new(8)],
            [TerminalId::new(7)],
        );
        assert_eq!(counters.take_project().get(), 11);
        assert_eq!(counters.take_pane().get(), 10);
        assert_eq!(counters.take_tab().get(), 9);
        assert_eq!(counters.take_terminal().get(), 8);
    }

    #[test]
    fn ids_serialize_transparently() {
        let id = TabId::new(42);
        assert_eq!(serde_json::to_string(&id).expect("serializa"), "42");
        let decoded: TabId = serde_json::from_str("42").expect("deserializa");
        assert_eq!(decoded, id);
    }
}
