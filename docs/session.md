# Sesión e historial de terminal

Axfterm guarda dos cosas distintas:

- **`session.json`**: qué proyectos, paneles, pestañas, perfiles y foco había.
- **`scrollback/`**: el contenido de cada terminal (pantalla + historial).

Ambos viven en el mismo directorio de configuración:

- Windows: `%APPDATA%\axfterm\`
- macOS: `~/Library/Application Support/axfterm/`
- Linux: `~/.config/axfterm/` o `$XDG_CONFIG_HOME/axfterm/`

## Qué se restaura al reabrir

1. Se restauran los proyectos, su host (`local` o WSL), el perfil de proyecto,
   el layout de paneles, las pestañas, el perfil de cada una y el foco.
2. **Solo arranca la pestaña activa de cada panel visible.** Las pestañas
   ocultas no crean procesos: esperan a que las actives.
3. Al activar una pestaña se carga su historial y se arranca el perfil guardado
   en esa pestaña. El PTY arranca con el tamaño de la instantánea y el primer
   frame lo ajusta al hueco real de la ventana (el historial se refluye). El
   prompt nuevo aparece debajo del contenido anterior, con el cursor visible y
   la vista al final.
4. Si una pestaña no tenía historial, se abre una terminal normal.
5. Si el archivo de historial está dañado o es de una versión futura, se ignora,
   se abre el mismo perfil desde cero y se muestra un aviso.
6. El estado de los agentes no se persiste: es runtime-only y se redetecta
   cuando el PTY vuelve a arrancar. Ver `docs/agents.md`.

El proceso anterior no sobrevive al cierre de Axfterm: se restaura el contenido,
no el PTY. Un daemon de sesiones está fuera de v0.1.

## Cuándo se guarda

Al cerrar Axfterm de forma ordenada (botón de cerrar, `Ctrl+Q` o el cierre de
Windows que llega a la app) se pide a cada terminal viva su buffer, se escribe
en disco y después se guarda `session.json`.

Un cierre forzado (Administrador de tareas, crash o corte de luz) no guarda la
última tanda de historial. La sesión anterior sigue siendo válida, pero el
historial puede ser el del cierre anterior.

## Versión y migración

`session.json` usa `version = 3` desde los proyectos WSL:

- Cada proyecto guarda `host` (`{"kind":"local"}` o
  `{"kind":"wsl","distro":"Ubuntu"}`) y, si aplica, `default_profile`.
- Las sesiones v2 no tienen `host`: se cargan como proyectos locales y
  `default_profile = null`. La migración no toca el archivo hasta el siguiente
  guardado.
- Una versión futura se rechaza sin modificar el archivo y se arranca con un
  workspace limpio, como en la config.

## Formato y límites

Cada terminal tiene su propio archivo:

```text
scrollback/<epoch>-<terminal_id>.snap
```

- `<epoch>` identifica la ejecución de la app. Si `session.json` se recupera
  desde cero, se usa un epoch nuevo para no mezclar archivos viejos con
  pestañas nuevas.
- El contenido es una transcripción ANSI del grid: texto, colores, atributos y
  caracteres anchos. No se guardan enlaces, selección, posición de scroll ni
  modos de ratón. Al restaurar se fuerza el cursor visible y el final del
  historial: un programa que lo dejó oculto (por ejemplo, `docker compose`
  v5 con logs en streaming) no hereda ese estado y el prompt nuevo queda
  usable.
- Tope: **2 MiB por terminal**. Si se supera, se conservan las líneas más
  recientes y se avisa al restaurar. El scrollback vivo tiene su propio límite
  configurable (0–50 000 líneas) y un tope de 32 MiB por terminal
  (`terminal.scrollback_lines`); ambos límites se aplican por separado.
- Escritura atómica. Un archivo a medias nunca sustituye al anterior.

## Privacidad

El historial contiene exactamente lo que se mostró en la terminal, incluidos
posibles tokens o rutas privadas. Se guarda en el perfil del usuario, sin cifrar
y sin salir del equipo. Para borrarlo, cierra Axfterm y elimina la carpeta
`scrollback/`.

## Diagnóstico

- Si al reabrir una pestaña aparece «No se pudo restaurar el historial…», el
  archivo se descartó y la terminal se abrió desde cero. El aviso incluye el
  motivo.
- Si aparece «Historial restaurado parcialmente», el archivo llegó al límite de
  2 MiB y solo se conservaron las líneas más recientes.
- Los archivos de pestañas cerradas se eliminan al cerrar. Los de una ejecución
  anterior se podan cuando la sesión arranca limpia.
