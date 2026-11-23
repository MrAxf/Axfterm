# Progreso — M0 Fundaciones

## Entorno de medición

Windows 11 (10.0.26300), AMD Ryzen 7 7730U, 16 GB. Las mediciones de render son
CPU de prepaint+paint (no FPS: GPUI va con vsync), en perfil `release` salvo que
se indique `debug`.

## Spikes

- 2026-11-23: **Cargo con menor carga**. El workspace limita Cargo a un job y
  los tests a un hilo; `cargo-low.ps1` mantiene prioridad `BelowNormal` y
  serializa los wrappers del mismo workspace en la sesión de Windows. Los
  overrides siguen disponibles y `scripts/test-cargo-low.ps1` prueba el wrapper.
  Los límites no garantizan la ausencia de bloqueos ni serializan Cargo directo.
- 2026-11-22: **Tooling de build OK**. `scripts/cargo-low.ps1` ejecuta cargo a
  prioridad `BelowNormal` (heredada por rustc y el linker) con `-j 4`, y
  `.cargo/config.toml` fija `build.jobs = 4`; `scripts/check.ps1` usa el wrapper.
  Antes, fmt/clippy/test lanzaban 16 `rustc` en un APU de 15 W y congelaban el
  escritorio. Verificado: cargo/rustc en `BelowNormal` durante un `check`.

- 2026-11-21: **S3 VT OK** (`axf_terminal/examples/vt_dump.rs`, 8 escenarios,
  0 fallos). Confirma SGR 16/256/truecolor, alt screen (1049 conserva la posición
  del cursor), CUP/DECTCEM, wide chars + spacer, resize con reflow
  (`FLAG_WRAPLINE`), scrollback y `damage()` (Full tras resize/scroll, Partial por
  línea al escribir). Las respuestas al PTY funcionan: DSR `6n`, tamaño en
  caracteres `18t` (PtyWrite), tamaño en píxeles `14t` (TextAreaSizeRequest) y
  OSC 10 (ColorRequest). `vte::ansi::Processor` + `Term::new` es el camino.
- 2026-11-21: **S2 PID tree OK** (`axf_terminal/examples/pid_tree.rs`). Cadena
  real de 3 procesos (pwsh → pwsh → pwsh). `refresh_processes(All)` p50
  13,5 ms / p95 42,5 ms; BFS + `refresh(Some)` p50 0,14 ms / p95 8,8 ms. Matar
  la raíz deja 2 huérfanos vivos: `axf_agents` debe escanear el subárbol completo
  y el cierre de pestaña debe matarlo. 4 procesos del sistema casan con nombres
  de agente, así que el match se limita al subárbol del PTY.
- 2026-11-21: **S1 PTY OK** (`axf_terminal/examples/pty_smoke.rs`). Spawn de
  `pwsh` 54–69 ms, primer byte 8–10 ms después, round-trip de comando 19–24 ms
  en régimen (la primera orden incluye ~0,5 s de PSReadLine). Throughput crudo
  (stdout binario) 1,14 MiB en 0,93 s ≈ 1,2 MiB/s; con `[Console]::Out` de
  PowerShell baja a ~45 KiB/s, así que el cuello es el host de PowerShell y no
  ConPTY. `resize` 80x24 → 100x30 verificado, `exit 7` propagado, `kill()`
  termina el proceso. Hallazgo crítico: `pwsh` emite DSR (`ESC[6n`) al arrancar
  y se bloquea sin respuesta; además la secuencia puede cruzar dos lecturas, así
  que el respondedor debe llevar estado entre chunks (en M1 lo genera el
  `EventListener` de alacritty).
- 2026-11-21: **S4 Render OK con margen** (`axf_terminal/examples/grid_bench.rs`,
  release). 80x24: total p50 0,44 ms / p95 1,01 ms (presupuesto 8 ms). 200x50:
  total p50 1,84 ms / p95 2,11 ms (presupuesto 16 ms); paint 1,47 ms y shaping
  0,36–0,73 ms. En debug los mismos casos dan 3,4 ms y 12,0 ms: el perfil manda.
  El paint domina y el "incremental" no gana porque clonar un `ShapedLine` por
  fila cuesta más que shapear (GPUI ya cachea layouts por texto+runs). Para M1:
  cache persistente por índice de fila invalidada por damage, sin hash por frame
  y pintando desde la cache sin clonar; GPUI reconstruye la escena entera, el
  damage ahorra shaping, no pintado.
- 2026-11-21: **S6 Clipboard OK** (`axf_terminal/src/paste.rs`, 9 tests).
  `normalize_paste` convierte `\r\n`/`\r`/`\n` a `\r` sin bracketed paste, deja
  `\n` y envuelve con `ESC[200~`/`ESC[201~` cuando la app lo pide (DECSET 2004),
  descarta NUL y conserva tabs, Unicode y backslashes de rutas Windows.
- 2026-11-21: **S5 Teclado/IME OK** (`crates/app/examples/keylog.rs`).
  Verificado en ventana real: `Esc` → `0x1b`; `Ctrl+Shift+V` pegó el comando de
  37 chars como 37 bytes tras `normalize_paste` (sin corromper espacios ni
  guiones); `Ctrl+Shift+C` copió el buffer IME (vacío, 0 chars). El ejemplo
  registra `KeyDownEvent` (key, key_char, mods, prefer_character_input) y el IME
  vía `EntityInputHandler` + `ElementInputHandler`/`window.handle_input`, y
  imprime la tabla `Keystroke → bytes PTY` para AltGr, flechas, dead keys e IME.
  Nota para M1: el mapeo `Ctrl+letra` de la tabla ignora Shift; `Ctrl+Shift+C`
  se resuelve como acción de app (copiar) y no debe llegar como `0x03`. La
  precedencia binding de app → terminal es la que fija AGENTS.md.
- 2026-11-21: **S7 Iconos OK** (`axf_ui/examples/icons.rs`). Los 6 assets cargan
  y se pintan: Tabler `terminal-2`/`layout-sidebar` con `foreground`,
  `settings`/`plus` con `accent`, `rust.svg` monócromo y `rust.svg` con el color
  de marca `#F74C00` desde `crates/ui/src/brand.rs` (excepción documentada).
  `rust.svg` vendorizado de simple-icons (CC0), tests de assets en verde.

## Bloques

- 2026-11-21: **Corrección del gate: líneas de render, fuente y ratón**. El
  volcado de celdas (`AXFTERM_TRACE_CELLS`, F12) descartó subrayados y colores
  dinámicos. Las líneas de 1 px venían de la altura de línea: `font_size * 1.25`
  (16,25 px) dejaba huecos entre glifos de bloque y bordes de caja. Ahora se
  deriva de la fuente como `ascent − descent` (GPUI devuelve el descent
  negativo): Consolas 13 px → 15,22 px, exactamente la altura de `█`. Se añadió
  guardia monoespaciada (avance de `M` vs `i`) con fallback a Cascadia Mono,
  Consolas o Courier New. Ratón: tracking DECSET 1000/1002/1003, SGR 1006 y
  X10, clic/arrastre/movimiento/rueda con modificadores y tests; la dirección
  de la rueda se invirtió tras la prueba manual. Diagnóstico disponible por
  entorno: `AXFTERM_TRACE_PTY`, `AXFTERM_TRACE_CELLS`, `AXFTERM_TRACE_FONT`,
  toggles de render y `cargo run -p axf_terminal --example trace_replay`.
  `fmt`, clippy, 25 tests de terminal y release en verde. Pendiente: selección,
  scrollback con rueda, focus in/out y overlay de preedit.

- 2026-11-21: **Gate M0 OK — terminal real en la app**. `axfterm` monta un
  `TerminalView` sobre `PtySession` (hilo lector + `ParserCommand` acotado),
  hilo parser con `Term`/`Processor` que publica frames con damage, y
  `TerminalElement` con shaping cacheado por fila, fondo por runs y cursor. La
  entrada imprimible va por el `EntityInputHandler` (IME/WM_CHAR) y las teclas
  de control/secuencias por `input::encode`; duplicar ambos caminos duplicaba
  cada tecla y se corrigió con `is_text_input`. Verificado con `opencode 2.3.2`:
  TUI en alt screen, texto y backspace, resize 145×44 → 104×35 → 143×43
  columnas en ConPTY con reflow sin artefactos, y cierre de ventana con
  `opencode` vivo: `taskkill /T /F` sobre el árbol deja 0 procesos del
  subárbol. El camino tecla→PTY se comprobó con un `echo` a archivo y `Ctrl+C`
  real produjo `^C` y un prompt nuevo. La ventana recorta su tamaño inicial al
  área visible del monitor (DPI 150 % aquí).
  `fmt`, clippy, `test --workspace` (17 tests de terminal) y release en verde.
  El cierre usa `taskkill` síncrono una sola vez, fuera del camino caliente.
  Pendiente para M1: selección/copia, scrollback visible, overlay de preedit,
  ratón, árbol de procesos en Unix y validación con teclado real de AltGr/IME
  en la vista final.

- 2026-11-21: bloque Decisiones (ADR) completado. ADR 0001–0005 en
  `.memory/adr/` (motor VT/PTY, GPUI encapsulado con escape, workspace y
  proyectos, config/rutas/versionado y límites de v0.1). `AGENTS.md` alineado:
  solo `axf_ui` declara `gpui-kit`, modelo con `PaneId`/`Focus`/paneles vacíos,
  versionado de config/sesión y fuera de alcance ampliado. `app` pasó a importar
  GPUI desde `axf_ui::gpui` y ya no depende de `gpui-kit`; el derive de
  `actions!` necesita un alias `gpui` en scope. `fmt`, clippy, test y release en
  verde.

- 2026-11-21: bloque Shell GPUI completado. `gpui-kit =0.10.0` (fija
  `gpui-pre 0.3.12`), ventana 1280x800 centrada con tema oscuro, `AssetSource`
  propio con 4 iconos Tabler vendorizados (fallback a los assets de gpui-kit),
  shell placeholder (sidebar + centro + status bar) y acciones `ToggleSidebar`
  (`Ctrl+B`) y `Quit` (`Ctrl+Q`) con contexto `Workspace`. Tests de assets en
  verde; ventana verificada manualmente. `fmt`, clippy, test y release en verde.
- 2026-11-21: bloque Repo completado. `git init` en `main`, `.gitignore`,
  `rust-toolchain.toml` (stable), workspace Rust 2024 (MSRV 1.85) con los 6
  crates (`axfterm`, `axf_ui`, `axf_terminal`, `axf_workspace`, `axf_agents`,
  `axf_config`), `rustfmt.toml`, lints de workspace, `scripts/check.ps1`,
  licencia dual MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`) y `NOTICE`.
  `fmt`, `clippy -D warnings`, `test --workspace` y `build --release` en verde.
- 2026-11-21: creados `AGENTS.md` y `.memory` con hitos, tareas y criterios de hecho.
