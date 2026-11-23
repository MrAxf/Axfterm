# Progreso — M6 Teclado y paleta

## 2026-11-23 — Registro, keymap y paleta

- `actions.rs` centraliza `ActionMeta` (id, etiqueta, categoría, contexto,
  chord por defecto, keywords) y une cada id a su acción GPUI. La paleta, la
  ayuda y los keybindings leen de ahí; `keys.rs` sigue siendo la fuente de los
  chords por defecto.
- `[keymap]` en `config.toml`: `chord = "accion"`, subsecciones
  `workspace`/`sidebar`/`terminal` y `unbind = ["id"]`. Se validan ids
  desconocidos, chords inválidos, duplicados por contexto, `settings`
  reservado y las teclas de terminal (`Ctrl+C/D/Z`, `Tab`, `Esc`, flechas).
  Los atajos sin modificador o que pisan a otra acción se explican; la recarga
  en caliente vuelve a instalar defaults + cambios y los `unbind` como
  `NoAction`.
- Paleta (`Ctrl+Shift+K`): fuzzy con plegado de acentos, ranking estable,
  resaltado, recientes/frecuencia y navegación `↑↓`/`Tab`/`Enter`/`Esc`.
  Fuentes: acciones, pestañas, paneles, proyectos (revela la fila en la
  sidebar), perfiles (nueva pestaña con su icono), agentes en atención, temas
  y fuentes de UI/terminal (preview en memoria).
- Pantalla de atajos (`Ctrl+Alt+K`) generada desde el registro; salto al
  siguiente agente en atención (`Ctrl+Alt+J`). `docs/keymap.md` se genera desde
  el registro y un test impide que se desincronice.
- Tests: fuzzy ES/acentos y ranking, conflictos/unbind/contextos/chords
  inválidos, teclas del PTY intactas, salto cíclico entre agentes en atención
  y documentación generada.

Los menús contextuales conservan su composición específica con los iconos de
perfil; la paleta cubre el acceso keyboard-first.

## 2026-11-23 — Ajuste visual y navegación de la paleta

- La selección sigue a las flechas aunque el foco esté en el `Input`: los
  bindings se registran también con `Palette > Input`, más profundo que el
  `MoveUp/MoveDown` de gpui-kit.
- `ScrollHandle` en la paleta: `next`, `previous` y cada filtrado hacen
  `scroll_to_item`; la lista se desplaza con la selección.
- Filas más legibles: texto principal y subtítulo mayores, altura con padding
  vertical y separación entre filas; título/subtítulo comparten un bloque
  compacto.
- Scroll nativo (`overflow_y_scroll`) sobre el `ScrollHandle` propio: el
  wrapper `overflow_y_scrollbar` creaba su propio handle y el teclado no podía
  desplazarlo. Las filas llevan `flex_shrink_0`; sin él, el flex column las
  comprimía hasta perder el padding y no había overflow que desplazar.
- Enter ejecuta el comando con `Window::dispatch_action`, que encola el
  despacho; `App::dispatch_action` reentraba en la ventana en curso y se perdía.

## 2026-11-23 — Separación de comandos y búsqueda

- La paleta única se parte en dos superficies con el mismo estado, navegación,
  scroll y recientes:
  - **Comandos** (`Ctrl+Shift+K`): solo `ActionMeta` del registro.
  - **Búsqueda** (`Ctrl+P`): proyectos, paneles, pestañas, perfiles y agentes
    en atención. El buscador de la sidebar abre esta paleta.
- `ActionMeta.palette` separa «visible en comandos» de `enabled` (atajos y
  ayuda). Se ocultan de la paleta, sin borrar ids de `[keymap]`, los comandos
  que ya cubren la búsqueda (`tab.next/previous`, `pane.focus_*`) o el modal de
  tema (`theme.toggle_variant/next/previous/reload/open_folder/import/export`),
  además de las propias paletas.
- Temas y fuentes salen de la paleta: el modal «Tema y fuentes» gana dos
  `Select` (UI y terminal) con «Sistema» y preview en memoria. Hasta M7 no se
  escribe `config.toml`.
- `Ctrl+P` pisa el historial de PSReadLine en la terminal; es una decisión
  explícita y se puede cambiar en `[keymap]`.
- Tests nuevos: cada acción registrada construye su acción GPUI, los comandos
  ocultos siguen existiendo, los modos filtran sus tipos y la lista de fuentes
  preselecciona la actual. `docs/keymap.md` incluye `palette.search`.

## 2026-11-23 — Modo y filtro dentro de la paleta

- La paleta ya no se cierra para cambiar de superficie: un `ToggleGroup`
  segmentado `Comandos | Búsqueda` en la cabecera alterna con `Ctrl+Tab` /
  `Ctrl+Shift+Tab` sin perder la consulta ni el filtro.
- En Búsqueda aparece una segunda fila de segmentos `Todo | Pestañas | Paneles |
  Proyectos | Perfiles | Agentes`, clicable y con atajos `Ctrl+1–6`. El filtro
  se combina con el texto y solo se aplica en este modo.
- Al cambiar de modo se vuelven a recolectar las entradas para reflejar
  pestañas o proyectos nuevos. La selección se conserva por id si la entrada
  sigue visible; si no, pasa a la primera fila.
- Tras pulsar un segmento el foco vuelve al input. El pie muestra `Ctrl+Tab` y,
  en Búsqueda, `Ctrl+1–6`.
- Tests nuevos: orden y etiquetas de los filtros, coincidencia por tipo y texto
  vacío con filtro activo.

## 2026-11-23 — Pasada de UI de la paleta

- El input ocupa todo el ancho, con icono de búsqueda, botón de limpiar y
  `aria_label`. Modo y filtro bajan a una fila secundaria: modo a la izquierda
  y filtro a la derecha, con tooltips que muestran sus atajos.
- Ancho de `46em` y alto de lista calculado desde el viewport (con reserva extra
  en Búsqueda por si los filtros envuelven); deja de desbordarse a 200 %.
- Fondo del overlay con `ui.overlay`; sin colores literales.
- Filas: carril fijo de icono, iconos `Md`, título `1em`, subtítulo `0.8125em`,
  etiqueta de tipo en carril fijo, selección con `ui.selection` y hover con
  `surface_raised`. Se quita el cursor de enlace.
- Estado vacío compacto con icono, título y pista contextual; sin acciones
  clicables para no romper el foco del input.
- Pie en dos grupos: navegación a la izquierda y atajos de superficie a la
  derecha.
- Métricas nuevas en `UiMetrics` (`palette_*`), `EmptyState::compact` y
  `TextField` con icono prefijo y etiqueta accesible.

## 2026-11-23 — Cierre de M6

- Gate manual sin ratón validado: abrir terminales con varios perfiles, cambiar
  entre Comandos y Búsqueda, filtrar por tipo y ejecutar acciones con `Enter`.
- `check.ps1` (fmt, clippy y tests) y build `--release` en verde.
- Hito cerrado. Siguiente: M7 panel de ajustes.
