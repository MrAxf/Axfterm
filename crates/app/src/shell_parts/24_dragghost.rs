enum DragGhost {
    /// Etiqueta simple (divisores).
    Label(SharedString),
    /// Copia visual de la pestaña arrastrada.
    Tab {
        title: SharedString,
        icon: ProfileIconSource,
        status: Option<TabStatus>,
    },
}
impl DragGhost {
    fn new(label: impl Into<SharedString>) -> Self {
        Self::Label(label.into())
    }

    fn tab(title: SharedString, icon: ProfileIconSource, status: Option<TabStatus>) -> Self {
        Self::Tab {
            title,
            icon,
            status,
        }
    }
}
impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        match self {
            Self::Label(label) => div()
                .px(metrics.spacing_sm())
                .py(metrics.spacing_xxs())
                .rounded(metrics.radius_sm())
                .bg(ui.surface_raised)
                .border_1()
                .border_color(ui.accent)
                .text_color(ui.text)
                .text_size(metrics.font_size)
                .child(label.clone())
                .into_any_element(),
            Self::Tab {
                title,
                icon,
                status,
            } => div()
                .w(metrics.tab_width())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.accent)
                .bg(ui.surface_raised)
                // Ligeramente translúcida: deja ver el destino debajo sin
                // perder legibilidad.
                .opacity(0.8)
                .overflow_hidden()
                .child(
                    TabView::new("drag-ghost", title.clone())
                        .icon_source(icon.clone())
                        .status(*status)
                        .active(true)
                        .focused(true)
                        .full_width(),
                )
                .into_any_element(),
        }
    }
}
