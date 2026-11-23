//! Modelo de la paleta de comandos: fuentes, filtrado difuso y selección.
//!
//! La paleta es la superficie keyboard-first del producto. Cada entrada lleva
//! un `PaletteTarget` ejecutable; añadir una fuente futura (contenedores,
//! puertos) es añadir variantes a `PaletteKind` y al recolector del shell, sin
//! tocar el matcher ni el render.

include!("palette_parts/01_palettemode.rs");
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
