//! Selector de perfil preferido por agente.
//!
//! La lista ofrece «Perfil por defecto» (`None`) y los perfiles actuales del
//! borrador. Se reconstruye cuando cambia la lista de perfiles.

use axf_config::{Profile, ProfileId};
use axf_ui::gpui::SharedString;
use axf_ui::gpui::component::IndexPath;
use axf_ui::gpui::component::searchable_list::{SearchableListItem, SearchableVec};

/// Valor confirmado en el selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProfileValue {
    /// Usar el perfil predeterminado de la configuración.
    Default,
    /// Perfil concreto del borrador.
    Profile(ProfileId),
}

/// Una fila del selector.
#[derive(Clone, Debug)]
pub(crate) struct ProfileChoice {
    title: SharedString,
    value: ProfileValue,
}

impl SearchableListItem for ProfileChoice {
    type Value = ProfileValue;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

/// Lista lista para `SelectState`.
pub(crate) type ProfileList = SearchableVec<ProfileChoice>;

/// Construye la lista con «Perfil por defecto» primero.
pub(crate) fn profile_list(
    profiles: &[Profile],
    current: Option<&ProfileId>,
) -> (ProfileList, Option<IndexPath>) {
    let mut items = Vec::with_capacity(profiles.len() + 1);
    items.push(ProfileChoice {
        title: "Perfil por defecto".into(),
        value: ProfileValue::Default,
    });
    for profile in profiles {
        items.push(ProfileChoice {
            title: SharedString::from(profile.name.clone()),
            value: ProfileValue::Profile(profile.id.clone()),
        });
    }
    let selected = current
        .and_then(|id| profiles.iter().position(|profile| &profile.id == id))
        .map(|index| IndexPath::new(index + 1))
        .unwrap_or(IndexPath::new(0));
    (SearchableVec::from(items), Some(selected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axf_ui::gpui::component::searchable_list::SearchableListDelegate as _;

    #[test]
    fn default_is_first_and_current_profile_is_preselected() {
        let profiles = vec![
            Profile::new("a", "A", "cmd"),
            Profile::new("b", "B", "pwsh"),
        ];
        let (list, selected) = profile_list(&profiles, Some(&ProfileId::new("b")));
        assert_eq!(list.items_count(0), 3);
        assert_eq!(selected, Some(IndexPath::new(2)));

        let (_, default) = profile_list(&profiles, None);
        assert_eq!(default, Some(IndexPath::new(0)));
    }
}
