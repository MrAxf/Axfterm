//! Métricas derivadas del tamaño base de la fuente y de la escala global.
//!
//! La regla de producto es que ningún tamaño de la UI sea una constante en
//! píxeles fuera de este módulo (AGENTS.md). El único valor fijo es el grosor
//! mínimo de una línea de 1 px físico, que no puede expresarse como fracción de
//! la fuente sin desaparecer en pantallas de bajo DPI.

include!("metrics_parts/01_min_scale.rs");
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
    fn palette_scales_and_fits_the_viewport() {
        let small = UiMetrics::scaled(12.0, 1.0, 1.0);
        let large = UiMetrics::scaled(16.0, 2.0, 1.0);

        assert!(large.palette_width() > small.palette_width());
        assert!(large.palette_icon_lane() > small.palette_icon_lane());
        assert!(large.palette_label_size() > small.palette_label_size());
        assert!(large.palette_detail_size() > small.palette_detail_size());

        let list = small.palette_list_max_height(px(560.0), false);
        assert!(list > px(0.0));
        assert!(list <= small.font_size * 22.0);

        let zoomed = large.palette_list_max_height(px(560.0), true);
        assert!(zoomed >= large.font_size * 3.0);
        assert!(zoomed <= px(560.0));
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
