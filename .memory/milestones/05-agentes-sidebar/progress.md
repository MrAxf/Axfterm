# Progreso — M5 Agentes y sidebar

## 2026-11-25 — Reanudación por sesión

- OSC 7501 aporta el id de sesión (`ses_…`) en sus registros hijos; se conserva
  como `AgentSession.external_id` y se persiste por pestaña como
  `resume_session`. Al restaurar se lanza `opencode --session <id>` (o
  `copilot --resume=<id>`) y dos pestañas del mismo proyecto dejan de abrir la
  misma conversación. Sin id se mantiene `--continue` como fallback.

## 2026-11-23 — Rediseño de selección en curso

- La terminal activa usa el fondo de selección del tema y peso tipográfico,
  sin indicador lateral añadido. El cursor de teclado conserva su borde de
  foco independiente y los estados de agente mantienen icono y texto.
- Falta comprobar la apariencia y el foco en una ventana real actualizada.

## 2026-11-22 — Ajuste de UI y 1 sesión por terminal

- Reducido a 1 sesión de agente activa por terminal en la reconciliación y en
  OSC 7501 (`sessions.truncate(1)`), eliminando chips y contadores redundantes.
- La sidebar vuelve al filtro `Todas | Agentes` (compatibilidad con `"terminals"`
  en `session.json`), mostrando la misma tarjeta limpia de dos líneas en ambas
  vistas y ordenando por prioridad en `Agentes`.
- El badge de estado sobre el icono de pestaña y de terminal se desplaza hacia
  fuera (`-agent_badge_offset`) con anillo concéntrico (`p(hairline)`) sin
  `border_1` (centrado exacto del SVG con `flex_shrink_0`), `TabStatus::Working`
  usa tono `Accent` y gira con `Animation::repeat()`.
- `Tab` y `Shift+Tab` se registran en el contexto `Terminal` (`SendTab` /
  `SendBacktab`) para que gpui-kit no los intercepte con la navegación de foco
  global cuando la terminal está enfocada.
- Al guardar una instantánea con `ALT_SCREEN` activa (TUI a pantalla completa),
  se captura `inactive_grid()` (la pantalla principal) para que la TUI quede
  limpia al reabrir.
- `Tab.resume_agent` persiste si había un agente vivo al cerrar y al restaurar
  el PTY ejecuta `opencode --continue` o `copilot --continue`.

## 2026-11-22 — M5 implementado (pendiente gate manual)

- `axf_agents`: `AgentKind`, estados con prioridad, `AgentSession` runtime,
  `AgentReport`/`AgentTransition`, agregados y matcher puro de procesos con
  snapshots; escáner `sysinfo` (`ProcessScanner`). Se reconocen `opencode`,
  `opencode2`, `copilot`, `copilot-runtime` y el fallback Node/Bun, con poda de
  ayudantes como `opencode-pty`.
- `axf_terminal`: PID raíz expuesto; OSC 7501 completo (estados, kinds,
  progreso, `app`, base64 con límites, saneado, `clear` y respuesta a `?`);
  OSC 9/777/1337 como atención; `TerminalEvent::ProgramStatus`, contador de
  bytes por frame y `TerminalSignal` que la vista entrega a la app.
- `axf_workspace`: `Tab.agents` runtime-only (no se serializa), agregado por
  pestaña, reconocimiento al enfocar y retirada de sesiones sin proceso.
  `SidebarState.section` persiste `Terminales`/`Agentes`.
- `axf_config`: sección `[agents]` con `enabled`, `scan_interval_ms` (1000–2000)
  y avisos por estado; plantilla y validación.
- `app`: escáner en segundo plano, reconciliación por PID, fallback de salida y
  quiescencia, notificaciones con cola/deduplicación/cooldown, marca `!` en el
  título, sidebar con secciones, chip por terminal con agente, filas de sesión,
  selección por teclado y salto a la pestaña. El icono de pestaña/terminal usa
  la marca del agente y un círculo de estado superpuesto.
- `axf_ui`: `TabStatus` añade `Finished`/`Error`; tonos y badges de agente.
- Tests: 17 en `axf_agents`, 6 nuevos de OSC 7501 en `axf_terminal` y nuevos de
  sidebar/reconciliación en `app`. `docs/agents.md` y CLIs falsos en
  `crates/terminal/tests/fixtures/`.
- Verificación: `cargo fmt`, clippy `-D warnings`, `cargo test --workspace` y
  release en verde. Gate manual con CLIs reales pendiente en una sesión
  interactiva.
- Movidos a `99-futuro`: sidebar estructural (`SidebarHost`, trait
  `SidebarPanel`, slots) y lanzar agentes desde la app.

## 2026-11-22 — Replanteo del hito

- La detección deja de depender de heurísticas de pantalla: OSC 7501 es la
  fuente principal de estado y el árbol de procesos aporta atribución, icono y
  fallback. ADR 0009.
- Se modelan varias sesiones por pestaña y la sidebar pasa a `Terminales` y
  `Agentes`; cada terminal con agentes muestra debajo de su fila un chip con
  icono, número de sesiones y estado agregado.
- Los estados nuevos son `Finished` y `Error`; la prioridad es
  `NeedsAttention > Working > Error > Finished > Idle > Stopped`.
- Las notificaciones son in-app y en el título, con limpieza al enfocar.

## 2026-11-22 — Renumeración

- El milestone pasa de M7 a M5 al adelantarse antes que la paleta. La UI de
  sidebar ya estaba adelantada; faltan la detección real, la paleta de M6 y la
  sección de ajustes de M7.

## 2026-11-22 — Adelanto de la UI de sidebar

- Rehecha la barra lateral con acciones (`Nueva terminal` y abrir proyecto),
  disparador de la futura paleta, filtro `Todas | Agentes`, cabeceras de
  proyecto sin icono y tarjetas de terminal de dos líneas.
- Cada terminal muestra perfil y contexto relativo al proyecto; el estado del
  proceso o del agente queda en un icono compacto con tooltip.
- Añadidos `Kbd`, `Button::shortcut/disabled/tooltip`, métricas de sidebar y
  chords centralizados en `crates/app/src/keys.rs`.
- Pendiente para M5: detección real de agentes (el filtro ya está preparado),
  duración/última actividad y la paleta real de M6.

## 2026-11-22 — Ajustes de la barra lateral

- El botón de abrir proyecto usa la variante rellena de `IconButton`, con la
  misma altura que `Nueva terminal`.
- La lista de proyectos y terminales queda enmarcada por separadores arriba y
  abajo.
- Cada terminal muestra un botón de cerrar al hacer hover, en el mismo carril
  que el icono de estado para no mover el contenido.
