//! Normalización del texto pegado antes de escribirlo al PTY.
//!
//! El portapapeles de Windows entrega `\r\n`; el PTY espera `\r` para "Enter".
//! Cuando la aplicación pide bracketed paste (DECSET 2004), el texto se envuelve
//! y los saltos se dejan como `\n` para que la aplicación decida qué hacer con
//! el pegado multilínea en vez de ejecutarlo línea a línea.

/// Envoltorio inicial de bracketed paste.
pub const BRACKETED_PASTE_START: &[u8] = b"\x1b[200~";
/// Envoltorio final de bracketed paste.
pub const BRACKETED_PASTE_END: &[u8] = b"\x1b[201~";

/// Convierte texto del portapapeles en los bytes que van al PTY.
///
/// - `\r\n` y `\r` sueltos se normalizan a `\n`.
/// - Se descartan los NUL (Windows los deja en algunos portapapeles).
/// - Con `bracketed`, el texto se envuelve y conserva los `\n`.
/// - Sin `bracketed`, cada `\n` se convierte en `\r` (Enter del PTY).
///
/// Un texto vacío no produce bytes: pegar "nada" no debe enviar envoltorios.
pub fn normalize_paste(text: &str, bracketed: bool) -> Vec<u8> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                normalized.push('\n');
            }
            '\0' => {}
            other => normalized.push(other),
        }
    }

    let mut bytes = Vec::with_capacity(normalized.len() + BRACKETED_PASTE_START.len() + 8);
    if bracketed {
        bytes.extend_from_slice(BRACKETED_PASTE_START);
        bytes.extend_from_slice(normalized.as_bytes());
        bytes.extend_from_slice(BRACKETED_PASTE_END);
        return bytes;
    }

    for ch in normalized.chars() {
        if ch == '\n' {
            bytes.push(b'\r');
        } else {
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> Vec<u8> {
        normalize_paste(text, false)
    }

    fn bracketed(text: &str) -> Vec<u8> {
        normalize_paste(text, true)
    }

    #[test]
    fn empty_paste_sends_nothing() {
        assert!(plain("").is_empty());
        assert!(bracketed("").is_empty());
    }

    #[test]
    fn crlf_becomes_enter() {
        assert_eq!(plain("a\r\nb"), b"a\rb");
    }

    #[test]
    fn lone_cr_becomes_enter() {
        assert_eq!(plain("a\rb"), b"a\rb");
    }

    #[test]
    fn lf_becomes_enter() {
        assert_eq!(plain("a\nb"), b"a\rb");
    }

    #[test]
    fn multiline_executes_each_line() {
        assert_eq!(plain("echo 1\r\necho 2"), b"echo 1\recho 2");
    }

    #[test]
    fn bracketed_wraps_and_keeps_newlines() {
        let bytes = bracketed("a\r\nb");
        let mut expected = BRACKETED_PASTE_START.to_vec();
        expected.extend_from_slice(b"a\nb");
        expected.extend_from_slice(BRACKETED_PASTE_END);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn nul_is_dropped() {
        assert_eq!(plain("a\0b"), b"ab");
        assert_eq!(bracketed("a\0b"), {
            let mut expected = BRACKETED_PASTE_START.to_vec();
            expected.extend_from_slice(b"ab");
            expected.extend_from_slice(BRACKETED_PASTE_END);
            expected
        });
    }

    #[test]
    fn tabs_and_unicode_pass_through() {
        assert_eq!(plain("a\tbñ"), "a\tbñ".as_bytes());
        assert_eq!(plain("á\r\nü"), "á\rü".as_bytes());
    }

    #[test]
    fn windows_path_backslashes_are_preserved() {
        assert_eq!(plain(r"C:\Proyectos\axfterm"), br"C:\Proyectos\axfterm");
    }
}
