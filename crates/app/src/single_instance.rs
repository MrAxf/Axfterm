//! Instancia única y apertura de carpetas desde CLI/Explorer (M9).
//!
//! En Windows, la primera instancia crea un mutex con nombre y un named pipe
//! local. Las siguientes reenvían sus argumentos por el pipe y salen. El pipe
//! es local a la máquina y no usa red. En otras plataformas el módulo deja
//! pasar todas las instancias (el empaquetado de v0.1 es Windows).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use async_channel::{Receiver, Sender};

/// Nombre del mutex de instancia.
#[cfg(windows)]
const MUTEX_NAME: &str = "Local\\Axfterm.SingleInstance";

/// Nombre del pipe local; `PIPE_REJECT_REMOTE_CLIENTS` impide conexiones remotas.
#[cfg(windows)]
const PIPE_NAME: &str = r"\\.\pipe\axfterm-single-instance";

/// Tope de un mensaje reenviado.
pub(crate) const MAX_MESSAGE_BYTES: usize = 64 * 1024;

static RECEIVER: OnceLock<Receiver<Vec<PathBuf>>> = OnceLock::new();

/// Resultado de intentar tomar la instancia.
pub(crate) enum Instance {
    /// Esta es la instancia principal; el guard debe vivir hasta el cierre.
    Primary(InstanceGuard),
    /// Ya había una instancia; los argumentos se reenvían y se sale.
    Secondary,
}

/// Mantiene vivo el mutex y el hilo del pipe.
pub(crate) struct InstanceGuard {
    #[cfg(windows)]
    mutex: windows_sys::Win32::Foundation::HANDLE,
}

impl std::fmt::Debug for InstanceGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("InstanceGuard")
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if !self.mutex.is_null() {
                // SAFETY: handle creado por CreateMutexW y aún no cerrado.
                unsafe { windows_sys::Win32::Foundation::CloseHandle(self.mutex) };
            }
        }
    }
}

/// Toma la instancia y prepara el receptor de rutas.
pub(crate) fn acquire() -> Instance {
    #[cfg(windows)]
    {
        match acquire_windows() {
            Some(mutex) => Instance::Primary(InstanceGuard { mutex }),
            None => Instance::Secondary,
        }
    }
    #[cfg(not(windows))]
    {
        Instance::Primary(InstanceGuard {})
    }
}

/// Canal por el que la instancia principal recibe rutas de las secundarias.
/// Solo se puede tomar una vez; el shell lo consume en una tarea de UI.
pub(crate) fn take_receiver() -> Option<Receiver<Vec<PathBuf>>> {
    RECEIVER.get().cloned()
}

/// Reenvía las rutas a la instancia principal. Devuelve `false` si no había
/// ninguna escuchando (por ejemplo, si acaba de cerrarse).
pub(crate) fn forward(paths: &[PathBuf]) -> bool {
    #[cfg(windows)]
    {
        forward_windows(paths)
    }
    #[cfg(not(windows))]
    {
        let _ = paths;
        false
    }
}

/// Normaliza un argumento de CLI/Explorer: resuelve el archivo a su carpeta y
/// elimina el prefijo `\\?\` que `canonicalize` añade en Windows.
pub(crate) fn project_folder(path: PathBuf) -> PathBuf {
    let candidate = std::fs::canonicalize(&path).unwrap_or(path);
    let candidate = strip_verbatim(candidate);
    match std::fs::metadata(&candidate) {
        Ok(metadata) if metadata.is_file() => candidate
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(candidate),
        _ => candidate,
    }
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.as_os_str().to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest.to_string());
        }
    }
    path
}

/// Serializa rutas como líneas UTF-8; rechaza mensajes por encima del tope.
pub(crate) fn encode_message(paths: &[PathBuf]) -> Vec<u8> {
    let mut text = String::new();
    for path in paths {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&path.to_string_lossy());
    }
    text.push('\n');
    text.into_bytes()
}

/// Decodifica el mensaje del pipe ignorando líneas vacías.
pub(crate) fn decode_message(bytes: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(windows)]
fn acquire_windows() -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let name = wide(MUTEX_NAME);
    // SAFETY: nombre UTF-16 terminado en NUL; sin atributos de seguridad.
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if mutex.is_null() {
        // Sin mutex no se puede garantizar la instancia única; se deja pasar
        // como principal para no impedir el arranque.
        return Some(mutex);
    }
    // SAFETY: GetLastError se lee inmediatamente después de CreateMutexW.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // SAFETY: handle válido; esta instancia no lo usará.
        unsafe { CloseHandle(mutex) };
        return None;
    }

    let (sender, receiver) = async_channel::bounded(16);
    let _ = RECEIVER.set(receiver);
    std::thread::Builder::new()
        .name("axf-instance-pipe".into())
        .spawn(move || pipe_loop(sender))
        .ok();
    Some(mutex)
}

#[cfg(windows)]
fn pipe_loop(sender: Sender<Vec<PathBuf>>) {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile, WriteFile};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    let name = wide(PIPE_NAME);
    loop {
        // SAFETY: nombre válido y buffers acotados. Sin SECURITY_ATTRIBUTES.
        let pipe = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_INBOUND,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                0,
                MAX_MESSAGE_BYTES as u32,
                0,
                std::ptr::null(),
            )
        };
        if pipe == INVALID_HANDLE_VALUE {
            return;
        }
        // SAFETY: `pipe` es un named pipe recién creado.
        let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) } != 0;
        // SAFETY: se consulta el error justo después de la llamada.
        let connected = connected || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        if connected {
            let mut buffer = vec![0u8; MAX_MESSAGE_BYTES];
            let mut read = 0u32;
            // SAFETY: buffer propio y longitud correcta.
            let ok = unsafe {
                ReadFile(
                    pipe,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    &mut read,
                    std::ptr::null_mut(),
                )
            } != 0;
            if ok {
                let paths = decode_message(&buffer[..usize::try_from(read).unwrap_or(0)]);
                if !paths.is_empty() && sender.send_blocking(paths).is_err() {
                    // El shell ya no escucha; se cierra el servidor.
                    // SAFETY: handle del pipe.
                    unsafe {
                        DisconnectNamedPipe(pipe);
                        CloseHandle(pipe);
                    }
                    return;
                }
            }
            // Respuesta mínima para que el cliente sepa que se procesó.
            let mut written = 0u32;
            // SAFETY: escritura sobre el pipe conectado.
            unsafe {
                WriteFile(pipe, b"ok".as_ptr(), 2, &mut written, std::ptr::null_mut());
            }
        }
        // SAFETY: se desconecta y se cierra el handle del ciclo.
        unsafe {
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
        }
    }
}

#[cfg(windows)]
fn forward_windows(paths: &[PathBuf]) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING, WriteFile};

    if paths.is_empty() {
        return true;
    }
    let message = encode_message(paths);
    if message.len() > MAX_MESSAGE_BYTES {
        return false;
    }
    let name = wide(PIPE_NAME);

    for attempt in 0..20 {
        // SAFETY: nombre válido y sin atributos de seguridad.
        let pipe = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if pipe != INVALID_HANDLE_VALUE {
            let mut written = 0u32;
            // SAFETY: escritura del mensaje en el pipe abierto.
            let ok = unsafe {
                WriteFile(
                    pipe,
                    message.as_ptr(),
                    message.len() as u32,
                    &mut written,
                    std::ptr::null_mut(),
                )
            } != 0;
            // SAFETY: handle válido.
            unsafe { CloseHandle(pipe) };
            return ok;
        }
        if attempt + 1 < 20 {
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
    false
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "axfterm-instance-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn message_round_trip_keeps_paths_with_spaces() {
        let paths = vec![
            PathBuf::from(r"C:\Proyectos\con espacio\app"),
            PathBuf::from("/home/juan/código"),
        ];
        let message = encode_message(&paths);
        assert!(message.len() < MAX_MESSAGE_BYTES);
        assert_eq!(decode_message(&message), paths);
    }

    #[test]
    fn empty_lines_are_ignored() {
        assert!(decode_message(b"\r\n\n").is_empty());
    }

    #[test]
    fn a_file_argument_opens_its_folder() {
        let dir = temp_dir("file");
        let file = dir.join("nota.txt");
        std::fs::write(&file, b"hola").expect("archivo");
        let folder = project_folder(file);
        assert_eq!(
            folder,
            std::fs::canonicalize(&dir)
                .map(strip_verbatim)
                .expect("dir")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_directory_argument_is_kept() {
        let dir = temp_dir("dir");
        let folder = project_folder(dir.clone());
        assert!(folder.is_dir());
        assert!(!folder.to_string_lossy().starts_with(r"\\?\"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
