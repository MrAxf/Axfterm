# M1 — Motor de terminal

Objetivo: una terminal usable a diario en Windows, con PTY, VT completo,
render eficiente, input fiable y clipboard, medible y sin corrupción al
redimensionar.

## Criterio de hecho

- [x] `pwsh`, `vim`, `opencode` y un TUI (`btop` o similar) funcionan
      (validados con PTY/headless, ventana real y teclado real).
- [x] Colores 16/256/truecolor, cursor, selección y scrollback correctos.
- [x] Resize sin corrupción ni pérdida de contenido.
- [x] Ctrl+C, flechas, AltGr, dead keys e IME correctos (tests, commit IME y
      teclado real).
- [x] Benchmarks dentro del presupuesto (frame e input).
- [x] Tests de encoding y VT en verde.
