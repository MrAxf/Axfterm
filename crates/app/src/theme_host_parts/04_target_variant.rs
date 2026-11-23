fn target_variant(appearance: Appearance, system_dark: bool) -> ThemeVariant {
    match appearance {
        Appearance::Light => ThemeVariant::Light,
        Appearance::Dark => ThemeVariant::Dark,
        Appearance::System if system_dark => ThemeVariant::Dark,
        Appearance::System => ThemeVariant::Light,
    }
}
fn system_is_dark(cx: &App) -> bool {
    matches!(
        cx.window_appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}
fn resolved_font_sources(config: &Config, base: &std::path::Path) -> Vec<PathBuf> {
    config
        .fonts
        .sources
        .iter()
        .map(|source| config.resolve_font_source(base, source))
        .collect()
}
