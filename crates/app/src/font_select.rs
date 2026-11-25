//! Selector de familias tipográficas del panel de ajustes.
//!
//! Envuelve el `Select` de gpui-kit con la lista de fuentes instaladas y una
//! opción «Sistema» que representa `None`. Solo se ofrecen familias que el
//! sistema ya tiene instaladas; las recomendadas van primero.

use axf_ui::gpui::SharedString;
use axf_ui::gpui::component::IndexPath;
use axf_ui::gpui::component::searchable_list::{SearchableListItem, SearchableVec};

/// Valor confirmado en el selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FontValue {
    /// Dejar que el sistema resuelva la fuente.
    System,
    /// Familia instalada.
    Family(SharedString),
}

/// Una fila del selector.
#[derive(Clone, Debug)]
pub(crate) struct FontChoice {
    title: SharedString,
    value: FontValue,
}

impl SearchableListItem for FontChoice {
    type Value = FontValue;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

/// Lista lista para `SelectState`.
pub(crate) type FontList = SearchableVec<FontChoice>;

/// Construye la lista con «Sistema» primero, las familias recomendadas después
/// y el resto en orden alfabético.
pub(crate) fn font_list(
    fonts: &[String],
    current: Option<&str>,
    preferred: &[&str],
) -> (FontList, Option<IndexPath>) {
    let mut ordered: Vec<&String> = Vec::with_capacity(fonts.len());
    for family in preferred {
        if let Some(found) = fonts.iter().find(|candidate| candidate == family)
            && !ordered.contains(&found)
        {
            ordered.push(found);
        }
    }
    for family in fonts {
        if !ordered.contains(&family) {
            ordered.push(family);
        }
    }
    let mut items = Vec::with_capacity(ordered.len() + 1);
    items.push(FontChoice {
        title: "Sistema".into(),
        value: FontValue::System,
    });
    for family in &ordered {
        items.push(FontChoice {
            title: SharedString::from((*family).clone()),
            value: FontValue::Family(SharedString::from((*family).clone())),
        });
    }
    let selected = current
        .and_then(|family| ordered.iter().position(|candidate| *candidate == family))
        .map(|index| IndexPath::new(index + 1))
        .unwrap_or(IndexPath::new(0));
    (SearchableVec::from(items), Some(selected))
}

/// Texto exacto de la fuente activa, con la familia resuelta como referencia.
pub(crate) fn font_actual_label(current: Option<&str>, resolved: &str) -> String {
    match current {
        None => format!("Actual: Sistema ({resolved})"),
        Some(family) if family == resolved => format!("Actual: {family}"),
        Some(family) => format!("Actual: {family} (no instalada; se usa {resolved})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axf_ui::fonts::{MONO_PREFERRED, UI_PREFERRED};
    use axf_ui::gpui::component::searchable_list::SearchableListDelegate as _;

    #[test]
    fn system_is_first_and_current_font_is_preselected() {
        let fonts = vec!["Cascadia Mono".to_owned(), "Consolas".to_owned()];
        let (list, selected) = font_list(&fonts, Some("Consolas"), MONO_PREFERRED);
        assert_eq!(selected, Some(IndexPath::new(2)));
        assert_eq!(list.items_count(0), 3);

        let (_, system) = font_list(&fonts, None, MONO_PREFERRED);
        assert_eq!(system, Some(IndexPath::new(0)));

        let (_, missing) = font_list(&fonts, Some("No instalada"), MONO_PREFERRED);
        assert_eq!(missing, Some(IndexPath::new(0)));
    }

    #[test]
    fn preferred_families_go_first_without_duplicates() {
        let fonts = vec![
            "Arial".to_owned(),
            "Consolas".to_owned(),
            "Segoe UI".to_owned(),
            "Zilla Slab".to_owned(),
        ];
        let (list, selected) = font_list(&fonts, Some("Consolas"), UI_PREFERRED);
        assert_eq!(list.items_count(0), 5);
        assert_eq!(selected, Some(IndexPath::new(3)));
    }

    #[test]
    fn actual_label_explains_missing_and_system_fonts() {
        assert_eq!(
            font_actual_label(None, "Segoe UI"),
            "Actual: Sistema (Segoe UI)"
        );
        assert_eq!(
            font_actual_label(Some("Consolas"), "Consolas"),
            "Actual: Consolas"
        );
        assert_eq!(
            font_actual_label(Some("Fira Code"), "Segoe UI"),
            "Actual: Fira Code (no instalada; se usa Segoe UI)"
        );
    }
}
