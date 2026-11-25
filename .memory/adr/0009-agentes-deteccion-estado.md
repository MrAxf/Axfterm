# ADR 0009 — Detección y estado de agentes

- **Estado**: Aceptado
- **Fecha**: 2026-11-22
- **Decidores**: Juan Andrés

## Contexto

La sidebar ya muestra terminales por proyecto, pero no sabe si dentro de una
terminal corre un agente de IA ni si está trabajando, esperando al usuario o
ha terminado. El producto necesita una señal fiable para operar varias
terminales con agentes a la vez sin revisarlas una por una.

Restricciones reales:

- Windows primero; WSL es un perfil de terminal habitual.
- Sin red ni telemetría: la detección y el estado son locales.
- La UI no puede bloquearse esperando procesos ni I/O.
- Un agente puede ocupar una terminal entera, cambiar de estado muchas veces
  por turno y terminar dejando un resultado que el usuario aún no ha visto.
- El estado no debe sobrevivir a un reinicio como si el proceso siguiera vivo.
- Hay versiones antiguas de los CLIs que no reportan estado.

OpenCode 2 (`app=opencode`) y GitHub Copilot CLI 1.0.107+ emiten **OSC 7501
(Program Status Protocol)**, que reporta `idle`, `working`, `blocked`, `done`,
`error` y `clear`, con `kind`, `progress` y un mensaje. Eso evita adivinar el
estado por la pantalla; el árbol de procesos sigue siendo necesario para
atribuir el agente a la pestaña, elegir su icono y cubrir versiones antiguas.

## Decisión

### OSC 7501 como fuente principal

- El parser de terminal reconoce `OSC 7501 ; pares ST` y publica un evento de
  estado de programa con `state`, `kind`, `progress`, `app`, `title` y `msg`.
- `msg` y `title` se decodifican de base64 con los límites del protocolo y se
  rechazan si contienen caracteres de control. No se interpretan como markup.
- Se responde a `OSC 7501 ; ?` con `?`, `states` y `kinds`; un programa que
  pregunta recibe confirmación aunque el usuario no vea nada en pantalla.
- `done` y `error` sobreviven a la salida del proceso; `working` y `blocked` se
  descartan cuando el proceso termina.
- El estado del programa se asocia a la sesión por `app` y por la sesión más
  reciente del mismo tipo en esa terminal. El árbol de procesos aporta la
  identidad y el PID.

### Árbol de procesos para atribución y fallback

- `TerminalSession`/`TerminalView` exponen el PID raíz del PTY.
- `axf_agents` recibe un snapshot de procesos y devuelve los agentes del
  subárbol de cada raíz. El matcher es puro y testeable.
- Se reconocen `opencode.exe` (también para el alias `opencode2`), `copilot.exe`
  y `copilot-runtime.exe`. Como fallback se inspecciona el argv de Node/Bun.
- Se elige el proceso agente más cercano al shell y se podan sus ayudantes
  (`opencode-pty`, runtimes hijos) para no duplicar sesiones.
- El escaneo se hace cada 1-2 s en segundo plano, con un único refresco global
  de `sysinfo` y comparación posterior por raíz.
- OSC 9/777/1337, BEL, la actividad de salida, la quiescencia y el código de
  salida son señales de fallback. Un proceso que termina sin reporte final se
  cierra como `Finished` si salió bien o `Error` si falló.
- OpenCode 2 no emite `done`: al terminar un turno pasa de `working` a `idle`
  (registro raíz) o `clear` (registro de sesión). Axfterm trata ese paso como
  `Finished` para que el aviso de terminado sea observable; el resto de agentes
  conserva la semántica estricta de `done`.

### Modelo de sesiones

- Cada proceso de agente detectado es una `AgentSession`.
- Una pestaña puede tener varias sesiones (`Tab.agents: Vec<AgentSession>`).
- Estados: `Idle`, `Working`, `NeedsAttention`, `Finished`, `Error` y
  `Stopped`.
- Prioridad de presentación:
  `NeedsAttention > Working > Error > Finished > Idle > Stopped`.
- El estado es runtime-only: no se serializa en `session.json`. Al arrancar no
  hay badges de agentes que ya no existen.
- Al mover una pestaña de panel se conservan sus sesiones; al cerrar la pestaña
  se retiran.
- Al enfocar una pestaña, `NeedsAttention`, `Finished` y `Error` pasan a
  `Idle`. Las sesiones cuyo proceso ya no existe se retiran.

### Presentación

- La sidebar se divide en `Terminales` y `Agentes`.
- `Terminales` conserva la agrupación por proyecto. Cada terminal con agentes
  muestra debajo de su fila un chip con el icono del agente dominante, el número
  de sesiones y la etiqueta del estado de mayor prioridad. El tono sigue la
  prioridad.
- `Agentes` agrupa las sesiones por proyecto y las ordena por prioridad y
  actividad. Cada fila muestra agente, terminal, estado, mensaje, progreso y
  duración. Clic o Enter enfoca la pestaña y selecciona la sesión.
- El icono de la terminal y de la pestaña usa la marca del agente dominante y
  lleva un círculo superpuesto: verde/check al terminar, ámbar/! si requiere
  atención, neutro/spinner si trabaja y rojo/alerta si falla.
- El estado agregado de la pestaña usa la sesión de mayor prioridad; si hay
  empate, la más reciente.

### Notificaciones

- Se notifica en la app al pasar a `done`, `blocked` o `error`, con cola,
  deduplicación y cooldown.
- El título de la ventana marca que hay atención pendiente.
- No se muestra toast si la pestaña ya está enfocada; el icono y el chip sí se
  actualizan.
- No hay notificaciones del sistema ni red en v0.1.

### Configuración

- `[agents]` en `config.toml`: `enabled`, `scan_interval_ms`,
  `notify_finished`, `notify_error` y `notify_attention`.
- La sección Agentes del panel de ajustes de M7 edita estos campos con el mismo
  borrador y `toml_edit` que el resto.

## Consecuencias

### Positivas

- El estado es preciso mientras el CLI hable OSC 7501, sin parsear la pantalla.
- El árbol de procesos cubre la atribución, los iconos y las versiones antiguas.
- WSL funciona aunque `sysinfo` no vea los procesos Linux, porque OSC 7501 viaja
  por el PTY.
- La UI no se bloquea y el estado no se persiste como si fuera real tras un
  reinicio.

### Negativas y riesgos

- OSC 7501 no está en todas las versiones; el fallback es más impreciso y puede
  requerir ajustes de umbral.
- Varios agentes del mismo tipo en una terminal pueden producir reportes
  ambiguos; se aplican a la sesión más reciente y el árbol conserva el conteo.
- El escaneo global de `sysinfo` tiene coste; se mide y se mantiene en 1-2 s.
- Los nombres de proceso pueden cambiar entre versiones; el matcher se cubre con
  snapshots y se documenta.

## Alternativas

- **Parsear la pantalla**: frágil, depende de la TUI y de idioma; rechazado.
- **Solo árbol de procesos**: no distingue trabajar de esperar o terminar;
  rechazado como fuente principal, se conserva como fallback.
- **Plugins o hooks por agente**: requieren configurar cada CLI y no cubren
  otros agentes; fuera de v0.1.
- **Notificaciones nativas del sistema**: añaden WinRT y permisos; se posponen.
- **Persistir el estado de agentes**: restauraría badges falsos; rechazado.

## Referencias

- `.memory/milestones/05-agentes-sidebar/tasks.md` — implementación y tests.
- `AGENTS.md` — agentes, hilos, teclado y minimalismo.
- [Program Status Protocol (OSC 7501)](https://control-codes.page/proposals/osc-7501/)
- [OpenCode: soporte de OSC 7501](https://github.com/anomalyco/opencode/issues/54179)
- [Copilot CLI: soporte de OSC 7501](https://github.com/github/copilot-cli/issues/5112)
