# Changelog

Todas las fechas usan el calendario local del proyecto. El formato sigue
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el versionado
semántico.

## [0.3.0] — 2026-11-26

### Cambiado

- Escala única de controles en `rem`: `Xs` 1,25, `Sm` 1,5 (por defecto) y `Md`
  2,0. Botones, campos y selectores propios y de gpui-kit comparten altura,
  texto, icono y padding.
- La bienvenida y las acciones principales de la sidebar usan `Md`; formularios,
  paleta, WSL y steppers usan `Sm`.
- Los botones con acción + menú se agrupan en una sola silueta con divisoria y
  estado abierto.
- Un único `SegmentedControl` sustituye el segmentado propio de la sidebar, los
  toggles de la paleta y los radios de Apariencia.
- El switch usa `SwitchField` con geometría en `rem` y estados propios.
- La sidebar proyecta su ancho con el tamaño de fuente activo y deja de truncar
  «Nueva terminal» al subir la escala.
- La bienvenida con proyecto sin terminales usa dos botones de kit `Md` en la
  misma fila y con la misma altura.

### Corregido

- Hover y pulsado visibles en botones primarios, secundarios, ghost y menús
  desplegables, incluidos los que viven sobre superficies elevadas.
- El cursor de mano cubre todos los controles accionables; menús, campos y
  chrome de ventana conservan la flecha nativa y el divisor usa cursor de
  redimensionado.
- Las filas, pestañas, tarjetas, rail y steppers usan `surface_hover` y
  `surface_active` en lugar de opacidades de `surface_raised`.
- La acción «Nueva terminal» de la sidebar se alinea con «Buscar…» en lugar de
  centrar su icono y su texto.
- Studio reinicia la escala por escena y el manifiesto registra `ui_scale`, para
  que una captura normal no herede la escala 1,25 de otra.
- Los switches apagados usan una pista neutra y los encendidos el acento; ya no
  comparten fondo.
- El foco de botones e icon-buttons ya no cambia su geometría.
- Los campos y botones de una misma fila miden lo mismo.

## [0.2.0] — 2026-11-25

### Añadido

- Accesos rápidos a agentes: el pie de la sidebar y el desplegable del «+» de la
  pestaña abren OpenCode o Copilot en una pestaña nueva del proyecto actual,
  con `Ctrl+Alt+1` / `Ctrl+Alt+2`, paleta y `[keymap]`. El perfil preferido por
  agente se consume por fin, validado contra el host del proyecto.
- El pie de la sidebar usa icon-buttons de agente —preparados para crecer— y
  añade una línea con la versión y el resumen de proyectos, terminales y
  agentes activos.
- Al restaurar una pestaña se reanuda la sesión concreta del agente reportada
  por OSC 7501 (`opencode --session <id>`, `copilot --resume=<id>`); dos
  pestañas del mismo proyecto ya no abren la misma conversación. Sin id se
  mantiene `--continue`.

### Corregido

- El empaquetado ya no puede publicar un release sin el icono de la app: una
  build release falla si `rc.exe` no puede embeber el icono y los metadatos,
  `scripts/package.ps1` verifica que el `.exe` contiene `axfterm.ico` y el
  `.ico` viaja en el zip y se instala junto al ejecutable en el MSI.
- Los avisos in-app dejan de ser transparentes sobre el terminal: usan la
  superficie elevada del tema y una sombra de overlay.
- Ajustes › Atajos vuelve a tener una sola región de scroll: la rueda y la
  scrollbar recorren la misma lista y llegan hasta el final.
- El anillo de foco del buscador de Atajos ya no se recorta en el borde superior
  del área desplazable.

## [0.1.1] — 2026-11-25

### Añadido

- Comando de agente por perfil (`[profiles.agent_commands]`): OpenCode puede ser
  `opencode` en PowerShell y `opencode2` en WSL, también al reanudar.
- Shift+Enter inserta una nueva línea (LF) en agentes como OpenCode y Claude
  Code; Enter sigue enviando el prompt.
- Arrastrar archivos sobre una terminal inserta sus rutas citadas para el shell
  del perfil (Windows, Git Bash o WSL), sin ejecutarlas.
- Durante un drag de archivos, mantener el cursor sobre una pestaña o una fila
  de la sidebar activa esa terminal; los proyectos plegados se expanden.

### Cambiado

- OpenCode aparece como «OpenCode» en sidebar, ajustes y textos de la app; las
  claves estables de config y sesión siguen en minúsculas.

### Corregido

- OpenCode 2 no emite `done`: el paso de `working` a `idle`/`clear` se
  interpreta como terminado, de modo que el aviso, el chip y la marca del
  título aparecen al acabar una tarea.
- Al restaurar el historial de una terminal se fuerza el cursor visible y el
  final del scrollback: un programa que lo dejó oculto (por ejemplo,
  `docker compose` v5 con logs en streaming) ya no deja el prompt nuevo sin
  cursor.
- El PTY de una pestaña restaurada arranca con el tamaño de su instantánea y el
  primer frame lo ajusta a la ventana. Una pestaña WSL ya no se queda esperando
  una respuesta de cursor fuera de su vista y muestra el prompt debajo del
  historial.

## [0.1.0] — 2026-11-25

Primera release para uso diario en Windows x64.

### Añadido

- Workspace con proyectos, paneles divisibles, pestañas arrastrables, zoom y
  restauración de sesión versionada.
- Terminal real con ConPTY, motor VT, colores, selección, copiar/pegar, resize
  con reflow y scrollback persistente por pestaña.
- Temas built-in y de usuario, fuentes de UI y terminal independientes, iconos
  de perfil y extracción del icono del ejecutable.
- Perfiles de terminal con detección local, comando tokenizado y errores
  accionables.
- Detección local de opencode y GitHub Copilot con OSC 7501 y árbol de procesos.
- Atajos configurables, paleta de comandos y búsqueda, y pantalla de ayuda.
- Panel de ajustes con borrador, validación, preview y escritura que conserva
  comentarios.
- Proyectos en WSL, con host y perfil por proyecto.
- Scrollback configurable (0–50 000 líneas) con tope de 32 MiB por terminal.
- Instancia única: una segunda instancia reenvía la carpeta y sale.
- Apertura de carpetas desde CLI y desde el menú contextual de Explorer.
- Icono y metadatos de Windows embebidos en el ejecutable.
- Logs opt-in con rotación y crash reports locales acotados.
- MSI per-user y zip portable reproducibles, con hashes SHA-256.

### Cambiado

- La barra de título usa la marca de Axfterm en lugar del glifo provisional.
- `session.json` sube a versión 3 (host y perfil por proyecto); las versiones 1
  y 2 se migran al cargar.

### Seguridad y privacidad

- Sin telemetría ni red por defecto.
- Los logs solo se escriben con `AXFTERM_LOG`; los crash reports se quedan en
  `<config>/logs` y se limitan a 10 archivos.
- El scrollback y la sesión no se cifran ni se sincronizan; viven en el perfil
  del usuario.

### Pendiente de validación manual

- Matriz Windows 10/11 y DPI 100/150/200.
- Teclados ES/CJK e IME chino/japonés.
- Gate de un día completo de uso real.
- Firma de código y auto-update: fuera de v0.1.
