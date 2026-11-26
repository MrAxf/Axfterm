# Axfterm Studio (solo desarrollo)

Axfterm Studio es una utilidad de desarrollo para navegar la aplicación y
capturar la UI y el sistema de diseño como PNG. No entra en el binario de
release: todo vive detrás de la feature Cargo `studio`.

```powershell
pwsh scripts/studio.ps1 -All -Hidden
```

Compila `axfterm` con `--features studio`, arranca una instancia aislada y
Reproduce las escenas de `studio/scenes/` (38 en v0.2). Las capturas quedan en
`target/studio/shots/` y el índice en `target/studio/manifest.json`. Cada escena
parte de escala 1,0; el manifiesto guarda el DPI (`scale`) y la escala de UI
(`ui_scale`) de cada captura.

## Aislamiento

- `AXFTERM_CONFIG_DIR` apunta a `target/studio/home`: no se lee ni se escribe
  `%APPDATA%\axfterm`.
- Antes de cada tanda se borran `session.json` y `scrollback/` del perfil de
  Studio.
- El fixture `demo` crea tres proyectos en `target/studio/projects` (atlas,
  orbit y lumen), un split y agentes sintéticos. Los agentes reales se pausan
  mientras el fixture está activo.
- El fixture `projects` registra dos proyectos sin terminales; sirve para
  revisar la bienvenida con «Abrir carpeta» y «Nueva terminal» en la misma fila.
- Studio copia un tema personalizado fixture desde `studio/fixtures/themes/`
  para capturar también la Apariencia con una paleta no integrada.
- El modo `-Hidden` coloca la ventana fuera del escritorio para no tapar el
  trabajo del usuario. El frame se pinta con `Window::draw` antes de capturar.

## Scripts

### `scripts/studio.ps1`

| Parámetro | Uso |
| --- | --- |
| `-All` | reproduce todas las escenas |
| `-Scene <nombre>` | reproduce una o varias escenas |
| `-Serve` | deja Studio abierto para explorar |
| `-Hidden` | ventana fuera de pantalla |
| `-NoBuild` | no recompila; usa `target/debug/axfterm.exe` |
| `-Dir <ruta>` | cambia el directorio de estado y capturas |

```powershell
pwsh scripts/studio.ps1 -Scene gallery.actions
pwsh scripts/studio.ps1 -Scene app.workspace -Hidden
pwsh scripts/studio.ps1 -Serve
```

### `scripts/devtools.ps1`

Cliente de bajo nivel. Lee `session.json` y envía un comando JSONL al puerto
loopback de Studio.

```powershell
pwsh scripts/devtools.ps1 state
pwsh scripts/devtools.ps1 open app.settings.appearance
pwsh scripts/devtools.ps1 shot settings-terminal
pwsh scripts/devtools.ps1 shot gallery-actions -Clip 0,0,1280,800
pwsh scripts/devtools.ps1 theme 'One Light'
pwsh scripts/devtools.ps1 variant light
pwsh scripts/devtools.ps1 key tab
pwsh scripts/devtools.ps1 quit
```

## Protocolo

- TCP en `127.0.0.1`, puerto efímero (o `--studio-port`).
- Una petición JSON por línea y una respuesta por línea.
- Token obligatorio, escrito en `target/studio/session.json`.
- Límites: 64 KiB por línea, 8 clientes, 60 s de espera.

Petición:

```json
{"id":1,"token":"…","cmd":{"type":"open","surface":"gallery.actions"}}
```

Respuesta correcta:

```json
{"id":1,"ok":true,"state":{"surface":"gallery.actions","theme":"One Dark","variant":"dark","width":1440.0,"height":900.0,"ui_scale":1.0,"display_scale":1.5,"projects":3,"tabs":4,"panes":2,"gallery_modal":false,"shots_dir":"C:\\…\\target\\studio\\shots"}}
```

Captura:

```json
{"id":2,"ok":true,"shot":{"path":"C:\\…\\gallery-actions.png","width":2160,"height":1350,"scale":1.5}}
```

## Comandos

| Comando | Efecto |
| --- | --- |
| `state` | estado actual |
| `surfaces` | superficies disponibles |
| `open <superficie>` | abre una superficie determinista |
| `action <id>` | ejecuta una acción segura del registro |
| `theme <nombre>` | cambia el tema |
| `variant dark\|light` | fuerza la variante |
| `scale <0.8..2.0>` | escala global de la UI; cada escena arranca en 1,0 |
| `size <ancho>x<alto>` | redimensiona la ventana |
| `focus shell\|sidebar\|settings\|palette\|terminal` | mueve el foco |
| `key <tecla>` | envía una pulsación permitida a la ventana enfocada |
| `workspace empty\|demo` | reemplaza el workspace |
| `modal open\|close` | abre el modal de la galería |
| `wait <ms>` | espera antes del siguiente paso |
| `shot <nombre> [clip]` | captura la ventana en `shots/<nombre>.png` |
| `quit` | cierra la aplicación guardando la sesión aislada |

`clip` es `x,y,ancho,alto` en píxeles lógicos.

Las acciones que abren diálogos nativos (`project.open`, `theme.import`,
`theme.export`, `theme.open_folder`) y las de portapapeles se rechazan.
`key` solo acepta `tab`, `shift-tab`, flechas, `enter`, `space`, `escape`,
`secondary-s` y `secondary-enter`; no permite enviar texto arbitrario ni atajos
de cierre.

## Superficies

```text
app.onboarding
app.onboarding.project       # fixture projects: dos acciones
app.workspace
app.settings.appearance | profiles | profiles.editor | shortcuts | shortcuts.capture | agents
app.palette.commands
app.palette.search?q=<consulta>
app.shortcuts
app.sidebar.terminals
app.sidebar.agents
app.wsl                 # fixture sin WSL
app.wsl.live            # modal real (requiere WSL)
app.wsl.project         # proyecto real en /home (requiere WSL)
gallery.foundations
gallery.actions
gallery.inputs
gallery.navigation
gallery.feedback
gallery.overlays
gallery.compositions
gallery.icons
gallery.themes
```

## Galería del sistema de diseño

La galería es una superficie del mismo shell. Se puede abrir con
`open gallery.actions` o arrancar directamente con `--studio-gallery`.

Secciones:

- **Fundaciones**: tokens UI, ANSI 16, tipografía, espaciados, radios, alturas,
  iconos y colores de marca.
- **Acciones**: botones en todas las variantes, tonos y tamaños; icon buttons,
  switches y atajos.
- **Entradas**: `TextField`, `Input`, botones, switches, `Select` y radios de
  gpui-kit.
- **Navegación**: pestañas, menú, sidebar, fila de paleta y fila de ajustes.
- **Feedback**: estados de agente, avisos y estados vacíos.
- **Overlays**: modal y tooltip.
- **Composiciones**: tab strip, sidebar con agente, barra de título, cabecera
  de ajustes y fila de paleta.
- **Iconos**: todos los iconos vendorizados con nombre y tamaño.
- **Temas**: los seis temas incluidos con muestras.

La galería también se puede abrir sin la app:

```powershell
cargo run -p axf_ui --features gallery --example gallery
```

## Escenas

Cada archivo de `studio/scenes/` es un JSON con pasos secuenciales:

```json
{
  "name": "gallery.actions",
  "steps": [
    { "cmd": "theme", "args": ["One Dark"] },
    { "cmd": "variant", "args": ["dark"] },
    { "cmd": "size", "args": ["1440x900"] },
    { "cmd": "open", "args": ["gallery.actions"] },
    { "cmd": "shot", "args": ["gallery-actions"] }
  ]
}
```

Para añadir una superficie nueva:

1. Añádela a `SURFACES` en `crates/app/src/studio/protocol.rs`.
2. Impleméntala en `crates/app/src/shell_parts/25_studio_surface.rs`.
3. Crea la escena en `studio/scenes/`.
4. Ejecuta `pwsh scripts/studio.ps1 -Scene <nombre>`.

## Comprobaciones

```powershell
pwsh -File scripts/check.ps1
pwsh -File scripts/cargo-low.ps1 clippy -p axfterm --features studio --all-targets -- -D warnings
pwsh -File scripts/studio.ps1 -All -NoBuild
```

La captura usa el readback DirectX de GPUI (`Window::render_to_image`), por lo
que en Windows se habilitan `gpui/test-support` y
`gpui-pre-windows/test-support` solo para la feature `studio`. El release no
activa ninguna de las dos.
