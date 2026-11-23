//! Estado de composición IME: preedit separado del commit.
//!
//! El overlay de preedit se pinta en la vista; este módulo solo guarda el texto
//! marcado y decide qué se escribe al PTY. Así la lógica se puede probar sin
//! ventana ni PTY.

use std::ops::Range;

/// Texto marcado por el IME antes del commit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImeState {
    preedit: String,
}

impl ImeState {
    /// `true` si hay texto marcado.
    pub fn is_empty(&self) -> bool {
        self.preedit.is_empty()
    }

    /// Texto marcado actual.
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// Rango UTF-16 que el IME debe considerar marcado.
    pub fn marked_range(&self) -> Option<Range<usize>> {
        (!self.preedit.is_empty()).then(|| 0..self.preedit.encode_utf16().count())
    }

    /// Reemplaza el texto marcado sin escribirlo al PTY.
    pub fn set_preedit(&mut self, text: &str) {
        self.preedit.clear();
        self.preedit.push_str(text);
    }

    /// Quita el texto marcado.
    pub fn clear(&mut self) {
        self.preedit.clear();
    }

    /// Confirma un texto: limpia el preedit y devuelve los bytes para el PTY.
    pub fn commit(&mut self, text: &str) -> Vec<u8> {
        self.preedit.clear();
        text.as_bytes().to_vec()
    }

    /// Texto para un rango pedido por la plataforma. Solo existe el preedit.
    pub fn text_for_range(&self, range: Range<usize>) -> Option<String> {
        if self.preedit.is_empty() {
            return None;
        }
        let units: Vec<u16> = self.preedit.encode_utf16().collect();
        let start = range.start.min(units.len());
        let end = range.end.min(units.len());
        String::from_utf16(&units[start..end]).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preedit_is_not_committed() {
        let mut ime = ImeState::default();
        ime.set_preedit("に");
        assert_eq!(ime.preedit(), "に");
        assert_eq!(ime.marked_range(), Some(0..1));
        assert!(ime.commit("").is_empty());
        assert!(ime.is_empty());
    }

    #[test]
    fn commit_clears_preedit_and_returns_bytes() {
        let mut ime = ImeState::default();
        ime.set_preedit("にほ");
        let bytes = ime.commit("日本");
        assert_eq!(bytes, "日本".as_bytes());
        assert!(ime.is_empty());
    }

    #[test]
    fn marked_range_uses_utf16_units() {
        let mut ime = ImeState::default();
        ime.set_preedit("日本");
        assert_eq!(ime.marked_range(), Some(0..2));
        ime.set_preedit("🙂");
        assert_eq!(ime.marked_range(), Some(0..2));
    }

    #[test]
    fn text_for_range_returns_only_the_preedit() {
        let mut ime = ImeState::default();
        assert_eq!(ime.text_for_range(0..1), None);
        ime.set_preedit("ok");
        assert_eq!(ime.text_for_range(0..2).as_deref(), Some("ok"));
        assert_eq!(ime.text_for_range(0..8).as_deref(), Some("ok"));
        assert_eq!(ime.text_for_range(2..2).as_deref(), Some(""));
    }
}
