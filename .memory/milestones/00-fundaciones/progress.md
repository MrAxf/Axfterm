# Progreso — M0 Fundaciones

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
