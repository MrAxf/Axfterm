//! Modelo de la paleta de comandos: fuentes, filtrado difuso y selección.
//!
//! La paleta es la superficie keyboard-first del producto. Cada entrada lleva
//! un `PaletteTarget` ejecutable; añadir una fuente futura (contenedores,
//! puertos) es añadir variantes a `PaletteKind` y al recolector del shell, sin
//! tocar el matcher ni el render.

use std::collections::HashMap;

use axf_ui::components::ProfileIconSource;
use axf_ui::gpui::{Entity, ScrollHandle, SharedString};

use axf_ui::gpui::component::input::InputState;

use crate::fuzzy;
use axf_config::ProfileId;
use axf_workspace::{PaneId, ProjectId, TabId};

/// Qué paleta está abierta. Comparten estado, navegación y recientes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaletteMode {
    /// Acciones del registro.
    Commands,
    /// Navegación a pestañas, paneles, proyectos, perfiles y agentes.
    Search,
}

impl PaletteMode {
    pub(crate) fn placeholder(self) -> &'static str {
        match self {
            Self::Commands => "Ejecutar un comando…",
            Self::Search => "Buscar pestañas, paneles, proyectos, perfiles…",
        }
    }

    pub(crate) fn empty(self) -> &'static str {
        match self {
            Self::Commands => "Ningún comando coincide con la búsqueda.",
            Self::Search => "Ninguna pestaña, panel o proyecto coincide.",
        }
    }

    /// Si la entrada pertenece a esta paleta.
    pub(crate) fn includes(self, kind: PaletteKind) -> bool {
        match self {
            Self::Commands => kind == PaletteKind::Command,
            Self::Search => kind != PaletteKind::Command,
        }
    }
}

/// De dónde sale una entrada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaletteKind {
    Command,
    Tab,
    Pane,
    Project,
    Profile,
    Agent,
}

impl PaletteKind {
    /// Tipos que se pueden filtrar en la búsqueda, en el orden de `Ctrl+2..6`.
    pub(crate) const SEARCH_FILTERS: [Self; 5] = [
        Self::Tab,
        Self::Pane,
        Self::Project,
        Self::Profile,
        Self::Agent,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Command => "Comando",
            Self::Tab => "Pestaña",
            Self::Pane => "Panel",
            Self::Project => "Proyecto",
            Self::Profile => "Perfil",
            Self::Agent => "Agente",
        }
    }

    /// Etiqueta plural para los filtros y los estados vacíos.
    pub(crate) fn filter_label(self) -> &'static str {
        match self {
            Self::Command => "Comandos",
            Self::Tab => "Pestañas",
            Self::Pane => "Paneles",
            Self::Project => "Proyectos",
            Self::Profile => "Perfiles",
            Self::Agent => "Agentes",
        }
    }

    /// Si la entrada pasa el filtro activo (`None` = Todo).
    pub(crate) fn matches_filter(self, filter: Option<Self>) -> bool {
        filter.is_none_or(|kind| kind == self)
    }

    pub(crate) fn order(self) -> usize {
        match self {
            Self::Command => 0,
            Self::Tab => 1,
            Self::Pane => 2,
            Self::Project => 3,
            Self::Profile => 4,
            Self::Agent => 5,
        }
    }
}

/// Texto vacío según la paleta y el filtro activo.
pub(crate) fn empty_text(mode: PaletteMode, filter: Option<PaletteKind>) -> String {
    match (mode, filter) {
        (PaletteMode::Search, Some(kind)) => {
            format!("No hay resultados en {}.", kind.filter_label())
        }
        _ => mode.empty().to_owned(),
    }
}

/// Icono de una entrada.
#[derive(Clone, Debug)]
pub(crate) enum PaletteIcon {
    Svg(&'static str),
    Profile(ProfileIconSource),
}

/// Qué hace una entrada al confirmarla.
#[derive(Clone, Debug)]
pub(crate) enum PaletteTarget {
    /// Acción del registro, ejecutada por el despachador de GPUI.
    Command(&'static str),
    FocusTab(TabId),
    FocusPane(PaneId),
    RevealProject(ProjectId),
    NewTabWithProfile(ProfileId),
    JumpToAttention(TabId),
}

/// Entrada completa de la paleta.
#[derive(Clone, Debug)]
pub(crate) struct PaletteItem {
    /// Identificador estable para recientes y frecuencia.
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
    /// Texto extra por el que buscar (keywords, ruta, id).
    pub(crate) search: String,
    pub(crate) kind: PaletteKind,
    pub(crate) icon: PaletteIcon,
    pub(crate) shortcut: Option<String>,
    pub(crate) target: PaletteTarget,
}

/// Fila ya filtrada y puntuada.
#[derive(Clone, Debug)]
pub(crate) struct PaletteRow {
    /// Índice en `PaletteState::items`.
    pub(crate) item: usize,
    pub(crate) score: i32,
    /// Índices del texto original que deben resaltarse.
    pub(crate) indices: Vec<usize>,
}

/// Recientes y frecuencia de uso, en memoria de la sesión.
#[derive(Clone, Debug, Default)]
pub(crate) struct PaletteHistory {
    recent: Vec<String>,
    frequency: HashMap<String, u32>,
}

impl PaletteHistory {
    /// Anota una ejecución; lo más reciente va primero.
    pub(crate) fn record(&mut self, id: &str) {
        self.recent.retain(|existing| existing != id);
        self.recent.insert(0, id.to_owned());
        self.recent.truncate(16);
        *self.frequency.entry(id.to_owned()).or_default() += 1;
    }

    fn bonus(&self, id: &str) -> i32 {
        let recent = self
            .recent
            .iter()
            .position(|existing| existing == id)
            .map(|index| 48 - (index as i32) * 3)
            .unwrap_or(0);
        let frequency = self.frequency.get(id).copied().unwrap_or(0) as i32 * 4;
        recent + frequency
    }
}

/// Estado visible de la paleta.
pub(crate) struct PaletteState {
    pub(crate) mode: PaletteMode,
    /// Filtro de tipo en Búsqueda; `None` muestra todo y en Comandos se ignora.
    pub(crate) filter: Option<PaletteKind>,
    pub(crate) input: Entity<InputState>,
    pub(crate) items: Vec<PaletteItem>,
    pub(crate) rows: Vec<PaletteRow>,
    pub(crate) selected: usize,
    pub(crate) scroll: ScrollHandle,
}

impl PaletteState {
    pub(crate) fn new(
        mode: PaletteMode,
        input: Entity<InputState>,
        items: Vec<PaletteItem>,
        history: &PaletteHistory,
    ) -> Self {
        let items: Vec<PaletteItem> = items
            .into_iter()
            .filter(|item| mode.includes(item.kind))
            .collect();
        let mut state = Self {
            mode,
            filter: None,
            input,
            items,
            rows: Vec::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
        };
        state.refilter("", history);
        state
    }

    /// Cambia de paleta sin cerrar el overlay, conservando consulta y filtro.
    pub(crate) fn set_mode(
        &mut self,
        mode: PaletteMode,
        items: Vec<PaletteItem>,
        query: &str,
        history: &PaletteHistory,
    ) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.items = items
            .into_iter()
            .filter(|item| mode.includes(item.kind))
            .collect();
        self.refilter(query, history);
    }

    /// Cambia el filtro de tipo y vuelve a puntuar las filas.
    pub(crate) fn set_filter(
        &mut self,
        filter: Option<PaletteKind>,
        query: &str,
        history: &PaletteHistory,
    ) {
        self.filter = filter;
        self.refilter(query, history);
    }

    fn active_filter(&self) -> Option<PaletteKind> {
        match self.mode {
            PaletteMode::Search => self.filter,
            PaletteMode::Commands => None,
        }
    }

    pub(crate) fn empty_text(&self) -> String {
        empty_text(self.mode, self.active_filter())
    }

    /// Reordena las filas para la consulta y el filtro actuales.
    pub(crate) fn refilter(&mut self, query: &str, history: &PaletteHistory) {
        let previous = self.current().map(|row| self.items[row.item].id.clone());
        let filter = self.active_filter();
        let mut rows: Vec<PaletteRow> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(item, entry)| {
                if !entry.kind.matches_filter(filter) {
                    return None;
                }
                let haystack = format!(
                    "{} {} {} {} {}",
                    entry.label,
                    entry.detail.as_deref().unwrap_or(""),
                    entry.search,
                    entry.kind.label(),
                    entry.shortcut.as_deref().unwrap_or("")
                );
                let score = fuzzy::score(query, &haystack)?;
                let indices = if query.trim().is_empty() {
                    Vec::new()
                } else {
                    fuzzy::score(query, &entry.label)
                        .map(|matched| matched.indices)
                        .unwrap_or_default()
                };
                let boost = history.bonus(&entry.id);
                Some(PaletteRow {
                    item,
                    score: score.score + boost,
                    indices,
                })
            })
            .collect();

        rows.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| {
                    self.items[left.item]
                        .kind
                        .order()
                        .cmp(&self.items[right.item].kind.order())
                })
                .then_with(|| left.item.cmp(&right.item))
        });
        self.rows = rows;
        if self.rows.is_empty() {
            self.selected = 0;
        } else {
            self.selected = previous
                .as_deref()
                .and_then(|id| {
                    self.rows
                        .iter()
                        .position(|row| self.items[row.item].id == id)
                })
                .unwrap_or(0)
                .min(self.rows.len() - 1);
        }
        self.scroll_to_selected();
    }

    fn scroll_to_selected(&self) {
        if !self.rows.is_empty() {
            self.scroll.scroll_to_item(self.selected);
        }
    }

    pub(crate) fn next(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.rows.len();
        self.scroll_to_selected();
    }

    pub(crate) fn previous(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected + self.rows.len() - 1) % self.rows.len();
        self.scroll_to_selected();
    }

    pub(crate) fn current(&self) -> Option<&PaletteRow> {
        self.rows.get(self.selected)
    }

    pub(crate) fn item_at(&self, row: &PaletteRow) -> &PaletteItem {
        &self.items[row.item]
    }
}

/// Texto con tramos resaltados listo para pintar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HighlightSpan {
    pub(crate) text: SharedString,
    pub(crate) matched: bool,
}

/// Divide una etiqueta en tramos según los índices del matcher.
pub(crate) fn highlight(label: &str, indices: &[usize]) -> Vec<HighlightSpan> {
    if indices.is_empty() {
        return vec![HighlightSpan {
            text: label.to_owned().into(),
            matched: false,
        }];
    }
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_matched = false;
    for (index, ch) in label.chars().enumerate() {
        let matched = indices.contains(&index);
        if matched != current_matched && !current.is_empty() {
            spans.push(HighlightSpan {
                text: current.clone().into(),
                matched: current_matched,
            });
            current.clear();
        }
        current_matched = matched;
        current.push(ch);
    }
    if !current.is_empty() {
        spans.push(HighlightSpan {
            text: current.into(),
            matched: current_matched,
        });
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_splits_only_matched_runs() {
        let spans = highlight("Nueva terminal", &[0, 1, 2]);
        assert_eq!(spans[0].text.as_ref(), "Nue");
        assert!(spans[0].matched);
        assert_eq!(spans[1].text.as_ref(), "va terminal");
        assert!(!spans[1].matched);
    }

    #[test]
    fn history_boosts_recent_entries() {
        let mut history = PaletteHistory::default();
        assert_eq!(history.bonus("tab.new"), 0);
        history.record("tab.new");
        let first = history.bonus("tab.new");
        history.record("tab.close");
        assert!(history.bonus("tab.close") > history.bonus("tab.new"));
        assert!(history.bonus("tab.new") > 0);
        assert!(first > 0);
    }

    #[test]
    fn commands_mode_only_takes_registry_commands() {
        assert!(PaletteMode::Commands.includes(PaletteKind::Command));
        for kind in [
            PaletteKind::Tab,
            PaletteKind::Pane,
            PaletteKind::Project,
            PaletteKind::Profile,
            PaletteKind::Agent,
        ] {
            assert!(!PaletteMode::Commands.includes(kind));
        }
    }

    #[test]
    fn search_mode_only_takes_navigation() {
        assert!(!PaletteMode::Search.includes(PaletteKind::Command));
        for kind in [
            PaletteKind::Tab,
            PaletteKind::Pane,
            PaletteKind::Project,
            PaletteKind::Profile,
            PaletteKind::Agent,
        ] {
            assert!(PaletteMode::Search.includes(kind));
        }
    }

    #[test]
    fn modes_use_distinct_placeholders_and_empty_text() {
        assert_ne!(
            PaletteMode::Commands.placeholder(),
            PaletteMode::Search.placeholder()
        );
        assert_ne!(PaletteMode::Commands.empty(), PaletteMode::Search.empty());
    }

    #[test]
    fn search_filters_have_stable_order_and_labels() {
        assert_eq!(PaletteKind::SEARCH_FILTERS.len(), 5);
        let labels: Vec<&str> = PaletteKind::SEARCH_FILTERS
            .iter()
            .map(|kind| kind.filter_label())
            .collect();
        assert_eq!(
            labels,
            ["Pestañas", "Paneles", "Proyectos", "Perfiles", "Agentes"]
        );
    }

    #[test]
    fn filter_matches_only_its_kind() {
        assert!(PaletteKind::Tab.matches_filter(None));
        assert!(PaletteKind::Tab.matches_filter(Some(PaletteKind::Tab)));
        assert!(!PaletteKind::Tab.matches_filter(Some(PaletteKind::Pane)));
        assert!(!PaletteKind::Project.matches_filter(Some(PaletteKind::Profile)));
    }

    #[test]
    fn empty_text_names_the_active_filter() {
        assert_eq!(
            empty_text(PaletteMode::Search, Some(PaletteKind::Pane)),
            "No hay resultados en Paneles."
        );
        assert_eq!(
            empty_text(PaletteMode::Commands, Some(PaletteKind::Pane)),
            PaletteMode::Commands.empty()
        );
    }
}
