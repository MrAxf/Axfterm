//! Fuentes de UI y terminal, `add_fonts` y cadenas de fallback.
//!
//! La lectura de archivos puede hacerse fuera del hilo de UI; registrar las
//! fuentes (`add_fonts`) se hace una vez en el hilo de la app.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::gpui::{App, SharedString};

/// Tope por archivo de fuente.
pub const MAX_FONT_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Tope total de una carga de fuentes.
pub const MAX_FONT_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

/// Familias de UI recomendadas, en orden.
pub const UI_PREFERRED: &[&str] = &[
    "Inter",
    "Segoe UI Variable Text",
    "Segoe UI",
    "Noto Sans",
    "SF Pro Text",
    "Arial",
    "Helvetica",
];

/// Familias monoespaciadas recomendadas, en orden.
pub const MONO_PREFERRED: &[&str] = &[
    "CaskaydiaCove Nerd Font",
    "CaskaydiaCove NF",
    "JetBrainsMono Nerd Font",
    "JetBrains Mono",
    "Cascadia Code NF",
    "Cascadia Mono",
    "Consolas",
    "DejaVu Sans Mono",
    "Courier New",
];

/// Fallbacks de UI para emoji, CJK y símbolos Nerd Font.
pub const UI_FALLBACKS: &[&str] = &[
    "Segoe UI Emoji",
    "Segoe UI Symbol",
    "Apple Color Emoji",
    "Noto Color Emoji",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Malgun Gothic",
    "Symbols Nerd Font",
    "Nerd Fonts Symbols",
    "Arial Unicode MS",
];

/// Fallbacks del terminal.
pub const MONO_FALLBACKS: &[&str] = &[
    "Cascadia Mono",
    "Consolas",
    "Segoe UI Symbol",
    "Segoe UI Emoji",
    "Noto Color Emoji",
    "Noto Sans Mono CJK SC",
    "Microsoft YaHei",
    "Malgun Gothic",
    "Symbols Nerd Font",
    "Nerd Fonts Symbols",
];

/// Fuentes resueltas para la UI y el terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontSet {
    /// Familia de UI.
    pub ui: SharedString,
    /// Familia monoespaciada del terminal.
    pub mono: SharedString,
    /// Cadena de fallback de la UI.
    pub ui_fallbacks: Vec<SharedString>,
    /// Cadena de fallback del terminal.
    pub mono_fallbacks: Vec<SharedString>,
    /// Archivos registrados con éxito.
    pub loaded: usize,
    /// Errores de lectura o de registro.
    pub errors: Vec<String>,
    /// Avisos no fatales (fuente pedida no instalada, etc.).
    pub warnings: Vec<String>,
}

impl FontSet {
    /// Conjunto de sistema para el arranque sin configuración.
    pub fn system() -> Self {
        Self {
            ui: SharedString::from(".SystemUIFont"),
            mono: SharedString::from(".SystemUIFont"),
            ui_fallbacks: Vec::new(),
            mono_fallbacks: Vec::new(),
            loaded: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// Bytes leídos de las fuentes y errores de lectura.
#[derive(Clone, Debug, Default)]
pub struct PreparedFonts {
    /// Contenido de cada archivo válido.
    pub files: Vec<Vec<u8>>,
    /// Errores accionables.
    pub errors: Vec<String>,
}

/// Lee archivos y directorios de fuentes (recursivo, acotado y determinista).
pub fn read_sources(sources: &[PathBuf]) -> PreparedFonts {
    let mut prepared = PreparedFonts::default();
    let mut total = 0u64;
    let mut seen = std::collections::HashSet::new();
    for source in sources {
        if !source.exists() {
            prepared
                .errors
                .push(format!("No existe la fuente: {}", source.display()));
            continue;
        }
        if source.is_dir() {
            read_dir(source, 0, &mut seen, &mut prepared, &mut total);
        } else {
            read_file(source, &mut seen, &mut prepared, &mut total);
        }
    }
    prepared
}

fn read_dir(
    dir: &Path,
    depth: usize,
    seen: &mut std::collections::HashSet<PathBuf>,
    prepared: &mut PreparedFonts,
    total: &mut u64,
) {
    if depth > 3 {
        prepared.errors.push(format!(
            "Se ignora {}: directorio demasiado profundo",
            dir.display()
        ));
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        prepared
            .errors
            .push(format!("No se pudo leer el directorio {}", dir.display()));
        return;
    };
    let mut entries: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            read_dir(&entry, depth + 1, seen, prepared, total);
        } else {
            read_file(&entry, seen, prepared, total);
        }
    }
}

fn read_file(
    path: &Path,
    seen: &mut std::collections::HashSet<PathBuf>,
    prepared: &mut PreparedFonts,
    total: &mut u64,
) {
    let supported = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc" | "otc"
            )
        });
    if !supported {
        return;
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(canonical) {
        return;
    }
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            prepared
                .errors
                .push(format!("No se pudo leer {}: {error}", path.display()));
            return;
        }
    };
    if metadata.len() > MAX_FONT_FILE_BYTES {
        prepared.errors.push(format!(
            "{} supera el límite de 16 MiB por fuente",
            path.display()
        ));
        return;
    }
    if *total + metadata.len() > MAX_FONT_TOTAL_BYTES {
        prepared.errors.push(format!(
            "Se alcanzó el límite de 64 MiB de fuentes; se ignora {}",
            path.display()
        ));
        return;
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            *total += bytes.len() as u64;
            prepared.files.push(bytes);
        }
        Err(error) => prepared
            .errors
            .push(format!("No se pudo leer {}: {error}", path.display())),
    }
}

/// Registra los bytes de fuentes en el sistema de texto.
pub fn install(cx: &mut App, files: Vec<Vec<u8>>) -> Vec<String> {
    if files.is_empty() {
        return Vec::new();
    }
    let fonts: Vec<Cow<'static, [u8]>> = files.into_iter().map(Cow::Owned).collect();
    match cx.text_system().add_fonts(fonts) {
        Ok(()) => Vec::new(),
        Err(error) => vec![format!("No se pudieron registrar las fuentes: {error}")],
    }
}

/// Resuelve familias reales y cadenas de fallback contra las fuentes instaladas.
pub fn resolve(
    installed: &[String],
    ui_family: Option<&str>,
    mono_family: Option<&str>,
    loaded: usize,
    errors: Vec<String>,
) -> FontSet {
    let mut warnings = Vec::new();
    let ui = choose(
        ui_family,
        UI_PREFERRED,
        installed,
        ".SystemUIFont",
        &mut warnings,
    );
    let mono = choose(
        mono_family,
        MONO_PREFERRED,
        installed,
        ".SystemUIFont",
        &mut warnings,
    );
    FontSet {
        ui: SharedString::from(ui),
        mono: SharedString::from(mono),
        ui_fallbacks: fallbacks(UI_FALLBACKS, installed),
        mono_fallbacks: fallbacks(MONO_FALLBACKS, installed),
        loaded,
        errors,
        warnings,
    }
}

fn choose(
    requested: Option<&str>,
    preferred: &[&str],
    installed: &[String],
    system: &str,
    warnings: &mut Vec<String>,
) -> String {
    if let Some(requested) = requested.map(str::trim).filter(|name| !name.is_empty()) {
        if installed
            .iter()
            .any(|name| name.eq_ignore_ascii_case(requested))
        {
            return requested.to_string();
        }
        warnings.push(format!(
            "La fuente `{requested}` no está instalada; se usa la cadena recomendada."
        ));
    }
    if let Some(found) = preferred.iter().find(|candidate| {
        installed
            .iter()
            .any(|name| name.eq_ignore_ascii_case(candidate))
    }) {
        return (*found).to_string();
    }
    system.to_string()
}

fn fallbacks(preferred: &[&str], installed: &[String]) -> Vec<SharedString> {
    let mut result: Vec<SharedString> = preferred
        .iter()
        .filter(|candidate| {
            installed
                .iter()
                .any(|name| name.eq_ignore_ascii_case(candidate))
        })
        .map(|candidate| SharedString::from(*candidate))
        .collect();
    result.dedup();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn installed(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn requested_installed_font_wins() {
        let mut warnings = Vec::new();
        let picked = choose(
            Some("Mi Fuente"),
            UI_PREFERRED,
            &installed(&["Mi Fuente", "Segoe UI"]),
            ".SystemUIFont",
            &mut warnings,
        );
        assert_eq!(picked, "Mi Fuente");
        assert!(warnings.is_empty());
    }

    #[test]
    fn missing_requested_font_warns_and_falls_back() {
        let mut warnings = Vec::new();
        let picked = choose(
            Some("No Existe"),
            UI_PREFERRED,
            &installed(&["Segoe UI"]),
            ".SystemUIFont",
            &mut warnings,
        );
        assert_eq!(picked, "Segoe UI");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn preferred_order_is_respected() {
        let mut warnings = Vec::new();
        let picked = choose(
            None,
            UI_PREFERRED,
            &installed(&["Arial", "Noto Sans"]),
            ".SystemUIFont",
            &mut warnings,
        );
        assert_eq!(picked, "Noto Sans");
    }

    #[test]
    fn fallbacks_only_include_installed_families() {
        let list = fallbacks(
            UI_FALLBACKS,
            &installed(&["Segoe UI Emoji", "Noto Sans CJK SC"]),
        );
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], "Segoe UI Emoji");
    }

    #[test]
    fn read_sources_ignores_non_font_files_and_reports_missing() {
        let _guard = ENV_LOCK.lock().expect("lock");
        let dir = std::env::temp_dir().join(format!(
            "axfterm-fonts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("notas.txt"), b"no es fuente").expect("txt");
        std::fs::write(dir.join("fake.ttf"), b"datos").expect("ttf");
        let prepared = read_sources(&[dir.clone(), dir.join("missing.ttf")]);
        assert_eq!(prepared.files.len(), 1);
        assert_eq!(prepared.errors.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
