# M0 — Fundaciones y spikes

Objetivo: dejar un repo compilable, un shell GPUI con tema base y validar los
cuatro riesgos técnicos (PTY, render, teclado/IME, iconos) antes de construir
features.

## Criterio de hecho

- [ ] `cargo run -p axfterm` abre una ventana con una terminal real.
- [ ] Escribir, leer, colores, resize y Ctrl+C funcionan en Windows.
- [ ] El grid de prueba se pinta con frame estable y medición registrada.
- [ ] ADRs escritos y decisiones reflejadas en `AGENTS.md`.
- [ ] `fmt`, `clippy -D warnings` y `test --workspace` en verde.
