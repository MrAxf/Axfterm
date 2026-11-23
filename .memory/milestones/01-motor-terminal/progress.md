# Progreso — M1 Motor de terminal

## 2026-11-21 — Gate manual cerrado

- Teclado real validado: AltGr, dead keys (layout ES), IME y Ctrl+C/flechas.
- Pasada visual de TUI (`nvim`/`opencode` y TUI equivalente), scroll,
  selección, clipboard y resize sin corrupción.
- M1 cerrado; siguiente milestone: M2 (layout, proyectos y pestañas).

## 2026-11-21 — M1 implementado y medido

Implementación completa del motor sobre la base de M0, sin reescribir la
arquitectura.

### PTY

- `TerminalSession` (alias `PtySession`) con `spawn`, `write`, `resize`, `kill`,
  `try_wait` y `wait`. El proceso se espera en un hilo propio con `try_wait`
  (nunca bloquea la UI y tolera los casos de ConPTY que no señalan el cierre).
- Detección de shell `pwsh` > `powershell` > `cmd` y `$SHELL` en Unix. `cwd`
  saneado, `TERM=xterm-256color`, `COLORTERM=truecolor`, `TERM_PROGRAM` y
  versión.
- Canal acotado (64) y `send_blocking` en el lector: backpressure real. El
  parser agrupa hasta 256 KiB por frame y el canal de frames es de 2.
- Exit code publicado por el waiter y mostrado como banner "proceso
  terminado (código N)"; el estado no se pierde si EOF llega después.
- Cierre por árbol: `taskkill /T /F` en Windows; grupo de procesos con
  `kill -TERM/-KILL` en Unix.

### Motor VT

- `TerminalCore` encapsula `Term` + `Processor` y es testeable en línea;
  `EngineBench` expone el mismo camino para fixtures y benchmarks.
- Eventos nuevos: título/reset (`OSC 0/2`), cwd (`OSC 7`), bell, cursor de
  puntero (`OSC 22`), clipboard y salida. Escáner OSC incremental propio para
  lo que `vte` no entrega (cwd y OSC 22), con estado entre chunks.
- Selección y copia delegadas en alacritty (`Selection`, `selection_to_string`);
  scrollback con `display_offset`/`history_size`; scroll con rueda y
  `ScrollToBottom` al escribir.
- Hipervínculos `OSC 8` por fila, caracteres combinados (`zerowidth`), anchos
  con spacer y reflow/alt screen. El fallback de glifos lo aporta el sistema de
  texto de GPUI.

### Render

- Selección pintada por fila (simple y rectangular) y scrollbar proporcional.
- Padding, fondo y borde configurables desde `TerminalStyle`; la app los deriva
  del tema y del tamaño de fuente.
- Cache de shaping por fila con scratch buffers reutilizados: sin allocations
  por celda en el camino caliente.
- Overlay de preedit IME en el cursor, con fondo y subrayado.
- Formas de cursor DECSCUSR y OSC 22 mapeadas a `CursorStyle`.

### Input, ratón y clipboard

- Focus in/out (DECSET 1004) con listeners de ventana; el PTY recibe `ESC[I` /
  `ESC[O` solo si el modo está activo.
- IME separado en `ImeState` (preedit/commit, rango UTF-16) con tests.
- `Ctrl+Shift+C/V` y menú contextual (acciones `CopySelection` y
  `PasteSelection`); pegado normalizado, bracketed paste y límite de 1 MiB con
  aviso.
- Selección con arrastre, Shift+clic y Alt+clic rectangular; rueda local sin
  tracking y Shift+rueda con tracking.
- URLs planas y `OSC 8`: detección en la fila y confirmación explícita antes de
  abrir; solo se abren esquemas `http`/`https`.

### Calidad y mediciones

- `fmt`, `clippy --workspace --all-targets -- -D warnings`, `test --workspace`
  y `build --release` en verde.
- 50 tests unitarios + 17 fixtures VT + 4 tests de assets. Los tests de PTY
  usan sesión + parser reales para responder al DSR de ConPTY.
- `vt_dump` de M0 sigue en verde (8 escenarios, 0 fallos).
- Bench release (`engine_bench`): throughput sintético 89,46 MiB/s; resize
  200x50 ↔ 100x30 0,52 ms/ciclo; frame 200x50 0,10 ms/frame.
- Flood real por PTY (`flood`, PowerShell escribe 1 MiB): 2,46 MiB/s en
  0,407 s, 6 frames. El output masivo se coalesce y no bloquea el hilo de UI.
- `tui_smoke` release: `nvim -u NONE` entra en alt screen y sale con código 0
  en 0,26 s; `opencode` entra en alt screen y produce frames (se cierra a los
  4 s para el smoke).
- Arranque real de `axfterm.exe` verificado con captura de ventana: prompt de
  `pwsh`, título OSC de la shell, cursor de bloque y status bar visibles, sin
  panic de wiring.

### Pendiente para cerrar el gate

- Pasada manual con teclado real: AltGr, dead keys e IME en la vista final.
- Pasada visual en la ventana con `btop` (no instalado aquí) y confirmación a
  ojo de scroll/selección; `nvim` y `opencode` ya pasan el smoke headless.
- Parpadeo de cursor y shell integration quedan para milestones posteriores.
