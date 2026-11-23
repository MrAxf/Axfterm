fn trace_file(variable: &str, default_name: &str) -> Option<File> {
    let value = std::env::var_os(variable)?;
    let path = if value == "1" {
        std::env::temp_dir().join(default_name)
    } else {
        PathBuf::from(value)
    };
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .ok()
}
fn dump_snapshot(term: &Term<Listener>, file: &mut File, sequence: usize) -> std::io::Result<()> {
    let rows = term.screen_lines();
    let cols = term.columns();
    let content = term.renderable_content();
    writeln!(
        file,
        "=== snapshot {sequence} {cols}x{rows} ===\ndefaults fg={:?} bg={:?} cursor={:?}\nmode={:?}",
        content.colors[NamedColor::Foreground],
        content.colors[NamedColor::Background],
        content.colors[NamedColor::Cursor],
        content.mode,
    )?;

    let grid = term.grid();
    for line in 0..rows {
        let row = &grid[Line(line as i32)];
        let mut text = String::with_capacity(cols);
        let mut notes = Vec::new();

        for column in 0..cols {
            let cell = &row[Column(column)];
            if !cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                text.push(if cell.flags.contains(Flags::HIDDEN) {
                    ' '
                } else {
                    cell.c
                });
            }

            let custom_color = !matches!(cell.fg, Color::Named(NamedColor::Foreground))
                || !matches!(cell.bg, Color::Named(NamedColor::Background));
            if custom_color || !cell.flags.is_empty() || cell.hyperlink().is_some() {
                notes.push(format!(
                    "{column}:{:?} fg={:?} bg={:?} flags={:?}",
                    cell.c, cell.fg, cell.bg, cell.flags
                ));
            }
        }

        writeln!(file, "row {line:02}: {text}")?;
        if !notes.is_empty() {
            writeln!(file, "  {}", notes.join(" | "))?;
        }
    }

    writeln!(file)?;
    file.flush()
}
#[derive(Clone)]
struct Listener {
    writer: PtyWriter,
    metrics: Arc<Mutex<WindowMetrics>>,
    palette: TerminalPalette,
    events: Rc<RefCell<Vec<TerminalEvent>>>,
}
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                let _ = self.writer.write(text.as_bytes());
            }
            Event::TextAreaSizeRequest(format) => {
                let metrics = match self.metrics.lock() {
                    Ok(metrics) => *metrics,
                    Err(poisoned) => *poisoned.into_inner(),
                };
                let reply = format(WindowSize {
                    num_lines: metrics.rows.min(usize::from(u16::MAX)) as u16,
                    num_cols: metrics.cols.min(usize::from(u16::MAX)) as u16,
                    cell_width: metrics.cell_width,
                    cell_height: metrics.cell_height,
                });
                let _ = self.writer.write(reply.as_bytes());
            }
            Event::ColorRequest(index, format) => {
                let rgb = self.palette.named(index).to_rgb();
                let reply = format(Rgb {
                    r: (rgb.r.clamp(0.0, 1.0) * 255.0).round() as u8,
                    g: (rgb.g.clamp(0.0, 1.0) * 255.0).round() as u8,
                    b: (rgb.b.clamp(0.0, 1.0) * 255.0).round() as u8,
                });
                let _ = self.writer.write(reply.as_bytes());
            }
            Event::Title(title) => self
                .events
                .borrow_mut()
                .push(TerminalEvent::Title(Some(title))),
            Event::ResetTitle => self.events.borrow_mut().push(TerminalEvent::Title(None)),
            Event::Bell => self.events.borrow_mut().push(TerminalEvent::Bell),
            _ => {}
        }
    }
}
#[derive(Default)]
struct ScanOutput {
    events: Vec<TerminalEvent>,
    replies: Vec<Vec<u8>>,
}
#[derive(Default)]
struct OscScanner {
    state: OscState,
    body: Vec<u8>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum OscState {
    #[default]
    Ground,
    Esc,
    Osc,
    OscEsc,
}
const PROGRAM_STATUS_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";
const PROGRAM_STATUS_LIMIT: usize = 4096;
const PROGRAM_KEY_LIMIT: usize = 16;
impl OscScanner {
    fn advance(&mut self, bytes: &[u8]) -> ScanOutput {
        let mut output = ScanOutput::default();
        for &byte in bytes {
            match self.state {
                OscState::Ground => {
                    if byte == 0x1b {
                        self.state = OscState::Esc;
                    }
                }
                OscState::Esc => {
                    if byte == b']' {
                        self.body.clear();
                        self.state = OscState::Osc;
                    } else if byte != 0x1b {
                        self.state = OscState::Ground;
                    }
                }
                OscState::Osc => match byte {
                    0x07 => {
                        if let Some(event) = self.finish(&mut output) {
                            output.events.push(event);
                        }
                        self.state = OscState::Ground;
                    }
                    0x1b => self.state = OscState::OscEsc,
                    _ => {
                        if self.body.len() < OSC_LIMIT {
                            self.body.push(byte);
                        }
                    }
                },
                OscState::OscEsc => {
                    if byte == b'\\' {
                        if let Some(event) = self.finish(&mut output) {
                            output.events.push(event);
                        }
                        self.state = OscState::Ground;
                    } else {
                        // Un ESC que no forma ST aborta el OSC; si empieza otra
                        // secuencia, la procesamos desde Ground. Así una OSC
                        // truncada no se traga la siguiente OSC válida.
                        self.body.clear();
                        self.state = if byte == 0x1b {
                            OscState::Esc
                        } else {
                            OscState::Ground
                        };
                    }
                }
            }
        }
        output
    }

    fn finish(&mut self, output: &mut ScanOutput) -> Option<TerminalEvent> {
        let body = std::mem::take(&mut self.body);
        let (code, payload) = split_osc(&body)?;
        match code {
            7 => file_uri_to_path(payload).map(TerminalEvent::Cwd),
            9 | 777 | 1337 => notification_text(code, payload)
                .map(|text| TerminalEvent::Attention((!text.is_empty()).then_some(text))),
            22 => Some(TerminalEvent::MouseCursor(payload.to_owned())),
            7501 => {
                if is_program_status_query(payload) {
                    output.replies.push(PROGRAM_STATUS_REPLY.to_vec());
                    return None;
                }
                parse_program_status(payload).map(TerminalEvent::ProgramStatus)
            }
            _ => None,
        }
    }
}
fn is_program_status_query(payload: &str) -> bool {
    let payload = payload.trim();
    payload == "?" || payload.starts_with("?:")
}
fn parse_program_status(payload: &str) -> Option<ProgramStatus> {
    let payload = payload.trim();
    if payload.is_empty() || payload.len() > PROGRAM_STATUS_LIMIT {
        return None;
    }
    let mut status = ProgramStatus {
        state: ProgramState::Idle,
        kind: None,
        progress: None,
        app: None,
        title: None,
        message: None,
        id: None,
    };
    let mut state: Option<ProgramState> = None;

    for pair in payload.split(':') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.len() > PROGRAM_KEY_LIMIT {
            return None;
        }
        match key {
            "state" => {
                state = match value {
                    "idle" => Some(ProgramState::Idle),
                    "working" => Some(ProgramState::Working),
                    "blocked" => Some(ProgramState::Blocked),
                    "done" => Some(ProgramState::Done),
                    "error" => Some(ProgramState::Error),
                    "clear" => Some(ProgramState::Clear),
                    _ => None,
                };
            }
            "kind" => {
                status.kind = match value {
                    "permission" | "question" | "auth" => Some(value.to_owned()),
                    _ => None,
                };
            }
            "progress" => {
                status.progress = value.parse::<u8>().ok().filter(|value| *value <= 100);
            }
            "app" => status.app = valid_app(value),
            "title" => match decode_text(value, 256, 192) {
                Ok(text) => status.title = text,
                Err(()) => return None,
            },
            "msg" => match decode_text(value, 2732, 2048) {
                Ok(text) => status.message = text,
                Err(()) => return None,
            },
            "id" => match parse_id(value) {
                Ok(id) => status.id = id,
                Err(()) => return None,
            },
            _ => {}
        }
    }

    let state = state?;
    status.state = state;
    Some(status)
}
fn valid_app(value: &str) -> Option<String> {
    if value.is_empty() || value.len() > 32 {
        return None;
    }
    value
        .chars()
        .all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '+' | '-')
        })
        .then(|| value.to_owned())
}
fn parse_id(value: &str) -> Result<Option<String>, ()> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 128 {
        return Err(());
    }
    let segments: Vec<&str> = value.split('/').collect();
    if segments.len() > 8 {
        return Err(());
    }
    for segment in &segments {
        if segment.is_empty() || segment.len() > 32 {
            return Err(());
        }
        let valid = segment.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | '.' | ',' | '+' | '=' | '-')
        });
        if !valid {
            return Err(());
        }
    }
    Ok(Some(value.to_owned()))
}
fn decode_text(
    value: &str,
    encoded_limit: usize,
    decoded_limit: usize,
) -> Result<Option<String>, ()> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > encoded_limit {
        return Err(());
    }
    let decoded = base64_decode(value).ok_or(())?;
    if decoded.len() > decoded_limit {
        return Err(());
    }
    let text = String::from_utf8(decoded).map_err(|_| ())?;
    if text.chars().any(is_forbidden_text_character) {
        return Err(());
    }
    Ok((!text.is_empty()).then_some(text))
}
fn is_forbidden_text_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{202a}'
                | '\u{202b}'
                | '\u{202c}'
                | '\u{202d}'
                | '\u{202e}'
                | '\u{2066}'
                | '\u{2067}'
                | '\u{2068}'
                | '\u{2069}'
        )
}
fn base64_decode(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0u8;
    let mut padded = false;
    for &byte in bytes {
        if byte == b'=' {
            padded = true;
            continue;
        }
        if padded {
            return None;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}
fn notification_text(code: u32, payload: &str) -> Option<String> {
    let text = match code {
        9 => {
            // OSC 9;4 es progreso de ConEmu, no una notificación.
            if payload.trim_start().starts_with("4;") {
                return None;
            }
            payload
        }
        777 => {
            let mut parts = payload.split(';');
            let _kind = parts.next();
            let title = parts.next().unwrap_or_default();
            let body = parts.next().unwrap_or_default();
            if body.is_empty() { title } else { body }
        }
        1337 => {
            let payload = payload
                .strip_prefix("Notify=")
                .or_else(|| payload.strip_prefix("Notification="))
                .unwrap_or_default();
            payload.rsplit_once(';').map_or(payload, |(_, body)| body)
        }
        _ => return None,
    };
    let text: String = text
        .chars()
        .filter(|character| !is_forbidden_text_character(*character))
        .take(2048)
        .collect();
    Some(text.trim().to_owned())
}
fn split_osc(body: &[u8]) -> Option<(u32, &str)> {
    let separator = body.iter().position(|&byte| byte == b';')?;
    let code = std::str::from_utf8(&body[..separator]).ok()?.parse().ok()?;
    let payload = std::str::from_utf8(&body[separator + 1..]).ok()?;
    Some((code, payload))
}
fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let decoded = percent_decode(rest);

    #[cfg(windows)]
    {
        let decoded = decoded.strip_prefix("localhost").unwrap_or(&decoded);
        let decoded = decoded.strip_prefix('/').unwrap_or(decoded);
        // `file:///C:/...` llega como `/C:/...`; `file://server/share` como
        // `server/share` y se convierte en UNC.
        let looks_like_drive = decoded.as_bytes().get(1).is_some_and(|&byte| byte == b':');
        if looks_like_drive {
            return Some(PathBuf::from(decoded.replace('/', "\\")));
        }
        Some(PathBuf::from(format!(r"\\{}", decoded.replace('/', "\\"))))
    }
    #[cfg(not(windows))]
    {
        Some(PathBuf::from(decoded))
    }
}
