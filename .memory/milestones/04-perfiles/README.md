# M4 — Perfiles de terminal

Objetivo: que cada terminal se abra con un perfil declarado en `config.toml`
(nombre, línea de comandos e icono), con un perfil por defecto y detección
automática de las terminales instaladas en el primer arranque.

## Criterio de hecho

- [x] Un perfil declarado en config abre una pestaña con su comando y su icono.
- [x] El perfil por defecto se usa al crear, duplicar y reiniciar pestañas.
- [x] Primer arranque sin perfiles detecta shells nativos y WSL y los escribe.
- [x] Un comando o perfil inválido da error accionable, sin fallback silencioso.
- [x] Sesión v1 migra a v2 conservando pestañas, paneles y foco.
- [x] Icono propio o del ejecutable; fallback Tabler siempre visible.
- [x] Tests, clippy, fmt y release en verde; gate manual con PowerShell, cmd y
      WSL.

## Resultado

- `config.toml` v2: `[terminal] default_profile` y `[[profiles]]` con validación
  de id, nombre, comando, límites de ADR 0005 y default existente.
- Tokenizer propio sin shell (`crates/config/src/profiles.rs`), con rutas
  Windows, comillas y argumentos vacíos cubiertos por tests.
- Descubrimiento local y `wsl -l -q` en segundo plano con timeout; escritura
  atómica y `append_profiles` que conserva comentarios.
- `SpawnSpec` sustituye los candidatos hardcodeados de `axf_terminal`; un
  ejecutable ausente muestra el comando y acciones de reintento/perfil por
  defecto.
- `Tab.profile_id` + `auto_title`, sesión v2 y migración v1 → v2 con el perfil
  por defecto.
- Iconos: ruta manual, extracción HICON → PNG con alpha (spike S8), caché
  acotada por ejecutable + mtime y fallback Tabler.
- Menú «⋯» del panel con submenú «Con perfil», icono, nombre y predeterminado
  marcado. Hot reload solo afecta a pestañas nuevas.
- `docs/profiles.md` y ADR 0006/0005 documentan esquema, tokenizer, iconos,
  detección y límites.
