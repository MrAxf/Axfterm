//! Icono Tabler tintado por token.

use std::path::PathBuf;
use std::time::Duration;

use crate::gpui::{
    Animation, AnimationExt as _, App, ImageSource, IntoElement, ObjectFit, Pixels, RenderOnce,
    Styled, StyledImage as _, Transformation, Window, img, percentage, svg,
};
use crate::metrics::UiMetrics;
use crate::theme;

/// Tamaño relativo de un icono.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum IconSize {
    /// Indicador de estado.
    Xs,
    /// Chevron, cierre.
    #[default]
    Sm,
    /// Fila, botón.
    Md,
    /// Cabecera.
    Lg,
    /// Empty state.
    Xl,
    /// Medida explícita.
    Custom(Pixels),
}

impl IconSize {
    fn pixels(self, metrics: UiMetrics) -> Pixels {
        match self {
            Self::Xs => metrics.icon_xs(),
            Self::Sm => metrics.icon_sm(),
            Self::Md => metrics.icon_md(),
            Self::Lg => metrics.icon_lg(),
            Self::Xl => metrics.icon_xl(),
            Self::Custom(pixels) => pixels,
        }
    }
}

/// Icono SVG con tamaño relativo y color de token.
#[derive(IntoElement, Clone, Copy, Debug)]
pub struct Icon {
    path: &'static str,
    size: IconSize,
    color: Option<crate::gpui::Hsla>,
    spin: bool,
}

impl Icon {
    /// Crea un icono a partir de una ruta de `icons`.
    pub fn new(path: &'static str) -> Self {
        Self {
            path,
            size: IconSize::Md,
            color: None,
            spin: false,
        }
    }

    /// Tamaño relativo.
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Color explícito; por defecto, el texto del tema.
    pub fn color(mut self, color: crate::gpui::Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Rotación continua de 360° (para spinners de actividad).
    pub fn spin(mut self, spin: bool) -> Self {
        self.spin = spin;
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let color = self.color.unwrap_or(state.theme.ui.text);
        let element = svg()
            .path(self.path)
            .size(self.size.pixels(state.metrics))
            .flex_shrink_0()
            .text_color(color);
        if self.spin {
            element
                .with_animation(
                    "icon-spin",
                    Animation::new(Duration::from_millis(900)).repeat(),
                    |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
                )
                .into_any_element()
        } else {
            element.into_any_element()
        }
    }
}

/// Origen de un icono de perfil: SVG embebido o imagen en disco.
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileIconSource {
    /// Icono vendorizado (fallback Tabler).
    Svg(&'static str),
    /// Ruta a un PNG/JPG/ICO/SVG/BMP/TIFF.
    Image(PathBuf),
}

impl ProfileIconSource {
    /// Fallback Tabler si no hay imagen.
    pub fn fallback() -> Self {
        Self::Svg(crate::icons::TERMINAL)
    }

    /// Fuente a partir de una ruta opcional.
    pub fn from_path(path: Option<PathBuf>) -> Self {
        match path {
            Some(path) => Self::Image(path),
            None => Self::fallback(),
        }
    }
}

/// Icono de perfil con fallback Tabler si la imagen no carga.
#[derive(IntoElement, Clone, Debug)]
pub struct ProfileIcon {
    source: ProfileIconSource,
    size: IconSize,
    color: Option<crate::gpui::Hsla>,
}

impl ProfileIcon {
    /// Crea el icono con su origen.
    pub fn new(source: ProfileIconSource) -> Self {
        Self {
            source,
            size: IconSize::Md,
            color: None,
        }
    }

    /// Tamaño relativo a la fuente.
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Color para SVG y fallback; las imágenes conservan su color.
    pub fn color(mut self, color: crate::gpui::Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for ProfileIcon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = theme::state(cx);
        let pixels = self.size.pixels(state.metrics);
        let color = self.color.unwrap_or(state.theme.ui.text);
        match self.source {
            ProfileIconSource::Svg(path) => Icon::new(path)
                .size(self.size)
                .color(color)
                .into_any_element(),
            ProfileIconSource::Image(path) => {
                let size = self.size;
                img(ImageSource::from(path))
                    .size(pixels)
                    .flex_shrink_0()
                    .object_fit(ObjectFit::Contain)
                    .with_fallback(move || {
                        Icon::new(crate::icons::TERMINAL)
                            .size(size)
                            .color(color)
                            .into_any_element()
                    })
                    .into_any_element()
            }
        }
    }
}
