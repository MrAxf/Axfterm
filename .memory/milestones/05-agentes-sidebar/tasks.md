# Tareas — M5 Agentes y sidebar

## Modelo y estados

- [x] `AgentSession` por proceso: id runtime, tipo, estado, mensaje, progreso,
      PID y marcas de inicio/actividad.
- [x] `Tab.agents: Vec<AgentSession>`; varias sesiones por pestaña y estado
      runtime-only (no se serializa en la sesión).
- [x] Estados `Idle`, `Working`, `NeedsAttention`, `Finished`, `Error` y
      `Stopped`.
- [x] Prioridad `NeedsAttention > Working > Error > Finished > Idle > Stopped`.
- [x] Agregados por pestaña: agente dominante, estado de mayor prioridad y
      desglose de sesiones por estado.

## Detección de procesos

- [x] Exponer el PID del PTY desde `TerminalSession`/`TerminalView`.
- [x] `ProcessInfo` + matcher puro en `axf_agents`, con snapshots inyectables
      para tests.
- [x] Matcher `opencode`/`opencode2` (mismo `opencode.exe`), `copilot` y
      `copilot-runtime`; fallback Node/Bun por argv.
- [x] Elegir el proceso agente más cercano al shell y podar ayudantes como
      `opencode-pty`.
- [x] Escáner `sysinfo` en segundo plano, con intervalo configurable (1-2 s).
- [x] Atribución PID raíz -> terminal -> pestaña; sobrevive a mover la pestaña.
- [x] Soporte de más de un agente por pestaña, con exclusiones y falsos
      positivos cubiertos por tests.

## OSC 7501 y señales

- [x] Parser OSC 7501 con estados `idle`, `working`, `blocked`, `done`,
      `error` y `clear`.
- [x] `kind` (`permission`, `question`, `auth`), `progress`, `app`, `title` y
      `msg`; base64 con límites y saneado de caracteres de control.
- [x] Respuesta a `OSC 7501 ; ?` con estados y kinds soportados.
- [x] `TerminalEvent::ProgramStatus` y consulta; la vista los entrega a la app.
- [x] OSC 9/777/1337 y BEL como señales de atención y fallback.
- [x] Actividad de salida y código de salida como fallback cuando el agente no
      habla OSC 7501.
- [x] Desambiguación de reportes OSC cuando hay varias sesiones del mismo tipo
      en una pestaña: se aplican a la más reciente activa.

## Máquina de estados

- [x] Mapear `working` -> `Working`, `blocked` -> `NeedsAttention`,
      `done` -> `Finished`, `error` -> `Error` e `idle` -> `Idle`.
- [x] `done`/`error` sobreviven a la salida del proceso; `working` y `blocked`
      se cierran con el proceso.
- [x] Latch de atención, terminado y error hasta foco o confirmación.
- [x] Al enfocar: atención, terminado y error -> `Idle`; retirar sesiones cuyo
      proceso ya no existe.
- [x] Notificar solo transiciones a `done`, `error` y `blocked`, con
      deduplicación y cooldown.
- [x] Override manual de estado y detalle explicable en el tooltip.

## Sidebar y UI

- [x] Secciones `Terminales` y `Agentes` en lugar de `Todas | Agentes`.
- [x] Terminales: se conserva la agrupación por proyecto; cada terminal con
      agentes añade debajo de su fila un chip.
- [x] Chip: icono del agente dominante, número de sesiones y estado de mayor
      prioridad; tono y tooltip con el desglose.
- [x] Agentes: sesiones agrupadas por proyecto y ordenadas por prioridad y
      actividad.
- [x] Fila de sesión: agente, terminal, estado, mensaje, progreso y duración.
- [x] Clic o Enter en una sesión enfoca su pestaña, la selecciona y restaura la
      terminal si hace falta.
- [x] Icono de terminal y pestaña sustituido por la marca del agente dominante.
- [x] Círculo de estado sobre el icono: verde/check terminado, ámbar/! atención,
      neutro/spinner trabajando y rojo/alerta error.
- [x] Navegación por teclado y foco coherentes con la sidebar actual.
- [x] Persistencia de la sección activa, sin persistir el estado de los agentes.

## Notificaciones

- [x] Toast in-app con cola y deduplicación para terminado, error y atención.
- [x] Marca en el título de la ventana cuando alguna sesión requiere atención.
- [x] Sin toast si la pestaña ya está enfocada; el icono y el chip sí cambian.
- [x] Enfocar limpia los estados y retira las sesiones terminadas sin proceso.

## Configuración

- [x] Sección `[agents]` en `config.toml`: `enabled`, `scan_interval_ms`,
      `notify_finished`, `notify_error` y `notify_attention`.
- [x] Validación de rangos y recarga en caliente; plantilla documentada.
- [ ] La sección Agentes del panel de M7 edita estos campos (trasladada a la
      tarea «Agentes» de M7).

## Sidebar estructural

- [ ] `SidebarHost` izquierda/derecha, ancho persistido y colapso (movida a
      99-futuro; el ancho y el colapso ya existen en v0.1).
- [ ] Trait `SidebarPanel` + registro de paneles; el árbol de proyectos de M2 es
      un panel más (movida a 99-futuro).
- [ ] Mover panel entre slots, ocultar y mostrar (movida a 99-futuro).
- [ ] Placeholders de contenedores y puertos (sin lógica) (movida a 99-futuro).

## Lanzar agentes

- [ ] Abrir terminal con agente en el cwd del proyecto (movida a 99-futuro).
- [ ] Preferencia de perfil por agente y argumentos extra al abrir la pestaña
      (movida a 99-futuro).
- [ ] Abrir un agente con un perfil existente, sin duplicar la línea de
      comandos (movida a 99-futuro).
- [ ] Detección de binario faltante y guía de instalación (movida a 99-futuro).

## Tests y gate

- [x] Matcher con snapshots: `opencode`, `opencode2`, `copilot`,
      `copilot-runtime`, Node/Bun, ayudantes y falsos positivos.
- [x] Parser OSC 7501: estados, kinds, base64, límites y consulta.
- [x] Prioridad, transiciones, latch y limpieza al enfocar.
- [x] Sidebar: secciones, chip, orden, navegación y salto a la pestaña.
- [x] CLIs falsos en PowerShell y sh que emiten OSC 7501/OSC 9/BEL.
- [ ] Gate manual: dos proyectos, `opencode2` + `copilot`, varias sesiones por
      pestaña, avisos y salto por teclado (pendiente de sesión interactiva con
      los CLIs reales; los escenarios deterministas están cubiertos).
- [x] `docs/agents.md` con protocolo, estados, configuración y limitaciones.
