# Agentes

Axfterm detecta los agentes de IA que corren dentro de una terminal y los muestra
en la sidebar y en las pestañas. La detección es local: no hay red, telemetría ni
plugins.

Agentes reconocidos en v0.1:

- `opencode` y el alias `opencode2` (mismo `opencode.exe`).
- GitHub Copilot CLI: `copilot` y su runtime `copilot-runtime`.

## Fuente principal: OSC 7501

El programa reporta su estado por el PTY:

```text
OSC 7501 ; state=working : app=opencode : progress=40 : msg=QnVpbGRpbmc= ST
```

- `state` es obligatorio: `idle`, `working`, `blocked`, `done`, `error` o
  `clear`.
- `blocked` usa `kind` (`permission`, `question`, `auth`) y `msg` para explicar
  qué necesita.
- `progress` va de 0 a 100; fuera de rango cuenta como ausente.
- `app` identifica el programa (`opencode`, `copilot`). Es lo que atribuye el
  reporte a la sesión del tipo correcto.
- `title` y `msg` son base64 de UTF-8. Se rechaza cualquier reporte cuyo texto
  decodifique a un carácter de control o a una dirección bidi invisible.
- Los pares desconocidos se ignoran; una pareja mal formada se salta; si una
  clave se repite, gana la última. Un reporte que rompe un límite, trae un
  `state` desconocido o un `id` inválido se descarta entero.
- `clear` retira el registro y deja la sesión inactiva.

Límites aplicados: secuencia de 4096 bytes, clave de 16, `msg` de 2732 bytes
codificados y 2048 decodificados, `title` de 256/192, `app` de 32 y `id` de 128
con hasta 8 segmentos de 32.

Un programa puede preguntar si el terminal implementa el protocolo:

```text
OSC 7501 ; ? ST
```

Axfterm responde con `states` y `kinds`:

```text
OSC 7501 ; ? : states=idle,working,done,blocked,error : kinds=permission,question,auth ST
```

La respuesta no incluye ningún dato tomado de un reporte.

## Atribución por procesos

El árbol de procesos del PTY (`PID raíz` + `sysinfo`) atribuye cada agente a su
pestaña y cubre versiones antiguas del CLI que no hablan OSC 7501.

- Se reconoce `opencode.exe`, `opencode2.exe`, `copilot.exe` y
  `copilot-runtime.exe`.
- Como fallback se inspecciona el script que ejecutan Node, Bun o Deno
  (`node .../opencode`, `bun .../copilot`).
- Se elige el proceso agente más cercano al shell y se poda su subárbol, para no
  duplicar sesiones con ayudantes como `opencode-pty`.
- Se admiten varias sesiones por pestaña, también del mismo tipo, si son ramas
  distintas del árbol.
- El escaneo corre en segundo plano cada `agents.scan_interval_ms`. La UI solo
  lee el resultado; nunca espera al proceso.

Cuando un reporte OSC llega antes que el escaneo, la sesión nace sin PID y lo
adopta al primer escaneo del mismo tipo.

## Estados y prioridad

| Estado | Significado | Se conserva hasta |
| --- | --- | --- |
| `NeedsAttention` | El agente espera al usuario | Foco o confirmación |
| `Working` | Está procesando | Cambio de estado, fin del proceso o quiescencia |
| `Error` | Falló | Foco o confirmación |
| `Finished` | Terminó y el resultado no se ha visto | Foco o confirmación |
| `Idle` | Vivo sin trabajo activo | Cambio de estado |
| `Stopped` | Proceso terminado sin resultado | Foco |

La prioridad de presentación es
`NeedsAttention > Working > Error > Finished > Idle > Stopped`. Con empate gana la
sesión más reciente.

- `done` y `error` sobreviven a la salida del proceso; `working` y `blocked` se
  cierran con el proceso.
- OpenCode 2 no envía `done`: al acabar un turno reporta `idle` en el registro
  raíz y `clear` en el de la sesión. Axfterm interpreta ese paso de `working` a
  `idle`/`clear` como `Finished` para que el aviso de terminado exista; los
  `idle`/`clear` posteriores conservan el resultado pendiente hasta el foco.
  Un nuevo `working` reanuda el ciclo con normalidad.
- Al enfocar una pestaña, atención, terminado y error pasan a inactivo y se
  retiran las sesiones cuyo proceso ya no existe.
- Si el proceso desaparece sin reporte final, un `working` se cierra como
  `Finished`; un cierre del shell con código distinto de cero se cierra como
  `Error`.
- La campana (`BEL`), OSC 9, OSC 777 y OSC 1337 son señales de atención. OSC
  9;4 (progreso de ConEmu) no se confunde con una notificación.
- La actividad de salida y la quiescencia (~2,5 s) son el fallback para sesiones
  que nunca reportaron OSC 7501.

El estado de alertas es runtime-only: no se escribe en `session.json` y no
sobrevive a un reinicio. Sí se persiste qué agente seguía vivo al cerrar
(`resume_agent`) y, cuando el agente lo reportó por OSC 7501, el id de su sesión
(`resume_session`). Al reabrir la pestaña se descarta cualquier TUI en pantalla
alternativa (`ALT_SCREEN`) de la instantánea y se reanuda exactamente esa
conversación con `opencode --session <id>` o `copilot --resume=<id>`. Así dos
pestañas del mismo proyecto no abren la misma sesión: OpenCode comparte su tira
de pestañas entre terminales del mismo directorio y reporta varias, así que se
elige la que coincide con el título del terminal (`OC | <sesión>`). Si el título
está desactivado, se usa la única sesión observada o el último id válido. Si el
agente nunca reportó un id (versión antigua o todavía no ha trabajado), se
conserva el fallback `--continue` y puede coincidir con otra pestaña del mismo
directorio. Si el
perfil de la pestaña define `[profiles.agent_commands]`, se usa ese comando base
en lugar del global: por ejemplo, `opencode2 --session <id>` en Ubuntu (WSL).

## Sidebar

La sidebar tiene el filtro `Todas | Agentes`:

- **Todas**: proyectos y todas sus terminales.
- **Agentes**: solo los proyectos y terminales con una sesión de agente activa,
  ordenando las terminales de cada proyecto por prioridad y actividad.

Cada terminal tiene como máximo una sesión de agente activa. La segunda línea de
la tarjeta muestra el agente, el contexto, el progreso y el mensaje o estado; el
tooltip añade la duración. El icono de la terminal y de la pestaña usa la marca
del agente y lleva un círculo de estado superpuesto en la esquina inferior
derecha con anillo de separación: verde/check al terminar, ámbar/! si requiere
atención, neutro/spinner si trabaja y rojo/alerta si falla. El filtro activo se
persiste con la sesión.

## Avisos

Al pasar a `done`, `blocked` o `error` se muestra un toast in-app con cola de
hasta tres avisos, deduplicación y cooldown de 10 s. En OpenCode, cuyo CLI no
emite `done`, el paso de `working` a `idle` cuenta como terminado y produce el
mismo aviso. Si la pestaña ya está enfocada no se muestra toast, pero el icono,
el chip y el título sí se actualizan. El título de la ventana lleva `!` mientras
alguna sesión requiera atención.

No hay notificaciones nativas del sistema en v0.1.

## Configuración

```toml
[agents]
enabled = true
# Milisegundos entre escaneos de procesos (1000–2000).
scan_interval_ms = 1500
notify_finished = true
notify_error = true
notify_attention = true

# Perfil con el que abrir cada agente (id de [[profiles]]).
[agents.preferred_profiles]
opencode = "pwsh"
copilot = "pwsh"
```

Cada perfil puede cambiar el ejecutable del agente dentro de su propio shell:

```toml
[[profiles]]
id = "wsl-ubuntu"
name = "Ubuntu (WSL)"
command = 'wsl -d "Ubuntu"'

[profiles.agent_commands]
opencode = "opencode2"
```

Sin esta tabla se usan `opencode` y `copilot`. La reanudación añade
`--session <id>` (o `--resume=<id>` en Copilot) al comando configurado y cae a
`--continue` si no hay id; el alias se resuelve en el shell interactivo del
perfil, así que funciona también dentro de WSL.

Se recarga en caliente como el resto de `config.toml`. Un valor fuera de rango
es un error de campo accionable; no se corrige en silencio. Al desactivar la
detección se retiran las sesiones en memoria.

El panel de ajustes (`Ctrl+Alt+T`, sección Agentes) edita estos campos con
borrador y guardado atómico. `preferred_profiles` se valida contra los perfiles
existentes y lo consume el lanzador de agentes (M11); ver la sección siguiente y
`docs/settings.md`.

## Abrir agentes desde la app

- **Pie de la sidebar**: un acceso por agente. Abre una pestaña nueva en el
  proyecto actual y escribe el comando en su shell; al salir del agente se
  vuelve al prompt. Los accesos son iconos de marca, en una fila pensada para
  crecer con más agentes; el tooltip incluye el atajo y el estado del agente en
  el proyecto. Bajo ellos, el pie muestra la versión y el resumen del workspace
  (proyectos, terminales y agentes activos).
- **Desplegable del «+» de la pestaña**: «Abrir OpenCode» y «Abrir Copilot» en
  ese panel y su proyecto actual.
- **Teclado y paleta**: `Ctrl+Alt+1` y `Ctrl+Alt+2`, además de las acciones
  `agent.open_opencode` y `agent.open_copilot` en la paleta y en `[keymap]`.
  Los atajos no usan teclas reservadas del terminal y AltGr/IME se entregan como
  texto; con una terminal enfocada funcionan como el resto de acciones de app.

Cada gesto abre siempre una pestaña nueva; no se enfoca una sesión existente. El
perfil se elige así:

1. `[agents.preferred_profiles]` si existe y encaja con el host del proyecto.
2. Perfil del proyecto.
3. Perfil global.

Un preferido inexistente o incompatible (por ejemplo, un perfil WSL en un
proyecto local) produce un aviso y se usa el siguiente. El comando base sale de
`[profiles.agent_commands]` o del nombre del agente. Si el ejecutable no está
instalado, el error aparece en la propia terminal.

## Diagnóstico y pruebas

- `cargo test -p axf_agents` cubre el matcher con snapshots, la prioridad, el
  latch y la limpieza al enfocar.
- `cargo test -p axf_terminal --test vt` cubre OSC 7501, los límites, base64,
  la consulta y las señales OSC 9/777/1337.
- Los CLIs falsos de `crates/terminal/tests/fixtures/` emiten OSC 7501, OSC 9 y
  BEL para el gate manual.

Gate manual recomendado:

1. Abre dos proyectos.
2. En uno, arranca un CLI falso o `opencode2`; en el otro, `copilot`.
3. Comprueba que el chip y el icono cambian con `working`, `blocked`, `done` y
   `error`, y que la sección Agentes ordena por prioridad.
4. Deja una pestaña en segundo plano con atención pendiente: debe aparecer el
   toast y el `!` en el título; al enfocarla, ambos desaparecen.
5. Mueve una pestaña de panel: sus sesiones se conservan.
6. Abre OpenCode y Copilot desde el pie de la sidebar y desde el «+» de la
   pestaña: cada gesto crea una pestaña nueva, arranca en el cwd del proyecto y
   respeta el perfil preferido. Prueba también con la sidebar estrecha (iconos),
   con un perfil preferido de otra distro (aviso) y sin el binario (error en la
   terminal).

## Limitaciones conocidas

- OSC 7501 no está en todas las versiones de los CLIs; el fallback de procesos y
  salida es más impreciso.
- Varios agentes del mismo tipo en una terminal producen reportes ambiguos: se
  aplican a la sesión más reciente y el árbol conserva el conteo.
- Se valida el `id` de OSC 7501 y su jerarquía, pero v0.1 modela un único
  registro raíz por terminal; los ids no crean sesiones separadas por sí solos.
  Solo se guarda un id de sesión reanudable (`ses_…` o UUID) como
  `resume_session`; el registro sintético `opencode` se ignora. La sesión actual
  se reconoce por el título del terminal cuando OpenCode comparte la tira de
  pestañas; con `terminal.title=false` la elección puede ser ambigua.
- Sin `resume_session`, el fallback `--continue` reanuda la última sesión del
  directorio y dos pestañas del mismo proyecto pueden coincidir.
- `sysinfo` no ve procesos Linux dentro de WSL, así que ahí el estado depende de
  OSC 7501.
- Los accesos rápidos siempre abren una pestaña nueva: puede haber varias
  sesiones del mismo agente en un proyecto. Es deliberado y el tooltip avisa de
  que ya hay una activa.
- Los nombres de proceso pueden cambiar entre versiones; el matcher se cubre con
  snapshots y se documenta.
- Los avisos son in-app; no hay notificaciones del sistema ni sonido.
