# Axfterm — memoria del proyecto

Estado: M4 Perfiles de terminal cerrado. `config.toml` v2 declara
`[[profiles]]` con id, nombre, comando e icono; el descubrimiento local escribe
PowerShell, cmd, Git Bash, Nu y WSL en el primer arranque, la sesión v2 conserva
el perfil y los iconos salen del ejecutable con caché acotada y fallback Tabler.
Tests, clippy, fmt y release en verde; gate manual con pwsh, cmd y WSL.
Spikes S1–S8 hechos y medidos; ADR 0001–0009 aceptados en `adr/`.
Adelanto de M8: cada terminal guarda su historial en `scrollback/` y lo restaura
al activar su pestaña; el fallo visible abre el mismo perfil desde cero.
M5 Agentes y sidebar implementado: OSC 7501 + árbol de procesos, varias sesiones
por pestaña, chip por terminal, sidebar `Terminales`/`Agentes`, avisos in-app y
marca en el título. Tests deterministas en verde; queda el gate manual
interactivo con `opencode2`/`copilot` reales.
Siguiente: M6 teclado y paleta (el gate manual de M5 puede hacerse con la paleta
ya disponible).
Stack: Rust 2024 + gpui-kit + alacritty_terminal + portable-pty. Windows primero.

## Cómo leer

1. `tasks.md` — vista global por milestone.
2. `milestones/<slug>/README.md` — objetivo y criterio de hecho.
3. `milestones/<slug>/tasks.md` — tareas ejecutables.
4. `milestones/<slug>/progress.md` — avances, lo más reciente arriba.
5. `adr/` — decisiones de arquitectura aceptadas (0001–0009).

Leyenda: `[ ]` pendiente · `[~]` en curso · `[x]` hecho.

## Milestones

- [M0 Fundaciones](milestones/00-fundaciones/README.md) — cerrado
- [M1 Motor de terminal](milestones/01-motor-terminal/README.md) — cerrado
- [M2 Layout, proyectos y pestañas](milestones/02-layout-proyectos/README.md) — cerrado
- [M3 Sistema visual](milestones/03-sistema-visual/README.md) — cerrado
- [M4 Perfiles de terminal](milestones/04-perfiles/README.md) — cerrado
- [M5 Agentes y sidebar](milestones/05-agentes-sidebar/README.md)
- [M6 Teclado y paleta](milestones/06-teclado-paleta/README.md)
- [M7 Panel de ajustes](milestones/07-panel-ajustes/README.md)
- [M8 Persistencia y release](milestones/08-persistencia-release/README.md)
- [99 Futuro](milestones/99-futuro/README.md)

Reglas de producto y arquitectura: ver `AGENTS.md` en la raíz.
