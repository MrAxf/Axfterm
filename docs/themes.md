# Temas, fuentes y configuración visual

Axfterm lee `config.toml` y los temas de `themes/*.toml` del directorio de
configuración (ver `crates/config/src/paths.rs`):

| Plataforma | Directorio |
| --- | --- |
| Windows | `%APPDATA%\axfterm\` |
| macOS | `~/Library/Application Support/axfterm/` |
| Linux/BSD | `$XDG_CONFIG_HOME/axfterm/` o `~/.config/axfterm/` |

`AXFTERM_CONFIG_DIR` (ruta absoluta) solo se usa en tests y diagnóstico.

## `config.toml`

```toml
schema_version = 2

# system | light | dark
appearance = "system"

[ui]
theme = "One Dark"
# font_family = "Segoe UI"
font_size = 14.0
scale = 1.0

[terminal]
# font_family = "Cascadia Mono"
font_size = 13.0
line_height = 1.0
default_profile = "pwsh"

[fonts]
# Archivos .ttf/.otf/.ttc o directorios (Nerd Font, CJK, emoji).
sources = []

# Ver docs/profiles.md para [[profiles]].
```

- `schema_version` ausente se interpreta como `1` y se migra a `2` en memoria;
  la migración se guarda una sola vez conservando comentarios. Una versión
  futura no se reescribe: la app avisa y arranca con defaults.

- Los campos desconocidos son un error con archivo, línea y columna.
- `ui.font_size` admite 11–24 px; `ui.scale`, 0.8–2.0; `terminal.font_size`,
  8–32 px; `terminal.line_height`, 1.0–2.5.
- `appearance = "system"` elige la variante clara u oscura del sistema y la
  sigue en vivo cuando el SO cambia de apariencia. Si el tema activo no tiene
  pareja, se usa el tema por defecto de esa variante.
- Todos los tamaños de la UI derivan de `ui.font_size × ui.scale` en
  `crates/ui/src/metrics.rs`. El DPI del monitor se aplica al redondear
  hairlines y se refresca al mover la ventana entre monitores.
- `terminal.line_height = 1.0` conserva la altura natural del glifo (la que
  mantiene contiguas las cajas de una TUI). Un valor mayor añade aire sin
  romper la rejilla.

## Temas

Un tema es un TOML independiente en `themes/`. El nombre debe ser único y la
familia empareja variantes (`One Dark`/`One Light`).

```toml
[meta]
name = "Mi Tema"
family = "Mi Tema"          # opcional; si falta se deriva del nombre
author = "Yo"               # opcional
variant = "dark"            # "dark" | "light"

[ui]
background = "#1e2127"
surface = "#21242b"         # opcional: sidebar, barras
surface_raised = "#282c34"  # opcional: popovers, tarjetas
border = "#3a3f4b"
border_strong = "#4b5260"   # opcional: scrollbar, separadores
text = "#abb2bf"
muted = "#8d95a3"
subtle = "#6b7280"          # opcional: metadatos
accent = "#61afef"
accent_text = "#101317"     # opcional: se deriva para AA
danger = "#e06c75"
warning = "#e5c07b"
success = "#98c379"
info = "#56b6c2"            # opcional: usa accent
selection = "#61afef60"     # opcional; alfa recto
focus = "#61afef"           # opcional: usa accent

[terminal]
foreground = "#abb2bf"
background = "#1e2127"
cursor = "#61afef"
selection = "#61afef60"
dim_foreground = "#6b7280"  # opcional
ansi = [                    # exactamente 16
  "#1e2127", "#e06c75", "#98c379", "#e5c07b",
  "#61afef", "#c678dd", "#56b6c2", "#abb2bf",
  "#5c6370", "#e06c75", "#98c379", "#e5c07b",
  "#61afef", "#c678dd", "#56b6c2", "#ffffff",
]
```

- Colores: `#RGB`, `#RGBA`, `#RRGGBB` o `#RRGGBBAA`.
- Los campos obligatorios son `meta.name`, `meta.variant`, `ui.background`,
  `ui.surface`, `ui.border`, `ui.text`, `ui.muted`, `ui.accent`, `ui.danger`,
  `ui.warning`, `ui.success`, `terminal.*` y los 16 ANSI.
- Los opcionales se derivan con contraste legible. La paleta ANSI alimenta
  tanto el render como las respuestas a `OSC 10/11/12`.
- Un archivo roto no tumba la app: se ignora, se muestra el error con línea y
  campo, y el resto de temas sigue disponible.

### Built-in

| Familia | Oscuro | Claro |
| --- | --- | --- |
| One | One Dark | One Light |
| Monokai | Monokai | Monokai Light |
| Cobalt2 | Cobalt2 | Cobalt2 Light |

Los TOML viven en `crates/ui/assets/themes/` y se embeben en el binario.

## Hot reload

- `notify` observa `config.toml` y `themes/` con debounce de 250 ms.
- Un cambio de tema, fuente o escala recalcula métricas, limpia cachés de
  texto y repinta terminales existentes sin recrear PTYs.
- Un error de config conserva la última configuración válida y avisa con
  archivo/línea; no deja estado a medias.
- Los perfiles nuevos se aplican a las pestañas nuevas; las abiertas conservan
  su proceso y su `SpawnSpec` (ver `docs/profiles.md`).
- `Ctrl+Alt+Shift+T` fuerza una recarga completa.

## Fuentes

- `[ui].font_family` y `[terminal].font_family` son independientes.
- `[fonts].sources` acepta archivos y directorios (hasta 16 rutas, 16 MiB por
  archivo, 64 MiB por carga). Se registran con `add_fonts`.
- Si la familia pedida no está instalada, se avisa y se usa la cadena
  recomendada: Nerd Font → Cascadia/Consolas → sistema para mono; Inter/Segoe
  UI → sistema para UI.
- Los fallbacks de emoji, CJK y símbolos Nerd Font se pasan al terminal y se
  apoyan en el stack de la plataforma para la UI.
- Las ligaduras quedan fuera de v0.1 (ADR 0005): el shaping por celda del
  terminal no garantiza avances monoespaciados ni una rejilla estable.

## Acciones de tema

| Acción | Atajo | Uso |
| --- | --- | --- |
| Mostrar tema y fuentes | `Ctrl+Alt+T` | Modal con catálogo, tipografía y escala |
| Cerrar modal | `Esc` | Vuelve el foco a la terminal activa |
| Alternar variante | `Ctrl+Alt+V` | Previsualiza claro/oscuro |
| Tema siguiente/anterior | `Ctrl+Alt+N` / `Ctrl+Alt+P` | Cicla el catálogo |
| Recargar | `Ctrl+Alt+Shift+T` | Reload manual |
| Abrir carpeta de temas | `Ctrl+Alt+O` | Abre `themes/` en el sistema |
| Importar / exportar | `Ctrl+Alt+I` / `Ctrl+Alt+E` | Copia o guarda un TOML |

El archivo sigue siendo la fuente de verdad: la selección y la previsualización
del modal son de sesión y no reescriben `config.toml`.

Para inspección visual, `AXFTERM_SHOW_THEME_MODAL=1` abre el modal al arrancar
y `crates/ui/examples/gallery.rs` pinta todos los componentes con
`AXFTERM_GALLERY_FONT`.

## Contraste y estados

- Los built-in se validan en tests con contraste WCAG AA (≥ 4.5:1 texto/fondo y
  texto/acento; ≥ 3:1 para el anillo de foco).
- El estado nunca se comunica solo por color: cada píldora lleva etiqueta e
  icono distintos (`En ejecución`, `Trabajando`, `Atención`, `Inactivo`,
  `Detenido`).
- El foco visible usa el token `focus` en pestañas, paneles y controles; los
  divisores y el borde del panel enfocado lo heredan.
- `crates/ui/examples/gallery.rs` permite revisar la UI a dos densidades
  (`AXFTERM_GALLERY_FONT=12` y `16`) y con los seis temas.
