# Proyectos en WSL

Axfterm puede abrir una carpeta que vive dentro de una distribución WSL y
arrancar su terminal con `wsl -d <distro> --cd <ruta>`. Es una función de
Windows; no usa red, autenticación ni agentes remotos.

## Requisitos

- Windows 10 1809 o superior, o Windows 11, x64.
- WSL instalado y al menos una distribución registrada (`wsl -l -q`).
- Una versión de WSL que acepte `--cd` (Windows 11 o WSL actualizado). Si la
  versión no lo acepta, Axfterm muestra el error de WSL y no cae a otra carpeta.

## Abrir un proyecto

Junto a **Nueva terminal** hay un botón partido con el icono de carpeta:

- El icono de carpeta abre el selector nativo de carpetas.
- El caret a su derecha muestra las fuentes disponibles: **Carpeta local…** y
  **Proyecto en WSL…**.
- También puedes usar `Ctrl+Alt+Shift+O` o la paleta de comandos.

En el modal:

1. Elige la **distribución**.
2. Elige la **terminal** (los perfiles cuyo comando es `wsl -d <distro>`).
3. Navega por las carpetas o escribe una ruta.
4. Pulsa **Abrir**.

La carpeta debe ser una ruta POSIX absoluta (`/home/...`, `/mnt/c/...`). La barra
de ruta acepta `~` y rutas relativas a la carpeta actual. El navegador solo
muestra directorios.

Atajos del modal:

| Tecla | Acción |
| --- | --- |
| `Ctrl+L` | Editar la ruta |
| `Alt+Up` | Subir a la carpeta padre |
| `Alt+Home` | Ir al home de la distro |
| `Ctrl+Enter` | Abrir la carpeta actual |
| `Enter` | Entrar en la carpeta seleccionada |
| `Esc` | Cerrar el modal |

## Terminal del proyecto

Un proyecto WSL guarda el perfil elegido. Todas sus terminales nuevas, splits,
duplicados y el botón `+` del proyecto usan ese perfil; nunca el perfil global.
Si el perfil se borra o apunta a otra distro, la pestaña muestra un error
accionable y no arranca otro shell.

La sesión guarda el host y el perfil del proyecto (`session.json` v3). Si el
perfil ya no existe al restaurar, el proyecto se conserva y la terminal muestra
el error; no se borra el proyecto ni se cambia de host en silencio.

## Rutas y validación

- La ruta se valida con `realpath -e` o, si falta, con `test -d` + `pwd -P`.
  No se usa el código de `pwd` a solas: con un `--cd` inválido WSL imprime el
  error pero puede ejecutar `pwd` desde `/` con código 0.
- Las subcarpetas se listan con `find -L ... -printf '%f\0'` para soportar
  espacios y UTF-8; si la distro no trae ese `find`, se usa `ls -1p`.
- Todo proceso WSL se ejecuta sin shell, con `CREATE_NO_WINDOW`, timeout y fuera
  del hilo de UI.
- La salud del proyecto se comprueba en segundo plano. Una distro o carpeta
  ausente se muestra en la sidebar y en la pestaña sin bloquear el arranque.

## Límites conocidos

- Solo Windows.
- Los agentes dentro de WSL dependen de OSC 7501: `sysinfo` no ve procesos
  Linux, así que el fallback por árbol de procesos no está disponible.
- `/mnt/c/...` funciona, pero el rendimiento de 9p es menor que el de una ruta
  nativa de Linux.
- No se navegan archivos, solo carpetas.
- SSH, Docker/Podman y reconexión remota quedan fuera de esta función.
