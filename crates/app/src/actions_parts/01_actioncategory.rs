use crate::{
    AgentNextAttention, ClosePane, CloseTab, DuplicateTab, ExportTheme, FocusNextPane,
    FocusPreviousPane, FocusSidebar, ImportTheme, MoveTabNextPane, NewTab, NextTab, NextTheme,
    OpenCommandPalette, OpenProject, OpenSearchPalette, OpenSettings, OpenThemesFolder,
    OpenWslProject,
    PreviousTab, PreviousTheme, Quit, ReloadThemes, RenameTab, RestartTab, ShowShortcuts,
    SplitHorizontal, SplitVertical, ToggleSidebar, ToggleSidebarAgents, ToggleThemeVariant,
    ToggleZoom,
};
use axf_terminal::{CopySelection, PasteSelection};
use axf_ui::gpui::Action;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ActionCategory {
    App,
    Project,
    Pane,
    Tab,
    Profile,
    Theme,
    Agent,
    Help,
}
impl ActionCategory {
    pub(crate) const ALL: &'static [Self] = &[
        Self::App,
        Self::Project,
        Self::Pane,
        Self::Tab,
        Self::Profile,
        Self::Theme,
        Self::Agent,
        Self::Help,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::App => "Aplicación",
            Self::Project => "Proyectos",
            Self::Pane => "Paneles",
            Self::Tab => "Pestañas",
            Self::Profile => "Perfiles",
            Self::Theme => "Tema y fuentes",
            Self::Agent => "Agentes",
            Self::Help => "Ayuda",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct ActionMeta {
    /// Identificador estable usado en `config.toml`.
    pub(crate) id: &'static str,
    /// Etiqueta visible.
    pub(crate) label: &'static str,
    /// Grupo de la paleta y de la ayuda.
    pub(crate) category: ActionCategory,
    /// Chord por defecto en formato GPUI (`ctrl-shift-t`).
    pub(crate) default_chord: &'static str,
    /// Contextos GPUI donde se registra.
    pub(crate) contexts: &'static [&'static str],
    /// Palabras extra para la búsqueda.
    pub(crate) keywords: &'static [&'static str],
    /// `false` oculta la acción de la ayuda y de los atajos.
    pub(crate) enabled: bool,
    /// `false` la oculta de la paleta de comandos; sigue en atajos y ayuda.
    pub(crate) palette: bool,
}
pub(crate) const ALL_CONTEXTS: &[&str] = &[
    crate::WORKSPACE_CONTEXT,
    crate::SIDEBAR_CONTEXT,
    axf_terminal::TERMINAL_CONTEXT,
];
pub(crate) const TERMINAL_CONTEXT_ONLY: &[&str] = &[axf_terminal::TERMINAL_CONTEXT];
pub(crate) const ALL_CONTEXT_ACTIONS: &[ActionMeta] = &[
    meta(
        "app.quit",
        "Salir de Axfterm",
        ActionCategory::App,
        crate::keys::QUIT,
        ALL_CONTEXTS,
        &["cerrar", "exit", "quit"],
    ),
    // Las dos paletas no se listan a sí mismas: abrirlas desde dentro es ruido.
    hidden(meta(
        "palette.open",
        "Abrir la paleta de comandos",
        ActionCategory::App,
        crate::keys::COMMAND_PALETTE,
        ALL_CONTEXTS,
        &["comando", "command", "palette"],
    )),
    hidden(meta(
        "palette.search",
        "Abrir la búsqueda",
        ActionCategory::App,
        crate::keys::SEARCH_PALETTE,
        ALL_CONTEXTS,
        &[
            "buscar",
            "búsqueda",
            "search",
            "pestaña",
            "proyecto",
            "quick open",
        ],
    )),
    meta(
        "sidebar.toggle",
        "Alternar la barra lateral",
        ActionCategory::App,
        crate::keys::TOGGLE_SIDEBAR,
        ALL_CONTEXTS,
        &["sidebar", "barra", "lateral"],
    ),
    meta(
        "sidebar.focus",
        "Enfocar la barra lateral",
        ActionCategory::App,
        crate::keys::FOCUS_SIDEBAR,
        ALL_CONTEXTS,
        &["sidebar", "barra", "lateral", "foco"],
    ),
    meta(
        "sidebar.toggle_agents",
        "Alternar el filtro de agentes",
        ActionCategory::App,
        crate::keys::TOGGLE_SIDEBAR_AGENTS,
        ALL_CONTEXTS,
        &["agentes", "sidebar", "filtro"],
    ),
    meta(
        "project.open",
        "Abrir una carpeta como proyecto",
        ActionCategory::Project,
        crate::keys::OPEN_PROJECT,
        ALL_CONTEXTS,
        &["proyecto", "carpeta", "folder", "local"],
    ),
    meta(
        "project.open_wsl",
        "Abrir un proyecto en WSL",
        ActionCategory::Project,
        crate::keys::OPEN_WSL_PROJECT,
        ALL_CONTEXTS,
        &["proyecto", "wsl", "linux", "ubuntu", "distro"],
    ),
    meta(
        "tab.new",
        "Nueva terminal",
        ActionCategory::Tab,
        crate::keys::NEW_TAB,
        ALL_CONTEXTS,
        &["nueva", "terminal", "tab", "pestana"],
    ),
    meta(
        "tab.close",
        "Cerrar la pestaña",
        ActionCategory::Tab,
        crate::keys::CLOSE_TAB,
        ALL_CONTEXTS,
        &["cerrar", "tab", "pestana"],
    ),
    // Navegación directa: la búsqueda lista las pestañas y los paneles; estos
    // comandos conservan su atajo y su fila en la pantalla de atajos.
    hidden(meta(
        "tab.next",
        "Pestaña siguiente",
        ActionCategory::Tab,
        crate::keys::NEXT_TAB,
        ALL_CONTEXTS,
        &["siguiente", "tab", "pestana"],
    )),
    hidden(meta(
        "tab.previous",
        "Pestaña anterior",
        ActionCategory::Tab,
        crate::keys::PREVIOUS_TAB,
        ALL_CONTEXTS,
        &["anterior", "tab", "pestana"],
    )),
    meta(
        "tab.rename",
        "Renombrar la pestaña",
        ActionCategory::Tab,
        crate::keys::RENAME_TAB,
        ALL_CONTEXTS,
        &["renombrar", "titulo", "tab"],
    ),
    meta(
        "tab.duplicate",
        "Duplicar la pestaña",
        ActionCategory::Tab,
        crate::keys::DUPLICATE_TAB,
        ALL_CONTEXTS,
        &["duplicar", "copiar", "tab"],
    ),
    meta(
        "tab.restart",
        "Reiniciar la terminal",
        ActionCategory::Tab,
        crate::keys::RESTART_TAB,
        ALL_CONTEXTS,
        &["reiniciar", "restart", "terminal"],
    ),
    meta(
        "tab.move_next_pane",
        "Mover la pestaña al panel siguiente",
        ActionCategory::Tab,
        crate::keys::MOVE_TAB_NEXT_PANE,
        ALL_CONTEXTS,
        &["mover", "panel", "pane", "tab"],
    ),
    meta(
        "pane.split_horizontal",
        "Dividir en columnas",
        ActionCategory::Pane,
        crate::keys::SPLIT_HORIZONTAL,
        ALL_CONTEXTS,
        &["dividir", "split", "columna", "derecha"],
    ),
    meta(
        "pane.split_vertical",
        "Dividir en filas",
        ActionCategory::Pane,
        crate::keys::SPLIT_VERTICAL,
        ALL_CONTEXTS,
        &["dividir", "split", "fila", "abajo"],
    ),
    meta(
        "pane.close",
        "Cerrar el panel",
        ActionCategory::Pane,
        crate::keys::CLOSE_PANE,
        ALL_CONTEXTS,
        &["cerrar", "panel", "pane"],
    ),
    hidden(meta(
        "pane.focus_next",
        "Enfocar el panel siguiente",
        ActionCategory::Pane,
        crate::keys::FOCUS_NEXT_PANE,
        ALL_CONTEXTS,
        &["panel", "siguiente", "foco", "pane"],
    )),
    hidden(meta(
        "pane.focus_previous",
        "Enfocar el panel anterior",
        ActionCategory::Pane,
        crate::keys::FOCUS_PREVIOUS_PANE,
        ALL_CONTEXTS,
        &["panel", "anterior", "foco", "pane"],
    )),
    meta(
        "pane.toggle_zoom",
        "Alternar el zoom temporal del panel",
        ActionCategory::Pane,
        crate::keys::TOGGLE_ZOOM,
        ALL_CONTEXTS,
        &["zoom", "panel", "pane"],
    ),
    meta(
        "settings.open",
        "Abrir los ajustes",
        ActionCategory::App,
        crate::keys::SETTINGS,
        ALL_CONTEXTS,
        &["ajustes", "settings", "config", "tema", "fuente", "atajo"],
    ),
    // Id legado: se conserva para que un `[keymap]` existente siga funcionando,
    // pero el panel es ahora la única superficie de tema y fuentes.
    disabled(hidden(meta(
        "theme.info",
        "Abrir los ajustes (id legado)",
        ActionCategory::Theme,
        "",
        ALL_CONTEXTS,
        &["tema", "fuente", "ajustes", "theme"],
    ))),
    // El modal «Tema y fuentes» concentra variante, catálogo y botones de
    // gestión; en la paleta de comandos solo queda la entrada al modal.
    hidden(meta(
        "theme.toggle_variant",
        "Previsualizar la variante clara/oscura",
        ActionCategory::Theme,
        crate::keys::TOGGLE_THEME_VARIANT,
        ALL_CONTEXTS,
        &["tema", "claro", "oscuro", "variante"],
    )),
    hidden(meta(
        "theme.next",
        "Tema siguiente",
        ActionCategory::Theme,
        crate::keys::NEXT_THEME,
        ALL_CONTEXTS,
        &["tema", "siguiente"],
    )),
    hidden(meta(
        "theme.previous",
        "Tema anterior",
        ActionCategory::Theme,
        crate::keys::PREVIOUS_THEME,
        ALL_CONTEXTS,
        &["tema", "anterior"],
    )),
    hidden(meta(
        "theme.reload",
        "Recargar temas desde disco",
        ActionCategory::Theme,
        crate::keys::RELOAD_THEMES,
        ALL_CONTEXTS,
        &["tema", "recargar", "reload"],
    )),
    hidden(meta(
        "theme.open_folder",
        "Abrir la carpeta de temas",
        ActionCategory::Theme,
        crate::keys::OPEN_THEMES_FOLDER,
        ALL_CONTEXTS,
        &["tema", "carpeta", "folder"],
    )),
    hidden(meta(
        "theme.import",
        "Importar un tema",
        ActionCategory::Theme,
        crate::keys::IMPORT_THEME,
        ALL_CONTEXTS,
        &["tema", "importar"],
    )),
    hidden(meta(
        "theme.export",
        "Exportar el tema activo",
        ActionCategory::Theme,
        crate::keys::EXPORT_THEME,
        ALL_CONTEXTS,
        &["tema", "exportar"],
    )),
    meta(
        "agent.next_attention",
        "Ir al siguiente agente que requiere atención",
        ActionCategory::Agent,
        crate::keys::AGENT_NEXT_ATTENTION,
        ALL_CONTEXTS,
        &["agente", "atencion", "attention", "jump"],
    ),
    meta(
        "help.shortcuts",
        "Ver todos los atajos",
        ActionCategory::Help,
        crate::keys::SHOW_SHORTCUTS,
        ALL_CONTEXTS,
        &["atajos", "shortcuts", "teclado", "ayuda"],
    ),
];
pub(crate) const TERMINAL_ACTIONS: &[ActionMeta] = &[
    meta(
        "terminal.copy",
        "Copiar la selección",
        ActionCategory::Tab,
        crate::keys::COPY_SELECTION,
        TERMINAL_CONTEXT_ONLY,
        &["copiar", "copy", "seleccion"],
    ),
    meta(
        "terminal.paste",
        "Pegar en la terminal",
        ActionCategory::Tab,
        crate::keys::PASTE_SELECTION,
        TERMINAL_CONTEXT_ONLY,
        &["pegar", "paste"],
    ),
];
const fn meta(
    id: &'static str,
    label: &'static str,
    category: ActionCategory,
    default_chord: &'static str,
    contexts: &'static [&'static str],
    keywords: &'static [&'static str],
) -> ActionMeta {
    ActionMeta {
        id,
        label,
        category,
        default_chord,
        contexts,
        keywords,
        enabled: true,
        palette: true,
    }
}
const fn hidden(action: ActionMeta) -> ActionMeta {
    let mut action = action;
    action.palette = false;
    action
}
const fn disabled(action: ActionMeta) -> ActionMeta {
    let mut action = action;
    action.enabled = false;
    action
}
pub(crate) fn all() -> impl Iterator<Item = &'static ActionMeta> {
    ALL_CONTEXT_ACTIONS.iter().chain(TERMINAL_ACTIONS)
}
pub(crate) fn palette_actions() -> impl Iterator<Item = &'static ActionMeta> {
    all().filter(|action| action.enabled && action.palette)
}
pub(crate) fn by_id(id: &str) -> Option<&'static ActionMeta> {
    all().find(|action| action.id == id)
}
pub(crate) fn action_for(id: &str) -> Option<Box<dyn Action>> {
    Some(match id {
        "app.quit" => Box::new(Quit),
        "palette.open" => Box::new(OpenCommandPalette),
        "palette.search" => Box::new(OpenSearchPalette),
        "sidebar.toggle" => Box::new(ToggleSidebar),
        "sidebar.focus" => Box::new(FocusSidebar),
        "sidebar.toggle_agents" => Box::new(ToggleSidebarAgents),
        "project.open" => Box::new(OpenProject),
        "project.open_wsl" => Box::new(OpenWslProject),
        "tab.new" => Box::new(NewTab),
        "tab.close" => Box::new(CloseTab),
        "tab.next" => Box::new(NextTab),
        "tab.previous" => Box::new(PreviousTab),
        "tab.rename" => Box::new(RenameTab),
        "tab.duplicate" => Box::new(DuplicateTab),
        "tab.restart" => Box::new(RestartTab),
        "tab.move_next_pane" => Box::new(MoveTabNextPane),
        "pane.split_horizontal" => Box::new(SplitHorizontal),
        "pane.split_vertical" => Box::new(SplitVertical),
        "pane.close" => Box::new(ClosePane),
        "pane.focus_next" => Box::new(FocusNextPane),
        "pane.focus_previous" => Box::new(FocusPreviousPane),
        "pane.toggle_zoom" => Box::new(ToggleZoom),
        "theme.info" => Box::new(OpenSettings),
        "settings.open" => Box::new(OpenSettings),
        "theme.toggle_variant" => Box::new(ToggleThemeVariant),
        "theme.next" => Box::new(NextTheme),
        "theme.previous" => Box::new(PreviousTheme),
        "theme.reload" => Box::new(ReloadThemes),
        "theme.open_folder" => Box::new(OpenThemesFolder),
        "theme.import" => Box::new(ImportTheme),
        "theme.export" => Box::new(ExportTheme),
        "agent.next_attention" => Box::new(AgentNextAttention),
        "help.shortcuts" => Box::new(ShowShortcuts),
        "terminal.copy" => Box::new(CopySelection),
        "terminal.paste" => Box::new(PasteSelection),
        _ => return None,
    })
}
