//! Proyectos de la sidebar: solo clasificación, nunca dueños del layout.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ProjectId;

/// Proyecto registrado en la sidebar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Identificador estable.
    pub id: ProjectId,
    /// Carpeta raíz absoluta y normalizada (sin el prefijo verbatim de
    /// Windows).
    pub root: PathBuf,
    /// Nombre visible (por defecto, el de la carpeta).
    pub name: String,
}

impl Project {
    /// Crea un proyecto; no canonicaliza ni deduplica (eso es del workspace).
    pub fn new(id: ProjectId, root: PathBuf, name: String) -> Self {
        Self { id, root, name }
    }

    /// Nombre derivado de la ruta, con fallback a la ruta completa.
    pub fn default_name(root: &Path) -> String {
        root.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .or_else(|| {
                root.components()
                    .next_back()
                    .map(|component| component.as_os_str().to_string_lossy().into_owned())
            })
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| root.display().to_string())
    }
}

/// Filtro de la sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarSection {
    /// Proyectos y todas sus terminales.
    #[default]
    #[serde(alias = "terminals")]
    All,
    /// Solo las terminales que tienen un agente activo.
    Agents,
}

impl SidebarSection {
    /// Siguiente filtro al alternar con el teclado.
    pub const fn toggled(self) -> Self {
        match self {
            Self::All => Self::Agents,
            Self::Agents => Self::All,
        }
    }

    /// Etiqueta visible.
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "Todas",
            Self::Agents => "Agentes",
        }
    }
}

/// Lado de la ventana donde vive la sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarSide {
    /// Izquierda.
    #[default]
    Left,
    /// Derecha.
    Right,
}

/// Estado de la sidebar; se persiste con la sesión.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SidebarState {
    /// Visibilidad del panel lateral.
    pub visible: bool,
    /// Ancho en píxeles lógicos.
    pub width: f32,
    /// Proyectos desplegados.
    pub expanded: std::collections::BTreeSet<ProjectId>,
    /// Filtro de texto activo.
    pub filter: String,
    /// Sección visible (`Terminales` o `Agentes`).
    #[serde(default)]
    pub section: SidebarSection,
    /// Lado de la ventana.
    #[serde(default)]
    pub side: SidebarSide,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            visible: true,
            width: 240.0,
            expanded: std::collections::BTreeSet::new(),
            filter: String::new(),
            section: SidebarSection::default(),
            side: SidebarSide::default(),
        }
    }
}
