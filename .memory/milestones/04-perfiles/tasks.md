# Tareas — M4 Perfiles

## Esquema y validación

- [x] `ProfileId` tipado y estable; `Profile { id, name, command, icon }`
- [x] `[[profiles]]` en la raíz y `[terminal] default_profile`
- [x] Validación: id único y no vacío, nombre y comando no vacíos, `icon`
      opcional, `default_profile` existe, límites de ADR 0005
- [x] Tokenizer de `command` → ejecutable + argv con comillas dobles/simples y
      barras invertidas literales (rutas Windows); sin expansión de shell ni
      operadores
- [x] Tests del tokenizer: rutas con espacios, comillas escapadas, argumentos
      vacíos y comandos inválidos
- [x] Migración config v1 → v2 y plantilla de primer arranque generada con los
      perfiles detectados

## Descubrimiento

- [x] `axf_config::profiles::discover` con búsqueda en PATH y rutas conocidas
- [x] Windows: `pwsh`, `powershell`, `cmd`, `nu` y Git Bash
- [x] WSL en segundo plano: `wsl -l -q` con timeout, salida UTF-16 y dedupe por
      distro
- [x] Unix: `$SHELL`, `zsh`, `bash`, `fish`, `nu`, `sh`
- [x] Icono por defecto asignado desde el ejecutable; fallback genérico
- [x] Primer arranque o config sin perfiles: detectar y escribir de forma
      atómica una sola vez; aviso accionable si la escritura falla
- [x] Perfil de respaldo por plataforma si no se detecta ninguno

## Terminal

- [x] `SpawnSpec { program, args }` en `axf_terminal`; eliminar los candidatos
      hardcodeados de `session.rs`
- [x] Arrancar exactamente el comando del perfil; sin fallback silencioso
- [x] Error accionable si el ejecutable no existe o el PTY falla
- [x] Título inicial de pestaña desde el nombre del perfil; OSC sigue pudiendo
      sustituirlo salvo título manual

## Workspace, sesión y app

- [x] `Tab.profile_id`; se conserva al mover pestañas y paneles
- [x] Sesión v2 y migración v1 → v2 con el perfil por defecto
- [x] Nueva pestaña con el perfil por defecto; duplicar y reiniciar conservan el
      perfil de la pestaña
- [x] Menú del panel: «Nueva terminal» y lista «con perfil» con icono y nombre;
      el predeterminado va marcado
- [x] Perfil borrado o inexistente en una pestaña viva: aviso y acciones claras
- [x] Hot reload: las pestañas nuevas usan los perfiles nuevos; las abiertas no
      se recrean
- [x] Perfil por defecto inválido en config: error de campo accionable

## Iconos

- [x] Spike S8: extraer el icono de un `.exe` en Windows (HICON → PNG con alpha)
- [x] Si el spike falla: icono genérico y ruta manual siguen funcionando
- [x] `icon` como ruta relativa al directorio de config o absoluta; PNG, JPG,
      ICO, SVG, BMP y TIFF
- [x] Caché `icons/cache/` con clave ejecutable + mtime; limpieza acotada
- [x] Componente de icono de perfil en pestaña, menú y paleta, con tamaño desde
      métricas
- [x] Fallback `icons/tabler/terminal-2.svg` si la imagen no carga

## Documentación y tests

- [x] ADR 0006 y `docs/profiles.md` (esquema, tokenizer, iconos, detección)
- [x] AGENTS.md y ADR 0005 actualizados con perfiles y sus límites
- [x] Tests de config, descubrimiento, migración, spawn y sesión
- [x] Gate manual: PowerShell 7, cmd, WSL; borrar perfil en uso; arrancar con
      sesión v1 y con config v1
