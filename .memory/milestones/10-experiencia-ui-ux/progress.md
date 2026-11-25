# Progreso — M10 Rediseño de experiencia UI/UX

## 2026-11-25 — Correcciones de uso con agentes

- Los toasts usan `surface_raised`, texto del tema y sombra media: dejan de
  transparentarse sobre el terminal.
- Ajustes › Atajos usa una sola región de scroll. La lista interna ya no consume
  la rueda mientras la scrollbar exterior se quedaba a medio camino.
- El contenido desplazable de Ajustes reserva respiro superior para que el
  anillo de foco del buscador no se recorte.
- La etiqueta visible de `AgentKind::OpenCode` pasa a «OpenCode»; también los
  textos de sidebar, ajustes, plantilla de config y README. Las claves estables
  (`opencode`, `app=opencode`, `[agents.preferred_profiles]`) no cambian.
- Shift+Enter envía LF (`Ctrl+J`) para insertar nueva línea en OpenCode y otras
  TUIs sin someter el prompt; Enter sigue enviando CR.
- Arrastrar archivos sobre una terminal inserta las rutas citadas para el shell
  del perfil (Windows, Git Bash o WSL con `/mnt/<unidad>`), sin ejecutarlas y
  sin convertirse en un clic dentro de la TUI.
- Al arrastrar archivos, mantener el cursor 500 ms sobre una pestaña o una fila
  de la sidebar activa esa terminal; los proyectos plegados se expanden. Cada
  handler comprueba sus bounds y el viewport visible, y los temporizadores
  viejos se invalidan por secuencia.
- OpenCode 2 no emite `done`: el paso de `working` a `idle`/`clear` se interpreta
  como `Finished` para que aparezcan el toast, el chip y la marca del título.
  Un `idle`/`clear` posterior conserva el aviso pendiente hasta enfocar.
- Tests nuevos en `axf_agents` y `axf_terminal`; `check.ps1` y build release en
  verde.

## 2026-11-24 — Milestone creado y baseline visual

- Se reprodujeron y revisaron las 34 escenas de `studio/scenes/`; las capturas
  están en `target/studio/shots/` y el índice en `target/studio/manifest.json`.
- La revisión abarca workspace vacío y poblado, estados de agentes, variantes
  claras/oscuras, las familias One/Monokai/Cobalt2, tema personalizado,
  apariencia compacta/ampliada, paletas, siete secciones de ajustes, atajos y
  galería del sistema de diseño.
- Algunas capturas del workspace incluyen errores de inicialización de
  PowerShell (`Import-Clixml` y `InvalidOperation`) procedentes del entorno de
  prueba; deben aislarse antes de usarlas para comparar cambios.
- No se identificó un fallo concreto del intento de rediseño anterior; el
  usuario aclaró que se originó en un prompt antiguo.
- Pendiente: limpiar el fixture, documentar/priorizar recorridos y comenzar el
  rediseño guiado por esos hallazgos. No se cambiaron superficies de producción.

## Fixture de Studio, auditoría y búsqueda de sidebar

- El fixture `demo` añade `-NoProfile` a PowerShell solo en ese spawn. Los
  perfiles normales y los demás ejecutables conservan sus argumentos; hay test
  de PowerShell, perfil explícito, cmd y spawn no aislado.
- Las 35 escenas se reprodujeron y capturaron; el manifiesto contiene 35 nombres
  únicos. Los registros de Studio no muestran errores de inicialización y el
  prompt de PowerShell aparece limpio.
- La búsqueda ahora se ve como botón «Buscar…» con `Ctrl+P`, en vez de un campo
  truncado que parecía editable. El tema activo conserva su acento y la escena
  compacta valida ventana 1024×720, escala 1.25 y tema personalizado.
- `Button` y `IconButton` exponen rol y nombre accesible; los botones de texto
  anuncian su descripción/atajo y los icon-buttons usan etiqueta explícita o
  tooltip. Las acciones de búsqueda siguen disponibles por `Ctrl+P`.
- `.memory/milestones/10-experiencia-ui-ux/audit.md` recoge evidencia,
  prioridades y las superficies que se dejan intactas porque ya comunican
  correctamente sus acciones y estados.
- Las capturas cubren One Dark, One Light, Agent light, temas Cobalt2/Monokai y
  tema personalizado. La escena compacta cubre texto, densidad y resize.
- Pasaron `scripts/check.ps1`, build release, `scripts/test-cargo-low.ps1` y
  clippy de `axfterm` con `studio`; también `git diff --check`.
- M10 queda implementado antes de M9 por solicitud explícita. No se hicieron
  cambios cosméticos en superficies que ya comunican bien su tarea.

## 2026-11-24 — Reorganización del panel de ajustes

- El panel pasa a cuatro secciones: Apariencia, Perfiles, Atajos y Agentes
  (ADR 0010). Terminal, Fuentes y Avanzado desaparecen.
- Apariencia reúne tema, tipografía de la app y del terminal, tamaños, altura de
  línea y escala con sliders; los temas usan tarjetas compactas para no empujar
  la tipografía fuera de vista.
- Los selectores de fuente usan solo familias instaladas, permiten buscar y
  muestran la familia activa o la de respaldo. `[fonts].sources` se conserva y
  restaurar defaults ya no lo borra.
- Perfiles: acciones dentro de cada fila y alta/edición en modal; un perfil
  nuevo solo entra al aplicar. Atajos: lista por categorías y captura inline al
  pulsar Editar. Agentes: tarjetas con Select de perfil preferido.
- Avanzado se mueve al menú «Más» del pie; el modal deja de mostrar la versión de
  esquema y alinea las acciones a la derecha.
- Escenas actualizadas: se retiran terminal/fuentes/avanzado y se añaden
  `app.settings.profiles.editor` y `app.settings.shortcuts.capture`.

## 2026-11-25 — Scroll, overlays y padding de la paleta

- Las scrollbars de la paleta y de Ajustes vivían dentro del div con
  `overflow_y_scroll`, así que el offset las arrastraba con el contenido. Ahora
  cuelgan de un wrapper relativo, igual que en `overflow_y_scrollbar` de
  gpui-kit, y la lista conserva `track_scroll` para la navegación por teclado.
- La raíz de la paleta y el velo de `Modal` llaman a `occlude()`. Antes la
  terminal seguía en el hit-test y recibía rueda, movimiento (mouse reporting),
  pulsación y suelta a través de paleta, ajustes, atajos y diálogos.
- Las filas de la paleta usan `palette_row_padding_x()` (`spacing_md`): más aire
  horizontal y sin que el carril de la scrollbar pise el texto.
- Las tarjetas de ajustes (temas, tipografía, perfiles, atajos y agentes) pasan
  de `spacing_sm` a `spacing_md` de padding interior. La muestra monoespaciada de
  tipografía se limita al ancho de la tarjeta y envuelve en vez de tocar el borde
  o quedar recortada.
- Verificado con `scripts/check.ps1`, `cargo-low.ps1 build --release`,
  `test-cargo-low.ps1` y clippy con `--features studio`. Gate manual pendiente:
  reiniciar la app (la instancia en curso ocupa el mutex de Studio) y probar
  rueda/arrastre de scrollbar/velo en paleta y ajustes con mouse tracking activo.
