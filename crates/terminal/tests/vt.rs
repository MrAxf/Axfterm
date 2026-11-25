//! Fixtures VT: casos de referencia que fijan el comportamiento del motor.

use std::path::PathBuf;

use axf_terminal::{
    CellColor, CursorShape, EngineBench, MouseMode, ProgramState, ScreenFrame, SelectionPoint,
    SnapshotCell, SnapshotError, TerminalEvent, TerminalSnapshot, effective_scrollback,
};

fn cell(frame: &ScreenFrame, row: usize, col: usize) -> SnapshotCell {
    let line = frame
        .updates
        .iter()
        .find(|update| update.line == row)
        .expect("la fila solicitada está en el frame");
    line.cells[col]
}

fn row_text(frame: &ScreenFrame, row: usize) -> String {
    frame
        .updates
        .iter()
        .find(|update| update.line == row)
        .expect("la fila solicitada está en el frame")
        .cells
        .iter()
        .map(|cell| cell.ch)
        .collect()
}

fn program_statuses(frame: &ScreenFrame) -> Vec<axf_terminal::ProgramStatus> {
    frame
        .events
        .iter()
        .filter_map(|event| match event {
            TerminalEvent::ProgramStatus(status) => Some(status.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn scrollback_never_exceeds_the_memory_budget() {
    // El ancho encoge el número de líneas efectivas; nunca supera lo pedido.
    let narrow = effective_scrollback(50_000, 80);
    let wide = effective_scrollback(50_000, 200);
    assert!(narrow > 0);
    assert!(narrow <= 50_000);
    assert!(wide < narrow);
    assert_eq!(effective_scrollback(0, 200), 0);
    assert_eq!(effective_scrollback(10_000, 0), 0);
}

#[test]
fn sgr_16_colors_and_attributes() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance(b"\x1b[1;31mX\x1b[0m");
    let frame = engine.frame();
    let x = cell(&frame, 0, 0);
    assert_eq!(x.ch, 'X');
    assert!(x.attrs.bold);
    assert_eq!(x.fg, CellColor::Indexed(1));
    assert!(!x.attrs.italic);
}

#[test]
fn sgr_256_and_truecolor() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance(b"\x1b[38;5;196mA\x1b[38;2;1;2;3mB");
    let frame = engine.frame();
    assert_eq!(cell(&frame, 0, 0).fg, CellColor::Indexed(196));
    assert_eq!(cell(&frame, 0, 1).fg, CellColor::Rgb(0x01_02_03));
}

#[test]
fn underline_reverse_dim_and_strike() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance(b"\x1b[4;7;2;9mA");
    let frame = engine.frame();
    let a = cell(&frame, 0, 0);
    assert!(a.attrs.underline);
    assert!(a.attrs.reverse);
    assert!(a.attrs.dim);
    assert!(a.attrs.strikethrough);
}

#[test]
fn alt_screen_preserves_the_main_screen() {
    let mut engine = EngineBench::new(20, 4, 8, 16);
    engine.advance(b"main");
    engine.advance(b"\x1b[?1049h\x1b[HALT");
    let frame = engine.frame();
    assert!(frame.mode.alt_screen);
    assert_eq!(cell(&frame, 0, 0).ch, 'A');

    engine.advance(b"\x1b[?1049l");
    let frame = engine.frame();
    assert!(!frame.mode.alt_screen);
    assert_eq!(row_text(&frame, 0).trim_end(), "main");
}

#[test]
fn wrap_moves_to_the_next_line() {
    let mut engine = EngineBench::new(5, 3, 8, 16);
    engine.advance(b"abcdef");
    let frame = engine.frame();
    assert_eq!(row_text(&frame, 0), "abcde");
    assert_eq!(cell(&frame, 1, 0).ch, 'f');
}

#[test]
fn cursor_position_and_shape() {
    let mut engine = EngineBench::new(10, 5, 8, 16);
    engine.advance(b"\x1b[2;3H");
    let frame = engine.frame();
    let cursor = frame.cursor.expect("cursor visible");
    assert_eq!((cursor.line, cursor.column), (1, 2));
    assert_eq!(cursor.shape, CursorShape::Block);

    engine.advance(b"\x1b[3 q");
    let frame = engine.frame();
    assert_eq!(
        frame.cursor.expect("cursor visible").shape,
        CursorShape::Underline
    );

    engine.advance(b"\x1b[5 q");
    let frame = engine.frame();
    assert_eq!(
        frame.cursor.expect("cursor visible").shape,
        CursorShape::Beam
    );
}

#[test]
fn scrollback_and_display_offset() {
    let mut engine = EngineBench::new(10, 4, 8, 16);
    for line in 0..12 {
        engine.advance(format!("linea {line}\r\n").as_bytes());
    }
    let frame = engine.frame();
    assert!(frame.history_size >= 4);

    engine.scroll(2);
    let frame = engine.frame();
    assert_eq!(frame.display_offset, 2);

    engine.scroll(-2);
    let frame = engine.frame();
    assert_eq!(frame.display_offset, 0);
}

#[test]
fn wide_characters_have_a_spacer_cell() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance("界".as_bytes());
    let frame = engine.frame();
    let first = cell(&frame, 0, 0);
    let second = cell(&frame, 0, 1);
    assert_eq!(first.ch, '界');
    assert!(first.attrs.wide);
    assert!(second.attrs.wide_spacer);
}

#[test]
fn combining_characters_travel_with_the_base_cell() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance("e\u{301}".as_bytes());
    let frame = engine.frame();
    let row = frame
        .updates
        .iter()
        .find(|update| update.line == 0)
        .expect("fila 0");
    assert_eq!(row.cells[0].ch, 'e');
    assert!(row.combining.contains(&(0, '\u{301}')));
}

#[test]
fn osc8_hyperlinks_are_reported_per_row() {
    let mut engine = EngineBench::new(40, 3, 8, 16);
    engine.advance(b"\x1b]8;;https://example.com\x07link\x1b]8;;\x07");
    let frame = engine.frame();
    let row = frame
        .updates
        .iter()
        .find(|update| update.line == 0)
        .expect("fila 0");
    assert_eq!(row.links.len(), 1);
    assert_eq!(row.links[0].uri, "https://example.com");
    assert_eq!(row.links[0].start, 0);
    assert_eq!(row.links[0].end, 4);
}

#[test]
fn truncated_osc_does_not_swallow_the_next_one() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance(b"\x1b]7;file:///C:/bad\x1b[31m");
    engine.advance(b"\x1b]7;file:///C:/good\x07");
    let frame = engine.frame();
    let saw_good = frame.events.iter().any(|event| match event {
        TerminalEvent::Cwd(path) => path.ends_with("good"),
        _ => false,
    });
    assert!(saw_good, "eventos: {:?}", frame.events);
}

#[test]
fn osc22_reports_the_requested_pointer() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance(b"\x1b]22;pointer\x07");
    let frame = engine.frame();
    assert!(
        frame
            .events
            .contains(&TerminalEvent::MouseCursor("pointer".into()))
    );
}

#[test]
fn osc_title_and_bell_are_events() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance(b"\x1b]2;hola\x07\x07");
    let frame = engine.frame();
    assert!(
        frame
            .events
            .contains(&TerminalEvent::Title(Some("hola".into())))
    );
    assert!(frame.events.contains(&TerminalEvent::Bell));
}

#[cfg(windows)]
#[test]
fn osc7_reports_a_windows_cwd() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance(b"\x1b]7;file:///C:/Users/Juan%20Andres/axfterm\x07");
    let frame = engine.frame();
    assert!(frame.events.contains(&TerminalEvent::Cwd(PathBuf::from(
        r"C:\Users\Juan Andres\axfterm"
    ))));
}

#[cfg(not(windows))]
#[test]
fn osc7_reports_a_unix_cwd() {
    let mut engine = EngineBench::new(10, 3, 8, 16);
    engine.advance(b"\x1b]7;file:///tmp/axfterm\x07");
    let frame = engine.frame();
    assert!(
        frame
            .events
            .contains(&TerminalEvent::Cwd(PathBuf::from("/tmp/axfterm")))
    );
}

#[test]
fn selection_returns_the_selected_text() {
    let mut engine = EngineBench::new(20, 4, 8, 16);
    engine.advance(b"hola\r\nmundo");
    let text = engine
        .select_and_copy(
            SelectionPoint { row: 0, col: 0 },
            SelectionPoint { row: 0, col: 3 },
            false,
        )
        .expect("selección no vacía");
    assert_eq!(text, "hola");
}

#[test]
fn resize_reflows_the_visible_grid() {
    let mut engine = EngineBench::new(20, 4, 8, 16);
    engine.advance(b"abcdefghijklmno");
    engine.resize(8, 4);
    let frame = engine.frame();
    assert_eq!(frame.cols, 8);
    let visible: String = (0..frame.rows)
        .map(|row| row_text(&frame, row))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(visible.contains("ijklmno"), "visible: {visible:?}");
    assert!(frame.history_size >= 1);

    engine.resize(20, 4);
    engine.scroll(100);
    let frame = engine.frame();
    assert_eq!(frame.cols, 20);
    let all: String = (0..frame.rows)
        .map(|row| row_text(&frame, row))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all.contains("abcdefghijklmno"), "historial: {all:?}");
}

#[test]
fn private_modes_are_exposed_to_input_and_render() {
    let mut engine = EngineBench::new(20, 4, 8, 16);
    engine.advance(b"\x1b[?2004h\x1b[?1004h\x1b[?1000h\x1b[?1006h");
    let frame = engine.frame();
    assert!(frame.mode.bracketed_paste);
    assert!(frame.mode.focus_in_out);
    assert_eq!(frame.mode.mouse_mode, MouseMode::Click);
    assert!(frame.mode.sgr_mouse);
}

#[test]
fn snapshot_roundtrip_restores_text_and_history() {
    let mut engine = EngineBench::new(20, 4, 8, 16);
    for line in 0..10 {
        engine.advance(format!("linea {line}\r\n").as_bytes());
    }
    let snapshot = engine.snapshot();
    assert!(!snapshot.is_empty());
    assert!(!snapshot.truncated());
    assert_eq!((snapshot.cols(), snapshot.rows()), (20, 4));

    let decoded = TerminalSnapshot::decode(&snapshot.encode()).expect("decodifica");
    let mut restored = EngineBench::from_snapshot(decoded, 20, 4, 8, 16);
    let frame = restored.frame();
    assert!(frame.history_size >= 6, "historial: {}", frame.history_size);
    assert_eq!(row_text(&frame, 2).trim_end(), "linea 9");
    assert!(frame.events.is_empty(), "la reproducción no emite eventos");

    restored.scroll(1);
    let frame = restored.frame();
    assert_eq!(row_text(&frame, 3).trim_end(), "linea 9");
}

#[test]
fn snapshot_roundtrip_preserves_colors_and_wide_chars() {
    let mut engine = EngineBench::new(12, 2, 8, 16);
    engine.advance("A\x1b[31m红\x1b[0mB".as_bytes());
    let snapshot = engine.snapshot();

    let decoded = TerminalSnapshot::decode(&snapshot.encode()).expect("decodifica");
    let mut restored = EngineBench::from_snapshot(decoded, 12, 2, 8, 16);
    let frame = restored.frame();
    assert_eq!(cell(&frame, 0, 0).ch, 'A');
    assert_eq!(cell(&frame, 0, 1).ch, '红');
    assert_eq!(cell(&frame, 0, 1).fg, CellColor::Indexed(1));
    assert!(cell(&frame, 0, 1).attrs.wide);
    assert!(cell(&frame, 0, 2).attrs.wide_spacer);
    assert_eq!(cell(&frame, 0, 3).ch, 'B');
    assert_eq!(cell(&frame, 0, 3).fg, CellColor::Default);
}

#[test]
fn snapshot_truncation_keeps_the_newest_lines() {
    let mut engine = EngineBench::new(20, 4, 8, 16);
    for line in 0..40 {
        engine.advance(format!("linea {line}\r\n").as_bytes());
    }
    let snapshot = engine.snapshot_with_limit(120);
    assert!(snapshot.truncated());
    assert!(!snapshot.is_empty());

    let decoded = TerminalSnapshot::decode(&snapshot.encode()).expect("decodifica");
    let mut restored = EngineBench::from_snapshot(decoded, 20, 4, 8, 16);
    let frame = restored.frame();
    let visible: String = (0..frame.rows)
        .map(|row| row_text(&frame, row))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(visible.contains("linea 39"), "visible: {visible:?}");
}

#[test]
fn snapshot_roundtrip_preserves_sgr_attributes() {
    let mut engine = EngineBench::new(20, 2, 8, 16);
    engine.advance(b"\x1b[1;2;3;4;7;8;9;38;2;10;20;30mA");
    let decoded = TerminalSnapshot::decode(&engine.snapshot().encode()).expect("decodifica");
    let mut restored = EngineBench::from_snapshot(decoded, 20, 2, 8, 16);
    let frame = restored.frame();
    let a = cell(&frame, 0, 0);
    assert!(a.attrs.bold);
    assert!(a.attrs.dim);
    assert!(a.attrs.italic);
    assert!(a.attrs.underline);
    assert!(a.attrs.reverse);
    assert!(a.attrs.hidden);
    assert!(a.attrs.strikethrough);
    assert_eq!(a.fg, CellColor::Rgb(0x0a_14_1e));
}

#[test]
fn snapshot_discards_alt_screen_tui_and_restores_main_screen() {
    let mut engine = EngineBench::new(20, 4, 8, 16);
    engine.advance(b"PS C:\\> opencode\r\n");
    engine.advance(b"\x1b[?1049h\x1b[HTUI PANTALLA COMPLETA");
    let snapshot = engine.snapshot();
    let decoded = TerminalSnapshot::decode(&snapshot.encode()).expect("decodifica");
    let mut restored = EngineBench::from_snapshot(decoded, 20, 4, 8, 16);
    let frame = restored.frame();
    let visible: String = (0..frame.rows)
        .map(|row| row_text(&frame, row))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        visible.contains("PS C:\\> opencode"),
        "visible: {visible:?}"
    );
    assert!(
        !visible.contains("TUI PANTALLA COMPLETA"),
        "la TUI no debe quedar congelada: {visible:?}"
    );
}

#[test]
fn corrupt_snapshots_are_rejected() {
    let mut engine = EngineBench::new(20, 4, 8, 16);
    engine.advance(b"hola");
    let bytes = engine.snapshot().encode();

    let mut checksum = bytes.clone();
    let last = checksum.len() - 1;
    checksum[last] ^= 0xff;
    assert!(matches!(
        TerminalSnapshot::decode(&checksum),
        Err(SnapshotError::Checksum)
    ));

    let mut version = bytes;
    let space = version
        .iter()
        .position(|byte| *byte == b' ')
        .expect("cabecera");
    version[space + 1] = b'9';
    assert!(matches!(
        TerminalSnapshot::decode(&version),
        Err(SnapshotError::UnsupportedVersion(9))
    ));
}

#[test]
fn osc_7501_parses_states_kinds_base64_and_progress() {
    let mut engine = EngineBench::new(40, 4, 8, 16);
    engine.advance(
        b"\x1b]7501;state=working:app=opencode:progress=37:msg=QnVpbGRpbmcgZG9jcw==\x1b\\",
    );
    let frame = engine.frame();
    let statuses = program_statuses(&frame);
    assert_eq!(statuses.len(), 1);
    let status = &statuses[0];
    assert_eq!(status.state, ProgramState::Working);
    assert_eq!(status.app.as_deref(), Some("opencode"));
    assert_eq!(status.progress, Some(37));
    assert_eq!(status.message.as_deref(), Some("Building docs"));

    engine.advance(
        b"\x1b]7501;state=blocked:kind=question:progress=10:msg=T3ZlcndyaXRlIGNvbmZpZy50b21sPw==\x1b\\",
    );
    let frame = engine.frame();
    let statuses = program_statuses(&frame);
    assert_eq!(statuses[0].state, ProgramState::Blocked);
    assert_eq!(statuses[0].kind.as_deref(), Some("question"));
    assert_eq!(
        statuses[0].message.as_deref(),
        Some("Overwrite config.toml?")
    );

    engine.advance(b"\x1b]7501;state=done:msg=RG9jcyBidWlsdA==\x1b\\");
    let frame = engine.frame();
    assert_eq!(program_statuses(&frame)[0].state, ProgramState::Done);

    engine.advance(b"\x1b]7501;state=clear\x1b\\");
    let frame = engine.frame();
    assert_eq!(program_statuses(&frame)[0].state, ProgramState::Clear);
}

#[test]
fn osc_7501_rejects_invalid_reports_whole() {
    let mut engine = EngineBench::new(40, 4, 8, 16);
    // Estado desconocido, base64 inválido, control decodificado y clave larga.
    engine.advance(b"\x1b]7501;state=zombi\x1b\\");
    engine.advance(b"\x1b]7501;state=working:msg=no-es-base64!\x1b\\");
    engine.advance(b"\x1b]7501;state=working:msg=YQo=\x1b\\");
    engine.advance(b"\x1b]7501;state=working:esta-clave-es-demasiado-larga=1\x1b\\");
    let frame = engine.frame();
    assert!(program_statuses(&frame).is_empty());
}

#[test]
fn osc_7501_accepts_unpadded_base64_and_unknown_keys() {
    let mut engine = EngineBench::new(40, 4, 8, 16);
    engine.advance(b"\x1b]7501;state=working:future=1:msg=QnVpbGRpbmcgZG9jcw\x1b\\");
    let frame = engine.frame();
    let statuses = program_statuses(&frame);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].message.as_deref(), Some("Building docs"));
}

#[test]
fn osc_7501_answers_the_support_query() {
    let mut engine = EngineBench::new(40, 4, 8, 16);
    engine.advance(b"\x1b]7501;?\x1b\\");
    let written = String::from_utf8_lossy(&engine.written()).into_owned();
    assert!(written.contains("]7501;?"));
    assert!(written.contains("states=idle,working,done,blocked,error"));
    assert!(written.contains("kinds=permission,question,auth"));
    let frame = engine.frame();
    assert!(program_statuses(&frame).is_empty());
}

#[test]
fn osc_7501_sequences_survive_chunked_output() {
    let mut engine = EngineBench::new(40, 4, 8, 16);
    engine.advance(b"\x1b]7501;state=work");
    engine.advance(b"ing:app=copilot\x1b");
    engine.advance(b"\\");
    let frame = engine.frame();
    let statuses = program_statuses(&frame);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].state, ProgramState::Working);
    assert_eq!(statuses[0].app.as_deref(), Some("copilot"));
}

#[test]
fn osc_notifications_feed_attention_and_ignore_progress() {
    let mut engine = EngineBench::new(40, 4, 8, 16);
    engine.advance(b"\x1b]9;Build finished\x1b\\");
    engine.advance(b"\x1b]9;4;1;50\x1b\\");
    engine.advance(b"\x1b]777;notify;Titulo;Cuerpo\x1b\\");
    engine.advance(b"\x1b]1337;Notify=Titulo;Cuerpo\x1b\\");
    let frame = engine.frame();
    let messages: Vec<_> = frame
        .events
        .iter()
        .filter_map(|event| match event {
            TerminalEvent::Attention(message) => Some(message.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        messages,
        vec![
            Some("Build finished".into()),
            Some("Cuerpo".into()),
            Some("Cuerpo".into()),
        ]
    );
}
