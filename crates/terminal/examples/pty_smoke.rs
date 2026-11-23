//! Spike S1 — PTY sobre ConPTY.
//!
//! Valida lo mínimo que M1 necesita de `portable-pty`: spawn de `pwsh`, lectura
//! en hilo aparte, escritura, resize, exit code y kill. Además toma las primeras
//! mediciones (spawn→primer byte, round-trip de un comando y throughput).
//!
//! Ejecutar:
//!
//! ```powershell
//! cargo run -p axf_terminal --example pty_smoke
//! ```
//!
//! Sale con código ≠ 0 si alguna comprobación falla, para poder usarlo como
//! verificación rápida del entorno Windows.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "spike de medición: su salida es el entregable"
)]

use std::error::Error;
use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

const ROWS: u16 = 24;
const COLS: u16 = 80;
const RESIZED_ROWS: u16 = 30;
const RESIZED_COLS: u16 = 100;

type SpikeResult<T> = Result<T, Box<dyn Error>>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pty_smoke: FALLO: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> SpikeResult<()> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: ROWS,
        cols: COLS,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut command = CommandBuilder::new("pwsh");
    command.args(["-NoLogo", "-NoProfile"]);
    command.env("TERM", "xterm-256color");

    let spawn_started = Instant::now();
    let mut child = pair.slave.spawn_command(command)?;
    let root_pid = child.process_id();
    println!(
        "spawn: pid={root_pid:?} en {:.1} ms",
        spawn_started.elapsed().as_secs_f64() * 1e3
    );

    let writer = Writer::new(pair.master.take_writer()?);
    let mut reader = Reader::spawn(pair.master.try_clone_reader()?, writer.clone());

    // 1. Primer byte: no hay marcador todavía, así que basta con leer algo.
    // `pwsh` arranca pidiendo la posición del cursor (DSR 6n); sin respuesta se
    // queda esperando, que es justo el riesgo que valida este spike.
    let (first_byte, _) = reader.read_any(Duration::from_secs(20))?;
    println!(
        "spawn->primer byte: {:.1} ms",
        first_byte.as_secs_f64() * 1e3
    );

    // 2. Round-trip de un comando. El marcador se construye en runtime para que
    // el eco de la línea tecleada no lo contenga. Se mide dos veces: la primera
    // incluye el arranque de PSReadLine; la segunda es el régimen estable.
    let echo_started = Instant::now();
    write_line(&writer, "Write-Host ('AXF' + '_PTY_OK')")?;
    reader.read_until(b"AXF_PTY_OK", Duration::from_secs(20))?;
    println!(
        "round-trip comando (1º): {:.1} ms",
        echo_started.elapsed().as_secs_f64() * 1e3
    );

    let echo_started = Instant::now();
    write_line(&writer, "Write-Host ('AXF' + '_OK2')")?;
    reader.read_until(b"AXF_OK2", Duration::from_secs(20))?;
    println!(
        "round-trip comando (2º): {:.1} ms",
        echo_started.elapsed().as_secs_f64() * 1e3
    );

    // 3. Throughput: bytes crudos a la consola, sin el formateador de PowerShell
    // (que es lo que dominaba la primera medición). ~600 KiB de datos.
    let bytes_target = 600usize * 1024;
    let throughput_started = Instant::now();
    write_line(
        &writer,
        &format!("$s = 'x' * {bytes_target}; [Console]::Out.Write($s)"),
    )?;
    write_line(&writer, "Write-Host ('AXF' + '_DONE')")?;
    let (elapsed, bytes) = reader.read_until(b"AXF_DONE", Duration::from_secs(60))?;
    let mib = bytes as f64 / (1024.0 * 1024.0);
    println!(
        "throughput: {mib:.2} MiB en {:.2} s ({:.2} MiB/s, {:.0} KiB/s)",
        elapsed.as_secs_f64(),
        mib / elapsed.as_secs_f64(),
        mib * 1024.0 / elapsed.as_secs_f64()
    );
    println!(
        "latencia total del lote: {:.1} ms",
        throughput_started.elapsed().as_secs_f64() * 1e3
    );

    // 3b. Throughput crudo: proceso dedicado que escribe N bytes a stdout, sin
    // PSReadLine ni el host de PowerShell. Separa el coste de ConPTY del de la
    // herramienta que genera el texto.
    {
        let raw_bytes = 1024 * 1024;
        let pair = pty_system.openpty(PtySize {
            rows: ROWS,
            cols: COLS,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut command = CommandBuilder::new("pwsh");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-Command",
            &format!(
                "$b = [byte[]]::new({raw_bytes}); [Array]::Fill($b, [byte]120); \
                 $o = [Console]::OpenStandardOutput(); $o.Write($b, 0, $b.Length); $o.Flush(); \
                 [Console]::Out.Write('AXF' + '_RAW_DONE')"
            ),
        ]);
        let raw_started = Instant::now();
        let mut raw_child = pair.slave.spawn_command(command)?;
        let raw_writer = Writer::new(pair.master.take_writer()?);
        let mut raw_reader = Reader::spawn(pair.master.try_clone_reader()?, raw_writer);
        let (elapsed, bytes) = raw_reader.read_until(b"AXF_RAW_DONE", Duration::from_secs(60))?;
        let _ = raw_child.kill();
        let _ = wait_with_timeout(raw_child.as_mut(), Duration::from_secs(10))?;
        let mib = (bytes as f64 - b"AXF_RAW_DONE".len() as f64) / (1024.0 * 1024.0);
        println!(
            "throughput crudo: {mib:.2} MiB en {:.2} s ({:.2} MiB/s) [+{:.1} ms de arranque]",
            elapsed.as_secs_f64(),
            mib / elapsed.as_secs_f64(),
            raw_started.elapsed().as_secs_f64() * 1e3
        );
    }

    // 4. Resize ConPTY: el cambio debe llegar al proceso hijo.
    let resize_started = Instant::now();
    pair.master.resize(PtySize {
        rows: RESIZED_ROWS,
        cols: RESIZED_COLS,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    std::thread::sleep(Duration::from_millis(300));
    write_line(
        &writer,
        "Write-Host ('COLS=' + $Host.UI.RawUI.WindowSize.Width)",
    )?;
    reader.read_until(b"COLS=100", Duration::from_secs(20))?;
    println!(
        "resize {COLS}x{ROWS} -> {RESIZED_COLS}x{RESIZED_ROWS}: OK en {:.1} ms",
        resize_started.elapsed().as_secs_f64() * 1e3
    );

    // 5. Exit code propagado.
    write_line(&writer, "exit 7")?;
    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(10))?;
    println!(
        "exit: code={} success={} en {:.1} ms",
        status.exit_code(),
        status.success(),
        resize_started.elapsed().as_secs_f64() * 1e3
    );
    if status.exit_code() != 7 {
        return Err(format!("se esperaba exit=7, llegó {}", status.exit_code()).into());
    }

    // 6. Kill de un PTY vivo (rama que usa el cierre de pestañas).
    let pair = pty_system.openpty(PtySize {
        rows: ROWS,
        cols: COLS,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let mut command = CommandBuilder::new("pwsh");
    command.args(["-NoLogo", "-NoProfile", "-Command", "Start-Sleep 60"]);
    let mut victim = pair.slave.spawn_command(command)?;
    let kill_started = Instant::now();
    victim.kill()?;
    let killed = wait_with_timeout(victim.as_mut(), Duration::from_secs(10))?;
    println!(
        "kill: success={} en {:.1} ms",
        killed.success(),
        kill_started.elapsed().as_secs_f64() * 1e3
    );

    println!("RESULTADO S1: PTY OK (spawn, lectura, escritura, resize, exit y kill verificados)");
    Ok(())
}

fn write_line(writer: &Writer, line: &str) -> SpikeResult<()> {
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\r")?;
    writer.flush()?;
    Ok(())
}

/// Escritor compartido entre el hilo principal (comandos) y el lector (respuesta
/// a DSR). En M1 esta respuesta la genera el `EventListener` de
/// `alacritty_terminal`, no un caso especial como este.
#[derive(Clone)]
struct Writer {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Writer {
    fn new(inner: Box<dyn Write + Send>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    fn write_all(&self, bytes: &[u8]) -> SpikeResult<()> {
        self.inner
            .lock()
            .expect("lock del escritor del PTY")
            .write_all(bytes)?;
        Ok(())
    }

    fn flush(&self) -> SpikeResult<()> {
        self.inner
            .lock()
            .expect("lock del escritor del PTY")
            .flush()?;
        Ok(())
    }
}

fn wait_with_timeout(
    child: &mut (dyn portable_pty::Child + Send + Sync),
    timeout: Duration,
) -> SpikeResult<portable_pty::ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if started.elapsed() > timeout {
            return Err(format!("timeout esperando el cierre tras {timeout:?}").into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

struct Reader {
    rx: Receiver<Vec<u8>>,
    tail: Vec<u8>,
}

impl Reader {
    fn spawn(mut reader: Box<dyn Read + Send>, writer: Writer) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 16 * 1024];
            // El DSR puede cruzar el límite de una lectura: guardar los últimos
            // bytes para no perder la secuencia y dejar a `pwsh` esperando.
            let mut scan_tail: Vec<u8> = Vec::new();
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        let chunk = buffer[..read].to_vec();
                        let mut scan = Vec::with_capacity(scan_tail.len() + chunk.len());
                        scan.extend_from_slice(&scan_tail);
                        scan.extend_from_slice(&chunk);
                        let dsr = scan
                            .windows(4)
                            .filter(|window| *window == b"\x1b[6n")
                            .count();
                        for _ in 0..dsr {
                            writer
                                .write_all(b"\x1b[1;1R")
                                .expect("respuesta DSR al PTY");
                        }
                        let keep = scan.len().min(3);
                        scan_tail = scan[scan.len() - keep..].to_vec();
                        if tx.send(chunk).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Self {
            rx,
            tail: Vec::new(),
        }
    }

    fn read_any(&mut self, timeout: Duration) -> SpikeResult<(Duration, usize)> {
        let started = Instant::now();
        match self.rx.recv_timeout(timeout) {
            Ok(chunk) => {
                let bytes = chunk.len();
                self.tail.extend_from_slice(&chunk);
                Ok((started.elapsed(), bytes))
            }
            Err(error) => Err(recv_error(error, &self.tail).into()),
        }
    }

    fn read_until(&mut self, needle: &[u8], timeout: Duration) -> SpikeResult<(Duration, usize)> {
        let started = Instant::now();
        let mut total = 0usize;
        loop {
            if let Some(offset) = find(&self.tail, needle) {
                self.tail.drain(..offset + needle.len());
                return Ok((started.elapsed(), total));
            }

            let remaining = timeout
                .checked_sub(started.elapsed())
                .ok_or_else(|| timeout_error(needle, &self.tail))?;
            match self.rx.recv_timeout(remaining) {
                Ok(chunk) => {
                    total += chunk.len();
                    self.tail.extend_from_slice(&chunk);
                    // Mantener solo lo suficiente para el marcador pendiente.
                    if self.tail.len() > 1 << 20 {
                        let keep = needle.len().max(64);
                        let cut = self.tail.len() - keep;
                        self.tail.drain(..cut);
                    }
                }
                Err(error) => return Err(recv_error(error, &self.tail).into()),
            }
        }
    }

    /// Suma bytes hasta que el PTY cierra (fin del proceso hijo).
    #[expect(
        dead_code,
        reason = "queda como utilidad del spike; el throughput usa marcador"
    )]
    fn read_to_end(&mut self, timeout: Duration) -> SpikeResult<(Duration, usize)> {
        let started = Instant::now();
        let mut total = 0usize;
        loop {
            let remaining = timeout
                .checked_sub(started.elapsed())
                .ok_or("timeout esperando el cierre del PTY")?;
            match self.rx.recv_timeout(remaining) {
                Ok(chunk) => total += chunk.len(),
                Err(RecvTimeoutError::Disconnected) => return Ok((started.elapsed(), total)),
                Err(error) => return Err(recv_error(error, &[]).into()),
            }
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn recv_error(error: RecvTimeoutError, tail: &[u8]) -> String {
    let start = tail.len().saturating_sub(400);
    format!(
        "sin datos del PTY ({error:?}); último output: {:?}",
        String::from_utf8_lossy(&tail[start..])
    )
}

fn timeout_error(needle: &[u8], tail: &[u8]) -> Box<dyn Error> {
    // Recortar la cola: un timeout con megabytes de output hace ilegible el error.
    let start = tail.len().saturating_sub(400);
    let text = format!(
        "timeout esperando {:?}; último output: {:?}",
        String::from_utf8_lossy(needle),
        String::from_utf8_lossy(&tail[start..])
    );
    text.into()
}
