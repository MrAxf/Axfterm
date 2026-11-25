//! Proyectos de la sidebar: solo clasificación, nunca dueños del layout.

use std::path::{Path, PathBuf};

use axf_config::ProfileId;
use serde::{Deserialize, Serialize};

use crate::ProjectId;

/// Host donde viven los archivos de un proyecto.
///
/// `root` se interpreta en el dialecto de cada host: una ruta local de Windows
/// para `Local` y una ruta POSIX para `Wsl`. El motor de terminal no conoce
/// esta distinción; la app la traduce a un `SpawnSpec` (ADR 0011).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectHost {
    /// Carpeta del sistema de archivos de la app.
    #[default]
    Local,
    /// Carpeta dentro de una distribución WSL.
    Wsl {
        /// Nombre de la distro tal como lo devuelve `wsl -l -q`.
        distro: String,
    },
}

impl ProjectHost {
    /// Host local.
    pub const fn local() -> Self {
        Self::Local
    }

    /// Host WSL con una distro concreta.
    pub fn wsl(distro: impl Into<String>) -> Self {
        Self::Wsl {
            distro: distro.into(),
        }
    }

    /// `true` si los archivos no viven en el sistema de archivos de la app.
    pub const fn is_remote(&self) -> bool {
        !matches!(self, Self::Local)
    }

    /// Distro WSL, si aplica.
    pub fn wsl_distro(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Wsl { distro } => Some(distro),
        }
    }

    /// Etiqueta corta para la sidebar.
    pub fn label(&self) -> &str {
        match self {
            Self::Local => "Local",
            Self::Wsl { distro } => distro,
        }
    }
}

/// Proyecto registrado en la sidebar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Identificador estable.
    pub id: ProjectId,
    /// Carpeta raíz absoluta en el dialecto del host.
    ///
    /// Para `Local` se guarda normalizada (sin el prefijo verbatim de Windows).
    /// Para `Wsl` es una ruta POSIX y nunca se pasa a `std::fs`.
    pub root: PathBuf,
    /// Host de la carpeta.
    #[serde(default)]
    pub host: ProjectHost,
    /// Perfil con el que se abren las terminales del proyecto.
    ///
    /// `None` significa «perfil global por defecto». Un proyecto WSL siempre lo
    /// fija para no arrancar un shell local con una ruta Linux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<ProfileId>,
    /// Nombre visible (por defecto, el de la carpeta).
    pub name: String,
}

impl Project {
    /// Crea un proyecto local; no canonicaliza ni deduplica (eso es del
    /// workspace).
    pub fn new(id: ProjectId, root: PathBuf, name: String) -> Self {
        Self {
            id,
            root,
            host: ProjectHost::Local,
            default_profile: None,
            name,
        }
    }

    /// Crea un proyecto con host explícito.
    pub fn with_host(id: ProjectId, root: PathBuf, host: ProjectHost, name: String) -> Self {
        Self {
            id,
            root,
            host,
            default_profile: None,
            name,
        }
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

    /// Nombre sugerido para la carpeta en el host indicado.
    pub fn suggested_name(host: &ProjectHost, root: &Path) -> String {
        match host {
            ProjectHost::Local => Self::default_name(root),
            ProjectHost::Wsl { distro } if root == Path::new("/") => distro.clone(),
            ProjectHost::Wsl { .. } => Self::default_name(root),
        }
    }

    /// Ruta legible para tooltips: `Ubuntu · /home/juan/app`.
    pub fn display_root(&self) -> String {
        match &self.host {
            ProjectHost::Local => self.root.display().to_string(),
            ProjectHost::Wsl { distro } => format!("{distro} · {}", self.root.display()),
        }
    }

    /// Perfil que deben usar las terminales nuevas del proyecto.
    pub fn profile_or_global<'a>(&'a self, global: &'a ProfileId) -> &'a ProfileId {
        self.default_profile.as_ref().unwrap_or(global)
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
