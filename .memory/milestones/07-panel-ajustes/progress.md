# Progreso — M7 Panel de ajustes

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
