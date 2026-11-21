# AGENTS.md — Axfterm

## Qué es

Axfterm es un terminal workspace nativo, rápido y keyboard-first para Windows
(macOS/Linux después), construido en Rust 2024 sobre GPUI. Agrupa proyectos,
paneles y pestañas de terminal, y muestra agentes de IA (opencode y GitHub
Copilot) con su estado en la barra lateral.

## Estado

v0.1 en desarrollo. La fuente de verdad del progreso es `.memory/`: leer
`.memory/README.md` antes de tocar código.

## Stack fijado

- Rust stable, edition 2024, MSRV 1.85 (`rust-toolchain.toml`).
- `gpui-kit` 0.10 (pin de `gpui-pre` 0.3.12) para shell, dock, componentes base
  y test-support. Fijar versiones exactas (`=0.10.0`).
- `alacritty_terminal` 0.26 para VT y estado de pantalla.
- `portable-pty` 0.9 para PTY (ConPTY en Windows).
- `sysinfo` 0.40 para árbol de procesos.
- `toml` 1.x + `notify` 9 para configuración y hot reload.
- `serde`, `thiserror`, `anyhow`, `tracing`, `smallvec`, `unicode-width`.

Licencias: GPUI/gpui-kit y alacritty son Apache-2.0; portable-pty es MIT;
Tabler es MIT; simple-icons es CC0. Mantener `NOTICE` actualizado.

## Restricciones de producto (no negociables)

1. La UI y la terminal comparten tema. No hay colores literales fuera de
   `crates/ui/theme/`.
2. Todo tamaño (espaciados, alturas, radios, iconos) se deriva del tamaño base
   de la fuente. No hay píxeles mágicos fuera de `metrics.rs`.
3. El teclado manda: toda acción es alcanzable sin ratón y con combinaciones
   configurables.
4. El foco decide: con una terminal enfocada, una tecla sin binding va al PTY,
   nunca a la app.
5. Nunca bloquear el hilo de UI: PTY, disco, procesos y red viven fuera de él.
6. Windows primero. El código específico de plataforma va tras `cfg` o
   adaptadores, nunca dentro de la lógica de dominio.
7. Sin telemetría ni red por defecto. La detección de agentes es local.
8. Minimalismo: una acción primaria por superficie; nada que no comunique
   estado o permita actuar.

## Arquitectura

### Crates

- `crates/app` (`axfterm`) — binario, shell GPUI, acciones, paleta, wiring.
- `crates/ui` (`axf_ui`) — tokens, tema, iconos y componentes propios.
- `crates/terminal` (`axf_terminal`) — PTY, motor VT, input encoding,
  `TerminalElement`, selección.
- `crates/workspace` (`axf_workspace`) — proyectos, `PaneTree`, pestañas, foco,
  persistencia de sesión.
- `crates/agents` (`axf_agents`) — detección de procesos y máquina de estados.
- `crates/config` (`axf_config`) — esquema TOML, defaults, migraciones, reload.

Dependencias permitidas: `app -> {ui, terminal, workspace, agents, config}`;
`workspace -> {terminal, agents, config, ui}`; `ui` no conoce terminal ni
agentes; `agents` no conoce GPUI; `config` no conoce terminal.

### Modelo

```text
WindowState { projects: Vec<Project>, active_project, sidebars: SidebarState }
Project     { root, layout: PaneTree, terminals: HashMap<TerminalId, Terminal> }
Pane        { tabs: Vec<TabId>, active: TabId, ratio }
Tab         { terminal_id, title, agent: Option<AgentBinding> }
```

- `TerminalId` y `TabId` son estables y serializables para restaurar sesión.
- Una pestaña pertenece a un panel, pero puede moverse a otro sin recrear el
  PTY.
- Cerrar un panel con procesos vivos pide confirmación; cerrar el último panel
  cierra el proyecto.

### Hilos y datos

- Un hilo lector por PTY envía bytes a un parser en hilo dedicado.
- El parser publica `Snapshot + Damage` por canal al hilo UI.
- El hilo UI solo consume damage y pinta; nunca espera I/O ni hace `join`.
- Output masivo se coalesce: máximo un repintado por frame y el input siempre
  tiene prioridad.
- Resize = reflow + resize del PTY, con debounce por frame.

### Render de terminal

- `TerminalElement` implementa `Element` (request_layout / prepaint / paint).
- Fondos por runs en quads; texto por filas con shaping cacheado.
- Cache por fila: `(texto, attrs) -> ShapedLine`; invalidar solo filas dañadas.
- Cursor, selección, scrollbar y padding fuera del grid.
- Cambios de fuente, DPI o tema recalculan métricas y limpian caches.
- Presupuesto: frame < 8 ms a 80x24 y < 16 ms a 200x50.

### Teclado

- Contextos GPUI: `Workspace`, `Terminal`, `Palette`, `Sidebar`.
- Orden de dispatch: overlay/paleta -> keybinding del contexto -> terminal ->
  IME.
- Los atajos de app exigen modificador. `Ctrl+C/D/Z`, Tab, Esc y flechas quedan
  libres para la terminal.
- La config valida conflictos y teclas reservadas; `unbind` explícito. Nunca
  robar una tecla en silencio.
- IME: preedit + commit. En Windows, AltGr y dead keys no se escriben al PTY;
  se dejan al manejador de texto de la plataforma.

### Temas y fuentes

- Un tema = tokens UI + paleta ANSI 16 + cursor/selección + variante light/dark.
- Built-in: One Dark, Monokai, Cobalt2. Temas de usuario en TOML.
- Fuente de UI y fuente de terminal independientes; fallback por glifo (Nerd
  Font incluida).
- Iconos: Tabler SVG (outline, tintado por token) y marcas de simple-icons en
  color.
- Cambiar tema, fuente o tamaño base recalcula métricas y repinta sin reiniciar.

### Agentes

- Escaneo cada 1-2 s del árbol de procesos del PTY (PID raíz + `sysinfo`).
- Match por nombre/argv: `opencode`, `copilot` (evitar falsos positivos).
- Estados: `Idle`, `Working`, `NeedsAttention`, `Stopped`.
- Prioridad: `NeedsAttention` > `Working` > `Idle` > `Stopped`.
- Señales: bell, OSC 9/777/1337, tasa de salida, quiescencia, salida del
  proceso. La atención se mantiene hasta foco o confirmación.
- La UI solo lee `AgentBinding`; nunca bloquea esperando al proceso.
- Futuro (no v0.1): eventos del servidor/plugin de OpenCode.

### Persistencia

- Config: `%APPDATA%\axfterm\config.toml` y `themes/`. En Unix,
  `~/.config/axfterm/`; en macOS, `~/Library/Application Support/axfterm/`.
- Sesión: `session.json` junto a la config, con versión y migraciones.
- Si config o sesión están corruptas: backup, aviso y arranque con defaults.
- Hot reload de config con errores accionables, sin panics.

## Convenciones de código

- `rustfmt` por defecto; `snake_case`, `CamelCase`, crates con prefijo `axf_`.
- Errores tipados con `thiserror` en bordes; `anyhow` solo en `app` y tests.
- Sin `unwrap`/`expect` en rutas de runtime. Sin `unsafe` sin comentario de
  seguridad y justificación.
- `tracing` para logs; nada de `println!` fuera de ejemplos.
- `#[expect]` en vez de `#[allow]` cuando sea posible.
- Los comandos se registran una sola vez en el registro de acciones y de ahí
  salen keybinds, menús, paleta y documentación.
- Tests junto al módulo; fixtures en `tests/fixtures/`.

## Calidad

Comandos obligatorios antes de cerrar una tarea (PowerShell):

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

- Tests unitarios de parser, input encoding, config y máquina de estados.
- Tests de PTY con scripts fixture (PowerShell y sh).
- UI smoke tests con el test-support de gpui-kit.
- Benchmarks: arranque, throughput de output, resize, fuzzy de paleta.
- Criterio de hecho por tarea: compila, tests verdes, clippy limpio, docs o
  config actualizadas si aplica, y una línea en `progress.md`.

## Flujo de trabajo

1. Leer `.memory/README.md` y el milestone activo.
2. Trabajar una tarea coherente; no mezclar refactors ajenos.
3. Marcar `[~]` al empezar y `[x]` al terminar; añadir línea a `progress.md`.
4. No añadir dependencias sin justificar peso, licencia y mantenimiento.
5. Nunca borrar tareas: marcarlas como hechas o moverlas a `99-futuro`.

## Fuera de alcance v0.1

SSH/remoto, plugins de terceros, contenedores y puertos, split dentro de una
pestaña, ligaduras, kitty graphics/sixel, búsqueda en scrollback, daemon de
sesiones, worktrees por proyecto, auto-update y empaquetado macOS/Linux.

## Riesgos conocidos

- **GPUI es pre-1.0**: fijar versiones exactas y encapsular la API en
  `crates/ui` para poder cambiar de base.
- **gpui-kit pinea `gpui-pre`**: si una actualización rompe, el escape es
  `gpui-base` + componentes propios.
- **ConPTY + IME/AltGr**: validar con teclados ES y CJK desde M0.
- **Overflow y drag de pestañas**: geometría pura con tests; nada de lógica
  enterrada en el render.
- **Agentes heurísticos**: falsos positivos inevitables; el estado debe ser
  explicable y permitir override manual.
