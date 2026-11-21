# Tareas — M1 Motor de terminal

## PTY

- [ ] `TerminalSession`: spawn, write, resize, kill y wait
- [ ] Detección de shell: `pwsh` > `powershell` > `cmd`; `$SHELL` en Unix
- [ ] cwd inicial y entorno (`TERM=xterm-256color`, `COLORTERM=truecolor`)
- [ ] Hilo lector + canal + backpressure ante output masivo
- [ ] Cierre, exit code y estado "proceso terminado" en la UI
- [ ] Matar el árbol de procesos al cerrar pestaña

## Motor VT

- [ ] `alacritty_terminal::Term` + `Config` + scrollback
- [ ] Parser en hilo dedicado con snapshot y damage por fila
- [ ] Resize/reflow y alternancia de pantalla
- [ ] Paleta ANSI 16/256/truecolor; bold, italic, underline, reverse, dim
- [ ] OSC: título, cwd, hipervínculos básicos; bell
- [ ] Caracteres anchos, combinados y fallback de glifos

## Render

- [ ] `TerminalElement` con quads de fondo y texto por filas
- [ ] Cache de shaping por fila e invalidación por damage
- [ ] Cursor (bloque/barra/subrayado) y parpadeo
- [ ] Selección de texto y scrollbar
- [ ] Padding, fondo y borde del tema
- [ ] Repintado sin allocations por celda en el camino caliente

## Input

- [ ] Printable, Enter, Tab, Backspace, Esc
- [ ] Flechas, Home/End, Insert/Delete, PageUp/PageDown, F1-F12
- [ ] Modos app-cursor y bracketed paste
- [ ] Combinaciones Ctrl/Alt/Shift
- [ ] Focus in/out
- [ ] IME commit + overlay de preedit
- [ ] AltGr y dead keys en Windows

## Clipboard e hipervínculos

- [ ] Copiar/pegar con `Ctrl+Shift+C/V` y menú contextual
- [ ] Pegado multilínea normalizado y bracketed paste seguro
- [ ] Detección de URLs + abrir con confirmación

## Calidad

- [ ] Tabla de casos de encoding (unit tests)
- [ ] Fixtures VT: SGR, alt screen, wrap, cursor, scroll
- [ ] Bench: throughput, resize y frame; flood con `yes`
- [ ] Gate manual con `pwsh`, `vim`, `opencode` y `btop`
