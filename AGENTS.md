# AGENTS.md — Axfterm

## Qué es

Axfterm es un terminal workspace nativo, rápido y keyboard-first para Windows
(macOS/Linux después), construido en Rust 2024 sobre GPUI. Reúne las terminales
de cada proyecto en una barra lateral, con paneles y pestañas en el área
central, y muestra agentes de IA (opencode y GitHub Copilot) con su estado en la
misma barra.

## Estado

v0.1 en desarrollo. La fuente de verdad del progreso es `.memory/`: leer
`.memory/README.md` antes de tocar código. Las decisiones estructurales están en
`.memory/adr/` (0001–0009) y no se contradicen desde el código.

## Stack fijado

- Rust stable, edition 2024, MSRV 1.85 (`rust-toolchain.toml`).
- `gpui-kit` 0.10 (pin de `gpui-pre` 0.3.12) para shell, dock, componentes base
  y test-support. Fijar versiones exactas (`=0.10.0`).
- `alacritty_terminal` 0.26 para VT y estado de pantalla.
- `portable-pty` 0.9 para PTY (ConPTY en Windows).
- `sysinfo` 0.40 para árbol de procesos.
- `toml` 1.x + `notify` 8.2.1 para configuración y hot reload. `notify` 9 exige
  Rust 1.91 y rompería la MSRV 1.85; se fija la misma versión que ya usa
  gpui-kit.
- `toml_edit` para que el panel de ajustes escriba `config.toml` sin perder
  comentarios ni orden.
- `windows-sys` (shell/GDI) y `png` para extraer y cachear el icono del
  ejecutable en Windows; ambos ya están en el árbol por gpui/portable-pty.
- `serde`, `thiserror`, `anyhow`, `tracing`, `smallvec`, `unicode-width`.

Licencias: GPUI/gpui-kit y alacritty son Apache-2.0; portable-pty es MIT;
Tabler es MIT; simple-icons es CC0; `toml_edit`, `windows-sys` y `png` son
MIT/Apache-2.0. Mantener `NOTICE` actualizado.

## Restricciones de producto (no negociables)

1. La UI y la terminal comparten tema. No hay colores literales fuera de
   `crates/ui/theme/`.
2. Todo tamaño (espaciados, alturas, radios, iconos) se deriva del tamaño base
   de la fuente. No hay píxeles mágicos fuera de `metrics.rs`. Los controles
   usan solo tres alturas en `rem`: `Xs` (1,25), `Sm` (1,5, por defecto) y `Md`
   (2), equivalentes a `XSmall`, `Small` y `Medium` de gpui-kit. La sidebar
   guarda su ancho en la base de 14 px y lo proyecta con el tamaño de fuente
   activo; los segmentados usan `SegmentedControl` y los switches
   `SwitchField`.
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
9. Los proyectos son una clasificación de la sidebar. No hay selector, pestañas
   ni breadcrumb de proyecto en el área central.

## Arquitectura

Decisiones estructurales registradas en `.memory/adr/`: 0001 motor VT/PTY,
0002 GPUI encapsulado, 0003 workspace y proyectos, 0004 config y versionado,
0005 límites de v0.1, 0006 perfiles de terminal, 0007 panel de ajustes y
0011 proyectos WSL y hosts de proyecto.

### Crates

- `crates/app` (`axfterm`) — binario, shell GPUI, acciones, paleta, panel de
  ajustes, wiring.
- `crates/ui` (`axf_ui`) — tokens, tema, iconos, extracción del icono de
  ejecutable y componentes propios.
- `crates/terminal` (`axf_terminal`) — PTY, motor VT, input encoding,
  `TerminalElement`, selección. Arranca un `SpawnSpec`; no elige shell.
- `crates/workspace` (`axf_workspace`) — proyectos, `PaneTree`, pestañas, foco,
  persistencia de sesión.
- `crates/agents` (`axf_agents`) — detección de procesos y máquina de estados.
- `crates/config` (`axf_config`) — esquema TOML, perfiles, descubrimiento,
  defaults, migraciones, escritura y reload.

Dependencias permitidas: `app -> {ui, terminal, workspace, agents, config}`;
`workspace -> {terminal, agents, config, ui}`; `terminal -> ui` (solo para los
tipos de GPUI que `axf_ui` reexporta como `axf_ui::gpui`, nunca `gpui-kit`
directo); `ui` no conoce terminal ni agentes; `agents` no conoce GPUI; `config`
no conoce terminal. **Solo `crates/ui` declara `gpui-kit`**; el resto, incluido
`app`, importa GPUI desde `axf_ui::gpui` (ADR 0002).

### Modelo

```text
WindowState { projects: Vec<Project>, layout: PaneTree,
              tabs: HashMap<TabId, Tab>, panes: HashMap<PaneId, Pane>,
              focus: Focus, sidebars: SidebarState }
Project     { id, root, host, default_profile, name }
SidebarState { visible, width, expanded: Set<ProjectId>, filter }
PaneTree    = Split { axis, ratio, first, second } | Leaf(PaneId)
Pane        { tabs: Vec<TabId>, active: Option<TabId> }
Tab         { project_id, terminal_id, profile_id, title,
              agents: Vec<AgentSession> }
Profile     { id, name, command, icon, agent_commands }
Focus       { pane: PaneId, tab: Option<TabId> }
```

- `TerminalId`, `PaneId`, `TabId` y `ProfileId` son estables y serializables
  para restaurar sesión; el contador de IDs también se persiste.
- Los perfiles viven en `config.toml`, no en la sesión: la pestaña guarda solo
  `profile_id`. `default_profile` debe existir; el estado "cero perfiles" no es
  válido.
- El orden de `projects` es el orden de la sidebar.
- `Project` es solo una clasificación de la sidebar: host, carpeta raíz,
  perfil de proyecto y nombre; sin layout ni terminales propios.
- `ProjectHost` es `Local` o `Wsl { distro }`. `root` se interpreta en el
  dialecto del host y solo `Local` se pasa a `std::fs`; un proyecto WSL usa una
  ruta POSIX y fija su perfil en `default_profile` (ADR 0011).
- Un panel puede mezclar pestañas de proyectos distintos. Una pestaña pertenece
  a un panel y a un proyecto, y puede moverse a otro panel sin recrear el PTY ni
  cambiar de proyecto.
- El proyecto actual se deriva de la pestaña enfocada (`focus`); no existe
  `active_project`.
- El foco apunta a un panel y a su pestaña activa. Un panel sin pestañas y el
  workspace vacío son estados válidos; solo el último panel puede quedar vacío,
  los demás se colapsan al cerrar su última pestaña.
- Cerrar un panel con procesos vivos pide confirmación. Cerrar la última
  pestaña deja el proyecto vacío en la sidebar; quitarlo es una acción explícita
  que cierra sus pestañas, con confirmación si hay procesos vivos.

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

- Contextos GPUI: `Workspace`, `Terminal`, `Palette`, `Sidebar`, `Settings` y
  `WslProject`.
- Orden de dispatch: overlay/paleta/panel de ajustes -> keybinding del contexto
  -> terminal -> IME.
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

### Perfiles

- `config.toml` (schema v2) declara `[[profiles]]` con `id`, `name`, `command`
  e `icon` opcional, más `[terminal] default_profile`.
- `command` es una línea que se tokeniza a ejecutable + argv; sin shell, sin
  variables de entorno y sin operadores. El terminal recibe `SpawnSpec` y no
  elige shell por su cuenta.
- Sin `icon`, en Windows se extrae el icono del ejecutable y se cachea; si no,
  se usa el fallback de Tabler. La extracción es un adaptador de plataforma en
  `axf_ui`.
- Primer arranque o config sin perfiles: descubrimiento local (PATH, WSL en
  segundo plano) y escritura atómica una sola vez. Después manda el archivo.
- Un perfil inexistente o un ejecutable que no arranca es un error visible en
  la pestaña, nunca un fallback silencioso a otro shell.
- `[profiles.agent_commands]` permite sobrescribir el comando base de un agente
  en ese perfil (`opencode = "opencode2"` en WSL), validado como una línea de
  ejecutable + argumentos. Sin override se usan `opencode` y `copilot`; la
  reanudación añade `--continue`.
- La pestaña conserva su `profile_id` al moverse, duplicarse y restaurarse; el
  título inicial es el nombre del perfil.

### Agentes

- OSC 7501 es la fuente principal de estado; el árbol de procesos del PTY
  (PID raíz + `sysinfo`) atribuye el agente a su pestaña y sirve de fallback.
- Match por nombre/argv: `opencode`/`opencode2`, `copilot`/`copilot-runtime`
  (evitar falsos positivos y podar procesos ayudantes).
- Cada proceso de agente es una `AgentSession`; una pestaña puede tener varias.
  El estado es runtime-only y no se persiste en la sesión.
- Estados: `Idle`, `Working`, `NeedsAttention`, `Finished`, `Error`, `Stopped`.
- Prioridad: `NeedsAttention` > `Working` > `Error` > `Finished` > `Idle` >
  `Stopped`. Atención, terminado y error se limpian al enfocar.
- Señales: OSC 7501, bell, OSC 9/777/1337, tasa de salida, quiescencia y salida
  del proceso. El escaneo y el parser viven fuera del hilo de UI.
- La UI solo lee `AgentSession`; nunca bloquea esperando al proceso.
- Futuro (no v0.1): eventos del servidor/plugin de OpenCode.

### Persistencia

- Config: `%APPDATA%\axfterm\config.toml` y `themes/`. En Unix,
  `~/.config/axfterm/` (o `$XDG_CONFIG_HOME/axfterm/`); en macOS,
  `~/Library/Application Support/axfterm/`. `AXFTERM_CONFIG_DIR` solo para tests
  y diagnóstico.
- Config y sesión llevan versión de esquema (`schema_version` / `version`) con
  migraciones encadenadas; una versión futura se rechaza con error, no se
  degrada. Escribir siempre es atómico (temporal + rename).
- Config v2 incluye los perfiles; sesión v3 incluye el host y el perfil de cada
  proyecto, además del `profile_id` de cada pestaña. Las migraciones v1 → v3 se
  conservan y se prueban en actualización.
- Sesión: `session.json` junto a la config, con versión y migraciones.
- Historial de terminal: un archivo ANSI por terminal en `scrollback/`, con
  `epoch` por ejecución, checksum y tope de 2 MiB (ADR 0008). Se guarda en cierre
  ordenado y se restaura de forma perezosa al activar cada pestaña; un archivo
  inválido cae al mismo perfil desde cero con aviso.
- Panel de ajustes: borrador validado, escritura con `toml_edit` preservando
  comentarios y `write_atomic`. Ante conflicto con un cambio externo se avisa y
  se pide decisión; nunca se pisa en silencio. No hay almacén paralelo.
- Si config o sesión están corruptas: backup fechado, aviso y arranque con
  defaults. Un error de hot reload conserva la última config válida.
- Logs normales opt-in con `AXFTERM_LOG` en `<config>/logs/axfterm.log`
  (rotación 2 MiB). Crash reports locales acotados a 10; nunca se envían.
- Hot reload de config con errores accionables, sin panics. Las escrituras del
  propio panel no se aplican dos veces.
- Límites de terminales, perfiles, scrollback, pegado e iconos: ADR 0005.
  `terminal.scrollback_lines` (0–50 000) se recorta por el tope de 32 MiB por
  terminal según el ancho; el slider afecta a terminales nuevas.
- Instancia única: la segunda instancia reenvía sus carpetas por un named pipe
  local (sin red) y sale. El MSI registra «Abrir con Axfterm» en Explorer.

## Convenciones de código

- `rustfmt` por defecto; `snake_case`, `CamelCase`, crates con prefijo `axf_`.
- Los archivos Rust de producción bajo `crates/*/src` no deben superar 500
  líneas; dividir responsabilidades en módulos cohesivos.
- Errores tipados con `thiserror` en bordes; `anyhow` solo en `app` y tests.
- Sin `unwrap`/`expect` en rutas de runtime. Sin `unsafe` sin comentario de
  seguridad y justificación.
- `tracing` para logs; nada de `println!` fuera de ejemplos.
- `#[expect]` en vez de `#[allow]` cuando sea posible.
- Los comandos se registran una sola vez en el registro de acciones y de ahí
  salen keybinds, menús, paleta y documentación.
- Tests junto al módulo; fixtures en `tests/fixtures/`.

## Herramientas de desarrollo

- `scripts/package.ps1` compila release y genera el zip portable y el MSI
  per-user en `dist/`, con hashes. WiX se ejecuta como herramienta local de
  dotnet (`.config/dotnet-tools.json`). Ver `docs/release.md`.
- `scripts/make-icon.ps1` regenera `crates/app/assets/axfterm.ico` desde la
  marca vectorial; no requiere herramientas externas.
- `scripts/studio.ps1` (feature Cargo `studio`) navega la app, abre la galería
  del sistema de diseño y captura PNGs en `target/studio`. Nunca se activa en
  release y usa un perfil aislado. Ver `docs/studio.md`.
- `scripts/devtools.ps1` es el cliente JSONL del servidor local de Studio.
- La galería reutilizable vive en `crates/ui/src/gallery/` y se abre con
  `cargo run -p axf_ui --features gallery --example gallery`.

## Calidad

Comandos obligatorios antes de cerrar una tarea (PowerShell):

```powershell
pwsh -File scripts/check.ps1          # fmt + clippy + test
pwsh -File scripts/cargo-low.ps1 build --release
pwsh -File scripts/test-cargo-low.ps1 # pruebas del wrapper
```

`scripts/check.ps1` también verifica que ningún archivo Rust de producción bajo
`crates/*/src` supere 500 líneas.

Equivalen a `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` y `cargo build --release`, con menor carga: Cargo usa
un job y los tests un hilo por defecto. El wrapper aplica prioridad
`BelowNormal` y serializa sus ejecuciones por workspace en la sesión de Windows.
**Usar el wrapper en vez de lanzar `cargo` directamente**; Cargo directo respeta
la configuración del workspace, pero no la prioridad ni el bloqueo del wrapper.
Ejecutar el wrapper desde la raíz del workspace; su bloqueo corresponde al
workspace que contiene el script, incluso si se pasa otro `--manifest-path`.
Se puede aumentar el paralelismo con `AXF_CARGO_JOBS` o `CARGO_BUILD_JOBS` y los
hilos de tests con `RUST_TEST_THREADS`, bajo responsabilidad del usuario.
Los límites no restringen la memoria, el disco ni los hilos internos del linker,
y un Cargo directo o un proceso huérfano puede coincidir con el wrapper.

- Tests unitarios de parser, input encoding, config y máquina de estados.
- Tests de perfiles: tokenizer de `command`, descubrimiento con PATH falso,
  migraciones v1 → v2 y escritura que conserva comentarios.
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
sesiones, worktrees por proyecto, auto-update y empaquetado macOS/Linux. Los
proyectos en WSL sí entran en v0.1 (M8, ADR 0011), pero SSH, Docker y Podman no.
También quedan fuera: ventana única sin pestañas desacoplables, perfiles por
proyecto, variables de entorno por perfil, extracción nativa de iconos en
macOS/Linux, shell integration (OSC 133), ARM64 y sincronización de
config/temas. Los números y el comportamiento al alcanzar un límite están en
ADR 0005. El panel de ajustes sí entra en v0.1 (M7): la config sigue siendo el
archivo y el panel escribe en él.

## Riesgos conocidos

- **GPUI es pre-1.0**: fijar versiones exactas y encapsular la API en
  `crates/ui` para poder cambiar de base.
- **gpui-kit pinea `gpui-pre`**: si una actualización rompe, el escape es
  `gpui-base` + componentes propios.
- **ConPTY + IME/AltGr**: validar con teclados ES y CJK desde M0.
- **Overflow y drag de pestañas**: geometría pura con tests; nada de lógica
  enterrada en el render.
- **Parser de la línea de comandos**: barras invertidas y rutas con espacios;
  reglas documentadas y tests antes de tocar el spawn.
- **Iconos de ejecutable**: APIs Win32 y DPI variable; spike previo, caché
  acotada y fallback Tabler siempre disponible.
- **Panel de ajustes y hot reload**: dos escritores sobre el mismo archivo;
  borrador, detección de conflicto y nunca sobrescribir en silencio.
- **Agentes con OSC 7501 y fallback de procesos**: los CLIs antiguos no reportan
  estado y pueden aparecer falsos positivos en el matcher; el estado debe ser
  explicable, medible y permitir override manual.
- **Proyectos WSL y `--cd`**: depende de la versión de WSL y de que el perfil
  apunte a la distro correcta. WSL consume una barra invertida al reenviar argv
  (`\0` de `find -printf` necesita `\\0`); validar en Windows 10/11 y WSL1/WSL2.
  Los agentes dentro de WSL dependen de OSC 7501 porque `sysinfo` no ve Linux.
- **Historial persistido**: contiene el texto que se mostró en la terminal,
  incluidos posibles secretos, y solo se guarda en cierre ordenado. Se almacena
  bajo el perfil del usuario, sin cifrar ni sincronizar (ADR 0008).
