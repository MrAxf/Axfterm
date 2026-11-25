# Progreso — M7 Panel de ajustes

## Validación final de teclado y gates

- En Studio visible, la navegación real por teclado seleccionó Monokai Light y
  actualizó tanto el tema activo como la preview UI/ANSI. Esc descartó el
  borrador y restauró One Dark sin persistir el experimento.
- `settings_manual_gate_changes_survive_config_reload` guarda y vuelve a leer
  un borrador combinado con tema, fuentes, perfil, atajo y ajustes de agentes.
- `pwsh -File scripts/studio.ps1 -All -Hidden` reprodujo las 34 escenas.
  También pasaron `scripts/check.ps1`, el build release, las pruebas del
  wrapper y clippy con `studio`.
- Quedan como gate manual las pruebas de conflicto externo, operaciones
  completas de perfiles/atajos/agentes y persistencia UI tras reiniciar.

## Sidebar y estados de agente

- Las filas de terminal con agente muestran primero su estado textual y después
  el mensaje dinámico, para que «Atención» o «Trabajando» sigan visibles cuando
  el detalle se trunca. El tooltip conserva el mensaje completo.
- Se añadió `app.sidebar.agents.light` para revisar los mismos estados con One
  Light; las escenas de agentes en One Dark y One Light se capturaron y
  revisaron. El test `sidebar_agent_metadata_shows_state_before_truncated_detail`
  protege el orden de estado y detalle.
- Los 34 escenarios, `scripts/check.ps1`, build release, pruebas del wrapper,
  clippy con `studio` y `git diff --check` pasaron.

## Validación de rediseño y escenas Studio

- Studio reprodujo las 34 escenas; las nuevas capturas de Apariencia confirman
  tema personalizado seleccionado y legibilidad a escala 1,25. Se revisaron
  también onboarding, workspace, perfiles, fuentes y búsqueda de la paleta.
- `scripts/check.ps1`, build release, pruebas del wrapper y clippy con `studio`
  pasaron. `git diff --check` no reportó errores.
- El workspace de Studio puede mostrar errores de inicio de PowerShell
  provenientes del perfil local; no son un defecto de la interfaz. El recorrido
  de teclado en Apariencia está validado; el resto del gate manual sigue abierto.

## 2026-11-24 — Rediseño de UI/UX en curso

- Studio ya permite revisar el flujo completo; se capturaron 34 escenas,
  incluidas las vistas de terminales, agentes y el estado vacío de agentes. Hay
  capturas repetibles del workspace en Cobalt2, Monokai y One Light. La
  salida de error del perfil PowerShell del fixture se separa de los hallazgos
  de producto.
- Apariencia usa el ancho disponible de la ventana en la galería de temas,
  mantiene dos muestras legibles por fila, identifica el tema actual y permite
  seleccionar tarjetas con Enter, Espacio o clic. Se revisó en One Dark y One
  Light sin desplazar el catálogo a una sola familia. El panel da más altura al
  área desplazable para mostrar completo el catálogo en ventanas estándar, con
  una escena adicional a 1024×700 para comprobar el ajuste.
- El onboarding conserva la acción principal en el área central y elimina el
  botón duplicado del empty state de la sidebar.
- En la paleta de búsqueda, los tramos resaltados ya no añaden espacios
  artificiales entre caracteres; la captura con la consulta `open` confirma que
  las etiquetas se leen de forma natural.
- Las 33 escenas, `scripts/check.ps1` (formato, clippy y suite completa), el
  build release, `scripts/test-cargo-low.ps1` y clippy con `studio` pasaron. Una
  prueba PTY tuvo un fallo transitorio en una ejecución anterior, pero pasó al
  repetirla y en la comprobación final.
- Pendiente: continuar el ajuste de flujos principales y secundarios, y validar
  interacción, escalado y ventanas estrechas antes del cierre.

## 2026-11-23 — Rediseño de Apariencia en curso

- Galería de temas con muestras de UI y terminal, selección visible y foco
  de teclado; tests y clippy pasan, y el build release se validó en un
  directorio de salida aislado.
- Pendiente la revisión visual interactiva y la validación de temas claros y
  personalizados; la interacción con la instancia de auditoría fue denegada.
- Una captura aportada por el usuario mostró que el modo de color desplazaba
  la galería debajo del pie. Se compactó ese control; falta comprobar
  visualmente la nueva disposición.
- Otras capturas mostraron una sola columna con espacio libre y un modo
  seleccionado poco visible. Se adaptó el ancho de las tarjetas al área
  disponible y se usó un grupo de radios horizontal; falta gate visual y de
  teclado para esta última versión.
- El aviso de guardado deja de mostrarse cuando hay nuevas modificaciones; se
  mantiene después de guardar mientras el borrador coincide con lo guardado.
- En Perfiles se separan las acciones generales de las del perfil seleccionado
  y se indica el objeto al que afectan; falta revisión visual y de teclado.
- Los controles propios de ajustes registran `tab_stop` en su `FocusHandle` y
  muestran un borde de foco. En Perfiles, las flechas desplazan el foco junto a
  la selección; falta verificar el recorrido en una ventana real.

## 2026-11-22 — Renumeración

- El milestone pasa de M6 a M7 para ejecutarse después de la paleta (M6), de la
  que dependen los atajos y el registro de acciones.
- La sección Agentes llega desde M5 y se implementa aquí con el resto de
  secciones.

## 2026-11-23 — Arranque de M7

- Plan: escritura de config que conserva comentarios y orden (`toml_edit`),
  borrador con preview y conflicto, panel overlay con secciones y navegación
  por teclado, y ampliación de `[agents]` con perfil preferido por agente.
- En curso: API de escritura en `axf_config`.

## 2026-11-23 — Cierre de implementación

- `axf_config::write_config` aplica solo los campos cambiados con `toml_edit`,
  conserva comentarios y orden, valida antes de escribir y usa `write_atomic`.
- `[agents.preferred_profiles]` entra en el esquema v2 con validación; el panel
  lo edita y lo guarda, y queda documentado como consumo de v0.2.
- Panel completo: siete secciones, rail con Enter/Espacio, perfiles con
  detección/edición/duplicado/borrado protegido/orden/icono, atajos con captura
  y `unbind`, agentes y avanzado.
- Preview de tema/fuentes en `ThemeHost`; Cancelar restaura el estado visual y
  los cambios externos se explican antes de recargar o sobrescribir.
- Layout del modal ajustado: rail y contenido con anchuras explícitas
  compensadas por DPI, pie siempre visible y barras de acciones con wrap.
- Incidente: `shell.rs` se aplanó accidentalmente al escribir con PowerShell.
  Se recuperó desde el respaldo, se repararon comentarios que se habían comido
  código y se dividieron `render_settings` y perfiles para evitar desbordes de
  pila en builds debug.
- Pendiente: gate manual (cambiar tema, fuente, perfil, atajo y agente desde el
  panel, reiniciar y comprobar persistencia).
