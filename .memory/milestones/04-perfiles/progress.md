# Progreso — M4 Perfiles

## 2026-11-22 — Corrección: PowerShell 7 carga el perfil del usuario

- El descubrimiento de `pwsh` ya no añade `-NoProfile`; solo `-NoLogo`. Así se
  respeta `$PROFILE`, igual que en Windows PowerShell. Test de regresión con un
  `pwsh` falso en `PATH`; `docs/profiles.md` y ADR 0006 actualizados. La config
  existente se corrige a mano porque el descubrimiento no se repite si ya hay
  perfiles.

## 2026-11-22 — M4 cerrado

- `axf_config`: `ProfileId`/`Profile`, esquema v2, tokenizer sin shell,
  validación de 32 perfiles/64/1024, migración v1 → v2 con `toml_edit`
  conservando comentarios y `append_profiles` sin duplicados.
- Descubrimiento: PATH y rutas conocidas en Windows (`pwsh`, `powershell`,
  `cmd`, `nu`, Git Bash) y Unix (`$SHELL`, zsh, bash, fish, nu, sh). WSL se
  consulta en segundo plano con timeout de 1,5 s y salida UTF-16/UTF-8; se
  fusiona una sola vez en la config.
- `axf_terminal`: `SpawnSpec { program, args }`; se eliminaron los candidatos
  hardcodeados. Un fallo de arranque conserva el comando y el perfil para la UI.
- `axf_workspace`: `Tab.profile_id` y `auto_title`, sesión v2 y migración v1 → v2
  con el perfil por defecto. Duplicar y mover conservan el perfil.
- `axf_ui`: `ProfileIcon`/`ProfileIconSource`, ruta manual (2 MiB), extracción
  HICON → PNG RGBA (spike S8), caché `icons/cache` de 64 archivos por ejecutable
  + mtime y fallback Tabler.
- `app`: pestañas nuevas con el perfil por defecto; menú del panel con lista
  «Con perfil» e icono; pestaña con perfil borrado ofrece «Usar perfil
  predeterminado»; hot reload solo afecta a pestañas nuevas; WSL se añade sin
  bloquear el primer frame.
- Verificación: `cargo fmt`, clippy `-D warnings`, tests y release en verde.
  Gate manual con `AXFTERM_CONFIG_DIR` temporal: la plantilla incluyó pwsh,
  Windows PowerShell, cmd, Git Bash y las distros WSL; arrancar con
  `default_profile` pwsh/cmd/wsl-ubuntu creó el proceso esperado. Config v1 +
  sesión v1 migraron a v2 con `profile_id` conservando paneles y foco.
- Documentación: `docs/profiles.md`; `docs/themes.md` pasa a v2; NOTICE añade
  `toml_edit`, `windows-sys` y `png`.
