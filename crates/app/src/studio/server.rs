//! Servidor TCP local de Studio.
//!
//! Una petición JSON por línea y una respuesta por línea. Solo escucha en
//! loopback y exige el token escrito en `session.json`.

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_channel::Sender;
use serde::{Deserialize, Serialize};

use super::protocol::{Reply, Request};

/// Trabajo entregado al hilo de UI.
pub(super) struct Incoming {
    pub request: Request,
    pub reply: SyncSender<String>,
}

/// Servidor en marcha.
pub(crate) struct ServerHandle {
    stop: Arc<AtomicBool>,
    session_path: PathBuf,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&self.session_path);
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionFile {
    pid: u32,
    port: u16,
    token: String,
    shots_dir: String,
    dir: String,
}

const MAX_CLIENTS: usize = 8;
const MAX_LINE: usize = 64 * 1024;
const REPLY_TIMEOUT: Duration = Duration::from_secs(60);

/// Arranca el listener y escribe `session.json`.
pub(super) fn start(dir: &Path, port: u16, tx: Sender<Incoming>) -> std::io::Result<ServerHandle> {
    std::fs::create_dir_all(dir)?;
    let shots = dir.join("shots");
    std::fs::create_dir_all(&shots)?;
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    listener.set_nonblocking(true)?;
    let actual_port = listener.local_addr()?.port();
    let token = make_token();
    let stop = Arc::new(AtomicBool::new(false));
    let clients = Arc::new(AtomicUsize::new(0));
    let session_path = dir.join("session.json");
    let session = SessionFile {
        pid: std::process::id(),
        port: actual_port,
        token: token.clone(),
        shots_dir: shots.display().to_string(),
        dir: dir.display().to_string(),
    };
    write_session(&session_path, &session)?;

    let stop_thread = Arc::clone(&stop);
    let clients_thread = Arc::clone(&clients);
    let token_thread = token.clone();
    thread::Builder::new()
        .name("axfterm-studio".into())
        .spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        if clients_thread.load(Ordering::Relaxed) >= MAX_CLIENTS {
                            write_error(stream, "busy");
                            continue;
                        }
                        clients_thread.fetch_add(1, Ordering::Relaxed);
                        let tx = tx.clone();
                        let token = token_thread.clone();
                        let stop = Arc::clone(&stop_thread);
                        let clients = Arc::clone(&clients_thread);
                        thread::Builder::new()
                            .name("axfterm-studio-client".into())
                            .spawn(move || {
                                handle_client(stream, tx, &token, &stop);
                                clients.fetch_sub(1, Ordering::Relaxed);
                            })
                            .ok();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => break,
                }
            }
        })?;

    Ok(ServerHandle { stop, session_path })
}

fn handle_client(stream: TcpStream, tx: Sender<Incoming>, token: &str, stop: &AtomicBool) {
    let Ok(read_stream) = stream.try_clone() else {
        return;
    };
    let mut writer = stream;
    let mut reader = BufReader::new(read_stream);
    let mut line = String::new();
    loop {
        line.clear();
        let Ok(read) = reader.read_line(&mut line) else {
            return;
        };
        if read == 0 || stop.load(Ordering::Relaxed) {
            return;
        }
        if line.len() > MAX_LINE {
            let _ = writeln!(
                writer,
                "{}",
                serde_json::to_string(&Reply::error(0, "línea demasiado larga"))
                    .unwrap_or_else(|_| "{\"id\":0,\"ok\":false}".into())
            );
            continue;
        }
        let request: Request = match serde_json::from_str(line.trim()) {
            Ok(request) => request,
            Err(error) => {
                let reply = Reply::error(0, format!("petición inválida: {error}"));
                let _ = writeln!(
                    writer,
                    "{}",
                    serde_json::to_string(&reply)
                        .unwrap_or_else(|_| "{\"id\":0,\"ok\":false}".into())
                );
                continue;
            }
        };
        if request.token != token {
            let reply = Reply::error(request.id, "token inválido");
            let _ = writeln!(
                writer,
                "{}",
                serde_json::to_string(&reply).unwrap_or_else(|_| "{\"id\":0,\"ok\":false}".into())
            );
            continue;
        }
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        if tx
            .send_blocking(Incoming {
                request,
                reply: reply_tx,
            })
            .is_err()
        {
            return;
        }
        match reply_rx.recv_timeout(REPLY_TIMEOUT) {
            Ok(response) => {
                if writeln!(writer, "{response}").is_err() {
                    return;
                }
            }
            Err(_) => {
                let timeout = Reply::error(0, "tiempo de espera agotado");
                let _ = writeln!(
                    writer,
                    "{}",
                    serde_json::to_string(&timeout)
                        .unwrap_or_else(|_| "{\"id\":0,\"ok\":false}".into())
                );
                return;
            }
        }
    }
}

fn write_error(mut stream: TcpStream, message: &str) {
    let reply = Reply::error(0, message);
    let _ = writeln!(
        stream,
        "{}",
        serde_json::to_string(&reply).unwrap_or_else(|_| "{\"id\":0,\"ok\":false}".into())
    );
}

fn write_session(path: &Path, session: &SessionFile) -> std::io::Result<()> {
    let body = serde_json::to_vec_pretty(session)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, body)?;
    std::fs::rename(&temporary, path)
}

fn make_token() -> String {
    let mut hasher = RandomState::new().build_hasher();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    hasher.write_u128(now);
    hasher.write_u32(std::process::id());
    format!("{:016x}{:016x}", hasher.finish(), now as u64)
}

#[cfg(test)]
mod tests {
    use super::make_token;

    #[test]
    fn tokens_are_long_and_unique() {
        let first = make_token();
        let second = make_token();
        assert!(first.len() >= 16);
        assert_ne!(first, second);
    }
}
