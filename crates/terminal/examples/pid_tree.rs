//! Spike S2 — árbol de procesos del PTY.
//!
//! Mide lo que `axf_agents` necesitará en M5: partir del PID raíz que entrega
//! `portable-pty`, descubrir descendientes con `sysinfo` y decidir si el coste
//! permite escanear cada 1–2 s. También comprueba qué pasa con los nietos
//! cuando muere el proceso raíz (el caso "agente huérfano").
//!
//! ```powershell
//! cargo run -p axf_terminal --example pid_tree
//! ```
//!
//! Falla con exit ≠ 0 si no encuentra el subárbol esperado.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "spike de medición: su salida es el entregable"
)]

use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use sysinfo::{Pid, ProcessesToUpdate, System};

type SpikeResult<T> = Result<T, Box<dyn Error>>;

/// Cadena que simula `pwsh -> opencode -> node`: dos niveles de `pwsh` reales,
/// así el árbol tiene la misma forma sin depender de que opencode esté
/// instalado.
const NESTED_COMMAND: &str =
    "pwsh -NoProfile -Command \"pwsh -NoProfile -Command 'Start-Sleep 60'\"";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pid_tree: FALLO: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> SpikeResult<()> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut command = CommandBuilder::new("pwsh");
    command.args(["-NoLogo", "-NoProfile", "-Command", NESTED_COMMAND]);
    let mut child = pair.slave.spawn_command(command)?;
    let root = child
        .process_id()
        .ok_or("portable-pty no expuso process_id()")?;
    println!("raíz: pid={root}");

    // Drenar el PTY y responder al DSR: `pwsh` no ejecuta el comando hasta que
    // el terminal contesta la posición del cursor. Sin esto el árbol no existe.
    let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
    if let Ok(mut reader) = pair.master.try_clone_reader() {
        let writer = Arc::clone(&writer);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8 * 1024];
            let mut scan_tail = Vec::new();
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                let mut scan = Vec::with_capacity(scan_tail.len() + read);
                scan.extend_from_slice(&scan_tail);
                scan.extend_from_slice(&buffer[..read]);
                let dsr = scan
                    .windows(4)
                    .filter(|window| *window == b"\x1b[6n")
                    .count();
                if dsr > 0
                    && let Ok(mut writer) = writer.lock()
                {
                    for _ in 0..dsr {
                        let _ = writer.write_all(b"\x1b[1;1R");
                    }
                    let _ = writer.flush();
                }
                let keep = scan.len().min(3);
                scan_tail = scan[scan.len() - keep..].to_vec();
            }
        });
    }

    // Dejar que pwsh arranque los dos niveles anidados.
    std::thread::sleep(Duration::from_millis(1500));

    let mut system = System::new()?;
    let root_pid = Pid::from_u32(root);

    let mut all_costs = Vec::new();
    let mut full_scans = Vec::new();
    let mut last_subtree = Vec::new();
    for round in 0..8 {
        let started = Instant::now();
        system.refresh_processes(ProcessesToUpdate::All, true);
        all_costs.push(started.elapsed());

        let started = Instant::now();
        let subtree = collect_subtree(&system, root_pid);
        full_scans.push(started.elapsed());

        if round == 7 {
            println!("\nsubárbol en el escaneo final:");
            for entry in &subtree {
                println!(
                    "  pid={} parent={} name={:<16} exe={}",
                    entry.pid,
                    entry
                        .parent
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "-".into()),
                    entry.name,
                    entry.exe.as_deref().unwrap_or("-")
                );
            }
            println!("  ({} procesos: raíz + descendientes)", subtree.len());
            last_subtree = subtree;
        }

        // Escaneo barato: refrescar solo los PID ya conocidos (sin descubrir
        // hijos nuevos). Sirve para separar "descubrir" de "actualizar".
        let known: Vec<Pid> = last_subtree.iter().map(|entry| entry.pid).collect();
        if !known.is_empty() {
            let started = Instant::now();
            system.refresh_processes(ProcessesToUpdate::Some(&known), false);
            full_scans.push(started.elapsed());
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    if last_subtree.len() < 2 {
        return Err(format!(
            "se esperaban al menos 2 procesos (raíz + descendiente), hay {}",
            last_subtree.len()
        )
        .into());
    }

    println!(
        "\ncoste refresh(All): p50={:.2} ms p95={:.2} ms ({} muestras)",
        percentile(&all_costs, 0.50),
        percentile(&all_costs, 0.95),
        all_costs.len()
    );
    println!(
        "coste BFS + refresh(Some): p50={:.3} ms p95={:.3} ms ({} muestras)",
        percentile(&full_scans, 0.50),
        percentile(&full_scans, 0.95),
        full_scans.len()
    );

    // Falsos positivos: cuántos procesos del sistema casan con nombres de
    // agentes. Justifica que el match se limite al subárbol del PTY.
    let agent_hits = system
        .processes()
        .values()
        .filter(|process| {
            let name = process.name().to_string_lossy().to_ascii_lowercase();
            name.contains("opencode") || name == "node" || name.contains("copilot")
        })
        .count();
    println!(
        "coincidencias de nombre de agente en todo el sistema: {agent_hits} (por eso se limita al subárbol)"
    );

    // Muerte del raíz: ¿quedan huérfanos?
    child.kill()?;
    std::thread::sleep(Duration::from_millis(800));
    system.refresh_processes(ProcessesToUpdate::All, true);
    let after = collect_subtree(&system, root_pid);
    println!(
        "tras matar el raíz: {} procesos del subárbol siguen vivos",
        after.len()
    );
    if !after.is_empty() {
        println!(
            "  huérfanos: {:?}",
            after.iter().map(|entry| entry.pid).collect::<Vec<_>>()
        );
    }

    println!(
        "RESULTADO S2: subárbol de {} procesos, descubrimiento a {:.2} ms p95",
        last_subtree.len(),
        percentile(&all_costs, 0.95)
    );
    Ok(())
}

struct ProcessEntry {
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    exe: Option<String>,
}

fn collect_subtree(system: &System, root: Pid) -> Vec<ProcessEntry> {
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (pid, process) in system.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(*pid);
        }
    }

    let mut result = Vec::new();
    let mut queue = vec![root];
    while let Some(pid) = queue.pop() {
        if let Some(process) = system.process(pid) {
            result.push(ProcessEntry {
                pid,
                parent: process.parent(),
                name: process.name().to_string_lossy().into_owned(),
                exe: process.exe().map(|path| path.display().to_string()),
            });
        }
        if let Some(kids) = children.get(&pid) {
            queue.extend(kids.iter().copied());
        }
    }
    result
}

fn percentile(samples: &[Duration], percentile: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut values: Vec<f64> = samples
        .iter()
        .map(|sample| sample.as_secs_f64() * 1e3)
        .collect();
    values.sort_by(|a, b| a.total_cmp(b));
    let index = ((values.len() - 1) as f64 * percentile).round() as usize;
    values[index]
}
