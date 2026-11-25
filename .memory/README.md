# Axfterm — memoria del proyecto

Estado: M9 implementado; quedan gates manuales de DPI 100/150/200, teclados
ES/CJK/IME y un día completo de uso. Icono y metadatos de Windows embebidos,
scrollback configurable con tope de 32 MiB, logs/crash opt-in, instancia única,
MSI y zip portable validados. M7 Panel de ajustes implementado; pendiente
validar por UI el conflicto externo, operaciones de perfiles/atajos/agentes y
persistencia tras reiniciar.
M6 Teclado y paletas cerrado. `config.toml` v2 declara `[[profiles]]` con id,
nombre, comando, icono y comandos de agente por perfil, además de
`[agents.preferred_profiles]`; el descubrimiento local
escribe PowerShell, cmd, Git Bash, Nu y WSL en el primer arranque, la sesión v3
conserva host y perfil por proyecto y los iconos salen del ejecutable con caché acotada y
fallback Tabler. Tests, clippy, fmt y release en verde; gate manual con pwsh,
cmd y WSL.
Spikes S1–S8 hechos y medidos; ADR 0001–0011 aceptados en `adr/`.
Adelanto de M9: cada terminal guarda su historial en `scrollback/` y lo restaura
al activar su pestaña; el fallo visible abre el mismo perfil desde cero.
M5 Agentes y sidebar implementado: OSC 7501 + árbol de procesos, varias sesiones
por pestaña, chip por terminal, sidebar `Terminales`/`Agentes`, avisos in-app y
marca en el título. Tests deterministas en verde; queda el gate manual
interactivo con `opencode2`/`copilot` reales.
M6 Teclado y paletas cerrado: registro de acciones, `[keymap]` con
contextos/unbind y validación de teclas de terminal, paleta única con modos
Comandos y Búsqueda, filtro por tipo, fuzzy, navegación y recientes, temas y
fuentes en su modal, y pantalla de atajos generada desde el registro. Tests en
verde y gate manual sin ratón validado.
M7 Panel de ajustes implementado: overlay con siete secciones, borrador con
validación y preview, escritura con `toml_edit` que conserva comentarios,
perfiles completos, atajos con captura, agentes y avanzado. Tests, preview
temático de teclado en Studio y round-trip combinado en verde; queda el gate
manual de conflicto externo, operaciones de perfiles/atajos/agentes y
persistencia UI tras reiniciar.
Reorganización del panel (ADR 0010): cuatro secciones (Apariencia, Perfiles,
Atajos y Agentes); tipografía y sliders en Apariencia; perfiles con acciones por
fila y modal; atajos con captura inline; avanzado en el menú «Más» del pie.
M8 implementado: proyectos alojados en WSL con host de proyecto, perfil fijo,
sesión v3, botón de carpeta partido junto a «Nueva terminal» y navegador de
carpetas WSL
(ADR 0011). Gate end-to-end con Ubuntu, nueva pestaña, split, reinicio y cierre;
quedan DPI 100/200, un cierre con proceso hijo largo y OSC 7 host-aware.
Persistencia/release pasa a M9 y el rediseño UI/UX a M10.
Stack: Rust 2024 + gpui-kit + alacritty_terminal + portable-pty. Windows primero.

## Cómo leer

1. `tasks.md` — vista global por milestone.
2. `milestones/<slug>/README.md` — objetivo y criterio de hecho.
3. `milestones/<slug>/tasks.md` — tareas ejecutables.
4. `milestones/<slug>/progress.md` — avances, lo más reciente arriba.
5. `adr/` — decisiones de arquitectura aceptadas (0001–0011).

Leyenda: `[ ]` pendiente · `[~]` en curso · `[x]` hecho.

## Milestones

- [M0 Fundaciones](milestones/00-fundaciones/README.md) — cerrado
- [M1 Motor de terminal](milestones/01-motor-terminal/README.md) — cerrado
- [M2 Layout, proyectos y pestañas](milestones/02-layout-proyectos/README.md) — cerrado
- [M3 Sistema visual](milestones/03-sistema-visual/README.md) — cerrado
- [M4 Perfiles de terminal](milestones/04-perfiles/README.md) — cerrado
- [M5 Agentes y sidebar](milestones/05-agentes-sidebar/README.md)
- [M6 Teclado y paleta](milestones/06-teclado-paleta/README.md)
- [M7 Panel de ajustes](milestones/07-panel-ajustes/README.md) — implementado; gate manual pendiente
- [M8 Proyectos en WSL](milestones/08-proyectos-wsl/README.md) — implementado; quedan gates de DPI, cierre prolongado y OSC 7
- [M9 Persistencia y release](milestones/09-persistencia-release/README.md) — implementado; quedan gates manuales de DPI, teclados ES/CJK y un día de uso
- [M10 Rediseño de experiencia UI/UX](milestones/10-experiencia-ui-ux/README.md) — implementado anticipadamente por solicitud
- [99 Futuro](milestones/99-futuro/README.md)

Reglas de producto y arquitectura: ver `AGENTS.md` en la raíz.
