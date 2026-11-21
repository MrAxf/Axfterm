# Tareas — M3 Sistema visual

## Tokens y métricas

- [ ] `metrics.rs`: `base_font_px` -> spacing, alturas, radios, icon sizes
- [ ] Escala global y por DPI/monitor
- [ ] Tokens semánticos: bg, surface, border, text, muted, accent, danger,
      warning, success
- [ ] Variantes light/dark y detección del tema del sistema

## Temas

- [ ] Esquema TOML: meta, UI tokens, terminal (ANSI 16, fg/bg, cursor, selección)
- [ ] Built-in: One Dark, Monokai, Cobalt2
- [ ] Loader y validador con errores por campo/línea
- [ ] Hot reload con `notify`
- [ ] Aplicar tema a gpui-kit y a la terminal
- [ ] Importar/exportar tema y "abrir carpeta de temas"

## Fuentes

- [ ] Fuente de UI y fuente de terminal separadas en config
- [ ] `add_fonts` desde archivo y directorio
- [ ] Cadena de fallback para Nerd Font, emoji y CJK
- [ ] Tamaño base, altura de línea y métricas de celda
- [ ] Ligaduras: investigar viabilidad; fuera de v0.1 si no es estable

## Iconos y assets

- [ ] Vendorizar subset Tabler (MIT) + script de actualización
- [ ] Marcas de simple-icons (CC0) en color
- [ ] Componente `icon()` con tamaño relativo y tint
- [ ] Pipeline de SVG embebido en el binario

## Componentes

- [ ] Tab, botón e icon-button
- [ ] Menú, input, toggle y tooltip
- [ ] Modal, toast y empty state
- [ ] Scrollbar themed y pills de estado

## Auditoría

- [ ] Contraste AA y foco visible en los 3 temas
- [ ] Estado nunca solo por color
- [ ] Revisión visual con 2 densidades de fuente
