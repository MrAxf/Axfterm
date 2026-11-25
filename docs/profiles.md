# Perfiles de terminal

Un perfil describe con qué programa arranca una terminal. El archivo
`config.toml` es la fuente de verdad; la sesión guarda solo el `id` del perfil.

## Esquema

```toml
schema_version = 2

[terminal]
default_profile = "pwsh"

[[profiles]]
id = "pwsh"
name = "PowerShell 7"
command = 'pwsh -NoLogo'
# icon = "icons/powershell.png"

[[profiles]]
id = "wsl-ubuntu"
name = "Ubuntu (WSL)"
command = 'wsl -d "Ubuntu"'
```

- `id`: slug estable y único (minúsculas, dígitos, `-` y `_`). Es lo que guarda
  la sesión; `name` se puede cambiar sin romperla.
- `name`: título inicial de la pestaña y etiqueta del menú. Máximo 64
  caracteres.
- `command`: una línea con ejecutable y argumentos. Máximo 1024 caracteres.
- `icon`: opcional. Ruta relativa al directorio de config o absoluta.
- `default_profile` debe existir en `[[profiles]]`. Un id repetido, vacío, un
  `name`/`command` vacío o un `default_profile` inexistente es un error de
  campo; no se corrige en silencio.
- Límites de v0.1 (ADR 0005): 32 perfiles, 64 caracteres de nombre, 1024 de
  comando y 2 MiB por imagen de icono.

Los perfiles conservan el orden del archivo en menús y listas. El estado "cero
perfiles" no es válido: si el archivo queda sin perfiles, la app vuelve a
detectar terminales y escribe el resultado una sola vez.

El panel de ajustes (`Ctrl+Alt+T`, sección Perfiles) permite alta, edición,
duplicado, orden, icono y borrado protegido, además de «Detectar terminales» con
vista previa y fusión sin duplicados. El `id` no cambia al renombrar y el perfil
en uso por una pestaña conserva su configuración. Ver `docs/settings.md`.

## Perfil del proyecto y WSL

Cada proyecto puede fijar su perfil de terminal. Un proyecto local lo deja
vacío y usa `[terminal] default_profile`; un proyecto WSL siempre guarda el
perfil elegido al abrirlo. Las terminales nuevas, splits, duplicados y el botón
`+` del proyecto usan ese perfil, nunca el global.

Un perfil es compatible con un proyecto WSL si su ejecutable es `wsl` o
`wsl.exe` y sus argumentos fijan la misma distro con `-d` o `--distribution`.
Axfterm añade `--cd <ruta>` antes de `--`, `-e` o `--exec`; el comando del perfil
no necesita escribir la carpeta. Si el perfil apunta a otra distro o no es WSL,
la pestaña muestra un error y no arranca otro shell.

La detección escribe un perfil `wsl-<slug>` por distro. Si instalas una distro
después del primer arranque, usa «Detectar terminales» para añadir su perfil.
Ver `docs/wsl.md`.

## Línea de comandos

`command` se tokeniza sin shell:

| Regla | Ejemplo |
| --- | --- |
| Espacios separan tokens | `pwsh -NoLogo` |
| Comillas dobles agrupan | `"C:\Program Files\PowerShell\7\pwsh.exe" -NoLogo` |
| Comillas simples agrupan | `'C:\Program Files\Git\bin\bash.exe' --login` |
| `\"` y `\'` escapan dentro de su comilla | `cmd "a\"b"` |
| Barras invertidas literales | `C:\Users\name` |
| Sin variables ni operadores | `a && b` llega como argv, no se ejecuta en shell |

Un comando vacío, sin ejecutable o con una comilla sin cerrar es un error de
validación. Si el ejecutable no existe al arrancar, la pestaña muestra el
comando y ofrece reintentar o usar el perfil predeterminado; nunca se cae a otro
shell. El ejecutable se resuelve en `PATH` antes de crear el PTY y se pasa como
ruta absoluta al terminal.

Los perfiles detectados no añaden `-NoProfile`: el shell debe cargar el perfil
del usuario (`$PROFILE` en PowerShell). Solo se pasa `-NoLogo` para ocultar el
banner. Si quieres un arranque sin perfil, añade `-NoProfile` a mano en el
`command` de ese perfil.

## Detección

Linux y macOS:

- `$SHELL` y, si faltan, `zsh`, `bash`, `fish`, `nu` y `sh` en `PATH`.

Windows:

- `pwsh`, `powershell`, `cmd`, `nu` y Git Bash.
- `wsl -l -q` en segundo plano, con timeout de 2,5 s y salida UTF-16 o UTF-8.
  Cada distro se convierte en un perfil `wsl-<slug>` con
  `wsl -d "<distro>"`.

La detección local se ejecuta al primer arranque, cuando el archivo no existe o
cuando no hay perfiles. Escribe la plantilla completa de forma atómica. WSL se
añade después con `append_profiles`, que conserva comentarios y no duplica por
id ni por comando. Una configuración con perfiles propios no se redetecta.

## Iconos

Orden de resolución:

1. Si `icon` existe y la imagen carga, se usa. Formatos soportados por GPUI:
   PNG, JPG/JPEG, ICO, WebP, GIF, BMP, TIFF y SVG.
2. Si `icon` falta, en Windows se extrae el icono del ejecutable con
   `ExtractIconExW` + GDI, se convierte a PNG RGBA y se cachea en
   `<config>/icons/cache/<hash>.png`. La clave incluye ruta, `mtime` y tamaño.
3. Si el icono no existe, supera 2 MiB, la extracción falla o la plataforma no
   lo soporta, se usa `icons/tabler/terminal-2.svg` sin aviso: el fallback es
   silencioso y el motivo solo queda en el log de diagnóstico.

La caché se limita a 64 archivos y se ordena por `mtime`; borrarla no pierde
configuración. En macOS/Linux la extracción nativa queda para post-v0.1 y el
fallback Tabler es siempre visible.

## Pestañas, sesión y hot reload

- Cada pestaña guarda `profile_id`. Moverla entre paneles, duplicarla o
  reiniciarla conserva el perfil.
- `session.json` usa `version = 3`. La migración v1 → v3 rellena el perfil por
  defecto en cada pestaña y marca los proyectos como locales; `auto_title`
  conserva el nombre automático para que quitar un rename vuelva al nombre del
  perfil.
- Una pestaña restaurada cuyo perfil ya no existe queda terminada y ofrece
  «Usar perfil predeterminado» o cerrar. No se arranca otro shell por sorpresa.
- Al recargar `config.toml`, las pestañas nuevas usan los perfiles nuevos; las
  abiertas conservan su proceso y su `SpawnSpec`. Un `default_profile` inválido
  conserva la última configuración válida y muestra el campo con error.
- El menú del panel («⋯») lista «Nueva terminal» y un submenú «Con perfil» con
  icono, nombre y el predeterminado marcado. En un proyecto WSL el submenú se
  oculta: sus terminales usan siempre el perfil fijado por el proyecto.
