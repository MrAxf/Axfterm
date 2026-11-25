# Tareas — M8 Proyectos en WSL

Leyenda: `[ ]` pendiente · `[~]` en curso · `[x]` hecho.

## Preparación

- [x] Spike S9: `wsl -d <distro> --cd <ruta>` bajo ConPTY con resize, Ctrl+C,
      Unicode y cierre del árbol (prueba `real_wsl_conpty_spawn`).
- [x] ADR 0011 y renumeración de milestones (M8 nueva; anterior M8 → M9,
      anterior M9 → M10).
- [x] Actualizar `.memory/README.md`, `tasks.md`, enlaces y referencias de ADRs.

## Modelo y sesión

- [x] `ProjectHost { Local, Wsl { distro } }` en `axf_workspace`.
- [x] `Project.host` y `Project.default_profile` con `serde(default)`.
- [x] Registrar/abrir proyectos con host; canonicalizar solo local y deduplicar
      por `(host, root)`.
- [x] Reparar sesiones por host y no normalizar rutas WSL con reglas Windows.
- [x] `SESSION_VERSION = 3`; migración v2 → v3 y tests de round-trip.
- [x] Tests de deduplicación, nombre POSIX y proyecto WSL sin tocar disco.

## Perfiles y adaptador WSL

- [x] `Profile::wsl_distro()`: tokeniza el comando, reconoce `wsl`/`wsl.exe` y
      extrae `-d`/`--distribution`.
- [x] Listado de distros reutilizable desde config, con timeout y UTF-16.
- [x] Adaptador `wsl` en la app: ejecutar sin ventana, timeout, decodificar
      salida y errores.
- [x] Validar ruta con `realpath -e` o `test -d` + `pwd -P`; no fiarse del
      código de `pwd` cuando `--cd` falla.
- [x] Resolver home con `--cd ~ -- pwd -P`.
- [x] Listar subcarpetas con `find -L ... -printf` y fallback `ls -1p`.
      `wsl.exe` consume una barra invertida: se envían `\\0` para obtener NUL.
- [x] Inyectar `--cd` antes de `--`/`-e`/`--exec`, reemplazando uno existente.
- [x] Tests de argumentos, espacios, UTF-8, distro incorrecta y perfil no WSL.

## Lanzamiento

- [x] `SpawnTarget { host, path }` y `spawn_terminal_with_target`.
- [x] Perfil de proyecto para nueva terminal, split, duplicado y `+` del proyecto.
- [x] Ocultar «Con perfil» en proyectos WSL.
- [x] Errores accionables si el perfil del proyecto falta o no es WSL.
- [x] CWD vivo de OSC 7: se ignora para WSL; duplicar usa la ruta del proyecto.

## UI

- [x] `DropdownButton` en sidebar y onboarding: acción local + caret.
- [x] Menú «Carpeta local…» / «Proyecto en WSL…» con atajos.
- [x] Acción `project.open_wsl`, registro, paleta, atajos y `Ctrl+Alt+Shift+O`.
- [x] Modal con distro, perfil, barra de ruta, `Subir`, `Inicio`, `Ir` y lista.
- [x] Lista virtualizada de directorios con `ListState`/`List`.
- [x] Estados de carga, vacío, permiso denegado, ruta inválida y sin perfiles.
- [x] Teclado: `Ctrl+L`, `Alt+Up`, `Alt+Home`, `Ctrl+Enter`, `Esc`; foco inicial
      en la lista y retorno al disparador.
- [x] Ancho/alto derivados de métricas; captura Studio a 1,5 de escala y tema
      claro.
- [ ] Revisar captura a 100/200 % de DPI.

## Integración y robustez

- [x] Badge WSL en la sidebar y tooltip con ruta y perfil.
- [x] Caché de salud de proyectos WSL; no llamar `is_dir` en render.
- [x] Estados de pestaña restaurada si falta la distro o la carpeta.
- [x] Gate manual: abrir un proyecto real desde el modal (Ctrl+Enter), nueva
      pestaña, split, reinicio y cierre del árbol.
- [~] Gate manual: al cerrar la pestaña desapareció su shell Linux; falta medir
      un árbol con un proceso hijo de larga duración.
- [ ] OSC 7 host-aware (pulido): conservar autoridad/URI y resolver según host.

## Documentación y cierre

- [x] `docs/wsl.md`: requisitos, flujo, `--cd`, perfiles, límites y errores.
- [x] `docs/session.md`: sesión v3 y migración.
- [x] `docs/profiles.md`: perfiles WSL y perfil de proyecto.
- [x] `AGENTS.md`: arquitectura de hosts, alcance y riesgos de WSL.
- [x] Escena `app.wsl` y fixture de Studio; superficies `app.wsl.live` y
      `app.wsl.project` para el gate local.
- [x] `pwsh -File scripts/check.ps1` y clippy con `--features studio`.
- [x] `cargo-low.ps1 build --release` y `test-cargo-low.ps1`.
