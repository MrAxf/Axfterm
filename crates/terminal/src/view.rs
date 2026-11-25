//! `TerminalView`: entidad GPUI que pinta el grid y traduce la entrada al PTY.
//!
//! El estado de filas vive en un `Rc<RefCell<_>>` compartido con el elemento de
//! render, igual que en el spike S4. El hilo parser es el único que interpreta
//! VT; aquí solo se aplican frames y se shapea lo dañado.

include!("view_parts/01_terminal_context.rs");
include!("view_parts/02_terminalview_new.rs");
include!("view_parts/03_terminalview_mouse_cell.rs");
include!("view_parts/04_std_fmt_debug_for_terminalview_fmt.rs");
include!("view_parts/05_element_for_terminalelement_id.rs");
include!("view_parts/07_terminalview_drop.rs");
include!("view_parts/06_paint_row_background.rs");
