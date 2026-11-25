# M8 — Proyectos en WSL

Objetivo: abrir proyectos alojados en el sistema de archivos de una distribución
WSL, con su propia terminal, un navegador de carpetas y restauración de sesión.
Es la primera implementación de `ProjectHost` y la base para SSH y contenedores
sin tocar el motor de terminal.

Solo Windows. No hay red: WSL es un host local.

## Criterio de hecho

- [x] Junto a «Nueva terminal» hay un botón partido con el icono de carpeta: el
      icono abre el selector nativo; el caret de su derecha muestra «Carpeta
      local…» y «Proyecto en WSL…».
- [x] El modal de WSL permite elegir distro, perfil de terminal y carpeta con un
      navegador de directorios, y abrir el proyecto.
- [x] El proyecto guarda su perfil WSL; toda terminal nueva, split o duplicado
      usa ese perfil y nunca el perfil global.
- [x] La sesión v3 persiste `host` y el perfil del proyecto, y migra sesiones v2
      como proyectos locales.
- [x] Reiniciar la app restaura proyecto, perfil, layout e historial; una distro
      o carpeta ausente deja un estado accionable sin borrar el proyecto.
- [ ] La detección de agentes por OSC 7501 funciona dentro de WSL (gate manual
      con `opencode2`/`copilot` reales).
- [x] Tests, clippy, fmt, `check.ps1`, release y pruebas del wrapper en verde.
- [ ] Gates pendientes: agentes OSC reales dentro de WSL, DPI 100/200 y cierre
      de un árbol con proceso hijo largo.

## Alcance

- `ProjectHost` (`Local`, `Wsl { distro }`) en `axf_workspace`.
- `Project.default_profile` para fijar el perfil del proyecto.
- Sesión v3 con migración v2 → v3.
- Adaptador WSL en la app: distros, validación, navegación, `--cd` y perfil.
- Botón partido y modal con navegador de carpetas.
- Badge de host, salud de proyecto y errores de restauración.
- `docs/wsl.md` y actualización de `docs/session.md` y `docs/profiles.md`.

## Fuera de alcance

- SSH, Docker/Podman y cualquier otro host remoto.
- Autenticación, reconexión, túneles o reenvío de puertos.
- Sondeo de procesos Linux con `pgrep`; los agentes dependen de OSC 7501.
- Navegar archivos que no sean directorios.
- Cambiar el perfil WSL de un proyecto ya creado (queda para «perfiles por
  proyecto» de v0.2).

## Referencias

- ADR 0011 — proyectos WSL y hosts de proyecto.
- ADR 0003 — modelo de proyectos, paneles y pestañas.
- ADR 0006 — perfiles de terminal.
- ADR 0009 — detección de agentes y límite del fallback de procesos en WSL.
