//! PTY y proceso del shell. En Windows usa ConPTY a través de `portable-pty`.
//!
//! La sesión expone `spawn`, `write`, `resize`, `kill` y `wait`. El proceso se
//! espera en un hilo propio para no bloquear nunca el hilo de UI; el código de
//! salida se publica como comando del parser y llega a la vista como evento.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, SlavePty, native_pty_system};

use crate::TerminalError;
use crate::engine::ParserCommand;

/// Programa y argumentos que debe arrancar el PTY.
///
/// Lo elige la app a partir de un perfil de `config.toml`; este crate no conoce
/// shells ni producto (ADR 0006).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnSpec {
    /// Ejecutable (absoluto o resoluble en PATH).
    pub program: String,
    /// Argumentos en orden, sin shell.
    pub args: Vec<String>,
}

impl SpawnSpec {
    /// Construye una especificación de arranque.
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    /// Texto legible para errores y diagnósticos.
    pub fn label(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

/// Tamaño inicial o de resize del PTY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PtySizeSpec {
    /// Columnas.
    pub cols: u16,
    /// Filas.
    pub rows: u16,
    /// Ancho de celda en píxeles.
    pub cell_width: u16,
    /// Alto de celda en píxeles.
    pub cell_height: u16,
}

impl PtySizeSpec {
    /// Construye un tamaño saneado (mínimo 1x1, píxeles saturados).
    pub fn new(cols: usize, rows: usize, cell_width: u16, cell_height: u16) -> Self {
        let cols = u16::try_from(cols).unwrap_or(u16::MAX).max(1);
        let rows = u16::try_from(rows).unwrap_or(u16::MAX).max(1);
        Self {
            cols,
            rows,
            cell_width,
            cell_height,
        }
    }

    fn to_pty(self) -> PtySize {
        PtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: self.cols.saturating_mul(self.cell_width),
            pixel_height: self.rows.saturating_mul(self.cell_height),
        }
    }
}

/// Escritor compartido entre la UI y el parser (respuestas DSR, etc.).
#[derive(Clone)]
pub struct PtyWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl std::fmt::Debug for PtyWriter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PtyWriter").finish_non_exhaustive()
    }
}

impl PtyWriter {
    /// Crea un escritor a partir de cualquier destino `Send`.
    pub(crate) fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(writer)),
        }
    }

    /// Escritor de diagnóstico que descarta todo (tests y benchmarks).
    #[expect(
        dead_code,
        reason = "utilidad de diagnóstico conservada para spikes de PTY"
    )]
    pub(crate) fn sink() -> Self {
        Self::new(Box::new(std::io::sink()))
    }

    /// Escribe bytes al PTY y hace flush; no bloquea al hilo de UI salvo el
    /// coste del syscall.
    pub fn write(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut writer = match self.inner.lock() {
            Ok(writer) => writer,
            Err(poisoned) => poisoned.into_inner(),
        };
        writer.write_all(bytes)?;
        writer.flush()
    }
}

/// Estado de salida del proceso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitInfo {
    /// Código de salida, si el sistema lo expone.
    pub code: Option<u32>,
    /// `true` si el proceso terminó con éxito.
    pub success: bool,
}

impl ExitInfo {
    fn from_status(status: &portable_pty::ExitStatus) -> Self {
        Self {
            code: Some(status.exit_code()),
            success: status.success(),
        }
    }

    /// Texto corto para la UI.
    pub fn label(self) -> String {
        match (self.success, self.code) {
            (true, Some(code)) => format!("proceso terminado (código {code})"),
            (true, None) => "proceso terminado".into(),
            (false, Some(code)) => format!("proceso terminado con error (código {code})"),
            (false, None) => "proceso terminado con error".into(),
        }
    }
}

/// Sesión de un shell vivo.
pub struct TerminalSession {
    master: Option<Arc<Mutex<Box<dyn MasterPty + Send>>>>,
    slave: Option<Box<dyn SlavePty + Send>>,
    writer: Option<PtyWriter>,
    reader: Option<JoinHandle<()>>,
    waiter: Option<JoinHandle<()>>,
    pid: Option<u32>,
    program: String,
    exit: Arc<Mutex<Option<ExitInfo>>>,
    closed: bool,
}

impl std::fmt::Debug for TerminalSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalSession")
            .field("pid", &self.pid)
            .field("program", &self.program)
            .field("exit", &self.try_wait())
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl TerminalSession {
    /// Abre un PTY y arranca exactamente el programa de `spec`.
    ///
    /// Sin fallback: si el ejecutable no existe o el PTY falla, el error sube a
    /// la pestaña con el comando que se intentó (ADR 0006).
    pub fn spawn(
        cwd: &Path,
        size: PtySizeSpec,
        output: async_channel::Sender<ParserCommand>,
        spec: &SpawnSpec,
    ) -> Result<Self, TerminalError> {
        let pair = native_pty_system()
            .openpty(size.to_pty())
            .map_err(|error| TerminalError::Pty(format!("openpty: {error}")))?;

        let cwd = usable_cwd(cwd);
        let mut command = CommandBuilder::new(&spec.program);
        command.args(&spec.args);
        if !cwd.as_os_str().is_empty() {
            command.cwd(&cwd);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "axfterm");
        command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| TerminalError::Spawn {
                program: spec.program.clone(),
                message: error.to_string(),
            })?;

        let pid = child.process_id();
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| TerminalError::Pty(format!("take_writer: {error}")))?;
        let writer = PtyWriter::new(writer);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| TerminalError::Pty(format!("try_clone_reader: {error}")))?;

        let waiter_output = output.clone();
        let reader_thread = thread::Builder::new()
            .name("axf-terminal-reader".into())
            .spawn(move || read_loop(reader, output))
            .map_err(|error| TerminalError::Thread(error.to_string()))?;

        let exit = Arc::new(Mutex::new(None));
        let waiter_exit = Arc::clone(&exit);
        // El hilo de espera publica el código de salida; nunca se une desde la
        // UI. `child` se mueve aquí porque `wait` es bloqueante.
        let waiter = thread::Builder::new()
            .name("axf-terminal-waiter".into())
            .spawn(move || wait_loop(child, waiter_exit, waiter_output))
            .map_err(|error| TerminalError::Thread(error.to_string()))?;

        Ok(Self {
            master: Some(Arc::new(Mutex::new(pair.master))),
            slave: Some(pair.slave),
            writer: Some(writer),
            reader: Some(reader_thread),
            waiter: Some(waiter),
            pid,
            program: spec.program.clone(),
            exit,
            closed: false,
        })
    }

    /// Arranca un programa concreto. Solo para tests, ejemplos y diagnóstico.
    #[doc(hidden)]
    pub fn spawn_program(
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        size: PtySizeSpec,
        output: async_channel::Sender<ParserCommand>,
    ) -> Result<Self, TerminalError> {
        let spec = SpawnSpec::new(program, args.iter().map(|arg| (*arg).to_string()).collect());
        Self::spawn(cwd.unwrap_or(Path::new("")), size, output, &spec)
    }

    /// Programa que arrancó el PTY (`pwsh`, `cmd`, etc.).
    pub fn program(&self) -> &str {
        &self.program
    }

    /// PID raíz del PTY, si el sistema lo expuso.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Escritor compartido para la entrada de teclado.
    pub fn writer(&self) -> Option<PtyWriter> {
        self.writer.clone()
    }

    /// Redimensiona el PTY. ConPTY propaga el cambio al shell.
    pub fn resize(&self, size: PtySizeSpec) {
        let Some(master) = &self.master else {
            return;
        };
        let master = match master.lock() {
            Ok(master) => master,
            Err(poisoned) => poisoned.into_inner(),
        };
        let _ = master.resize(size.to_pty());
    }

    /// Mata el árbol de procesos sin cerrar la sesión.
    pub fn kill(&mut self) {
        if let Some(pid) = self.pid {
            kill_process_tree(pid);
        }
    }

    /// Código de salida si el proceso ya terminó.
    pub fn try_wait(&self) -> Option<ExitInfo> {
        self.exit.lock().ok().and_then(|exit| *exit)
    }

    /// Espera al proceso. Solo para tests y apagados ordenados; la UI nunca
    /// llama a este método.
    pub fn wait(&mut self) -> Option<ExitInfo> {
        if let Some(waiter) = self.waiter.take() {
            let _ = waiter.join();
        }
        self.try_wait()
    }

    /// Cierra la sesión y, en Windows, mata el árbol de procesos.
    ///
    /// El cierre es una operación única fuera del camino caliente. `taskkill
    /// /T /F` tarda decenas de milisegundos y evita los huérfanos que S2
    /// documentó al matar solo la raíz.
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;

        if let Some(pid) = self.pid {
            kill_process_tree(pid);
        }

        self.writer.take();
        self.master.take();
        self.slave.take();
        self.reader.take();
        self.waiter.take();
    }

    /// `true` si la sesión ya se cerró explícitamente.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.close();
    }
}

/// Alias histórico: M0 llamaba `PtySession` a la sesión del shell.
pub type PtySession = TerminalSession;

fn usable_cwd(cwd: &Path) -> PathBuf {
    if !cwd.as_os_str().is_empty() {
        // Nunca se entrega un CWD verbatim (`\\?\C:\...`): PowerShell lo
        // muestra cualificado con el proveedor en vez de como `C:\...`.
        let cwd = crate::paths::normalize(cwd);
        if cwd.is_dir() {
            return cwd;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Bucle lector: aplica backpressure real a través del canal acotado.
fn read_loop(mut reader: Box<dyn Read + Send>, output: async_channel::Sender<ParserCommand>) {
    let mut buffer = vec![0u8; 32 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                let _ = output.send_blocking(ParserCommand::Eof);
                break;
            }
            Ok(read) => {
                let bytes = buffer[..read].to_vec();
                if output.send_blocking(ParserCommand::Bytes(bytes)).is_err() {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                let _ = output.send_blocking(ParserCommand::Eof);
                break;
            }
        }
    }
}

/// Bucle de espera: publica el código de salida en cuanto el proceso muere.
///
/// Usa `try_wait` en lugar de `wait` para que un PTY que no señala el cierre
/// (ConPTY tiene casos así) no deje un hilo bloqueado para siempre.
fn wait_loop(
    mut child: Box<dyn Child + Send + Sync>,
    exit: Arc<Mutex<Option<ExitInfo>>>,
    output: async_channel::Sender<ParserCommand>,
) {
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(_) => return,
        }
    };

    let info = ExitInfo::from_status(&status);
    if let Ok(mut slot) = exit.lock() {
        *slot = Some(info);
    }
    let _ = output.send_blocking(ParserCommand::Exited(info));
}

#[cfg(windows)]
fn kill_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn kill_process_tree(pid: u32) {
    use std::process::{Command, Stdio};

    // portable-pty crea el hijo en una sesión propia; matar el grupo negativo
    // recorre los descendientes. Si no está disponible, `kill` de la raíz deja
    // el caso documentado de M1 para Unix.
    let group = format!("-{pid}");
    let _ = Command::new("kill")
        .args(["-TERM", "--", &group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("kill")
        .args(["-KILL", "--", &group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_spec_labels_the_command() {
        let spec = SpawnSpec::new("pwsh", vec!["-NoLogo".into()]);
        assert_eq!(spec.label(), "pwsh -NoLogo");
        assert_eq!(SpawnSpec::new("cmd", Vec::new()).label(), "cmd");
    }

    #[test]
    fn missing_program_reports_the_profile_command() {
        let (sender, _receiver) = async_channel::bounded(4);
        let spec = SpawnSpec::new("axfterm-programa-que-no-existe", Vec::new());
        let error =
            TerminalSession::spawn(Path::new(""), PtySizeSpec::new(20, 5, 8, 16), sender, &spec)
                .expect_err("no debe arrancar");
        let message = error.to_string();
        assert!(message.contains("axfterm-programa-que-no-existe"));
        assert!(message.contains("config.toml"));
        assert!(matches!(error, TerminalError::Spawn { .. }));
    }

    #[test]
    fn size_is_sane_and_pixels_are_saturated() {
        let size = PtySizeSpec::new(0, 0, u16::MAX, u16::MAX);
        assert_eq!(size.cols, 1);
        assert_eq!(size.rows, 1);
        let pty = size.to_pty();
        assert_eq!(pty.pixel_width, u16::MAX);
        assert_eq!(pty.pixel_height, u16::MAX);
    }

    #[test]
    fn exit_label_is_explainable() {
        assert_eq!(
            ExitInfo {
                code: Some(0),
                success: true
            }
            .label(),
            "proceso terminado (código 0)"
        );
        assert_eq!(
            ExitInfo {
                code: Some(7),
                success: false
            }
            .label(),
            "proceso terminado con error (código 7)"
        );
    }

    #[test]
    fn reader_loop_stops_when_the_channel_is_closed() {
        let (sender, receiver) = async_channel::bounded(1);
        receiver.close();
        let input = std::io::Cursor::new(vec![b'a'; 64 * 1024]);
        read_loop(Box::new(input), sender);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn cwd_falls_back_to_the_current_directory() {
        let cwd = usable_cwd(Path::new("C:/esta/ruta/no/existe/axfterm"));
        assert_eq!(
            cwd,
            std::env::current_dir().expect("el directorio actual existe")
        );
    }

    #[cfg(windows)]
    #[test]
    fn cwd_is_never_verbatim() {
        let friendly = std::env::current_dir().expect("el directorio actual existe");
        let text = friendly.display().to_string();
        // Solo tiene sentido en una ruta de unidad corta (`C:\...`).
        if text.len() > 250 || text.as_bytes().get(1) != Some(&b':') {
            return;
        }
        let verbatim = PathBuf::from(format!(r"\\?\{text}"));
        assert_eq!(usable_cwd(&verbatim), friendly);
    }

    /// Ejecuta un programa a través de sesión + parser reales.
    ///
    /// ConPTY emite `ESC[6n` al arrancar; sin el `EventListener` del parser el
    /// hijo se queda esperando la respuesta. Por eso estas pruebas usan el
    /// camino de producción en vez de una sesión aislada.
    #[cfg(windows)]
    fn run_with_parser(
        program: &str,
        args: &[&str],
        timeout: std::time::Duration,
    ) -> (String, Option<ExitInfo>) {
        let (commands, inbox) = async_channel::bounded(64);
        let session = TerminalSession::spawn_program(
            program,
            args,
            None,
            PtySizeSpec::new(40, 8, 8, 16),
            commands.clone(),
        )
        .expect("el programa de prueba arranca");
        let writer = session.writer().expect("writer disponible");
        let (frames, frame_inbox) = async_channel::bounded(4);
        let parser = crate::engine::spawn_parser(
            commands,
            inbox,
            frames,
            crate::engine::ParserSetup {
                writer,
                palette: axf_ui::theme::TerminalPalette::placeholder(),
                initial: crate::GridSize::new(40, 8),
                cell_width: 8,
                cell_height: 16,
                restore: None,
            },
        )
        .expect("el parser arranca");

        fn collect_frame(
            frame: crate::engine::ScreenFrame,
            exit: &mut Option<ExitInfo>,
            rows: &mut Vec<String>,
        ) {
            for event in &frame.events {
                if let crate::engine::TerminalEvent::Exited(info) = event {
                    *exit = *info;
                }
            }
            if frame.full {
                *rows = frame
                    .updates
                    .iter()
                    .map(|update| update.cells.iter().map(|cell| cell.ch).collect())
                    .collect();
            }
        }

        let deadline = std::time::Instant::now() + timeout;
        let mut exit = None;
        let mut rows: Vec<String> = Vec::new();

        while std::time::Instant::now() < deadline {
            while let Ok(frame) = frame_inbox.try_recv() {
                collect_frame(frame, &mut exit, &mut rows);
            }
            if exit.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Fuerza un frame completo tras la salida para leer el grid final.
        let _ = parser.scroll(0);
        let frame_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut final_rows: Vec<String> = rows;
        while std::time::Instant::now() < frame_deadline {
            let mut got_full = false;
            while let Ok(frame) = frame_inbox.try_recv() {
                if frame.full {
                    final_rows = frame
                        .updates
                        .iter()
                        .map(|update| update.cells.iter().map(|cell| cell.ch).collect())
                        .collect();
                    got_full = true;
                }
            }
            if got_full {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        drop(parser);
        drop(session);
        (final_rows.join("\n"), exit)
    }

    #[cfg(windows)]
    #[test]
    fn exit_code_reaches_the_waiter() {
        let (text, exit) =
            run_with_parser("cmd", &["/C", "exit 7"], std::time::Duration::from_secs(15));
        let info = exit.expect("el parser vio el cierre");
        assert_eq!(info.code, Some(7), "texto: {text:?}");
        assert!(!info.success);
    }

    #[cfg(windows)]
    #[test]
    fn kill_ends_a_long_running_tree() {
        let (sender, receiver) = async_channel::bounded(64);
        let mut session = TerminalSession::spawn_program(
            "cmd",
            &["/C", "ping -n 30 127.0.0.1 >nul"],
            None,
            PtySizeSpec::new(20, 5, 8, 16),
            sender,
        )
        .expect("el programa largo arranca");

        session.resize(PtySizeSpec::new(40, 10, 8, 16));
        session
            .writer()
            .expect("writer disponible")
            .write(b"")
            .expect("escribir al PTY");
        session.kill();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline && session.try_wait().is_none() {
            while receiver.try_recv().is_ok() {}
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(session.try_wait().is_some(), "el árbol no terminó");
    }

    #[cfg(windows)]
    #[test]
    fn child_receives_term_environment() {
        let (text, exit) = run_with_parser(
            "cmd",
            &["/C", "echo %TERM% %COLORTERM%"],
            std::time::Duration::from_secs(15),
        );
        assert!(exit.is_some(), "texto: {text:?}");
        assert!(text.contains("xterm-256color"), "salida: {text:?}");
        assert!(text.contains("truecolor"), "salida: {text:?}");
    }
}
