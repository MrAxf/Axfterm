# Tareas — M3 Sistema visual

## Tokens y métricas

- [x] `metrics.rs`: `base_font_px` -> spacing, alturas, radios, icon sizes
- [x] Escala global y por DPI/monitor
- [x] Tokens semánticos: bg, surface, border, text, muted, accent, danger,
      warning, success
- [x] Variantes light/dark y detección del tema del sistema

## Temas

- [x] Esquema TOML: meta, UI tokens, terminal (ANSI 16, fg/bg, cursor, selección)
- [x] Built-in: One Dark, Monokai, Cobalt2
- [x] Loader y validador con errores por campo/línea
- [x] Hot reload con `notify`
- [x] Aplicar tema a gpui-kit y a la terminal
- [x] Importar/exportar tema y "abrir carpeta de temas"

## Fuentes

- [x] Fuente de UI y fuente de terminal separadas en config
- [x] `add_fonts` desde archivo y directorio
- [x] Cadena de fallback para Nerd Font, emoji y CJK
- [x] Tamaño base, altura de línea y métricas de celda
- [x] Ligaduras: investigar viabilidad; fuera de v0.1 si no es estable

## Iconos y assets

- [x] Vendorizar subset Tabler (MIT) + script de actualización
- [x] Marcas de simple-icons (CC0) en color
- [x] Componente `icon()` con tamaño relativo y tint
- [x] Pipeline de SVG embebido en el binario

## Componentes

- [x] Tab, botón e icon-button
- [x] Menú, input, toggle y tooltip
- [x] Modal, toast y empty state
- [x] Scrollbar themed y pills de estado
- [x] Refinamiento: pestañas chip, ancho compresible con scroll, icono del shell
      y paneles enfocados sin borde
- [x] Refinamiento: acciones `+` y menú junto a la última pestaña, sin
      desplegable de todas las pestañas e iconos de la barra más grandes
- [x] Refinamiento: márgenes de la tab strip, drag con la pestaña real,
      resaltado del destino y drop en bordes para crear paneles
- [x] Refinamiento: pestañas pegadas, flechas solo con overflow real, título
      por defecto con el nombre del shell y tooltip en cada pestaña
- [x] Refinamiento: auto-scroll del drag una sola vez y geometría de drop
      estable al desplazar la strip, con índice recalculado al soltar
- [x] Refinamiento: nombre manual de pestaña visible en todas las superficies,
      confirmación al perder el foco, cancelación con Escape y foco visible en
      el editor inline
- [x] Refinamiento: copia translúcida de la pestaña durante el drag
- [x] Refinamiento: barra de título propia con marca, toggle de sidebar fijo al
      borde de su columna, botón de ajustes y título `{proyecto}/{terminal}`
      centrado con truncado, y controles nativos de ventana
- [x] Refinamiento: sin barra de estado; proyecto, terminal, tema y avisos ya
      viven en la barra de título, el botón de ajustes y el toast

## Auditoría

- [x] Contraste AA y foco visible en los 3 temas
- [x] Estado nunca solo por color
- [x] Revisión visual con 2 densidades de fuente

