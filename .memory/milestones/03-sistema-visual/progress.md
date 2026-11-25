# Progreso — M3 Sistema visual

## 2026-11-24 — Axfterm Studio y galería automatizada

- Se añade `scripts/studio.ps1` (feature `studio`): navega superficies,
  reproduce escenas y captura PNGs con `Window::render_to_image` en un perfil
  aislado. La galería del sistema de diseño vive en `crates/ui/src/gallery/` y
  cubre tokens, ANSI, tipografía, primitivas, componentes de gpui-kit, iconos y
  composiciones. `pwsh scripts/studio.ps1 -All` genera 23 capturas y un
  manifiesto en `target/studio`. Ver `docs/studio.md`.

## 2026-11-23 — Contraste de selección en curso

- La selección de One Dark y Cobalt2 es menos opaca; sus acentos e identidad
  ANSI se conservan. Se comprueba el contraste del texto seleccionado sobre
  las superficies de los seis temas incluidos.
- La revisión visual de los cambios en claro, oscuro y temas externos sigue
  pendiente; el esquema de temas personalizados no cambia.

## 2026-11-22 — Sin barra de estado

- Se retira la barra inferior. Duplicaba información que ya está en la
  superficie: proyecto y terminal en la barra de título y en la sidebar, el tema
  en el botón de ajustes, y los avisos en el toast flotante.
- El acceso al modal de tema y fuentes queda en el botón de la barra de título y
  en `Ctrl+Alt+T`; desaparece el chip `tema · variante` de la barra retirada.
- Se elimina `render_status_bar` y la métrica `status_height`; el área central
  gana la altura que ocupaba la barra.

## 2026-11-22 — Nombre manual de pestaña

- `tab_title` resuelve primero `title` cuando `custom_title` está activo; antes
  el nombre manual se guardaba pero el `auto_title` del perfil lo tapaba en la
  pestaña, la sidebar y la barra superior. El orden queda: nombre manual,
  título OSC útil, nombre del perfil, título persistido y proyecto.
- El editor inline se distingue con borde de foco y texto normal, y Escape
  cancela devolviendo el foco a la terminal.
- Perder el foco confirma el nombre en vez de descartarlo. Si el texto no cambió
  respecto al original, no se toca el modelo ni se congela un título automático.
  Enter confirma y devuelve el foco; el blur respeta el destino del clic.

## 2026-11-22 — Barra de título propia

- La ventana usa chrome propio: barra superior con icono provisional, `AxfTerm`,
  toggle de la sidebar al borde de su columna y controles de ventana nativos
  (minimizar, maximizar/restaurar y cerrar) a la derecha en Windows.
- La barra mide 32 px lógicos de alto y 46 px por control, como el SO, y no
  depende de `ui.font_size`; el texto (12 px) y los iconos del chrome (10–16 px)
  también tienen medidas fijas. El resto de la UI sigue escalando con la fuente.
- El toggle de la sidebar mide 24 px, no se mueve al ocultar la sidebar y
  conserva `Ctrl+B`, tooltip y la acción persistida.
- A su izquierda vive el botón de ajustes (24 px, icono de engranaje). Hasta M7
  abre el modal de tema y fuentes con `Ctrl+Alt+T`; no invade la separación con
  el área central porque queda dentro de la columna de la sidebar.
- El centro de la barra muestra `{proyecto}/{terminal}` de la pestaña enfocada,
  en una línea y a 12 px. Se centra en la ventana compensando la diferencia
  entre sidebar y controles, tiene un tope de 480 px lógicos y termina en `…`
  cuando no cabe. Sin pestaña enfocada no muestra nada.
- Arrastre, doble clic para maximizar/restaurar, snap layouts y minimizar siguen
  siendo nativos (`WindowControlArea`). El cierre pasa por
  `on_window_should_close` → `request_quit`, así que guarda historial y sesión.
- Provisional hasta M9: la marca es el glifo de terminal en acento; el `.ico`
  del ejecutable y su extracción quedan en M9.

## 2026-11-22 — Pestañas contiguas y nombres de shell

- Los chips de pestaña van pegados: se elimina el hueco entre ellos y se
  conserva el padding lateral de la strip respecto a flechas y acciones.
- El indicador de inserción se dibuja dentro del propio chip (borde izquierdo,
  o derecho del último) y deja de formar parte del contenido scrollable: ya no
  inventa overflow, así que las flechas solo aparecen cuando las pestañas no
  caben de verdad. `auto_scroll_tab_drag` tampoco toca el offset sin overflow.
- El índice de inserción usa la geometría uniforme real de los chips
  (`clamp(strip / n, mínimo, objetivo)`) y su punto medio, en vez de asumir que
  siempre llenan la strip.
- El título por defecto es el nombre del shell (`PowerShell`, `Windows
  PowerShell`, `Símbolo del sistema`, `Bash`...). Los títulos OSC que solo
  anuncian la ruta del ejecutable se descartan.
- Cada pestaña muestra su nombre completo en un tooltip, útil cuando el título
  se trunca.
- El auto-scroll del drag se ejecuta una sola vez por movimiento. GPUI entrega
  `on_drag_move` a todos los elementos que lo registran, así que registrar uno
  por pestaña multiplicaba la velocidad del scroll con el número de pestañas.
- El índice de drop usa `ScrollHandle::bounds()` (viewport estable) en lugar de
  un `on_prepaint` que se desplazaba con el scroll: antes el puntero podía salir
  del área y el índice se desviaba más cuanto más se había desplazado la strip.
- Al soltar, el índice se recalcula con la posición real del puntero en vez de
  confiar en el último estado del drag.
- La copia que sigue al cursor durante el drag es translúcida, para que el
  destino se intuya debajo.

## 2026-11-22 — Refinamiento de pestañas, iconos y foco

- Las pestañas son chips con ancho objetivo fijo (`16 ×` fuente) que se
  comprimen uniformemente hasta un mínimo (`9 ×` fuente); al llegar al mínimo,
  la tab strip hace scroll horizontal con flechas clicables. Las flechas
  repintan la strip y no roban el foco del panel al pulsarlas.
- La pestaña activa usa `surface_raised` y esquinas redondeadas; se elimina el
  subrayado de acento y el icono de play de las terminales en ejecución.
- El icono de la pestaña refleja el shell del PTY: `pwsh`/`powershell` usan
  `brand-powershell` (Tabler) y el resto cae al icono genérico de terminal.
- El panel enfocado deja de dibujar borde (se conserva el grosor para no
  alterar el reflow del PTY); el borde del terminal pasa a ser responsabilidad
  del shell.
- Renombrar una pestaña selecciona el título anterior para sustituirlo al
  escribir.
- Se elimina el desplegable con todas las pestañas. `+` y el menú del panel
  quedan pegados a la última pestaña y, al desbordar, fijos en el borde derecho
  de la strip; las flechas siguen visibles para desplazar pestañas ocultas.
- Los iconos de la barra crecen y salen de `UiMetrics`: icono del shell, `+`,
  flechas y caret del menú a `1 ×` la fuente; cierre y estado a `0.875 ×`.
  `IconButton::icon_size` permite separar el icono del área pulsable.
- La strip respira: padding y separación `spacing_sm` respecto a los bordes y
  entre pestañas y acciones, sin tocar la geometría de compresión.
- Al arrastrar, el ghost es una copia real de la pestaña (icono, título, estado,
  cierre y borde de acento) que sigue al cursor.
- El destino del drop se explica solo: dentro de la strip aparece una barra de
  inserción entre chips; sobre un panel, el centro resalta el cuerpo entero
  (mover) y los bordes resaltan la mitad donde nacerá el panel (dividir).
- Soltar en un borde crea el panel a izquierda/derecha/arriba/abajo según la
  zona, mueve la pestaña sin recrear el PTY y sale del zoom temporal para que el
  split sea visible. Si la pestaña era la única de su panel, el origen vacío
  colapsa como manda ADR 0003.
- `PaneTree::split_with_side` y `Workspace::split_pane_with_tab` concentran la
  operación en el modelo; la UI solo decide eje y lado.
- El índice de inserción se calcula con la geometría de la strip y el offset de
  scroll, y el drop dentro del mismo panel descuenta la posición que deja libre
  la pestaña al extraerse: la barra y el resultado siempre coinciden.

## 2026-11-21 — M3 cerrado

Sistema visual propio en `axf_ui`: tokens semánticos, métricas derivadas de la
fuente, temas TOML built-in y de usuario, fuentes separadas UI/terminal,
iconos vendorizados y componentes. La app aplica todo en caliente sin reiniciar
ni recrear PTYs.

### Tokens y métricas (`axf_ui`)

- `UiMetrics` parte de `font_size × scale` y deriva espaciados, alturas de
  control/tab/fila/estado, iconos, radios, divisor, padding y pasos de scroll.
  El DPI del monitor redondea hairlines a la rejilla física.
- Tokens semánticos (`UiTokens`): background, surface, surface_raised, border,
  border_strong, text, muted, subtle, accent/accent_text, danger, warning,
  success, info, selection, focus y overlay. `UiTokens::placeholder` cubre
  tests y arranque sin tema.
- `theme::state` es el estado global (`Theme`, métricas, `FontSet`, tamaños de
  UI/terminal y altura de línea). `theme::install` proyecta el tema a gpui-kit
  (input, menú, diálogo, scrollbar, listas, tablas) y repinta.
- Variantes clara/oscura y detección de la apariencia del sistema
  (`WindowAppearance`), con pareja por familia, seguimiento en vivo y fallback
  documentado.

### Temas

- Esquema TOML en `axf_ui::theme`: `[meta]` (name, family, author, variant),
  `[ui]` (tokens con derivación de opcionales) y `[terminal]` (fg/bg, cursor,
  selección, dim y 16 ANSI). Colores `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`.
- Built-in embebidos en `crates/ui/assets/themes/`: One Dark/Light,
  Monokai/Light y Cobalt2/Light. Los tests garantizan que parsean, que cada
  familia tiene ambas variantes y que texto/muted/acento/foco pasan AA.
- Loader con `deny_unknown_fields`, validación por campo, línea y columna, y
  errores que no impiden cargar el resto de temas. `load_dir` ignora temporales
  y archivos ocultos.
- Hot reload de `config.toml` y `themes/` con `notify`, debounce de 250 ms y
  canal acotado. Un error de config conserva la última válida; un cambio de
  tema/fuente/tamaño reaplica métricas y refresca las terminales vivas.
- Importar (valida antes de copiar), exportar el tema activo y abrir `themes/`
  con el sistema. El modal de tema lista el catálogo, variante, fuentes, rutas
  y permite previsualizar la variante opuesta.

### Fuentes

- `[ui]` y `[terminal]` tienen familia y tamaño independientes; `[fonts].sources`
  acepta archivos o directorios (16 rutas, 16 MiB/archivo, 64 MiB/carga).
- `add_fonts` registra `.ttf/.otf/.ttc/.otc`; se informa de archivos ausentes o
  demasiado grandes. La familia pedida se usa solo si está instalada; si no, se
  avisa y se aplica la cadena recomendada.
- Fallbacks explícitos para Nerd Font, emoji y CJK (Segoe UI Emoji/Symbol,
  Noto, Microsoft YaHei, Malgun Gothic, Symbols Nerd Font). El terminal recibe
  la cadena en `Font`; la UI se apoya en el stack de GPUI.
- `terminal.line_height` controla la altura de celda (1.0 = natural). La
  terminal recalcula métricas, limpia cachés de shaping y reflowea al cambiar.
- Ligaduras: descartadas en v0.1 (ADR 0005). El shaping por celda no garantiza
  avances estables; queda documentado en `docs/themes.md`.

### Iconos y assets

- Tabler vendorizado (MIT): 38 iconos outline en
  `crates/ui/assets/icons/tabler/`, tabla única en `icons::ALL` y macro de
  `include_bytes!`. `scripts/update-icons.ps1` descarga y actualiza la lista.
- simple-icons (CC0): Rust, OpenCode y GitHub Copilot. `brand::tint` usa el
  color oficial o el token de texto para marcas monocromas.
- Componente `Icon` con tamaños relativos (`Xs..Xl`) y tint por token.
  `Assets` sirve primero los SVG propios y hace fallback a gpui-kit; los tests
  comparan `icons::ALL` con la tabla embebida.

### Componentes (`axf_ui::components`)

- `Button` (filled/outline/ghost, tonos y tamaños), `IconButton` con tooltip
  nativo, `Tab` con icono/estado/cierre, `StatusPill` y `Pill`,
  `EmptyState`, `TextField`, `Toggle`, `Menu`, `Modal`, `Toast` y `Tooltip`.
- El estado nunca es solo color: cada `TabStatus` tiene etiqueta e icono
  distintos y un test garantiza que no se repiten.
- Scrollbar, menús contextuales, input multilínea y diálogos de confirmación
  siguen viniendo de gpui-kit, pero con los tokens del tema propio.

### Integración en la app

- `main` carga config, fuentes y tema antes de abrir la ventana; `theme_host`
  mantiene catálogo, selección, watcher y revisiones de estilo.
- La sidebar usa el `EmptyState` compacto y botones/icon-buttons propios; la
  tab strip usa `Tab` y conserva drag & drop, overflow, menú y renombrado.
- Acciones nuevas: `Ctrl+Alt+T` (tema y fuentes), `Ctrl+Alt+V` (variante),
  `Ctrl+Alt+N/P` (ciclo), `Ctrl+Alt+Shift+T` (recarga), `Ctrl+Alt+O`
  (carpeta), `Ctrl+Alt+I/E` (importar/exportar).
- Chip de tema en la barra de estado, toast de avisos y modal de tema.
- Un cambio de tema/fuente/DPI actualiza `TerminalStyle` y llama a
  `TerminalView::update_style`: recalcula métricas, limpia cachés, fuerza
  relayout y resize del PTY, sin recrear el proceso.

### Verificación

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace` y `cargo build --release` en verde.
- Tests: 12 config (incluido watcher y recuperación), 50 terminal, 17 fixtures
  VT, 32 workspace, 34 UI (contraste, temas, assets, métricas, fuentes y
  estados) y 1 del shell.
- Verificación visual real con capturas:
  - One Dark 14 px y terminal real.
  - Hot reload One Dark 14 → Monokai 17.6/15 px: fondo, acento, tipografías,
    filas y terminal repintados y refloweados; toast de “Configuración
    recargada”.
  - One Light a 12 px (sistema claro fuerzado con `appearance = "light"`).
  - Tema de usuario `Mi Tema` en `themes/`: carga, valida y aplica (fondo
    violeta, acento rosa, chip “Mi Tema · Oscuro”).
  - Modal de tema con catálogo, toggle, fuentes, rutas y acciones; cierra con
    `Esc`, el botón o el velo.
  - Galería de componentes a 12 px: botones, iconos, tab strip, estados,
    input, toggle, menú, empty state y marcas.

### Notas y decisiones

- `notify` 9 exige Rust 1.91 y rompe la MSRV 1.85; se fija `notify = 8.2.1`,
  la misma versión que ya usa gpui-kit, y el lock fija `notify-types` 2.1.0
  (2.3.x también exige 1.91). `AGENTS.md` actualizado.
- No se embebe un archivo Nerd Font: se detectan y usan las familias
  instaladas; el empaquetado de fuentes queda para M9. El fallback y
  `add_fonts` están cubiertos.
- El parser conserva la paleta con la que arrancó para responder `OSC 10/11/12`;
  el repintado usa la paleta nueva. Reiniciar la pestaña sincroniza ambas.
- El modal de tema es una superficie de descubrimiento, no una GUI de ajustes:
  la selección/preview de sesión no reescribe `config.toml`.

### Pendiente fuera de M3

- Pasada manual de teclado real con las nuevas acciones (se solapa con M6).
- Empaquetar una Nerd Font y validar CJK/emoji en release (M9).
- Detección real de agentes para poblar `TabStatus` en la UI (M5).
