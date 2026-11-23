# Tareas — M1 Motor de terminal

> Base del gate M0: `axf_terminal` ya tiene `PtySession`, hilo parser con
> frames/damage, `TerminalElement` y input básico (IME + control). M1 endurece
> y completa esa base; no se reescribe desde cero.

## PTY

- [x] `TerminalSession`: spawn, write, resize, kill y wait
- [x] Detección de shell: `pwsh` > `powershell` > `cmd`; `$SHELL` en Unix
- [x] cwd inicial y entorno (`TERM=xterm-256color`, `COLORTERM=truecolor`)
- [x] Hilo lector + canal + backpressure ante output masivo
- [x] Cierre, exit code y estado "proceso terminado" en la UI
- [x] Matar el árbol de procesos al cerrar pestaña

## Motor VT

- [x] `alacritty_terminal::Term` + `Config` + scrollback
- [x] Parser en hilo dedicado con snapshot y damage por fila
- [x] Resize/reflow y alternancia de pantalla
- [x] Paleta ANSI 16/256/truecolor; bold, italic, underline, reverse, dim
- [x] OSC: título, cwd, hipervínculos básicos; bell (+ OSC 22 de cursor)
- [x] Caracteres anchos, combinados y fallback de glifos (fallback del sistema
      de texto de GPUI)

## Render

- [x] `TerminalElement` con quads de fondo y texto por filas
- [x] Cache de shaping por fila e invalidación por damage
- [x] Cursor (bloque/barra/subrayado); parpadeo pendiente
- [x] Selección de texto y scrollbar
- [x] Métricas de celda y línea desde la fuente; altura natural `ascent − descent`
- [x] Padding, fondo y borde configurables del tema
- [x] Repintado sin allocations por celda en el camino caliente

## Input

- [x] Printable, Enter, Tab, Backspace, Esc (teclas de texto por IME)
- [x] Flechas, Home/End, Insert/Delete, PageUp/PageDown, F1-F12
- [x] Modos app-cursor y bracketed paste
- [x] Combinaciones Ctrl/Alt/Shift (AltGr por el manejador de texto)
- [x] Focus in/out (DECSET 1004)
- [x] IME commit + overlay de preedit
- [x] AltGr y dead keys en Windows (tests automáticos y validación con
      teclado real)

## Ratón

- [x] Tracking DECSET 1000/1002/1003 y SGR 1006 + X10
- [x] Clic, liberación, arrastre, movimiento y rueda con modificadores
- [x] Tests del codificador (SGR, X10, UTF-8, rueda, motion)
- [x] Selección de texto, copia y scrollback con rueda sin tracking
- [x] Forma del cursor pedida por la aplicación (DECSCUSR y OSC 22)

## Clipboard e hipervínculos

- [x] Copiar/pegar con `Ctrl+Shift+C/V` y menú contextual
- [x] Pegado multilínea normalizado y bracketed paste seguro
- [x] Detección de URLs + abrir con confirmación (solo http/https)

## Calidad

- [x] Tabla de casos de encoding (unit tests)
- [x] Fixtures VT: SGR, alt screen, wrap, cursor, scroll, wide/combining,
      OSC, selección, resize y modos privados
- [x] Bench: throughput, resize y frame; flood con un proceso real
- [x] Gate manual con `pwsh`, `vim`, `opencode` y `btop`/TUI equivalente
      (PTY/headless, captura de la app, teclado real y pasada visual)
