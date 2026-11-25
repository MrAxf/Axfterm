//! Protocolo JSONL de Axfterm Studio.

use serde::{Deserialize, Serialize};

/// Petición de un cliente.
#[derive(Debug, Deserialize)]
pub(super) struct Request {
    /// Identificador elegido por el cliente.
    pub id: u64,
    /// Token escrito en `session.json`.
    pub token: String,
    /// Comando a ejecutar.
    pub cmd: Command,
}

/// Comandos aceptados por el servidor.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Command {
    /// Estado actual de la sesión de Studio.
    State,
    /// Lista de superficies disponibles.
    Surfaces,
    /// Abre una superficie determinista.
    Open { surface: String },
    /// Ejecuta una acción segura del registro.
    Action { id: String },
    /// Cambia el tema por nombre.
    Theme { name: String },
    /// Fuerza la variante clara u oscura.
    Variant { variant: String },
    /// Cambia la escala global de la UI.
    Scale { value: f32 },
    /// Redimensiona la ventana en píxeles lógicos.
    Size { width: f32, height: f32 },
    /// Enfoca una zona del shell.
    Focus { target: String },
    /// Envía una tecla permitida a la ventana enfocada.
    Key { key: String },
    /// Cambia el workspace de demostración.
    Workspace { fixture: String },
    /// Abre o cierra el modal de la galería.
    Modal { open: bool },
    /// Espera antes del siguiente paso (capturas con terminales reales).
    Wait { ms: u64 },
    /// Captura la ventana actual.
    Shot {
        name: String,
        #[serde(default)]
        clip: Option<Clip>,
    },
    /// Cierra la aplicación.
    Quit,
}

/// Recorte opcional en píxeles lógicos.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(super) struct Clip {
    /// Origen X.
    pub x: f32,
    /// Origen Y.
    pub y: f32,
    /// Ancho.
    pub width: f32,
    /// Alto.
    pub height: f32,
}

/// Respuesta del servidor.
#[derive(Debug, Serialize)]
pub(super) struct Reply {
    /// Identificador de la petición.
    pub id: u64,
    /// `true` si el comando se aplicó.
    pub ok: bool,
    /// Estado posterior al comando.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Snapshot>,
    /// Superficies disponibles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surfaces: Option<Vec<String>>,
    /// Datos de la captura.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shot: Option<ShotInfo>,
    /// Error legible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Reply {
    /// Respuesta correcta con estado.
    pub(super) fn ok(id: u64, state: Snapshot) -> Self {
        Self {
            id,
            ok: true,
            state: Some(state),
            surfaces: None,
            shot: None,
            error: None,
        }
    }

    /// Respuesta correcta sin estado (comandos de espera).
    pub(super) fn done(id: u64) -> Self {
        Self {
            id,
            ok: true,
            state: None,
            surfaces: None,
            shot: None,
            error: None,
        }
    }

    /// Respuesta de error.
    pub(super) fn error(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            state: None,
            surfaces: None,
            shot: None,
            error: Some(error.into()),
        }
    }
}

/// Estado que devuelve `state`.
#[derive(Clone, Debug, Serialize)]
pub(super) struct Snapshot {
    /// Superficie activa.
    pub surface: String,
    /// Nombre del tema.
    pub theme: String,
    /// Variante (`dark` o `light`).
    pub variant: String,
    /// Ancho lógico.
    pub width: f32,
    /// Alto lógico.
    pub height: f32,
    /// Escala de la UI.
    pub ui_scale: f32,
    /// Escala del display.
    pub display_scale: f32,
    /// Proyectos del workspace.
    pub projects: usize,
    /// Pestañas del workspace.
    pub tabs: usize,
    /// Paneles del workspace.
    pub panes: usize,
    /// Modal de la galería abierto.
    pub gallery_modal: bool,
    /// Directorio de capturas.
    pub shots_dir: String,
}

/// Información de una captura escrita.
#[derive(Clone, Debug, Serialize)]
pub(super) struct ShotInfo {
    /// Ruta absoluta del PNG.
    pub path: String,
    /// Ancho físico en píxeles.
    pub width: u32,
    /// Alto físico en píxeles.
    pub height: u32,
    /// Ancho lógico.
    pub logical_width: f32,
    /// Alto lógico.
    pub logical_height: f32,
    /// Escala del display usada.
    pub scale: f32,
}

/// Superficies que entiende `open`.
pub(super) const SURFACES: &[&str] = &[
    "app.onboarding",
    "app.workspace",
    "app.settings.appearance",
    "app.settings.profiles",
    "app.settings.profiles.editor",
    "app.settings.shortcuts",
    "app.settings.shortcuts.capture",
    "app.settings.agents",
    "app.palette.commands",
    "app.palette.search",
    "app.shortcuts",
    "app.sidebar.terminals",
    "app.sidebar.agents",
    "app.wsl",
    "app.wsl.live",
    "app.wsl.project",
    "gallery.foundations",
    "gallery.actions",
    "gallery.inputs",
    "gallery.navigation",
    "gallery.feedback",
    "gallery.overlays",
    "gallery.compositions",
    "gallery.icons",
    "gallery.themes",
];

pub(super) fn is_allowed_key(key: &str) -> bool {
    matches!(
        key,
        "tab"
            | "shift-tab"
            | "up"
            | "down"
            | "left"
            | "right"
            | "enter"
            | "space"
            | "escape"
            | "secondary-s"
            | "secondary-enter"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_shot_with_clip() {
        let request: Request = serde_json::from_str(
            r#"{"id":4,"token":"t","cmd":{"type":"shot","name":"x","clip":{"x":1,"y":2,"width":3,"height":4}}}"#,
        )
        .expect("petición válida");
        assert_eq!(request.id, 4);
        match request.cmd {
            Command::Shot { name, clip } => {
                assert_eq!(name, "x");
                let clip = clip.expect("clip");
                assert_eq!(clip.width, 3.0);
            }
            other => panic!("comando inesperado: {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_commands() {
        assert!(
            serde_json::from_str::<Request>(r#"{"id":1,"token":"t","cmd":{"type":"teleport"}}"#)
                .is_err()
        );
    }

    #[test]
    fn key_command_accepts_only_navigation_and_settings_save_keys() {
        assert!(is_allowed_key("tab"));
        assert!(is_allowed_key("shift-tab"));
        assert!(is_allowed_key("enter"));
        assert!(is_allowed_key("secondary-s"));
        assert!(!is_allowed_key("secondary-q"));
        assert!(!is_allowed_key("a"));
    }

    #[test]
    fn surfaces_are_unique_and_namespaced() {
        let mut seen = std::collections::HashSet::new();
        for surface in SURFACES {
            assert!(seen.insert(*surface), "superficie duplicada: {surface}");
            assert!(
                surface.starts_with("app.") || surface.starts_with("gallery."),
                "superficie sin espacio de nombres: {surface}"
            );
        }
    }
}
