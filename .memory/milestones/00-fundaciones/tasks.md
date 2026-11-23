# Tareas — M0 Fundaciones

## Repo

- [x] `git init` + `.gitignore` (`target/`, `dist/`, `*.log`, `.idea/`, `.DS_Store`)
- [x] `rust-toolchain.toml` con stable y edition 2024
- [x] `Cargo.toml` de workspace con los 6 crates vacíos
- [x] `rustfmt.toml` y lints de workspace (clippy)
- [x] `scripts/check.ps1` con fmt + clippy + test
- [x] `LICENSE` y `NOTICE` con dependencias y assets de terceros

## Shell GPUI

- [x] `main.rs` con gpui-kit: ventana 1280x800, tema oscuro base, quit limpio
- [x] `AssetSource` embebido con iconos Tabler vendorizados
- [x] Layout placeholder: sidebar izquierda vacía + área central + status bar
- [x] Acción de ejemplo + keybinding mínimo para validar el dispatch

## Spikes

Orden: PTY → VT → render → teclado/IME → clipboard → iconos, con PID tree en
paralelo. Cada spike vive en `crates/<crate>/examples/` (sin crate nuevo), cierra
con una línea de resultados en `progress.md` y alimenta un ADR. Al promover el
código a M1, el ejemplo se borra o queda como herramienta de diagnóstico.

Decisiones de arranque: GPUI se consume desde `axf_ui` (`pub use gpui_kit as
gpui`), no con dependencia directa; las versiones quedan fijadas con `=`;
las mediciones del spike se toman con `Instant` y `criterion` llega en M1.

- [x] S1 PTY (`axf_terminal/examples/pty_smoke.rs`): spawn de `pwsh`, reader
      thread, write, resize ConPTY, exit code, EOF y kill; latencia
      spawn→primer byte y throughput. Dep: `portable-pty =0.9.0`.
      Resultado: spawn ~60 ms, round-trip 19–24 ms, 1,2 MiB/s crudo; DSR
      obligatorio y con estado entre lecturas. Ver `progress.md`.
- [x] S2 PID tree (`axf_terminal/examples/pid_tree.rs`): `Child::process_id()` +
      `sysinfo =0.40.4` limitado al subárbol; cadena `pwsh → pwsh` como sustituto
      de `pwsh → opencode → node`; coste de refresco y falsos positivos.
      Resultado: 3 niveles, refresh(All) p95 42 ms, BFS p95 9 ms, 2 huérfanos al
      matar la raíz. API para `axf_agents` sin PTY ni GPUI.
- [x] S3 VT (`axf_terminal/examples/vt_dump.rs`): `Term` +
      `vte::ansi::Processor`; fixtures de SGR 16/256/truecolor, alt screen,
      cursor/DECTCEM, wide chars, resize/reflow, scrollback y `damage()`.
      `EventListener` con `PtyWrite` (respuesta a DSR), `TextAreaSizeRequest` y
      `ColorRequest`. El ejemplo falla con exit≠0 si una comprobación no cuadra.
      Resultado: 8 escenarios, 0 fallos.
- [x] S4 Render (`axf_terminal/examples/grid_bench.rs`): `Element` con quads y
      shaping cacheado por fila; 80x24 y 200x50, daño completo e incremental;
      p50/p95 de CPU de prepaint+paint (no FPS), presupuesto 8/16 ms.
      Resultado (release): p95 1,0 ms a 80x24 y 2,1 ms a 200x50; paint domina y
      el clon por fila cuesta más que shapear.
- [x] S5 Teclado/IME (`crates/app/examples/keylog.rs`): `KeyDownEvent`
      (`key`, `key_char`, `prefer_character_input`), `Ctrl+C`, flechas, AltGr ES,
      dead keys e IME preedit/commit vía `EntityInputHandler` + `handle_input`.
      Cierre: tabla `Keystroke → bytes PTY` y huecos de plataforma.
      Resultado: verificado en ventana real (`Esc` → `0x1b`, pegado de 37 bytes
      con `Ctrl+Shift+V`, copia del buffer IME); tabla impresa en el ejemplo.
- [x] S6 Clipboard (`axf_terminal/src/paste.rs` + S5): normalización
      multilínea, bracketed paste (DECSET 2004), CRLF y NUL; tests unitarios.
      Resultado: 9 tests en verde.
- [x] S7 Iconos (`axf_ui`): Tabler tintado por token y marca simple-icons a
      color, con la excepción de colores de marca documentada.
      Resultado: 6 assets verificados (Tabler con `foreground`/`accent`,
      `rust.svg` monócromo y en `#F74C00`).

## Decisiones (ADR)

Detalle en `.memory/adr/`. Cada decisión cerrada aquí alimenta ese registro.

- [x] ADR 0001: motor VT/PTY (alacritty_terminal + portable-pty)
- [x] ADR 0002: gpui-kit con escape a gpui-base
- [x] ADR 0003: workspace con layout global y proyectos como clasificación de la
      sidebar (foco derivado, quitar proyecto explícito)
- [x] ADR 0004: config TOML, rutas y versionado
- [x] ADR 0005: límites de v0.1

## Gate

- [x] Demo: abrir terminal, ejecutar `opencode`, redimensionar y cerrar sin artefactos
- [x] Registro de mediciones iniciales en `progress.md` (S1–S4, release)
