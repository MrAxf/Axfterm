//! Codificación de eventos de ratón para las aplicaciones que activan tracking.
//!
//! Soporta X10 (DECSET 1000/1002/1003) y SGR 1006, que es lo que usan opencode,
//! `vim` y la mayoría de TUIs modernas.

/// Botón del ratón.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// Izquierdo.
    Left,
    /// Central.
    Middle,
    /// Derecho.
    Right,
}

/// Evento que se envía al PTY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    /// Pulsación.
    Press(MouseButton),
    /// Liberación.
    Release(MouseButton),
    /// Movimiento; `None` si no hay botón pulsado.
    Motion(Option<MouseButton>),
    /// Rueda hacia arriba.
    WheelUp,
    /// Rueda hacia abajo.
    WheelDown,
    /// Rueda horizontal a la izquierda.
    WheelLeft,
    /// Rueda horizontal a la derecha.
    WheelRight,
}

/// Modificadores que acompañan al evento.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseModifiers {
    /// Shift.
    pub shift: bool,
    /// Alt.
    pub alt: bool,
    /// Control.
    pub control: bool,
}

impl MouseModifiers {
    fn bits(self) -> u16 {
        u16::from(self.shift) * 4 + u16::from(self.alt) * 8 + u16::from(self.control) * 16
    }
}

/// Codifica un evento de ratón en la secuencia que espera el PTY.
///
/// `column` y `row` son 1-based; se sanean a mínimo 1.
pub fn encode(
    action: MouseAction,
    column: usize,
    row: usize,
    modifiers: MouseModifiers,
    sgr: bool,
    utf8: bool,
) -> Vec<u8> {
    let (button, final_byte) = match action {
        MouseAction::Press(button) => (button_code(button), b'M'),
        MouseAction::Release(button) => {
            if sgr {
                (button_code(button), b'm')
            } else {
                (3, b'M')
            }
        }
        MouseAction::Motion(button) => (32 + button.map_or(3, button_code), b'M'),
        MouseAction::WheelUp => (64, b'M'),
        MouseAction::WheelDown => (65, b'M'),
        MouseAction::WheelLeft => (66, b'M'),
        MouseAction::WheelRight => (67, b'M'),
    };

    let code = button + modifiers.bits();
    let column = column.max(1);
    let row = row.max(1);

    if sgr {
        return format!("\x1b[<{code};{column};{row}{}", final_byte as char).into_bytes();
    }

    let mut bytes = Vec::with_capacity(6);
    bytes.extend_from_slice(b"\x1b[M");
    push_x10(&mut bytes, u32::from(code), utf8);
    push_x10(&mut bytes, u32::try_from(column).unwrap_or(u32::MAX), utf8);
    push_x10(&mut bytes, u32::try_from(row).unwrap_or(u32::MAX), utf8);
    bytes
}

fn button_code(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

/// Añade un valor X10 sumando 32. Sin UTF-8 se limita a 223 para que el byte
/// resultante quepa en 8 bits.
fn push_x10(bytes: &mut Vec<u8>, value: u32, utf8: bool) {
    let adjusted = value.saturating_add(32);
    if utf8 {
        if let Some(ch) = char::from_u32(adjusted) {
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
            return;
        }
    }
    bytes.push(adjusted.min(255) as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sgr(action: MouseAction) -> Vec<u8> {
        encode(action, 10, 5, MouseModifiers::default(), true, false)
    }

    fn x10(action: MouseAction) -> Vec<u8> {
        encode(action, 10, 5, MouseModifiers::default(), false, false)
    }

    #[test]
    fn sgr_press_and_release() {
        assert_eq!(sgr(MouseAction::Press(MouseButton::Left)), b"\x1b[<0;10;5M");
        assert_eq!(
            sgr(MouseAction::Release(MouseButton::Left)),
            b"\x1b[<0;10;5m"
        );
        assert_eq!(
            sgr(MouseAction::Press(MouseButton::Right)),
            b"\x1b[<2;10;5M"
        );
    }

    #[test]
    fn x10_press_and_release() {
        assert_eq!(
            x10(MouseAction::Press(MouseButton::Left)),
            [b"\x1b[M".as_slice(), &[32, 42, 37]].concat()
        );
        assert_eq!(
            x10(MouseAction::Release(MouseButton::Left)),
            [b"\x1b[M".as_slice(), &[35, 42, 37]].concat()
        );
    }

    #[test]
    fn wheel_codes() {
        assert_eq!(sgr(MouseAction::WheelUp), b"\x1b[<64;10;5M");
        assert_eq!(sgr(MouseAction::WheelDown), b"\x1b[<65;10;5M");
        assert_eq!(sgr(MouseAction::WheelLeft), b"\x1b[<66;10;5M");
        assert_eq!(sgr(MouseAction::WheelRight), b"\x1b[<67;10;5M");
    }

    #[test]
    fn modifiers_are_added() {
        let bytes = encode(
            MouseAction::Press(MouseButton::Left),
            10,
            5,
            MouseModifiers {
                shift: true,
                alt: true,
                control: true,
            },
            true,
            false,
        );
        assert_eq!(bytes, b"\x1b[<28;10;5M");
    }

    #[test]
    fn motion_without_button_uses_35() {
        assert_eq!(sgr(MouseAction::Motion(None)), b"\x1b[<35;10;5M");
        assert_eq!(
            sgr(MouseAction::Motion(Some(MouseButton::Left))),
            b"\x1b[<32;10;5M"
        );
    }

    #[test]
    fn coordinates_are_clamped() {
        let bytes = encode(
            MouseAction::Press(MouseButton::Left),
            0,
            0,
            MouseModifiers::default(),
            true,
            false,
        );
        assert_eq!(bytes, b"\x1b[<0;1;1M");
    }

    #[test]
    fn x10_utf8_extends_large_coordinates() {
        let bytes = encode(
            MouseAction::Press(MouseButton::Left),
            300,
            5,
            MouseModifiers::default(),
            false,
            true,
        );
        assert_eq!(&bytes[..4], b"\x1b[M\x20");
        // 300 + 32 = 332 = U+014C -> C5 8C en UTF-8.
        assert_eq!(&bytes[4..6], &[0xC5, 0x8C]);
        assert_eq!(bytes[6], 37);
    }
}
