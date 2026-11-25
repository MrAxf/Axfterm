//! Codificación de teclas de GPUI a bytes del PTY.
//!
//! La tabla sale del spike S5. No es la tabla final de M1 (falta el resto de
//! secuencias y la configuración de keybindings), pero cubre lo necesario para
//! usar `pwsh` y `opencode` en el gate de M0.
//!
//! Regla de precedencia: un binding de app se resuelve antes de `on_key_down`;
//! aquí solo llegan teclas que pertenecen a la terminal.

use axf_ui::gpui::Keystroke;

use crate::engine::ModeFlags;

/// Traduce una pulsación a los bytes que espera el PTY.
pub fn encode(keystroke: &Keystroke, prefer_character_input: bool, mode: ModeFlags) -> Vec<u8> {
    let modifiers = keystroke.modifiers;

    // OpenCode, Claude Code y otras TUIs mapean Ctrl+J (LF) a «nueva línea».
    // Shift+Enter no se distingue de Enter en terminales sin protocolo Kitty,
    // así que se envía LF para no someter el prompt. Va antes del input de
    // caracteres para que un `key_char` de control no lo degrade a Enter.
    if modifiers.shift && matches!(keystroke.key.as_str(), "enter" | "return") {
        return vec![b'\n'];
    }

    // AltGr en Windows llega como ctrl+alt. Si el layout produjo un carácter,
    // ese carácter manda y no debe interpretarse como Ctrl+Alt.
    if prefer_character_input && let Some(text) = &keystroke.key_char {
        return text.as_bytes().to_vec();
    }
    if modifiers.control
        && modifiers.alt
        && let Some(text) = &keystroke.key_char
    {
        return text.as_bytes().to_vec();
    }

    // Alt (sin Ctrl) antepone ESC, como cualquier terminal.
    if modifiers.alt
        && !modifiers.control
        && let Some(text) = &keystroke.key_char
    {
        let mut bytes = Vec::with_capacity(text.len() + 1);
        bytes.push(0x1b);
        bytes.extend_from_slice(text.as_bytes());
        return bytes;
    }

    // Ctrl+tecla: el byte de control clásico.
    if modifiers.control && !modifiers.alt && !modifiers.platform {
        if let Some(byte) = control_byte(keystroke.key.as_str()) {
            return vec![byte];
        }
    }

    let app_cursor = mode.app_cursor;
    match keystroke.key.as_str() {
        "enter" | "return" => vec![b'\r'],
        "backtab" => b"\x1b[Z".to_vec(),
        "tab" if modifiers.shift => b"\x1b[Z".to_vec(),
        "tab" => vec![b'\t'],
        "backspace" => vec![0x7f],
        "escape" => vec![0x1b],
        "space" => vec![b' '],
        "up" if app_cursor => b"\x1bOA".to_vec(),
        "down" if app_cursor => b"\x1bOB".to_vec(),
        "right" if app_cursor => b"\x1bOC".to_vec(),
        "left" if app_cursor => b"\x1bOD".to_vec(),
        "up" => b"\x1b[A".to_vec(),
        "down" => b"\x1b[B".to_vec(),
        "right" => b"\x1b[C".to_vec(),
        "left" => b"\x1b[D".to_vec(),
        "home" if app_cursor => b"\x1bOH".to_vec(),
        "end" if app_cursor => b"\x1bOF".to_vec(),
        "home" => b"\x1b[H".to_vec(),
        "end" => b"\x1b[F".to_vec(),
        "insert" => b"\x1b[2~".to_vec(),
        "delete" => b"\x1b[3~".to_vec(),
        "pageup" => b"\x1b[5~".to_vec(),
        "pagedown" => b"\x1b[6~".to_vec(),
        "f1" => b"\x1bOP".to_vec(),
        "f2" => b"\x1bOQ".to_vec(),
        "f3" => b"\x1bOR".to_vec(),
        "f4" => b"\x1bOS".to_vec(),
        "f5" => b"\x1b[15~".to_vec(),
        "f6" => b"\x1b[17~".to_vec(),
        "f7" => b"\x1b[18~".to_vec(),
        "f8" => b"\x1b[19~".to_vec(),
        "f9" => b"\x1b[20~".to_vec(),
        "f10" => b"\x1b[21~".to_vec(),
        "f11" => b"\x1b[23~".to_vec(),
        "f12" => b"\x1b[24~".to_vec(),
        _ => keystroke
            .key_char
            .as_ref()
            .map(|text| text.as_bytes().to_vec())
            .unwrap_or_default(),
    }
}

/// Bytes de control para `Ctrl+tecla`.
fn control_byte(key: &str) -> Option<u8> {
    if key.len() == 1 {
        let byte = key.bytes().next()?;
        if byte.is_ascii_alphabetic() {
            return Some(byte.to_ascii_uppercase() & 0x1f);
        }
    }

    Some(match key {
        "[" => 0x1b,
        "\\" => 0x1c,
        "]" => 0x1d,
        "^" => 0x1e,
        "_" => 0x1f,
        "space" => 0x00,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use axf_ui::gpui::{Keystroke, Modifiers};

    use super::*;

    fn key(key: &str, key_char: Option<&str>, control: bool, alt: bool, shift: bool) -> Keystroke {
        Keystroke {
            modifiers: Modifiers {
                control,
                alt,
                shift,
                ..Default::default()
            },
            key: key.to_owned(),
            key_char: key_char.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn printable_goes_through() {
        let bytes = encode(
            &key("a", Some("a"), false, false, false),
            false,
            ModeFlags::default(),
        );
        assert_eq!(bytes, b"a");
    }

    #[test]
    fn enter_backspace_and_escape() {
        assert_eq!(
            encode(
                &key("enter", None, false, false, false),
                false,
                ModeFlags::default()
            ),
            b"\r"
        );
        assert_eq!(
            encode(
                &key("backspace", None, false, false, false),
                false,
                ModeFlags::default()
            ),
            b"\x7f"
        );
        assert_eq!(
            encode(
                &key("escape", None, false, false, false),
                false,
                ModeFlags::default()
            ),
            b"\x1b"
        );
    }

    #[test]
    fn shift_enter_inserts_a_newline() {
        assert_eq!(
            encode(
                &key("enter", None, false, false, true),
                false,
                ModeFlags::default()
            ),
            b"\n"
        );
        assert_eq!(
            encode(
                &key("enter", Some("\r"), false, false, true),
                true,
                ModeFlags::default()
            ),
            b"\n"
        );
        assert_eq!(
            encode(
                &key("return", None, false, false, true),
                false,
                ModeFlags::default()
            ),
            b"\n"
        );
        assert_eq!(
            encode(
                &key("enter", None, false, false, false),
                false,
                ModeFlags::default()
            ),
            b"\r"
        );
    }

    #[test]
    fn ctrl_c_is_etx() {
        assert_eq!(
            encode(
                &key("c", None, true, false, false),
                false,
                ModeFlags::default()
            ),
            vec![0x03]
        );
    }

    #[test]
    fn altgr_character_wins_over_ctrl_alt() {
        let bytes = encode(
            &key("2", Some("@"), true, true, false),
            false,
            ModeFlags::default(),
        );
        assert_eq!(bytes, b"@");
    }

    #[test]
    fn alt_prepends_escape() {
        let bytes = encode(
            &key("f", Some("f"), false, true, false),
            false,
            ModeFlags::default(),
        );
        assert_eq!(bytes, b"\x1bf");
    }

    #[test]
    fn arrows_follow_app_cursor_mode() {
        let arrow = key("up", None, false, false, false);
        assert_eq!(encode(&arrow, false, ModeFlags::default()), b"\x1b[A");
        assert_eq!(
            encode(
                &arrow,
                false,
                ModeFlags {
                    app_cursor: true,
                    ..Default::default()
                }
            ),
            b"\x1bOA"
        );
    }

    #[test]
    fn shift_tab_is_backtab() {
        assert_eq!(
            encode(
                &key("tab", None, false, false, true),
                false,
                ModeFlags::default()
            ),
            b"\x1b[Z"
        );
        assert_eq!(
            encode(
                &key("backtab", None, false, false, true),
                false,
                ModeFlags::default()
            ),
            b"\x1b[Z"
        );
    }

    #[test]
    fn unknown_key_without_character_is_empty() {
        assert!(
            encode(
                &key("f20", None, false, false, false),
                false,
                ModeFlags::default()
            )
            .is_empty()
        );
    }

    #[test]
    fn navigation_table_matches_xterm() {
        let table: &[(&str, &[u8])] = &[
            ("up", b"\x1b[A"),
            ("down", b"\x1b[B"),
            ("right", b"\x1b[C"),
            ("left", b"\x1b[D"),
            ("home", b"\x1b[H"),
            ("end", b"\x1b[F"),
            ("insert", b"\x1b[2~"),
            ("delete", b"\x1b[3~"),
            ("pageup", b"\x1b[5~"),
            ("pagedown", b"\x1b[6~"),
        ];
        for (name, expected) in table {
            assert_eq!(
                encode(
                    &key(name, None, false, false, false),
                    false,
                    ModeFlags::default()
                ),
                *expected,
                "tecla {name}"
            );
        }
    }

    #[test]
    fn function_table_matches_xterm() {
        let table: &[(&str, &[u8])] = &[
            ("f1", b"\x1bOP"),
            ("f2", b"\x1bOQ"),
            ("f3", b"\x1bOR"),
            ("f4", b"\x1bOS"),
            ("f5", b"\x1b[15~"),
            ("f6", b"\x1b[17~"),
            ("f7", b"\x1b[18~"),
            ("f8", b"\x1b[19~"),
            ("f9", b"\x1b[20~"),
            ("f10", b"\x1b[21~"),
            ("f11", b"\x1b[23~"),
            ("f12", b"\x1b[24~"),
        ];
        for (name, expected) in table {
            assert_eq!(
                encode(
                    &key(name, None, false, false, false),
                    false,
                    ModeFlags::default()
                ),
                *expected,
                "tecla {name}"
            );
        }
    }

    #[test]
    fn ctrl_covers_the_whole_ascii_alphabet() {
        for letter in b'a'..=b'z' {
            let name = (letter as char).to_string();
            let bytes = encode(
                &key(&name, None, true, false, false),
                false,
                ModeFlags::default(),
            );
            assert_eq!(bytes, vec![letter & 0x1f], "ctrl-{name}");
        }
    }

    #[test]
    fn ctrl_punctuation_matches_the_control_table() {
        let table: &[(&str, u8)] = &[
            ("[", 0x1b),
            ("\\", 0x1c),
            ("]", 0x1d),
            ("^", 0x1e),
            ("_", 0x1f),
            ("space", 0x00),
        ];
        for (name, expected) in table {
            assert_eq!(
                encode(
                    &key(name, None, true, false, false),
                    false,
                    ModeFlags::default()
                ),
                vec![*expected],
                "ctrl-{name}"
            );
        }
    }

    #[test]
    fn dead_key_without_a_character_emits_nothing() {
        // Una tecla muerta no produce texto; el carácter compuesto llega por el
        // commit del IME (`replace_text_in_range`), no por esta tabla.
        assert!(
            encode(
                &key("dead", None, false, false, false),
                false,
                ModeFlags::default()
            )
            .is_empty()
        );
    }

    #[test]
    fn prefer_character_input_wins_over_control_sequences() {
        let bytes = encode(
            &key("a", Some("á"), true, false, false),
            true,
            ModeFlags::default(),
        );
        assert_eq!(bytes, "á".as_bytes());
    }

    #[test]
    fn altgr_without_key_char_is_not_a_character() {
        let bytes = encode(
            &key("2", None, true, true, false),
            false,
            ModeFlags::default(),
        );
        assert!(bytes.is_empty());
    }

    #[test]
    fn shift_is_ignored_for_ctrl_letters_in_the_encoder() {
        // La precedencia binding de app -> terminal se resuelve antes de esta
        // tabla: `Ctrl+Shift+C/V` son acciones de Copiar/Pegar (AGENTS.md).
        assert_eq!(
            encode(
                &key("c", None, true, false, true),
                false,
                ModeFlags::default()
            ),
            vec![0x03]
        );
    }
}
