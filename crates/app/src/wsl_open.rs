//! Estado del modal «Abrir proyecto en WSL» y su lista de carpetas.

use axf_config::ProfileId;
use axf_ui::components::{Icon, IconSize};
use axf_ui::gpui::component::IndexPath;
use axf_ui::gpui::component::list::{ListDelegate, ListItem, ListState};
use axf_ui::gpui::component::searchable_list::{SearchableListItem, SearchableVec};
use axf_ui::gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement as _,
    SharedString, Styled as _, Window, div,
};
use axf_ui::icons;
use axf_ui::theme;

/// Una opción de un `Select` de texto.
#[derive(Clone, Debug)]
pub(crate) struct NameChoice {
    title: SharedString,
    value: String,
}

impl NameChoice {
    pub(crate) fn new(title: impl Into<SharedString>, value: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
        }
    }
}

impl SearchableListItem for NameChoice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

/// Lista para `SelectState`.
pub(crate) type NameList = SearchableVec<NameChoice>;

/// Construye una lista y preselecciona `current`.
pub(crate) fn name_list(
    items: &[(String, String)],
    current: Option<&str>,
) -> (NameList, Option<IndexPath>) {
    let choices: Vec<NameChoice> = items
        .iter()
        .map(|(value, title)| NameChoice::new(title.clone(), value.clone()))
        .collect();
    let selected = current
        .and_then(|current| items.iter().position(|(value, _)| value == current))
        .map(IndexPath::new)
        .or_else(|| (!items.is_empty()).then(|| IndexPath::new(0)));
    (SearchableVec::from(choices), selected)
}

/// Perfil WSL disponible para una distro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WslProfileChoice {
    pub(crate) id: ProfileId,
    pub(crate) name: String,
}

/// Estado completo del modal.
pub(crate) struct WslOpenState {
    pub(crate) focus: FocusHandle,
    pub(crate) distros: Vec<String>,
    pub(crate) distro: Option<String>,
    pub(crate) distro_select:
        Option<Entity<axf_ui::gpui::component::select::SelectState<NameList>>>,
    pub(crate) profiles: Vec<WslProfileChoice>,
    pub(crate) profile_id: Option<ProfileId>,
    pub(crate) profile_select:
        Option<Entity<axf_ui::gpui::component::select::SelectState<NameList>>>,
    pub(crate) path_input: Entity<axf_ui::gpui::component::input::InputState>,
    pub(crate) path: String,
    pub(crate) home: Option<String>,
    pub(crate) list: Entity<ListState<DirectoryDelegate>>,
    pub(crate) loading_distros: bool,
    pub(crate) loading_dirs: bool,
    pub(crate) opening: bool,
    pub(crate) error: Option<String>,
    pub(crate) profile_error: Option<String>,
    pub(crate) generation: u64,
    pub(crate) subscriptions: Vec<axf_ui::gpui::Subscription>,
}

impl WslOpenState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<'_, crate::shell::Shell>) -> Self {
        let focus = cx.focus_handle();
        let path_input = cx.new(|cx| {
            axf_ui::gpui::component::input::InputState::new(window, cx)
                .placeholder("/home/usuario/proyecto")
        });
        let list = cx.new(|cx| ListState::new(DirectoryDelegate::new(), window, cx));
        Self {
            focus,
            distros: Vec::new(),
            distro: None,
            distro_select: None,
            profiles: Vec::new(),
            profile_id: None,
            profile_select: None,
            path_input,
            path: String::new(),
            home: None,
            list,
            loading_distros: true,
            loading_dirs: false,
            opening: false,
            error: None,
            profile_error: None,
            generation: 0,
            subscriptions: Vec::new(),
        }
    }
}

/// Delegado de la lista de subcarpetas.
pub(crate) struct DirectoryDelegate {
    entries: Vec<String>,
    loading: bool,
    error: Option<String>,
    selected: Option<usize>,
    secondary_confirm: bool,
}

impl DirectoryDelegate {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            loading: false,
            error: None,
            selected: None,
            secondary_confirm: false,
        }
    }

    pub(crate) fn set_entries(&mut self, entries: Vec<String>) {
        self.entries = entries;
        self.error = None;
        self.loading = false;
        self.selected = None;
        self.secondary_confirm = false;
    }

    pub(crate) fn secondary_confirm(&self) -> bool {
        self.secondary_confirm
    }

    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        if loading {
            self.error = None;
        }
    }

    pub(crate) fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.entries.clear();
        self.loading = false;
    }

    pub(crate) fn entry(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(String::as_str)
    }
}

impl ListDelegate for DirectoryDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _: &App) -> usize {
        self.entries.len()
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<'_, ListState<Self>>,
    ) -> Option<Self::Item> {
        let name = self.entries.get(index.row)?.clone();
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let selected = self.selected == Some(index.row);
        Some(
            ListItem::new(("wsl-dir", index.row))
                .h(metrics.control_height_sm())
                .accessibility_label(name.clone())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(metrics.spacing_xs())
                        .w_full()
                        .rounded(metrics.radius_sm())
                        .px(metrics.spacing_xs())
                        .bg(if selected {
                            ui.accent.opacity(0.18)
                        } else {
                            ui.surface
                        })
                        .child(Icon::new(icons::FOLDER).size(IconSize::Sm).color(ui.muted))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(display_name(&name)),
                        )
                        .child(
                            Icon::new(icons::CHEVRON_RIGHT)
                                .size(IconSize::Xs)
                                .color(ui.subtle),
                        ),
                ),
        )
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<'_, ListState<Self>>,
    ) {
        self.selected = index.map(|index| index.row);
    }

    fn confirm(&mut self, secondary: bool, _: &mut Window, _: &mut Context<'_, ListState<Self>>) {
        self.secondary_confirm = secondary;
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<'_, ListState<Self>>,
    ) -> impl IntoElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let message = self
            .error
            .clone()
            .unwrap_or_else(|| "Sin subcarpetas".to_owned());
        div()
            .p(metrics.spacing_md())
            .text_center()
            .text_size(metrics.font_size * 0.8125)
            .text_color(if self.error.is_some() {
                ui.danger
            } else {
                ui.muted
            })
            .child(message)
            .into_any_element()
    }

    fn render_loading(
        &mut self,
        _: &mut Window,
        cx: &mut Context<'_, ListState<Self>>,
    ) -> impl IntoElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        div()
            .p(metrics.spacing_md())
            .text_center()
            .text_size(metrics.font_size * 0.8125)
            .text_color(ui.muted)
            .child("Leyendo carpetas…")
            .into_any_element()
    }
}

/// Une una carpeta con un nombre de subcarpeta.
pub(crate) fn join_posix(parent: &str, child: &str) -> String {
    if parent == "/" {
        format!("/{child}")
    } else {
        format!("{}/{child}", parent.trim_end_matches('/'))
    }
}

/// Carpeta padre; la raíz se queda en `/`.
pub(crate) fn parent_posix(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_owned();
    }
    match trimmed.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(index) => trimmed[..index].to_owned(),
    }
}

/// Expande `~` y resuelve una ruta relativa contra `current`.
pub(crate) fn expand_posix(input: &str, current: &str, home: Option<&str>) -> String {
    let input = input.trim();
    if input.is_empty() {
        return current.to_owned();
    }
    if input == "~" {
        return home.unwrap_or("/").to_owned();
    }
    if let Some(rest) = input.strip_prefix("~/") {
        return match home {
            Some(home) => join_posix(home, rest),
            None => join_posix("/", rest),
        };
    }
    if input.starts_with('/') {
        return input.to_owned();
    }
    join_posix(current, input)
}

/// Texto seguro para una fila: los controles se muestran como `?`.
pub(crate) fn display_name(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_control() {
                '?'
            } else {
                character
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_join_and_ascend() {
        assert_eq!(join_posix("/", "home"), "/home");
        assert_eq!(join_posix("/home/juan", "app"), "/home/juan/app");
        assert_eq!(parent_posix("/home/juan/app"), "/home/juan");
        assert_eq!(parent_posix("/home"), "/");
        assert_eq!(parent_posix("/"), "/");
    }

    #[test]
    fn input_expands_home_and_relative_paths() {
        assert_eq!(expand_posix("~", "/", Some("/home/juan")), "/home/juan");
        assert_eq!(
            expand_posix("~/codigo", "/", Some("/home/juan")),
            "/home/juan/codigo"
        );
        assert_eq!(expand_posix("app", "/home/juan", None), "/home/juan/app");
        assert_eq!(expand_posix("/mnt/c/x", "/home/juan", None), "/mnt/c/x");
    }

    #[test]
    fn control_characters_are_hidden_in_rows() {
        assert_eq!(display_name("a\nb"), "a?b");
    }
}
