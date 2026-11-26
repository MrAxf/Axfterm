use crate::gpui::{Pixels, px};
pub const MIN_SCALE: f32 = 0.8;
pub const MAX_SCALE: f32 = 2.0;
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
        px(self.unit() * 2.0)
    }

    /// Icono de un control extrapequeño.
    pub fn control_icon_xs(self) -> Pixels {
        px(self.unit() * 0.75)
    }

    /// Icono de un control pequeño.
    pub fn control_icon_sm(self) -> Pixels {
        px(self.unit() * 0.875)
    }

    /// Icono de un control medio.
    pub fn control_icon_md(self) -> Pixels {
        px(self.unit() * 1.0)
    }

    /// Padding horizontal de un control medio.
    ///
    /// A diferencia de `spacing_md`, este valor sigue la escala de gpui-kit
    /// (`px_2p5`) para que un `ControlSize::Md` propio y un `Size::Medium`
    /// proyectado midan exactamente lo mismo.
    pub fn control_padding_md(self) -> Pixels {
        px(self.unit() * 0.625)
    }

    /// Separación icono‑texto de un control compacto.
    pub fn control_icon_gap_sm(self) -> Pixels {
        px(self.unit() * 0.25)
    }

    /// Separación icono‑texto de una acción principal.
    pub fn control_icon_gap_md(self) -> Pixels {
        px(self.unit() * 0.5)
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

    /// Ancho de las muestras de temas, hasta dos por fila si hay espacio.
    pub fn settings_theme_card_width(self, available: Pixels) -> Pixels {
        ((available - self.spacing_sm()) / 2.0)
            .max(self.font_size * 12.0)
            .min(self.font_size * 22.0)
            .min(available.max(px(0.0)))
    }

    /// Ancho de la paleta de comandos y búsqueda.
    pub fn palette_width(self) -> Pixels {
        px(self.unit() * 46.0)
    }

    /// Alto máximo de la lista de la paleta para un viewport dado.
    ///
    /// Reserva la barra de título, los márgenes, la cabecera (input y controles)
    /// y el pie, de modo que a 200 % de escala la paleta siga cabiendo. En
    /// Búsqueda se reserva además una fila extra por si los filtros envuelven.
    pub fn palette_list_max_height(self, viewport: Pixels, with_filters: bool) -> Pixels {
        let toolbar = self.control_height_sm()
            + if with_filters {
                self.control_height_sm()
            } else {
                px(0.0)
            };
        let header =
            self.spacing_sm() * 2.0 + self.control_height_sm() + self.spacing_sm() + toolbar;
        let list_wrapper = self.spacing_sm() * 2.0;
        let footer = self.spacing_xs() * 2.0 + self.control_height_xs();
        let reserved =
            self.titlebar_height() + self.spacing_xl() * 2.0 + header + list_wrapper + footer;
        (viewport - reserved)
            .max(self.font_size * 3.0)
            .min(self.font_size * 22.0)
    }

    /// Padding vertical de una fila de la paleta.
    pub fn palette_row_padding(self) -> Pixels {
        self.spacing_sm()
    }

    /// Padding horizontal de una fila de la paleta.
    ///
    /// Las filas compactas piden 8–12 px a tamaño de fuente normal; `spacing_sm`
    /// (≈7 px) dejaba el icono y el atajo pegados al borde de la tarjeta.
    pub fn palette_row_padding_x(self) -> Pixels {
        self.spacing_md()
    }

    /// Carril fijo del icono de una fila de la paleta.
    pub fn palette_icon_lane(self) -> Pixels {
        px(self.unit() * 1.5)
    }

    /// Tamaño del título de una fila de la paleta.
    pub fn palette_label_size(self) -> Pixels {
        px(self.unit())
    }

    /// Tamaño del subtítulo de una fila de la paleta.
    pub fn palette_detail_size(self) -> Pixels {
        px(self.unit() * 0.8125)
    }

    /// Tamaño de la etiqueta de tipo de una fila de la paleta.
    pub fn palette_meta_size(self) -> Pixels {
        px(self.unit() * 0.75)
    }

    /// Altura de una fila de la sidebar.
    pub fn row_height(self) -> Pixels {
        px(self.unit() * 1.75)
    }

    /// Ancho visual de la sidebar a partir del valor guardado en sesión.
    ///
    /// La sesión conserva el ancho en la base histórica de 14 px; la UI lo
    /// proyecta con el tamaño de fuente activo para que la sidebar crezca en
    /// `rem` al cambiar la escala.
    pub fn sidebar_width(self, stored: f32) -> Pixels {
        px(stored * self.unit() / 14.0)
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
    ///
    /// La fila de acciones incluye el botón de carpeta y su menú, y la etiqueta
    /// creció con la escala de controles: el atajo solo cabe cuando la sidebar
    /// deja sitio para el texto completo. Por debajo queda en el tooltip.
    pub fn sidebar_actions_compact_width(self) -> Pixels {
        px(self.unit() * 24.0)
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
