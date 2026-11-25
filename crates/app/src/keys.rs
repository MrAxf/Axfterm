//! Chords por defecto de la aplicación y su presentación legible.
//!
//! El registro de acciones (`actions.rs`) los usa como valor inicial y la UI
//! formatea el chord efectivo con `format_chord`, de modo que cambiar un atajo
//! en `config.toml` se refleja en tooltips, menús y la ayuda sin duplicar
//! etiquetas.

/// Cerrar la aplicación.
pub(crate) const QUIT: &str = "ctrl-q";
/// Alternar la sidebar.
pub(crate) const TOGGLE_SIDEBAR: &str = "ctrl-b";
/// Abrir una carpeta local como proyecto.
pub(crate) const OPEN_PROJECT: &str = "ctrl-shift-o";
/// Abrir un proyecto alojado en WSL.
pub(crate) const OPEN_WSL_PROJECT: &str = "ctrl-alt-shift-o";
/// Crear una terminal con el perfil por defecto.
pub(crate) const NEW_TAB: &str = "ctrl-shift-t";
/// Cerrar la pestaña enfocada.
pub(crate) const CLOSE_TAB: &str = "ctrl-shift-w";
/// Pestaña siguiente.
pub(crate) const NEXT_TAB: &str = "ctrl-alt-right";
/// Pestaña anterior.
pub(crate) const PREVIOUS_TAB: &str = "ctrl-alt-left";
/// Mover la pestaña al panel siguiente.
pub(crate) const MOVE_TAB_NEXT_PANE: &str = "ctrl-shift-m";
/// Dividir en columnas.
pub(crate) const SPLIT_HORIZONTAL: &str = "ctrl-shift-e";
/// Dividir en filas.
pub(crate) const SPLIT_VERTICAL: &str = "ctrl-shift-d";
/// Cerrar el panel enfocado.
pub(crate) const CLOSE_PANE: &str = "ctrl-shift-x";
/// Enfocar el panel siguiente.
pub(crate) const FOCUS_NEXT_PANE: &str = "ctrl-alt-down";
/// Enfocar el panel anterior.
pub(crate) const FOCUS_PREVIOUS_PANE: &str = "ctrl-alt-up";
/// Renombrar la pestaña enfocada.
pub(crate) const RENAME_TAB: &str = "ctrl-shift-r";
/// Duplicar la pestaña enfocada.
pub(crate) const DUPLICATE_TAB: &str = "ctrl-shift-u";
/// Zoom temporal del panel.
pub(crate) const TOGGLE_ZOOM: &str = "ctrl-shift-z";
/// Reiniciar la terminal restaurada.
pub(crate) const RESTART_TAB: &str = "ctrl-shift-p";
/// Enfocar la sidebar.
pub(crate) const FOCUS_SIDEBAR: &str = "ctrl-alt-b";
/// Mostrar/ocultar el panel de ajustes.
pub(crate) const SETTINGS: &str = "ctrl-alt-t";
/// Alternar variante claro/oscuro.
pub(crate) const TOGGLE_THEME_VARIANT: &str = "ctrl-alt-v";
/// Tema siguiente.
pub(crate) const NEXT_THEME: &str = "ctrl-alt-n";
/// Tema anterior.
pub(crate) const PREVIOUS_THEME: &str = "ctrl-alt-p";
/// Recargar temas desde disco.
pub(crate) const RELOAD_THEMES: &str = "ctrl-alt-shift-t";
/// Abrir la carpeta de temas.
pub(crate) const OPEN_THEMES_FOLDER: &str = "ctrl-alt-o";
/// Importar un tema.
pub(crate) const IMPORT_THEME: &str = "ctrl-alt-i";
/// Exportar el tema activo.
pub(crate) const EXPORT_THEME: &str = "ctrl-alt-e";
/// Copiar la selección de la terminal.
pub(crate) const COPY_SELECTION: &str = "ctrl-shift-c";
/// Pegar en la terminal.
pub(crate) const PASTE_SELECTION: &str = "ctrl-shift-v";
/// Abrir la paleta de comandos.
pub(crate) const COMMAND_PALETTE: &str = "ctrl-shift-k";
/// Abrir la búsqueda de pestañas, paneles, proyectos y perfiles.
pub(crate) const SEARCH_PALETTE: &str = "ctrl-p";
/// Alternar el filtro de pestañas con agentes.
pub(crate) const TOGGLE_SIDEBAR_AGENTS: &str = "ctrl-alt-a";
/// Ir al siguiente agente que requiere atención.
pub(crate) const AGENT_NEXT_ATTENTION: &str = "ctrl-alt-j";
/// Ver la pantalla de atajos.
pub(crate) const SHOW_SHORTCUTS: &str = "ctrl-alt-k";

/// Formatea un chord de GPUI para mostrarlo en la UI.
///
/// `ctrl-shift-t` se convierte en `Ctrl+Shift+T`. Las flechas usan su símbolo
/// para que los tooltips sigan siendo legibles cuando se muestre navegación.
pub(crate) fn format_chord(chord: &str) -> String {
    chord
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| match part {
            "ctrl" => "Ctrl".to_owned(),
            "shift" => "Shift".to_owned(),
            "alt" => "Alt".to_owned(),
            "cmd" | "super" | "meta" | "win" => "Win".to_owned(),
            "left" => "←".to_owned(),
            "right" => "→".to_owned(),
            "up" => "↑".to_owned(),
            "down" => "↓".to_owned(),
            "space" => "Espacio".to_owned(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_use_readable_modifier_names() {
        assert_eq!(format_chord(NEW_TAB), "Ctrl+Shift+T");
        assert_eq!(format_chord(OPEN_PROJECT), "Ctrl+Shift+O");
        assert_eq!(format_chord(FOCUS_NEXT_PANE), "Ctrl+Alt+↓");
        assert_eq!(format_chord("space"), "Espacio");
    }
}
