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

- [ ] PTY: spawn de `pwsh`, leer stdout, escribir, resize y exit
- [ ] PID tree en ConPTY: PID raíz y descendientes vía `sysinfo`
- [ ] VT: `alacritty_terminal` parsea SGR, alt screen y cursor
- [ ] Render: `TerminalElement` con grid 200x50 a 120 Hz; medir frame y shaping
- [ ] Teclado: `Ctrl+C`, flechas, AltGr, dead keys e IME preedit/commit
- [ ] Clipboard: copiar/pegar texto y pegado multilínea
- [ ] Iconos: Tabler tintado + marca de simple-icons a color

## Decisiones (ADR)

- [ ] ADR 0001: motor VT/PTY (alacritty_terminal + portable-pty)
- [ ] ADR 0002: gpui-kit con escape a gpui-base
- [ ] ADR 0003: modelo proyectos/paneles/pestañas
- [ ] ADR 0004: config TOML, rutas y versionado
- [ ] ADR 0005: límites de v0.1

## Gate

- [ ] Demo: abrir terminal, ejecutar `opencode`, redimensionar y cerrar sin artefactos
- [ ] Registro de mediciones iniciales en `progress.md`
