//! Snapshot de procesos y matcher puro de agentes.
//!
//! El escáner (`sysinfo`) solo produce datos; toda la decisión vive en
//! `ProcessSnapshot::agents_for`, que es pura y se prueba con snapshots
//! sintéticos (ADR 0009).

use std::collections::{HashMap, VecDeque};

use sysinfo::{ProcessesToUpdate, System};

use crate::model::AgentKind;

/// Nombre de un proceso ayudante que nunca debe contarse como agente.
const HELPER_NAMES: &[&str] = &["opencode-pty"];

/// Proceso observado en un instante.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInfo {
    /// PID.
    pub pid: u32,
    /// PID del padre, si el sistema lo conoce.
    pub parent: Option<u32>,
    /// Nombre del ejecutable (puede incluir `.exe`).
    pub name: String,
    /// Línea de comandos completa.
    pub argv: Vec<String>,
}

impl ProcessInfo {
    /// Construye un proceso para snapshots de test o diagnóstico.
    pub fn new(
        pid: u32,
        parent: Option<u32>,
        name: impl Into<String>,
        argv: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            pid,
            parent,
            name: name.into(),
            argv: argv.into_iter().map(Into::into).collect(),
        }
    }
}

/// Conjunto de procesos de un escaneo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessSnapshot {
    /// Procesos en cualquier orden; el matcher no depende del orden.
    pub processes: Vec<ProcessInfo>,
}

/// Agente detectado dentro de un árbol de procesos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetectedAgent {
    /// Tipo de agente.
    pub kind: AgentKind,
    /// PID del proceso más cercano al shell.
    pub pid: u32,
}

impl ProcessSnapshot {
    /// Construye un snapshot desde una lista de procesos.
    pub fn new(processes: Vec<ProcessInfo>) -> Self {
        Self { processes }
    }

    /// Agentes del subárbol de `root`, del más cercano al shell hacia abajo.
    ///
    /// Un agente detectado poda su propio subárbol: sus ayudantes
    /// (`opencode-pty`, runtimes hijos) no generan sesiones duplicadas. Se
    /// admiten varias sesiones por pestaña, también del mismo tipo, cuando son
    /// ramas distintas.
    pub fn agents_for(&self, root: u32) -> Vec<DetectedAgent> {
        let mut by_pid: HashMap<u32, &ProcessInfo> = HashMap::with_capacity(self.processes.len());
        let mut children: HashMap<u32, Vec<&ProcessInfo>> = HashMap::new();
        for process in &self.processes {
            by_pid.insert(process.pid, process);
            if let Some(parent) = process.parent {
                children.entry(parent).or_default().push(process);
            }
        }
        for list in children.values_mut() {
            list.sort_by_key(|process| process.pid);
        }

        let Some(first) = by_pid.get(&root) else {
            return Vec::new();
        };
        let mut queue = VecDeque::new();
        queue.push_back(*first);
        let mut agents = Vec::new();
        while let Some(process) = queue.pop_front() {
            if let Some(kind) = classify(process) {
                agents.push(DetectedAgent {
                    kind,
                    pid: process.pid,
                });
                continue;
            }
            if let Some(list) = children.get(&process.pid) {
                queue.extend(list.iter().copied());
            }
        }
        agents
    }
}

/// Escáner de procesos en segundo plano.
///
/// No toca la UI: la app lo ejecuta en el executor de fondo y publica el
/// snapshot en el hilo de UI.
pub struct ProcessScanner {
    system: Option<System>,
}

impl std::fmt::Debug for ProcessScanner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProcessScanner")
            .finish_non_exhaustive()
    }
}

impl Default for ProcessScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessScanner {
    /// Escáner vacío; el primer `scan` puebla el sistema.
    pub fn new() -> Self {
        Self {
            system: System::new().ok(),
        }
    }

    /// Refresca y devuelve todos los procesos visibles.
    pub fn scan(&mut self) -> ProcessSnapshot {
        let Some(system) = self.system.as_mut() else {
            return ProcessSnapshot::default();
        };
        system.refresh_processes(ProcessesToUpdate::All, true);
        let mut processes: Vec<ProcessInfo> = system
            .processes()
            .values()
            .map(|process| ProcessInfo {
                pid: process.pid().as_u32(),
                parent: process.parent().map(|parent| parent.as_u32()),
                name: process.name().to_string_lossy().into_owned(),
                argv: process
                    .cmd()
                    .iter()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect(),
            })
            .collect();
        processes.sort_by_key(|process| process.pid);
        ProcessSnapshot { processes }
    }
}

/// Clasifica un proceso como agente, sin aceptar nombres parecidos.
fn classify(process: &ProcessInfo) -> Option<AgentKind> {
    let name = stem(&process.name);
    if HELPER_NAMES.contains(&name.as_str()) {
        return None;
    }
    match name.as_str() {
        "opencode" | "opencode2" => return Some(AgentKind::OpenCode),
        "copilot" | "copilot-runtime" => return Some(AgentKind::Copilot),
        _ => {}
    }

    // Fallback para instalaciones que ejecutan el CLI con un runtime:
    // `node .../opencode` o `bun .../copilot`. Se limita a runtimes conocidos
    // para no confundir un `cmd /C opencode` con un agente.
    if !matches!(name.as_str(), "node" | "bun" | "deno") {
        return None;
    }
    let script = process
        .argv
        .iter()
        .skip(1)
        .find(|argument| !argument.starts_with('-'))?;
    let script = stem(script);
    match script.as_str() {
        "opencode" | "opencode2" => Some(AgentKind::OpenCode),
        "copilot" | "copilot-runtime" => Some(AgentKind::Copilot),
        _ => None,
    }
}

/// Nombre base en minúsculas, sin ruta ni extensión.
fn stem(value: &str) -> String {
    let name = value
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(value)
        .trim_matches('"');
    let name = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name);
    name.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(processes: Vec<ProcessInfo>) -> ProcessSnapshot {
        ProcessSnapshot::new(processes)
    }

    fn shell(root: u32) -> ProcessInfo {
        ProcessInfo::new(root, Some(1), "pwsh.exe", ["pwsh.exe"])
    }

    #[test]
    fn detects_opencode_and_its_alias_next_to_the_shell() {
        let root = 100;
        let snapshot = snapshot(vec![
            shell(root),
            ProcessInfo::new(101, Some(root), "opencode.exe", ["opencode.exe"]),
            ProcessInfo::new(102, Some(root), "opencode2.exe", ["opencode2.exe"]),
        ]);
        let agents = snapshot.agents_for(root);
        assert_eq!(
            agents,
            vec![
                DetectedAgent {
                    kind: AgentKind::OpenCode,
                    pid: 101
                },
                DetectedAgent {
                    kind: AgentKind::OpenCode,
                    pid: 102
                },
            ]
        );
    }

    #[test]
    fn detects_copilot_and_prunes_its_runtime_helper() {
        let root = 200;
        let snapshot = snapshot(vec![
            shell(root),
            ProcessInfo::new(201, Some(root), "copilot.exe", ["copilot.exe"]),
            ProcessInfo::new(
                202,
                Some(201),
                "copilot-runtime.exe",
                ["copilot-runtime.exe"],
            ),
        ]);
        assert_eq!(
            snapshot.agents_for(root),
            vec![DetectedAgent {
                kind: AgentKind::Copilot,
                pid: 201
            }]
        );
    }

    #[test]
    fn detects_node_and_bun_scripts_by_argv() {
        let root = 300;
        let snapshot = snapshot(vec![
            shell(root),
            ProcessInfo::new(
                301,
                Some(root),
                "node.exe",
                ["node.exe", r"C:\tools\opencode\bin\opencode"],
            ),
            ProcessInfo::new(
                302,
                Some(root),
                "bun.exe",
                ["bun.exe", "/usr/local/bin/copilot-runtime"],
            ),
        ]);
        assert_eq!(
            snapshot.agents_for(root),
            vec![
                DetectedAgent {
                    kind: AgentKind::OpenCode,
                    pid: 301
                },
                DetectedAgent {
                    kind: AgentKind::Copilot,
                    pid: 302
                },
            ]
        );
    }

    #[test]
    fn prunes_the_opencode_pty_helper() {
        let root = 400;
        let snapshot = snapshot(vec![
            shell(root),
            ProcessInfo::new(401, Some(root), "opencode.exe", ["opencode.exe"]),
            ProcessInfo::new(402, Some(401), "opencode-pty.exe", ["opencode-pty.exe"]),
            ProcessInfo::new(403, Some(401), "opencode.exe", ["opencode.exe"]),
        ]);
        assert_eq!(
            snapshot.agents_for(root),
            vec![DetectedAgent {
                kind: AgentKind::OpenCode,
                pid: 401
            }]
        );
    }

    #[test]
    fn ignores_lookalikes_and_unrelated_runtimes() {
        let root = 500;
        let snapshot = snapshot(vec![
            shell(root),
            ProcessInfo::new(501, Some(root), "my-opencode.exe", ["my-opencode.exe"]),
            ProcessInfo::new(502, Some(root), "cmd.exe", ["cmd.exe", "/C", "opencode"]),
            ProcessInfo::new(
                503,
                Some(root),
                "node.exe",
                ["node.exe", "server.js", "opencode"],
            ),
            ProcessInfo::new(504, Some(root), "code.exe", ["code.exe"]),
        ]);
        assert!(snapshot.agents_for(root).is_empty());
    }

    #[test]
    fn a_shell_that_is_itself_an_agent_counts_once() {
        let root = 600;
        let snapshot = snapshot(vec![
            ProcessInfo::new(root, Some(1), "opencode.exe", ["opencode.exe"]),
            ProcessInfo::new(601, Some(root), "node.exe", ["node.exe", "worker.js"]),
        ]);
        assert_eq!(
            snapshot.agents_for(root),
            vec![DetectedAgent {
                kind: AgentKind::OpenCode,
                pid: root
            }]
        );
    }

    #[test]
    fn unknown_root_yields_no_agents() {
        assert!(snapshot(vec![shell(700)]).agents_for(999).is_empty());
    }
}
