//! Métricas derivadas del tamaño base de la fuente y de la escala global.
//!
//! La regla de producto es que ningún tamaño de la UI sea una constante en
//! píxeles fuera de este módulo (AGENTS.md). El único valor fijo es el grosor
//! mínimo de una línea de 1 px físico, que no puede expresarse como fracción de
//! la fuente sin desaparecer en pantallas de bajo DPI.

use crate::gpui::{Pixels, px};

/// Escala mínima aceptada por la configuración.
pub const MIN_SCALE: f32 = 0.8;
/// Escala máxima aceptada por la configuración.
pub const MAX_SCALE: f32 = 2.0;

/// Tamaños de la shell para un tamaño de fuente y una escala concretos.
///
/// `display_scale` es el factor DPI del monitor activo. GPUI ya entrega píxeles
/// lógicos; aquí solo se usa para redondear hairlines a la rejilla física.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiMetrics {
    /// Tamaño base de la fuente de UI, ya escalado.
    pub font_size: Pixels,
    /// Escala global elegida por el usuario.
    pub scale: f32,
    /// Factor DPI del monitor activo (1.0 = 100 %).
    pub display_scale: f32,
    /// Altura de línea de la UI.
    pub line_height: Pixels,
}

impl UiMetrics {
    /// Métricas para un tamaño base ya resuelto (compatibilidad).
    pub fn new(font_size: Pixels) -> Self {
        let scale = 1.0;
        Self {
            font_size,
            scale,
            display_scale: 1.0,
            line_height: font_size * 1.35,
        }
    }

    /// Métricas a partir del tamaño base en píxeles lógicos y la escala global.
    pub fn scaled(base_font_px: f32, scale: f32, display_scale: f32) -> Self {
        let scale = scale.clamp(MIN_SCALE, MAX_SCALE);
        let display_scale = if display_scale.is_finite() && display_scale > 0.0 {
            display_scale
        } else {
            1.0
        };
        let font_size = px(base_font_px * scale);
        Self {
            font_size,
            scale,
            display_scale,
            line_height: font_size * 1.35,
        }
    }

    /// Recalcula solo el factor DPI (cambio de monitor o de escala del sistema).
    pub fn with_display_scale(self, display_scale: f32) -> Self {
        Self::scaled(self.unit() / self.scale, self.scale, display_scale)
    }

    /// Unidad tipográfica en píxeles lógicos.
    fn unit(self) -> f32 {
        f32::from(self.font_size)
    }

    /// Paso de espaciado más pequeño (separación entre icono y texto).
    pub fn spacing_xxs(self) -> Pixels {
        px(self.unit() * 0.125)
    }

    /// Espaciado compacto entre elementos pegados.
    pub fn spacing_xs(self) -> Pixels {
        px(self.unit() * 0.25)
    }

    /// Espaciado por defecto entre elementos de una fila.
    pub fn spacing_sm(self) -> Pixels {
        px(self.unit() * 0.5)
    }

    /// Espaciado interior de un control.
    pub fn spacing_md(self) -> Pixels {
        px(self.unit() * 0.75)
    }

    /// Espaciado de una sección.
    pub fn spacing_lg(self) -> Pixels {
        px(self.unit())
    }

    /// Separación entre bloques.
    pub fn spacing_xl(self) -> Pixels {
        px(self.unit() * 1.5)
    }

    /// Separación de página.
    pub fn spacing_xxl(self) -> Pixels {
        px(self.unit() * 2.0)
    }

    /// Altura de un control compacto (chips, botones xsmall).
    pub fn control_height_xs(self) -> Pixels {
        px(self.unit() * 1.25)
    }

    /// Altura de un control pequeño.
    pub fn control_height_sm(self) -> Pixels {
        px(self.unit() * 1.5)
    }

    /// Altura de un control medio.
    pub fn control_height_md(self) -> Pixels {
        px(self.unit() * 1.75)
    }

    /// Altura de un control grande.
    pub fn control_height_lg(self) -> Pixels {
        px(self.unit() * 2.0)
    }

    /// Altura de la tab strip.
    pub fn tab_height(self) -> Pixels {
        px(self.unit() * 2.0)
    }

    /// Ancho objetivo de una pestaña antes de comprimirse.
    pub fn tab_width(self) -> Pixels {
        px(self.unit() * 16.0)
    }

    /// Ancho mínimo al que se comprime una pestaña antes de hacer scroll.
    pub fn tab_min_width(self) -> Pixels {
        px(self.unit() * 9.0)
    }

    /// Ancho de un aviso flotante. Es fijo para que el texto tenga un límite
    /// donde envolver y todos los avisos compartan la misma columna.
    pub fn toast_width(self) -> Pixels {
        px(self.unit() * 28.0)
    }

    /// Altura de una fila de la sidebar.
    pub fn row_height(self) -> Pixels {
        px(self.unit() * 1.75)
    }

    /// Altura de una tarjeta de terminal en la sidebar.
    pub fn sidebar_tab_height(self) -> Pixels {
        px(self.unit() * 3.0)
    }

    /// Icono de perfil dentro de una tarjeta de terminal.
    pub fn sidebar_tab_icon(self) -> Pixels {
        px(self.unit() * 1.5)
    }

    /// Carril fijo para el icono o el chevron de una fila de la sidebar.
    pub fn sidebar_icon_lane(self) -> Pixels {
        px(self.unit() * 1.75)
    }

    /// Ancho mínimo para mostrar el atajo dentro del botón de nueva terminal.
    pub fn sidebar_actions_compact_width(self) -> Pixels {
        px(self.unit() * 16.0)
    }

    /// Altura de la barra de título propia.
    ///
    /// Es la altura de la barra de título del SO (32 px lógicos en Windows) y
    /// no depende de la fuente de la UI.
    pub fn titlebar_height(self) -> Pixels {
        px(32.0)
    }

    /// Ancho de cada control de ventana (minimizar, maximizar, cerrar).
    ///
    /// Sigue la medida de la barra nativa de Windows (46 px lógicos).
    pub fn titlebar_control_width(self) -> Pixels {
        px(46.0)
    }

    /// Tamaño del texto de la barra de título, al estilo del SO.
    pub fn titlebar_font_size(self) -> Pixels {
        px(12.0)
    }

    /// Espaciado horizontal de la barra de título.
    pub fn titlebar_padding(self) -> Pixels {
        px(8.0)
    }

    /// Separación entre el icono de la app y su nombre.
    pub fn titlebar_gap(self) -> Pixels {
        px(8.0)
    }

    /// Icono de la app en la barra de título.
    pub fn titlebar_icon(self) -> Pixels {
        px(16.0)
    }

    /// Lado del botón que alterna la sidebar.
    pub fn titlebar_button(self) -> Pixels {
        px(24.0)
    }

    /// Icono del botón que alterna la sidebar.
    pub fn titlebar_button_icon(self) -> Pixels {
        px(14.0)
    }

    /// Icono de minimizar, maximizar y cerrar.
    pub fn titlebar_control_icon(self) -> Pixels {
        px(10.0)
    }

    /// Ancho máximo del título central `{proyecto}/{terminal}`.
    ///
    /// Es chrome de ventana, así que no depende de la fuente de la UI. Si el
    /// hueco libre es menor, el título se recorta por debajo de este tope.
    pub fn titlebar_title_max(self) -> Pixels {
        px(480.0)
    }

    /// Icono extrapequeño (indicador de estado).
    pub fn icon_xs(self) -> Pixels {
        px(self.unit() * 0.625)
    }

    /// Icono pequeño (chevrons, cierre de pestaña).
    pub fn icon_sm(self) -> Pixels {
        px(self.unit() * 0.75)
    }

    /// Icono medio (filas, botones).
    pub fn icon_md(self) -> Pixels {
        px(self.unit() * 0.875)
    }

    /// Icono de la barra de pestañas; iguala el tamaño del texto para que el
    /// icono del shell no quede diminuto dentro del chip.
    pub fn tab_icon(self) -> Pixels {
        px(self.unit())
    }

    /// Icono grande (cabeceras, empty states).
    pub fn icon_lg(self) -> Pixels {
        px(self.unit() * 1.125)
    }

    /// Icono de empty state.
    pub fn icon_xl(self) -> Pixels {
        px(self.unit() * 1.5)
    }

    /// Radio pequeño (chips).
    pub fn radius_sm(self) -> Pixels {
        px(self.unit() * 0.25)
    }

    /// Radio medio (botones, filas).
    pub fn radius_md(self) -> Pixels {
        px(self.unit() * 0.375)
    }

    /// Radio grande (modales, paneles).
    pub fn radius_lg(self) -> Pixels {
        px(self.unit() * 0.5)
    }

    /// Radio de píldora.
    pub fn radius_full(self) -> Pixels {
        px(9999.0)
    }

    /// Padding interior de un panel.
    pub fn panel_padding(self) -> Pixels {
        px(self.unit() * 0.5)
    }

    /// Hueco entre paneles.
    pub fn panel_gap(self) -> Pixels {
        px(self.unit() * 0.25)
    }

    /// Línea de 1 px físico para bordes y hairlines.
    pub fn hairline(self) -> Pixels {
        self.phys(1.0)
    }

    /// Grosor del indicador de la pestaña activa y de la tarjeta enfocada.
    pub fn tab_indicator(self) -> Pixels {
        px(self.unit() * 0.125).max(self.hairline())
    }

    /// Grosor del divisor entre paneles.
    pub fn splitter(self) -> Pixels {
        px(self.unit() * 0.125).max(self.hairline())
    }

    /// Desplazamiento de un chevron de overflow de pestañas.
    pub fn tab_scroll_step(self) -> Pixels {
        px(self.unit() * 7.5)
    }

    /// Banda junto al borde de la tab strip que activa el auto-scroll.
    pub fn drag_edge(self) -> Pixels {
        px(self.unit() * 1.75)
    }

    /// Paso de auto-scroll durante un drag de pestaña.
    pub fn drag_nudge(self) -> Pixels {
        px(self.unit() * 1.25)
    }

    /// Franja desde el borde de un panel en la que soltar una pestaña divide.
    pub fn tab_drop_edge_ratio(self) -> f32 {
        0.25
    }

    /// Grosor del indicador de inserción entre pestañas.
    pub fn tab_insert_width(self) -> Pixels {
        px(self.unit() * 0.125).max(self.hairline())
    }

    /// Redondea una medida lógica a la rejilla del monitor.
    pub fn phys(self, logical: f32) -> Pixels {
        px((logical * self.display_scale).round() / self.display_scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_size_derives_from_the_base() {
        let small = UiMetrics::scaled(12.0, 1.0, 1.0);
        let large = UiMetrics::scaled(18.0, 1.0, 1.0);
        assert!(large.tab_height() > small.tab_height());
        assert!(large.icon_md() > small.icon_md());
        assert!(large.spacing_lg() > small.spacing_lg());
        assert_eq!(small.radius_sm(), px(3.0));
        assert_eq!(small.icon_md(), px(10.5));
    }

    #[test]
    fn the_title_bar_uses_os_sizes_not_the_font() {
        let small = UiMetrics::scaled(11.0, 1.0, 1.0);
        let large = UiMetrics::scaled(24.0, 2.0, 2.0);
        assert_eq!(small.titlebar_height(), px(32.0));
        assert_eq!(large.titlebar_height(), px(32.0));
        assert_eq!(small.titlebar_control_width(), px(46.0));
        assert_eq!(large.titlebar_control_width(), px(46.0));
        assert_eq!(small.titlebar_font_size(), px(12.0));
        assert_eq!(large.titlebar_font_size(), px(12.0));
        assert_eq!(small.titlebar_button(), px(24.0));
        assert_eq!(large.titlebar_button(), px(24.0));
        assert_eq!(small.titlebar_padding(), px(8.0));
        assert_eq!(large.titlebar_padding(), px(8.0));
        assert_eq!(small.titlebar_gap(), px(8.0));
        assert_eq!(large.titlebar_gap(), px(8.0));
        assert_eq!(small.titlebar_control_icon(), px(10.0));
        assert_eq!(large.titlebar_control_icon(), px(10.0));
        assert_eq!(small.titlebar_title_max(), px(480.0));
        assert_eq!(large.titlebar_title_max(), px(480.0));
        assert!(small.titlebar_button() <= small.titlebar_height());
    }

    #[test]
    fn tab_widths_compress_from_a_fixed_max_to_a_min() {
        let metrics = UiMetrics::scaled(12.0, 1.0, 1.0);
        assert_eq!(metrics.tab_width(), px(16.0 * 12.0));
        assert_eq!(metrics.tab_min_width(), px(9.0 * 12.0));
        assert!(metrics.tab_width() > metrics.tab_min_width());

        let large = UiMetrics::scaled(18.0, 1.0, 1.0);
        assert!(large.tab_width() > metrics.tab_width());
        assert!(large.tab_min_width() > metrics.tab_min_width());
        assert_eq!(metrics.tab_icon(), px(12.0));
        assert!(large.tab_icon() > metrics.tab_icon());
        assert_eq!(metrics.tab_drop_edge_ratio(), 0.25);
        assert!(metrics.tab_insert_width() >= metrics.hairline());
        assert_eq!(metrics.toast_width(), px(28.0 * 12.0));
        assert!(large.toast_width() > metrics.toast_width());
    }

    #[test]
    fn sidebar_rows_scale_with_the_base_font() {
        let small = UiMetrics::scaled(12.0, 1.0, 1.0);
        assert_eq!(small.sidebar_tab_height(), px(36.0));
        assert_eq!(small.sidebar_tab_icon(), px(18.0));
        assert_eq!(small.sidebar_icon_lane(), px(21.0));
        assert_eq!(small.sidebar_actions_compact_width(), px(192.0));

        let large = UiMetrics::scaled(18.0, 1.0, 1.0);
        assert!(large.sidebar_tab_height() > small.sidebar_tab_height());
        assert!(large.sidebar_tab_icon() > small.sidebar_tab_icon());
        assert!(large.sidebar_icon_lane() > small.sidebar_icon_lane());
        assert!(large.sidebar_actions_compact_width() > small.sidebar_actions_compact_width());
    }

    #[test]
    fn scale_multiplies_the_base_and_is_clamped() {
        let normal = UiMetrics::scaled(14.0, 1.0, 1.0);
        let double = UiMetrics::scaled(14.0, 2.0, 1.0);
        assert_eq!(f32::from(double.font_size), 28.0);
        assert_eq!(
            f32::from(double.font_size),
            2.0 * f32::from(normal.font_size)
        );
        let clamped = UiMetrics::scaled(14.0, 9.0, 1.0);
        assert_eq!(f32::from(clamped.font_size), 28.0);
    }

    #[test]
    fn display_scale_rounds_hairlines_to_physical_pixels() {
        let hidpi = UiMetrics::scaled(14.0, 1.0, 1.5);
        assert_eq!(hidpi.hairline(), px(4.0 / 3.0));
        let normal = UiMetrics::scaled(14.0, 1.0, 1.0);
        assert_eq!(normal.hairline(), px(1.0));
    }

    #[test]
    fn invalid_display_scale_falls_back_to_one() {
        let metrics = UiMetrics::scaled(14.0, 1.0, 0.0);
        assert_eq!(metrics.display_scale, 1.0);
        let metrics = UiMetrics::scaled(14.0, 1.0, f32::NAN);
        assert_eq!(metrics.display_scale, 1.0);
    }
}
