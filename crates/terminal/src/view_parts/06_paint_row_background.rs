#[expect(
    clippy::too_many_arguments,
    reason = "el pintado por fila necesita origen, métricas y ventana"
)]
fn paint_row_background(
    cells: &[SnapshotCell],
    cols: usize,
    row: usize,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
    debug: DebugRender,
) {
    if debug.no_bg {
        return;
    }

    let overlap = if debug.bg_overlap { px(1.0) } else { px(0.0) };
    let mut run: Option<(usize, usize, Hsla)> = None;

    for (column, cell) in cells.iter().enumerate().take(cols.min(cells.len())) {
        let cell = *cell;
        if cell.attrs.wide_spacer {
            continue;
        }
        let width = if cell.attrs.wide { 2 } else { 1 };
        let (_, background) = resolve_cell_colors(&cell, &metrics.palette);
        let paint = cell.attrs.reverse || cell.bg != CellColor::Default;

        if !paint {
            if let Some((start, end, color)) = run.take() {
                paint_background_quad(
                    start,
                    end,
                    row,
                    origin,
                    cell_width,
                    line_height,
                    overlap,
                    color,
                    window,
                );
            }
            continue;
        }

        match run {
            Some((start, end, color)) if color == background && end == column => {
                run = Some((start, (column + width).min(cols), color));
            }
            Some((start, end, color)) => {
                paint_background_quad(
                    start,
                    end,
                    row,
                    origin,
                    cell_width,
                    line_height,
                    overlap,
                    color,
                    window,
                );
                run = Some((column, (column + width).min(cols), background));
            }
            None => run = Some((column, (column + width).min(cols), background)),
        }
    }

    if let Some((start, end, color)) = run {
        paint_background_quad(
            start,
            end,
            row,
            origin,
            cell_width,
            line_height,
            overlap,
            color,
            window,
        );
    }
}
#[expect(
    clippy::too_many_arguments,
    reason = "coordenadas del quad calculadas en el helper de runs"
)]
fn paint_background_quad(
    start: usize,
    end: usize,
    row: usize,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    overlap: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    if end <= start {
        return;
    }
    let x = origin.x + cell_width * start;
    let y = origin.y + line_height * row;
    window.paint_quad(fill(
        Bounds::new(
            point(x, y),
            size(cell_width * (end - start), line_height + overlap),
        ),
        color,
    ));
}
#[expect(
    clippy::too_many_arguments,
    reason = "la selección se pinta por fila con la geometría del grid"
)]
fn paint_selection_row(
    selection: SelectionSpan,
    row: usize,
    cols: usize,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
) {
    if row < selection.start_line || row > selection.end_line || cols == 0 {
        return;
    }

    let (start, end) = if selection.block || selection.start_line == selection.end_line {
        (selection.start_col, selection.end_col)
    } else if row == selection.start_line {
        (selection.start_col, cols - 1)
    } else if row == selection.end_line {
        (0, selection.end_col)
    } else {
        (0, cols - 1)
    };
    if end < start {
        return;
    }

    let x = origin.x + cell_width * start;
    let y = origin.y + line_height * row;
    window.paint_quad(fill(
        Bounds::new(
            point(x, y),
            size(cell_width * (end - start + 1), line_height),
        ),
        metrics.palette.selection.opacity(0.45),
    ));
}
#[expect(
    clippy::too_many_arguments,
    reason = "el subrayado de un enlace necesita los mismos datos que la fila"
)]
fn paint_hyperlink(
    link: &HyperlinkSpan,
    row: usize,
    cols: usize,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
) {
    let start = link.start.min(cols);
    let end = link.end.min(cols);
    if end <= start {
        return;
    }
    let x = origin.x + cell_width * start;
    let y = origin.y + line_height * (row + 1) - px(1.0);
    window.paint_quad(fill(
        Bounds::new(point(x, y), size(cell_width * (end - start), px(1.0))),
        metrics.palette.ansi(12).opacity(0.85),
    ));
}
fn paint_scrollbar(
    bounds: Bounds<Pixels>,
    rows: usize,
    display_offset: usize,
    history_size: usize,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
) {
    if history_size == 0 || rows == 0 {
        return;
    }

    let width = (metrics.cell_width * 0.25).max(px(2.0));
    let track_height = line_height * rows;
    let total = rows + history_size;
    let thumb_height =
        (f32::from(track_height) * (rows as f32 / total as f32)).max(f32::from(line_height) * 0.5);
    let max_top = (f32::from(track_height) - thumb_height).max(0.0);
    let offset = display_offset.min(history_size) as f32 / history_size as f32;
    let top = max_top * (1.0 - offset);

    let x = bounds.origin.x + bounds.size.width - width;
    let y = bounds.origin.y;
    window.paint_quad(fill(
        Bounds::new(point(x, y), size(width, track_height)),
        metrics.palette.foreground.opacity(0.06),
    ));
    window.paint_quad(fill(
        Bounds::new(point(x, y + px(top)), size(width, px(thumb_height))),
        metrics.palette.cursor.opacity(0.45),
    ));
}
fn paint_cursor(
    cursor: CursorSnapshot,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    metrics: &Metrics,
    window: &mut Window,
) {
    let x = origin.x + cell_width * cursor.column;
    let y = origin.y + line_height * cursor.line;
    let color = metrics.palette.cursor;

    let bounds = match cursor.shape {
        CursorShape::Beam => Bounds::new(point(x, y), size(px(2.0), line_height)),
        CursorShape::Underline => Bounds::new(
            point(x, y + line_height - px(2.0)),
            size(cell_width, px(2.0)),
        ),
        CursorShape::HollowBlock => Bounds::new(point(x, y), size(cell_width, line_height)),
        CursorShape::Block => Bounds::new(point(x, y), size(cell_width, line_height)),
    };

    let alpha = if matches!(cursor.shape, CursorShape::HollowBlock) {
        0.25
    } else {
        0.55
    };
    window.paint_quad(fill(bounds, color.opacity(alpha)));
}
fn is_text_input(keystroke: &Keystroke) -> bool {
    let modifiers = keystroke.modifiers;
    if modifiers.control || modifiers.alt {
        // AltGr en Windows es ctrl+alt; el carácter lo entrega el IME.
        return modifiers.control && modifiers.alt && keystroke.key_char.is_some();
    }
    keystroke.key_char.is_some() && !is_named_key(&keystroke.key)
}
fn is_web_url(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}
fn cursor_style_from_osc(shape: &str) -> CursorStyle {
    match shape.trim().to_ascii_lowercase().as_str() {
        "text" | "ibeam" | "i-beam" => CursorStyle::IBeam,
        "pointer" | "hand" | "pointinghand" => CursorStyle::PointingHand,
        "crosshair" | "cross" => CursorStyle::Crosshair,
        "grab" | "openhand" | "move" => CursorStyle::OpenHand,
        "grabbing" | "closedhand" => CursorStyle::ClosedHand,
        "ew-resize" | "col-resize" | "e-resize" | "w-resize" => CursorStyle::ResizeLeftRight,
        "ns-resize" | "row-resize" | "n-resize" | "s-resize" => CursorStyle::ResizeUpDown,
        "nesw-resize" => CursorStyle::ResizeUpLeftDownRight,
        "nwse-resize" => CursorStyle::ResizeUpRightDownLeft,
        _ => CursorStyle::Arrow,
    }
}
fn wheel_action(delta_x: f32, delta_y: f32) -> MouseAction {
    if delta_y.abs() >= delta_x.abs() {
        if delta_y < 0.0 {
            MouseAction::WheelDown
        } else {
            MouseAction::WheelUp
        }
    } else if delta_x < 0.0 {
        MouseAction::WheelRight
    } else {
        MouseAction::WheelLeft
    }
}
fn terminal_mouse_button(button: MouseButton) -> Option<TerminalMouseButton> {
    match button {
        MouseButton::Left => Some(TerminalMouseButton::Left),
        MouseButton::Middle => Some(TerminalMouseButton::Middle),
        MouseButton::Right => Some(TerminalMouseButton::Right),
        MouseButton::Navigate(_) => None,
    }
}
fn is_named_key(key: &str) -> bool {
    matches!(
        key,
        "enter"
            | "return"
            | "tab"
            | "backtab"
            | "backspace"
            | "escape"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "insert"
            | "delete"
            | "pageup"
            | "pagedown"
            | "f1"
            | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
    )
}
fn pixels_to_u16(pixels: Pixels) -> u16 {
    let value = f32::from(pixels).round();
    if value < 1.0 {
        1
    } else if value > f32::from(u16::MAX) {
        u16::MAX
    } else {
        value as u16
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct UrlCandidate {
    start: usize,
    end: usize,
    text: String,
}
fn find_urls(text: &str) -> Vec<UrlCandidate> {
    let chars: Vec<char> = text.chars().collect();
    let mut candidates = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let remaining: String = chars[index..].iter().collect();
        let lower = remaining.to_ascii_lowercase();
        let prefix = if lower.starts_with("https://") {
            "https://"
        } else if lower.starts_with("http://") {
            "http://"
        } else {
            index += 1;
            continue;
        };

        let start = index;
        let mut end = index + prefix.len();
        while end < chars.len() {
            let ch = chars[end];
            if ch.is_whitespace()
                || ch.is_control()
                || matches!(
                    ch,
                    '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            {
                break;
            }
            end += 1;
        }
        while end > start + prefix.len()
            && matches!(chars[end - 1], '.' | ',' | ';' | ':' | '!' | '?' | '…')
        {
            end -= 1;
        }
        if end > start + prefix.len() {
            candidates.push(UrlCandidate {
                start,
                end,
                text: chars[start..end].iter().collect(),
            });
        }
        index = end.max(index + 1);
    }
    candidates
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_starts_with_the_snapshot_size() {
        let mut engine = crate::EngineBench::new(20, 5, 8, 16);
        engine.advance(b"hola");
        let snapshot = engine.snapshot();
        let restored = initial_grid(Some(&snapshot));
        assert_eq!((restored.columns, restored.screen_lines), (20, 5));
        let fresh = initial_grid(None);
        assert_eq!((fresh.columns, fresh.screen_lines), (80, 24));
    }

    #[test]
    fn wheel_direction_matches_the_physical_wheel() {
        assert_eq!(wheel_action(0.0, -1.0), MouseAction::WheelDown);
        assert_eq!(wheel_action(0.0, 1.0), MouseAction::WheelUp);
        assert_eq!(wheel_action(-1.0, 0.0), MouseAction::WheelRight);
        assert_eq!(wheel_action(1.0, 0.0), MouseAction::WheelLeft);
    }

    #[test]
    fn only_web_urls_are_openable() {
        assert!(is_web_url("https://example.com"));
        assert!(is_web_url("http://example.com"));
        assert!(is_web_url("HTTPS://EXAMPLE.COM"));
        assert!(!is_web_url("file:///C:/Windows"));
        assert!(!is_web_url("javascript:alert(1)"));
    }

    #[test]
    fn osc22_maps_standard_cursor_names() {
        assert_eq!(cursor_style_from_osc("pointer"), CursorStyle::PointingHand);
        assert_eq!(cursor_style_from_osc("text"), CursorStyle::IBeam);
        assert_eq!(
            cursor_style_from_osc("ew-resize"),
            CursorStyle::ResizeLeftRight
        );
        assert_eq!(cursor_style_from_osc("default"), CursorStyle::Arrow);
        assert_eq!(cursor_style_from_osc("desconocido"), CursorStyle::Arrow);
    }

    #[test]
    fn detects_plain_urls() {
        let urls = find_urls("ver https://example.com/x y http://a.test");
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].text, "https://example.com/x");
        assert_eq!(urls[1].text, "http://a.test");
    }

    #[test]
    fn trims_url_punctuation() {
        let urls = find_urls("(https://example.com/x), fin.");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].text, "https://example.com/x");
    }

    #[test]
    fn ignores_incomplete_urls() {
        assert!(find_urls("http://").is_empty());
        assert!(find_urls("https:// ejemplo").is_empty());
    }
}
