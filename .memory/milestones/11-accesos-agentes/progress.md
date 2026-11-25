# Progreso — M11 Accesos rápidos a agentes

## 2026-11-25 — Pie con iconos e información básica

- Los accesos de agente del pie pasan a icon-buttons siempre: sin etiqueta ni
  umbral compacto, en una fila con wrap preparada para más agentes. El tooltip
  conserva etiqueta, atajo y estado en el proyecto.
- El pie añade debajo la versión de Axfterm y un resumen del workspace
  (proyectos, terminales y agentes vivos), con el detalle completo en el
  tooltip. Se retira `sidebar_agent_compact_width`.
- Tests nuevos del recuento de agentes vivos y del resumen en singular/plural.

## 2026-11-25 — Lanzador implementado

- ADR 0012 aceptado. Pie de la sidebar y desplegable del «+» abren OpenCode o
  Copilot con las acciones `agent.open_opencode` / `agent.open_copilot`
  (`Ctrl+Alt+1` / `Ctrl+Alt+2`), también en paleta y ayuda.
- Cada gesto abre una pestaña nueva con el perfil resuelto —preferido del
  agente validado por host, después el del proyecto y después el global— y
  escribe el comando al shell cuando el PTY está listo. Se respeta el override
  de `[profiles.agent_commands]` y el `--cd` de WSL.
- El pie muestra marca y etiqueta, pasa a iconos por debajo de
  `sidebar_agent_compact_width()` y enriquece el tooltip con el atajo y el
  estado del agente en el proyecto. El menú del panel separa los agentes de las
  operaciones de división.
- Tests nuevos de resolución de perfil (local, WSL, preferido inválido), comando
  de lanzamiento y métrica compacta. `scripts/check.ps1`, el build release, las
  pruebas del wrapper y el clippy de Studio pasan.
- La escena `app.sidebar.agent-actions` está creada, pero su captura quedó
  bloqueada porque la instancia instalada de Axfterm ocupaba el mutex de
  instancia única. Queda pendiente junto al gate manual con CLIs reales.

## 2026-11-25 — Hito creado

- Hito definido por solicitud: abrir OpenCode o Copilot en una pestaña nueva
  desde el pie de la sidebar y desde el desplegable del «+» de la pestaña.
- Decisiones: cada gesto abre siempre pestaña nueva; el perfil preferido del
  agente (`[agents.preferred_profiles]`) se usa si es compatible con el host del
  proyecto y, si no, el del proyecto; `Ctrl+Alt+1` / `Ctrl+Alt+2`; entrega en
  v0.2.
- El comando del agente se escribe en el shell de la pestaña nueva cuando el
  PTY está listo, igual que la reanudación actual, para conservar el shell al
  salir y el `--cd` de WSL.
- Sin implementación todavía. Pendiente el ADR 0012 y las escenas de referencia
  en Studio.
