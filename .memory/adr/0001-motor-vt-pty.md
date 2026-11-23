# ADR 0001 — Motor VT y PTY

- **Estado**: Aceptado
- **Fecha**: 2026-11-21
- **Decidores**: Juan Andrés
- **Spikes**: S1 (`pty_smoke`), S2 (`pid_tree`), S3 (`vt_dump`)

## Contexto

Axfterm necesita ejecutar shells reales en Windows (ConPTY), interpretar su
salida ANSI y mantener un estado de pantalla que el render pueda consumir sin
bloquear el hilo de UI. El motor debe cubrir, como mínimo:

- SGR de 16, 256 y truecolor; bold, dim, italic, underline y reverso.
- Pantalla alternativa (aplicaciones de pantalla completa), cursor y DECTCEM.
- Caracteres anchos (CJK), combining marks y emoji.
- Resize con reflow, scrollback y `damage()` incremental.
- Respuestas al PTY: DSR (`CSI 6n`), tamaño en caracteres (`CSI 18t`), tamaño en
  píxeles (`CSI 14t`) y color (`OSC 10/11`).
- Cierre limpio: propagar el exit code y no dejar procesos huérfanos.
- Detección de agentes en el subárbol del PTY, fuera del hilo de UI.

Se evaluaron tres vías: implementar el parser, usar una librería de terminal
completa (WezTerm) o extraer el motor de un terminal existente. El criterio no
fue solo cobertura, sino coste de mantenimiento, licencia, dependencias y
facilidad para adaptar el render.

## Decisión

### Dependencias

- `alacritty_terminal =0.26.0` (Apache-2.0) para VT, grid, scrollback, modos y
  `damage`.
- `portable-pty =0.9.0` (MIT) para PTY multiplataforma; en Windows usa ConPTY.
- Versiones fijadas con `=` y actualizadas de forma atómica (ver ADR 0002).
- `sysinfo =0.40.4` no entra en `axf_terminal`: el árbol de procesos es
  responsabilidad de `axf_agents` (S2).

### Arquitectura de hilos

- Un **hilo lector** por PTY bloqueado en `read`; no hace parsing ni toca GPUI.
- Un **hilo parser por terminal** que posee `alacritty_terminal::Term` y
  `vte::ansi::Processor`. El parser consume bytes, aplica el damage y publica
  `Snapshot + Damage` por un canal acotado al hilo de UI.
- El **hilo de UI** solo consume damage y pinta. No espera I/O, no hace `join` y
  no bloquea al cerrar una terminal.
- El output se coalesce: como máximo un repintado por frame; el input del
  usuario tiene prioridad sobre el output pendiente.
- El `EventListener` del `Term` es el único camino para devolver bytes al PTY:
  - `PtyWrite` → respuestas DSR y `CSI 18t`.
  - `TextAreaSizeRequest` → `CSI 14t` con tamaño en píxeles.
  - `ColorRequest` → `OSC 10/11` con los colores del tema.
  El parser conserva estado entre lecturas, así que una secuencia partida en dos
  chunks se responde una sola vez y completa. S1 confirmó que `pwsh` se bloquea
  esperando DSR al arrancar.
- `Term` debe ser `Send`: el listener usa `Arc<Mutex<_>>` o un canal, nunca
  `Rc`.

### Resize y cierre

- Resize = `Term::resize` (reflow) + resize del PTY, con debounce por frame.
  El damage posterior a un resize es completo; la cache de render se invalida
  entera.
- Cerrar una pestaña mata el **árbol** de procesos del PTY, no solo la raíz:
  S2 observó dos huérfanos al matar únicamente el proceso raíz.
- El exit code y el estado de salida se publican como evento para que la pestaña
  muestre "terminada" en vez de desaparecer sin explicación.

### Frontera y tipos

- Los tipos de `alacritty_terminal` quedan confinados en `axf_terminal`. La API
  pública expone tipos propios (`ScreenSnapshot`, `Damage`, `TerminalConfig`,
  `TerminalEvent`).
- No se introduce un trait `TerminalBackend` hasta que exista una segunda
  implementación real. La frontera es de módulo y de tipos, no de abstracción
  especulativa.
- Los colores `Named`/`Indexed`/`Spec` se resuelven contra la paleta ANSI del
  tema en el render; cambiar de tema repinta sin reiniciar el parser.

## Consecuencias

### Positivas

- Cobertura VT probada en S3: 8 escenarios y 0 fallos, incluidos alt screen,
  wide chars, reflow, scrollback y respuestas al PTY.
- Rendimiento suficiente: spawn 54–69 ms, round-trip 19–24 ms y ~1,2 MiB/s de
  throughput crudo; el cuello en PowerShell es el host, no ConPTY.
- `damage()` permite cachear shaping por fila y evita re-shapear todo el grid.
- El hilo de UI queda libre para input y repintado; el parser puede ir por
  delante del render sin bloquearlo.

### Negativas y riesgos

- `alacritty_terminal` es una librería extraída de un binario: su API no tiene
  el mismo compromiso de estabilidad que una librería "de producto". Mitigación:
  pin exacto, tipos confinados y tests de integración con fixtures.
- El damage puede ser `Full` tras resize o scroll; el render debe asumirlo y no
  intentar optimizarlo como incremental.
- DSR y otras respuestas deben llevar estado; un respondedor stateless por chunk
  corrompe la secuencia. Mitigación: el parser es el único que responde.
- ConPTY tiene peculiaridades de resize y cierre; se validan con tests de PTY y
  scripts fixture en M1.
- Los procesos hijos pueden sobrevivir a la raíz; el cierre debe recorrer el
  subárbol (S2).

## Alternativas

- **`wezterm-term`**: cobertura amplia, pero arrastra el modelo de WezTerm y su
  ritmo de cambios; el render y los eventos no encajan mejor que alacritty.
- **`vt100`/parser propio**: menos dependencias, pero habría que implementar
  reflow, alt screen, wide chars, scrollback y damage, con riesgo alto y sin
  mediciones.
- **ConPTY directo (`windows` crate)**: control total, pero reimplementa la capa
  multiplataforma y complica el soporte futuro de Unix.
- **Lanzar Alacritty como proceso**: añade una app externa y un protocolo de
  comunicación; descartado por producto y por rendimiento.

## Referencias

- `.memory/milestones/00-fundaciones/progress.md` — resultados de S1, S2 y S3.
- `crates/terminal/examples/pty_smoke.rs`, `pid_tree.rs`, `vt_dump.rs`.
- `AGENTS.md` — hilos, render, teclado, agentes.
- ADR 0003 — el PTY pertenece al `TerminalId`, no al panel.
