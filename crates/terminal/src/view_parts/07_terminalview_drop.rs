impl TerminalView {
    /// Inserta las rutas soltadas sobre la terminal, citadas para su shell.
    fn on_file_drop(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let style = self
            .session
            .as_ref()
            .map(|session| crate::paths::DropPathStyle::for_program(session.program()))
            .unwrap_or_default();
        let text = crate::paths::format_dropped_paths(paths.paths(), style);
        if text.is_empty() {
            return;
        }
        self.drag_hover = false;
        window.focus(&self.focus, cx);
        self.paste_text(&text, cx);
    }
}
