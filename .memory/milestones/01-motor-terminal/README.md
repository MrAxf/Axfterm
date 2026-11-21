# M1 — Motor de terminal

Objetivo: una terminal usable a diario en Windows, con PTY, VT completo,
render eficiente, input fiable y clipboard, medible y sin corrupción al
redimensionar.

## Criterio de hecho

- [ ] `pwsh`, `vim`, `opencode` y un TUI (`btop` o similar) funcionan.
- [ ] Colores 16/256/truecolor, cursor, selección y scrollback correctos.
- [ ] Resize sin corrupción ni pérdida de contenido.
- [ ] Ctrl+C, flechas, AltGr, dead keys e IME correctos.
- [ ] Benchmarks dentro del presupuesto (frame e input).
- [ ] Tests de encoding y VT en verde.
