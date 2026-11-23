//! Selector de familias tipográficas del panel de ajustes.
//!
//! Envuelve el `Select` de gpui-kit con la lista de fuentes instaladas y una
//! opción «Sistema» que representa `None`. El shell escribe el borrador y el
//! panel lo aplica a `config.toml` al guardar.

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

/// Construye la lista con «Sistema» primero y la familia actual seleccionada.
pub(crate) fn font_list(fonts: &[String], current: Option<&str>) -> (FontList, Option<IndexPath>) {
    let mut items = Vec::with_capacity(fonts.len() + 1);
    items.push(FontChoice {
        title: "Sistema".into(),
        value: FontValue::System,
    });
    for family in fonts {
        items.push(FontChoice {
            title: family.clone().into(),
            value: FontValue::Family(family.clone().into()),
        });
    }
    let selected = current
        .and_then(|family| fonts.iter().position(|candidate| candidate == family))
        .map(|index| IndexPath::new(index + 1))
        .unwrap_or(IndexPath::new(0));
    (SearchableVec::from(items), Some(selected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axf_ui::gpui::component::searchable_list::SearchableListDelegate as _;

    #[test]
    fn system_is_first_and_current_font_is_preselected() {
        let fonts = vec!["Cascadia Mono".to_owned(), "Consolas".to_owned()];
        let (list, selected) = font_list(&fonts, Some("Consolas"));
        assert_eq!(selected, Some(IndexPath::new(2)));
        assert_eq!(list.items_count(0), 3);

        let (_, system) = font_list(&fonts, None);
        assert_eq!(system, Some(IndexPath::new(0)));

        let (_, missing) = font_list(&fonts, Some("No instalada"));
        assert_eq!(missing, Some(IndexPath::new(0)));
    }
}
