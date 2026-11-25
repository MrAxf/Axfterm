# Auditoría de experiencia — M10

## Evidencia

- Referencia visual: las 34 escenas originales de Studio.
- Capturas limpias tras aislar el fixture: `target/studio/shots/app-*.png`.
- Evidencia del affordance anterior de búsqueda: captura previa
  `target/studio/shots/sidebar-terminals.png`.
- Escena añadida para revisar el cambio en una ventana compacta, escala 1.25 y
  tema de usuario: `app.sidebar.terminals.compact`.

## Hallazgos priorizados

| Prioridad | Escena / estado | Problema observado | Impacto / frecuencia | Decisión |
| --- | --- | --- | --- | --- |
| Media | `app.sidebar.terminals`, workspace poblado | La búsqueda parecía un campo de texto, aunque solo abre la paleta. La descripción larga se truncaba y el atajo no era visible. | Afecta una acción habitual para encontrar pestañas, paneles y proyectos. | Cambiarla a un botón compacto «Buscar…», mostrar `Ctrl+P` y conservar el texto explicativo en el tooltip. |
| Media | Capturas del workspace con PowerShell | El perfil del usuario podía emitir errores de inicialización dentro del fixture, haciendo que las capturas no fueran una referencia fiable. | Afecta toda reproducción de Studio que arranca PowerShell; no cambia el perfil normal. | Añadir `-NoProfile` solo al `SpawnSpec` del fixture `demo` y cubrir la política con test. |
| Media | Controles propios `Button` / `IconButton` | Los elementos interactivos basados en `div` no declaraban rol ni nombre accesible, por lo que podían quedar fuera del árbol de accesibilidad de GPUI. | Afecta controles de app y galería; dificulta navegar con tecnología asistiva. | Declarar `Role::Button`, usar etiqueta visible o tooltip como nombre, anunciar descripciones y atajos cuando existan. |
| Baja | `app.onboarding` | El estado inicial identifica que no hay proyectos, explica su función y ofrece «Abrir carpeta» como acción primaria. | Primer uso; jerarquía y siguiente paso ya explícitos. | Sin cambio para evitar duplicar acciones. |
| Baja | `app.sidebar.agents.empty` | El vacío explica dónde aparecerán opencode y Copilot; «Nueva terminal» permanece disponible en la cabecera de la sidebar. | Solo mientras no haya sesiones de agente. | Sin CTA duplicada; conservar el estado informativo. |
| Baja | `app.workspace`, `app.workspace.agents`, `app.workspace.cobalt2`, `app.workspace.monokai`, `app.workspace.one-light` | La sidebar, pestañas, paneles y estado de agente mantienen una jerarquía consistente en varios temas; la atención tiene icono y texto además del color. | Uso continuo del workspace. | Sin cambio estructural; respetar la clasificación de proyectos en la sidebar y no añadir breadcrumb central. |
| Baja | `app.palette.commands`, `app.palette.search`, `app.shortcuts` | La paleta distingue comandos y búsqueda, muestra filtros y ofrece indicaciones de teclado. | Acciones de navegación y búsqueda. | Sin cambio: no se observó ambigüedad que justifique alterar el flujo. |
| Baja | `app.settings.*` | Las siete secciones comparten navegación, título y pie de acciones; Apariencia enseña previews de temas y variante. | Edición ocasional de configuración. | Revisado el 2026-11-24: ver la iteración de reorganización del panel. |

## Dirección visual resultante

- Mantener el tema activo y sus tokens como fuente de color; no añadir colores
  literales ni un estilo global que neutralice temas personalizados.
- Conservar la densidad y estructura del workspace; aclarar la intención de una
  acción con su etiqueta y atajo antes de cambiar layout o componentes.
- Reutilizar `Button` y métricas existentes para búsqueda; no añadir tamaños,
  espaciados ni estados visuales paralelos.
- Exponer los roles y nombres accesibles en los botones propios; no cambiar el
  mapa de acciones para dar soporte a tecnología asistiva.
- Evitar duplicar la creación de terminales en el vacío de agentes mientras la
  acción existente siga visible en la cabecera.

## Iteración 2026-11-24 — reorganización del panel

| Prioridad | Escena / estado | Problema observado | Impacto / frecuencia | Decisión |
| --- | --- | --- | --- | --- |
| Media | `app.settings.profiles`, `app.settings.shortcuts` | Las acciones del perfil seleccionado quedaban bajo una lista larga; las filas de atajos parecían editables al pulsarlas. | Afecta a la edición frecuente de perfiles y atajos. | Mover las acciones a cada fila, abrir nuevo/editar en modal y capturar el atajo solo tras pulsar Editar. |
| Media | `app.settings.terminal`, `app.settings.fonts`, `app.settings.advanced` | Tres secciones con poco contenido: la tipografía estaba separada y las fuentes del sistema no necesitan orígenes propios; lo avanzado era una fila de botones. | Alarga la navegación y duplica controles. | Integrar tipografía en Apariencia y mover las acciones de archivo/restauración al menú «Más» del pie; las fuentes adicionales quedan solo en `config.toml`. |
| Media | `app.settings.appearance` | El catálogo de temas ocupaba todo el alto y ocultaba la tipografía y las métricas. | La configuración visual requería scroll. | Compactar las tarjetas de tema y usar sliders para tamaños y escala. |
| Baja | Modal de ajustes | La cabecera mostraba `config.toml · esquema v2` y el pie alineaba las acciones a la izquierda. | Ruido técnico y jerarquía difusa. | Quitar el texto de esquema y alinear estado a la izquierda y acciones a la derecha. |

## Iteración 2026-11-25 — eventos de ratón y scroll

| Prioridad | Escena / estado | Problema observado | Impacto / frecuencia | Decisión |
| --- | --- | --- | --- | --- |
| Alta | `app.palette.*`, `app.settings.*` | La barra de scroll colgaba del div desplazable y subía con el contenido. | La barra desaparecía del borde y no se podía arrastrar con contenido largo. | Moverla a un wrapper relativo fuera del área de scroll. |
| Alta | Paleta, ajustes, atajos y modales | La terminal de debajo recibía rueda, movimiento, pulsación y suelta porque el overlay no occluía el hit-test. | Con mouse tracking activo, la terminal reaccionaba a interacciones de la capa superior. | `occlude()` en la raíz de la paleta y en el velo de `Modal`. |
| Media | `app.palette.commands`, `app.palette.search` | Las filas quedaban justas de padding horizontal (≈7 px) y el carril de la scrollbar pisaba el contenido. | Lectura menos cómoda de la lista de opciones. | `palette_row_padding_x()` = `spacing_md`, sin cambiar la altura vertical de la fila. |
| Media | `app.settings.appearance` | Las tarjetas de ajustes usaban 7 px de padding interior y la muestra de tipografía llegaba al borde o se recortaba en columnas estrechas. | Contenido pegado al borde en todas las secciones de ajustes. | Padding `spacing_md` en las tarjetas y muestra monoespaciada limitada al ancho, con envoltura. |

