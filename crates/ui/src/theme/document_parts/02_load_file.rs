pub fn load_file(path: &Path) -> Result<Theme, ThemeError> {
    let metadata = std::fs::metadata(path).map_err(|error| ThemeError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if metadata.len() > MAX_THEME_BYTES as u64 {
        return Err(ThemeError::TooLarge {
            path: path.display().to_string(),
            limit: MAX_THEME_BYTES,
        });
    }
    let source = std::fs::read_to_string(path).map_err(|error| ThemeError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    parse_str(&source, &path.display().to_string())
}
pub fn load_dir(dir: &Path) -> ThemeCatalog {
    let mut catalog = ThemeCatalog::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return catalog;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.') && !name.ends_with('~'))
        })
        .collect();
    paths.sort();
    for path in paths {
        match load_file(&path) {
            Ok(theme) => catalog.themes.push(theme),
            Err(error) => catalog.errors.push(error),
        }
    }
    catalog
}
fn resolve(document: ThemeDocument, source: &str, label: &str) -> Result<Theme, ThemeError> {
    let mut problems: Vec<String> = Vec::new();

    if document.meta.name.trim().is_empty() {
        problems.push(with_line(
            source,
            "meta.name",
            "el nombre no puede estar vacío",
        ));
    } else if document.meta.name.chars().count() > 80 {
        problems.push(with_line(
            source,
            "meta.name",
            "el nombre admite como máximo 80 caracteres",
        ));
    }

    let family = document
        .meta
        .family
        .clone()
        .filter(|family| !family.trim().is_empty())
        .unwrap_or_else(|| derive_family(&document.meta.name));

    let background = parse_color(&document.ui.background, "background", source, &mut problems);
    let surface = parse_color(&document.ui.surface, "surface", source, &mut problems);
    let border = parse_color(&document.ui.border, "border", source, &mut problems);
    let text = parse_color(&document.ui.text, "text", source, &mut problems);
    let muted = parse_color(&document.ui.muted, "muted", source, &mut problems);
    let accent = parse_color(&document.ui.accent, "accent", source, &mut problems);
    let danger = parse_color(&document.ui.danger, "danger", source, &mut problems);
    let warning = parse_color(&document.ui.warning, "warning", source, &mut problems);
    let success = parse_color(&document.ui.success, "success", source, &mut problems);

    let surface_raised = document
        .ui
        .surface_raised
        .as_deref()
        .map(|value| parse_color(value, "surface_raised", source, &mut problems))
        .unwrap_or_else(|| surface.map(|color| color.lighten(0.06)));
    let border_strong = document
        .ui
        .border_strong
        .as_deref()
        .map(|value| parse_color(value, "border_strong", source, &mut problems))
        .unwrap_or_else(|| border.map(|color| color.lighten(0.12)));
    let subtle = document
        .ui
        .subtle
        .as_deref()
        .map(|value| parse_color(value, "subtle", source, &mut problems))
        .unwrap_or_else(|| muted.map(|color| color.mix(background.unwrap_or(color), 0.35)));
    let accent_text = document
        .ui
        .accent_text
        .as_deref()
        .map(|value| parse_color(value, "accent_text", source, &mut problems))
        .unwrap_or_else(|| accent.map(|color| color.readable_text()));
    let info = document
        .ui
        .info
        .as_deref()
        .map(|value| parse_color(value, "info", source, &mut problems))
        .unwrap_or(accent);
    let selection = document
        .ui
        .selection
        .as_deref()
        .map(|value| parse_color(value, "selection", source, &mut problems))
        .unwrap_or_else(|| accent.map(|color| Rgb::rgba(color.r, color.g, color.b, 96)));
    let focus = document
        .ui
        .focus
        .as_deref()
        .map(|value| parse_color(value, "focus", source, &mut problems))
        .unwrap_or(accent);

    if document.terminal.ansi.len() != 16 {
        problems.push(with_line(
            source,
            "ansi",
            &format!(
                "se esperan 16 colores ANSI y hay {}",
                document.terminal.ansi.len()
            ),
        ));
    }
    let term_foreground = parse_color(
        &document.terminal.foreground,
        "foreground",
        source,
        &mut problems,
    );
    let term_background = parse_color(
        &document.terminal.background,
        "background",
        source,
        &mut problems,
    );
    let term_cursor = parse_color(&document.terminal.cursor, "cursor", source, &mut problems);
    let term_selection = parse_color(
        &document.terminal.selection,
        "selection",
        source,
        &mut problems,
    );
    let term_dim = document
        .terminal
        .dim_foreground
        .as_deref()
        .map(|value| parse_color(value, "dim_foreground", source, &mut problems))
        .unwrap_or(term_foreground);
    let mut ansi = [Rgb::rgb(0, 0, 0); 16];
    for (index, value) in document.terminal.ansi.iter().take(16).enumerate() {
        match Rgb::parse(value) {
            Ok(color) => ansi[index] = color,
            Err(error) => problems.push(with_line(
                source,
                "ansi",
                &format!("ANSI COLOR[{index}]: {error}"),
            )),
        }
    }

    if !problems.is_empty() {
        return Err(ThemeError::Invalid {
            label: label.to_string(),
            problems: problems.join("\n"),
        });
    }

    let background = background.ok_or_else(|| ThemeError::Invalid {
        label: label.to_string(),
        problems: "faltan colores obligatorios".into(),
    })?;
    let ui = UiTokens {
        background: background.to_hsla(),
        surface: surface.unwrap_or(background).to_hsla(),
        surface_raised: surface_raised.unwrap_or(background.lighten(0.06)).to_hsla(),
        border: border.unwrap_or(background.lighten(0.15)).to_hsla(),
        border_strong: border_strong.unwrap_or(background.lighten(0.25)).to_hsla(),
        text: text.unwrap_or(Rgb::rgb(255, 255, 255)).to_hsla(),
        muted: muted.unwrap_or(Rgb::rgb(160, 160, 160)).to_hsla(),
        subtle: subtle.unwrap_or(Rgb::rgb(120, 120, 120)).to_hsla(),
        accent: accent.unwrap_or(Rgb::rgb(97, 175, 239)).to_hsla(),
        accent_text: accent_text.unwrap_or(Rgb::rgb(0, 0, 0)).to_hsla(),
        danger: danger.unwrap_or(Rgb::rgb(224, 108, 117)).to_hsla(),
        danger_text: danger
            .unwrap_or(Rgb::rgb(224, 108, 117))
            .readable_text()
            .to_hsla(),
        warning: warning.unwrap_or(Rgb::rgb(229, 192, 123)).to_hsla(),
        warning_text: warning
            .unwrap_or(Rgb::rgb(229, 192, 123))
            .readable_text()
            .to_hsla(),
        success: success.unwrap_or(Rgb::rgb(152, 195, 121)).to_hsla(),
        success_text: success
            .unwrap_or(Rgb::rgb(152, 195, 121))
            .readable_text()
            .to_hsla(),
        info: info.unwrap_or(Rgb::rgb(97, 175, 239)).to_hsla(),
        info_text: info
            .unwrap_or(Rgb::rgb(97, 175, 239))
            .readable_text()
            .to_hsla(),
        selection: selection.unwrap_or(Rgb::rgba(97, 175, 239, 96)).to_hsla(),
        focus: focus.unwrap_or(Rgb::rgb(97, 175, 239)).to_hsla(),
        overlay: Rgb::rgba(0, 0, 0, 150).to_hsla(),
    };

    let terminal = TerminalPalette::new(
        term_foreground
            .unwrap_or_else(|| Rgb::from(ui.text))
            .to_hsla(),
        term_background.unwrap_or(background).to_hsla(),
        term_cursor
            .unwrap_or_else(|| Rgb::from(ui.accent))
            .to_hsla(),
        term_selection
            .unwrap_or_else(|| Rgb::from(ui.selection))
            .to_hsla(),
        term_dim.unwrap_or_else(|| Rgb::from(ui.muted)).to_hsla(),
        ansi.map(Rgb::to_hsla),
    );

    Ok(Theme {
        meta: ThemeMeta {
            name: document.meta.name,
            author: document.meta.author,
            family,
            variant: document.meta.variant,
        },
        ui,
        terminal,
    })
}
fn derive_family(name: &str) -> String {
    for suffix in [" Dark", " Light", " Oscuro", " Claro"] {
        if let Some(family) = name.strip_suffix(suffix) {
            return family.to_string();
        }
    }
    name.to_string()
}
fn with_line(source: &str, field: &str, message: &str) -> String {
    match line_for(source, field) {
        Some(line) => format!("  {field} (línea {line}): {message}"),
        None => format!("  {field}: {message}"),
    }
}
fn line_for(source: &str, field: &str) -> Option<usize> {
    source.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim_start();
        let matches = trimmed.starts_with(&format!("{field} ="))
            || trimmed.starts_with(&format!("{field}="))
            || trimmed.starts_with(&format!("{field}."));
        matches.then_some(index + 1)
    })
}
fn parse_error(error: toml::de::Error, source: &str, label: &str) -> ThemeError {
    let (line, column) = error
        .span()
        .map(|span| line_column(source, span.start))
        .unwrap_or((1, 1));
    ThemeError::Parse {
        label: label.to_string(),
        message: error.message().to_string(),
        line,
        column,
    }
}
fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map(|tail| tail.chars().count() + 1)
        .unwrap_or(1);
    (line, column)
}
fn parse_color(
    value: &str,
    field: &'static str,
    source: &str,
    problems: &mut Vec<String>,
) -> Option<Rgb> {
    match Rgb::parse(value) {
        Ok(color) => Some(color),
        Err(error) => {
            problems.push(with_line(source, field, &error.to_string()));
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r##"
[meta]
name = "Test"
variant = "dark"

[ui]
background = "#1e2127"
surface = "#21242b"
border = "#3a3f4b"
text = "#abb2bf"
muted = "#7d8594"
accent = "#61afef"
danger = "#e06c75"
warning = "#e5c07b"
success = "#98c379"

[terminal]
foreground = "#abb2bf"
background = "#1e2127"
cursor = "#61afef"
selection = "#61afef60"
ansi = [
  "#1e2127", "#e06c75", "#98c379", "#e5c07b",
  "#61afef", "#c678dd", "#56b6c2", "#abb2bf",
  "#5c6370", "#e06c75", "#98c379", "#e5c07b",
  "#61afef", "#c678dd", "#56b6c2", "#ffffff",
]
"##;

    #[test]
    fn minimal_theme_resolves_derived_tokens() {
        let theme = parse_str(MINIMAL, "test").expect("tema válido");
        assert_eq!(theme.name(), "Test");
        assert_eq!(theme.meta.family, "Test");
        assert_eq!(
            theme.terminal.ansi(1),
            Rgb::parse("#e06c75").expect("rojo").to_hsla()
        );
        assert_ne!(theme.ui.surface_raised, theme.ui.surface);
        assert!(theme.ui.accent_text != theme.ui.accent);
    }

    #[test]
    fn invalid_field_reports_the_field_and_line() {
        let source = MINIMAL.replace("accent = \"#61afef\"", "accent = \"azul\"");
        let error = parse_str(&source, "test").expect_err("debe fallar");
        let message = error.to_string();
        assert!(message.contains("accent"), "{message}");
        assert!(message.contains("(línea "), "{message}");
    }

    #[test]
    fn wrong_ansi_count_is_rejected() {
        let source = MINIMAL.replace(
            "\"#61afef\", \"#c678dd\", \"#56b6c2\", \"#ffffff\",\n]",
            "\"#61afef\", \"#c678dd\", \"#56b6c2\",\n]",
        );
        let error = parse_str(&source, "test").expect_err("debe fallar");
        assert!(error.to_string().contains("16 colores ANSI"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let source = MINIMAL.replace("[meta]", "[meta]\nextra = 1");
        let error = parse_str(&source, "test").expect_err("debe fallar");
        assert!(matches!(error, ThemeError::Parse { .. }));
    }

    #[test]
    fn export_round_trips() {
        let theme = parse_str(MINIMAL, "test").expect("tema válido");
        let exported = theme.to_toml().expect("exporta");
        let reparsed = parse_str(&exported, "exportado").expect("reparsea");
        assert_eq!(theme.meta, reparsed.meta);
        assert_eq!(theme.ui.text, reparsed.ui.text);
        assert_eq!(theme.terminal.ansi(4), reparsed.terminal.ansi(4));
    }

    #[test]
    fn family_is_derived_from_the_name() {
        assert_eq!(derive_family("One Dark"), "One");
        assert_eq!(derive_family("One Light"), "One");
        assert_eq!(derive_family("Monokai"), "Monokai");
    }

    #[test]
    fn load_dir_keeps_valid_themes_and_reports_bad_files() {
        let dir = std::env::temp_dir().join(format!(
            "axfterm-themes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("bueno.toml"), MINIMAL).expect("tema bueno");
        std::fs::write(dir.join("roto.toml"), "esto no es toml").expect("tema roto");
        std::fs::write(dir.join(".oculto.toml"), MINIMAL).expect("oculto");
        let catalog = load_dir(&dir);
        assert_eq!(catalog.themes.len(), 1);
        assert_eq!(catalog.errors.len(), 1);
        assert_eq!(catalog.themes[0].meta.name, "Test");
        let _ = std::fs::remove_dir_all(dir);
    }
}
