use crate::TerminalError;
use crate::engine::ParserCommand;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, SlavePty, native_pty_system};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
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
