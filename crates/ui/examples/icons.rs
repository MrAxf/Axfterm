//! Spike S7 — iconos: Tabler tintado por token y marca de simple-icons a color.
//!
//! ```powershell
//! cargo run -p axf_ui --example icons
//! ```
//!
//! Comprobación visual: los Tabler cambian de color con el token del tema, la
//! misma marca de Rust se puede tintar en monócromo y, con `brand::RUST`, toma
//! su color oficial. La carga de assets está cubierta por los tests de
//! `assets.rs`, que incluyen el icono de marca vendorizado.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "spike visual: informa de los assets que va a pintar"
)]

use axf_ui::gpui::component::ActiveTheme;
use axf_ui::gpui::component::theme::{Theme, ThemeMode};
use axf_ui::gpui::{
    AppContext, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, div, px, size,
    svg,
};
use axf_ui::{Assets, brand, icons};

struct IconGallery {
    entries: Vec<(&'static str, &'static str, axf_ui::gpui::Hsla)>,
}

impl IconGallery {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        let theme = cx.theme();
        let foreground = theme.foreground;
        let accent = theme.accent;
        let brand_rust = brand::hsla(brand::RUST);

        let entries = vec![
            (icons::TERMINAL, "terminal-2 · token foreground", foreground),
            (
                icons::SIDEBAR,
                "layout-sidebar · token foreground",
                foreground,
            ),
            (icons::SETTINGS, "settings · token accent", accent),
            (icons::PLUS, "plus · token accent", accent),
            (icons::RUST, "rust · tintado monócromo", foreground),
            (icons::RUST, "rust · color de marca F74C00", brand_rust),
        ];

        for (path, label, _) in &entries {
            println!("icono {path} -> {label}");
        }

        Self { entries }
    }
}

impl Render for IconGallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let (background, foreground, muted, border, font_size) = {
            let theme = cx.theme();
            (
                theme.background,
                theme.foreground,
                theme.muted_foreground,
                theme.border,
                theme.font_size,
            )
        };
        let icon_size = font_size * 2.0;

        let rows = self
            .entries
            .iter()
            .map(|(path, label, color)| {
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(icon_size * 2.0)
                            .child(svg().path(*path).size(icon_size).text_color(*color)),
                    )
                    .child(div().child(*label))
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .p_6()
            .child(
                div()
                    .text_color(muted)
                    .pb_4()
                    .child("Iconos: Tabler tintado por token + simple-icons a color de marca"),
            )
            .children(rows)
    }
}

fn main() {
    axf_ui::gpui::application().with_assets(Assets).run(|cx| {
        axf_ui::gpui::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);

        let options = axf_ui::gpui::WindowOptions {
            window_bounds: Some(axf_ui::gpui::WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(720.0), px(520.0)),
                cx,
            ))),
            titlebar: Some(axf_ui::gpui::TitlebarOptions {
                title: Some("Axfterm — spike iconos".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = axf_ui::gpui::open_window(options, cx, |_window, cx| cx.new(IconGallery::new));
        if let Err(error) = result {
            eprintln!("icons: no se pudo abrir la ventana: {error}");
        }
    });
}
