# Panel de ajustes

El panel edita toda la configuración vigente sin dejar de ser una vista del
archivo: `config.toml` sigue siendo la fuente de verdad. Se abre con el icono de
la barra de título, con el menú del panel (`Ajustes…`) o con `Ctrl+Alt+T`, y se
cierra con Esc devolviendo el foco a la terminal o superficie anterior.

## Secciones

| Sección | Qué edita |
| --- | --- |
| Apariencia | Sistema/claro/oscuro, tema del catálogo, tipografía de la app y del terminal, tamaños, escala e historial por terminal. |
| Perfiles | Alta, edición, duplicado, borrado protegido, orden, icono, predeterminado y «Detectar terminales». |
| Atajos | Rebindeo, `unbind`, restauración y conflictos contra el registro de acciones. |
| Agentes | Activación, intervalo de escaneo, avisos y perfil preferido por agente. |

`Abrir config.toml`, `Abrir carpeta de temas` y `Restaurar valores por defecto`
viven en el menú «Más» del pie del modal, no en una sección propia.

Todo el panel es operable con teclado:

- `Tab` / `Shift+Tab` recorren secciones, campos y botones en orden visual.
- `Enter` o `Espacio` activan la sección, tarjeta o acción enfocada.
- `Ctrl+Tab` / `Ctrl+Shift+Tab` cambian de sección.
- Los sliders aceptan `←`/`→` (paso), `Page Up`/`Page Down` (diez pasos) y
  `Home`/`End` (extremos); el ratón hace lo mismo.
- El intervalo de escaneo de agentes conserva los botones `−`/`+`.
- `Ctrl+S` guarda; `Esc` descarta el borrador, o cancela antes una captura, una
  edición o una confirmación abierta.

## Borrador, preview y guardado

- El panel copia la config activa a un borrador. Nada se escribe hasta pulsar
  **Guardar**.
- Apariencia, tema, tipografía y métricas se previsualizan en vivo. **Descartar**
  restaura el estado visual anterior sin tocar `config.toml`.
- Al guardar se valida el borrador completo con las mismas reglas que el
  arranque, se comprueba que el archivo sigue teniendo el texto con el que se
  abrió el panel, se edita el documento actual con `toml_edit` y se vuelve a
  parsear antes de escribir de forma atómica. Si el archivo cambió en ese
  intervalo, no se escribe nada y se pide recargar o confirmar.
- Solo se actualizan las secciones cuyos valores difieren del borrador base. Los
  comentarios, el orden y el resto de claves se conservan; una sección intacta
  queda byte a byte igual.
- Si la validación o la escritura fallan, no se aplica nada y el pie muestra el
  campo y el error accionable.
- Tras guardar, el tema se aplica una sola vez; el evento del watcher provocado
  por la propia escritura se reconoce por contenido y no se repite.

## Conflictos con el exterior

El panel observa `config.toml` mientras está abierto:

- Si el borrador está limpio y el archivo cambia, el borrador y la config activa
  se recargan y se avisa. Cancelar no revierte un cambio externo ya aplicado.
- Si el borrador tiene cambios, el panel muestra el conflicto y ofrece
  **Recargar** (descarta el borrador), **Seguir editando** o
  **Sobrescribir y guardar** (confirmación explícita).
- Un archivo externo inválido no se aplica: se conserva el borrador y se explica
  el error.
- Nunca se fusiona ni se pisa un cambio externo en silencio.

## Perfiles

- Cada perfil es una tarjeta con su icono, nombre y comando. Las acciones
  (editar, duplicar, subir, bajar, predeterminado y eliminar) viven en la propia
  fila.
- **Nuevo perfil** y **Editar** abren un modal; el alta solo entra en el borrador
  al pulsar **Aplicar**.
- El modal incluye dos comandos de agente opcionales, **OpenCode** y **Copilot**.
  Vacío usa el comando global; si se rellena, ese perfil ejecuta y reanuda el
  agente con el comando indicado (`opencode2 --continue` en WSL, por ejemplo).
- El `id` es estable y no cambia al renombrar; la sesión guarda solo ese id.
- No se puede borrar el último perfil ni el predeterminado sin elegir otro.
- Borrar un perfil que usan pestañas abiertas pide confirmación: esas pestañas
  no podrán arrancar de nuevo hasta reiniciarlas con otro perfil.
- «Detectar terminales» busca shells en `PATH` en segundo plano, muestra solo lo
  que no existe y fusiona sin duplicar id ni comando.
- El icono propio es opcional; si falta, Windows usa el del ejecutable y, en
  último término, el icono Tabler del perfil.

## Tipografía y fuentes

- Apariencia ofrece solo familias instaladas en el sistema. El selector se puede
  filtrar por texto y muestra siempre `Actual: …`; si la familia configurada ya
  no existe, se indica la que se está usando como respaldo.
- El historial por terminal (0–50 000 líneas, paso 1000) se aplica a las
  terminales nuevas; el tope de 32 MiB por terminal puede recortarlo en anchos
  grandes.
- Los sliders de tamaño, altura de línea, historial y escala usan los mismos rangos que la
  validación de `config.toml`.
- `[fonts].sources` sigue existiendo en el esquema y se conserva al guardar,
  pero el panel ya no lo edita: las fuentes adicionales se declaran a mano.

## Atajos

- La lista sale del registro de acciones de M6, agrupada por categoría; la
  búsqueda filtra por nombre, id y palabras clave.
- **Editar** en una fila inicia la captura de la siguiente pulsación en esa misma
  fila. `Supr`/`Backspace` quita el atajo y **Restaurar** recupera el de por
  defecto; **Cancelar** o `Esc` salen sin cambios.
- Se rechazan:
  - teclas reservadas a la terminal (`Ctrl+C/D/Z`, Tab, Esc, flechas y
    navegación);
  - combinaciones sin modificador para acciones de app;
  - combinaciones que ya usa otra acción, con el nombre de la acción en conflicto.
- El resultado se escribe en `[keymap]` usando `unbind` cuando el atajo elegido
  sustituye al de por defecto. Los contextos `workspace`, `sidebar` y `terminal`
  siguen siendo los únicos de `[keymap.*]`.
- `theme.info` es un id legado: un archivo existente que lo enlace sigue
  abriendo el panel, pero la ayuda ya muestra `settings.open`.

## Agentes

```toml
[agents]
enabled = true
scan_interval_ms = 1500
notify_finished = true
notify_error = true
notify_attention = true

[agents.preferred_profiles]
opencode = "pwsh"
copilot = "pwsh"
```

`preferred_profiles` asocia cada agente reconocido (`opencode`, `copilot`) con un
id de `[[profiles]]`. Se valida que el perfil exista. Lo consume el lanzador de
agentes (M11): al abrir OpenCode o Copilot desde el pie de la sidebar o el
desplegable del «+» se usa ese perfil si encaja con el host del proyecto; si no,
se cae al perfil del proyecto y al global con un aviso. Los valores de detección
y avisos se aplican en caliente al guardar.

## Archivos y valores por defecto

- El menú «Más» del pie muestra la ruta de `config.toml` y la de `themes/` como
  subtítulo de **Abrir config.toml** y **Abrir carpeta de temas**.
- **Restaurar valores por defecto** pide confirmación y restaura apariencia,
  tipografía, atajos y agentes. Los perfiles y `[fonts].sources` se conservan; el
  predeterminado se ajusta si hace falta para que siga existiendo.

## Rutas

| Plataforma | Directorio |
| --- | --- |
| Windows | `%APPDATA%\axfterm\` |
| macOS | `~/Library/Application Support/axfterm/` |
| Linux/BSD | `$XDG_CONFIG_HOME/axfterm/` o `~/.config/axfterm/` |

`AXFTERM_CONFIG_DIR` permite usar un directorio temporal en tests y diagnóstico.

## Pruebas y gate manual

Automatizado:

- `cargo test -p axf_config` cubre round-trip con comentarios y orden, escritura
  atómica, validación y rechazo sin tocar el archivo.
- `cargo test -p axfterm` cubre perfiles, comandos de agente por perfil,
  rebindeo/`unbind`, conflictos de atajos, perfiles preferidos de agentes,
  restauración de defaults y política de conflicto externo. También verifica que
  un borrador combinado de apariencia, fuentes, perfil, atajo y agente se escribe
  y conserva al recargar `config.toml`, y que restaurar defaults conserva
  `[fonts].sources`.

Verificado interactivamente en Studio visible:

- Se navegó Apariencia con eventos reales de teclado, se seleccionó Monokai
  Light y se confirmó que la UI y su preview reflejan el tema. Esc descartó el
  borrador y restauró One Dark sin persistir el experimento.

Gate manual pendiente:

1. Recorre las cuatro secciones con `Ctrl+Tab` y prueba `Ctrl+S`; cancela una
   captura de atajo inline con Esc.
2. Mueve los sliders de tamaño, altura de línea y escala hasta los extremos;
   comprueba la etiqueta, la preview al soltar y que `Descartar` restaura todo.
3. Cambia la fuente de UI y de terminal; comprueba `Actual: …`, la preview y que
   `config.toml` no cambió tras `Descartar`.
4. Edita a mano `config.toml` con el panel abierto y el borrador limpio: debe
   recargarse con aviso. Repite con cambios sin guardar: debe pedir decisión.
5. Crea un perfil desde el modal, cancela otro alta y comprueba que no deja
   residuos; duplica, reordena y borra uno en uso. Reinicia y comprueba ids y
   orden.
6. Rebinde a `Ctrl+Alt+Shift+N`, intenta robar `Ctrl+D` y el atajo de otra
   acción, y recupera el valor por defecto con Restaurar.
7. Activa/desactiva agentes, cambia el intervalo y el perfil preferido desde el
   selector; reinicia y comprueba que se mantienen.
8. Usa el menú «Más»: abre `config.toml` y la carpeta de temas, y restaura valores
   por defecto aceptando y cancelando la confirmación.
