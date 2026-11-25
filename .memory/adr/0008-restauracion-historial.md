# ADR 0008 — Restauración de historial de terminal

- **Estado**: Aceptado
- **Fecha**: 2026-11-22
- **Decidores**: Juan Andrés

## Contexto

La sesión (`session.json`, ADR 0004) ya restaura proyectos, paneles, pestañas,
perfiles y foco, pero no el contenido de cada terminal: al reabrir, cada pestaña
muestra el estado «el proceso anterior ya terminó» y hay que reiniciarla. El
usuario pierde el scrollback de la sesión anterior.

Restaurar el proceso es imposible dentro de v0.1: los PTY viven dentro del
proceso de Axfterm y un daemon de sesiones está fuera de alcance
(ADR 0005). Por tanto, «restaurar» significa recuperar el buffer visible y el
historial, y arrancar un shell nuevo con el mismo perfil.

Abrir los 32 PTYs a la vez al arrancar tampoco es aceptable: el arranque y la
memoria se disparan y la mayoría de pestañas no se van a usar. La restauración
tiene que ser perezosa.

## Decisión

### Cuándo se guarda

- Solo en cierre ordenado (`request_quit`, que también cubre cerrar la ventana).
  Cada parser vivo recibe una petición de instantánea, la UI espera las
  respuestas y escribe los archivos antes de guardar `session.json` y salir.
- Un crash, un `taskkill` o un corte de luz pierde la última tanda. El
  autoguardado periódico queda para M9 si las mediciones lo justifican.

### Formato

- Un archivo por terminal en `<config>/scrollback/<epoch>-<terminal_id>.snap`.
  No se meten en `session.json`: su tope es 4 MiB y crecerían sin control.
- El cuerpo es una transcripción ANSI del grid (historial + pantalla) generada
  desde las celdas reales: texto, colores de 16/256/truecolor, negrita, dim,
  cursiva, subrayado, reverse, oculto, tachado, caracteres anchos y
  combinantes.
- Cabecera versionada y checksum FNV-1a 64. Un archivo con versión futura,
  checksum incorrecto, dimensiones absurdas o tamaño excesivo se descarta.
- No se guardan hipervínculos OSC 8, selección, modos de ratón, bracketed paste,
  pantalla alternativa ni posición de scroll. El historial se reabre abajo y el
  shell nuevo decide sus propios modos. Al restaurar se fuerza `ESC[?25h`: un
  programa que ocultó el cursor y no lo repuso (por ejemplo, `docker compose`
  v5 con logs en streaming) no deja la terminal sin prompt utilizable.
- Se conservan las líneas más recientes si la transcripción supera el límite.
  El flag `truncated` viaja en la cabecera para avisar al restaurar.
- Escritura atómica con `axf_config::write_atomic`.

### Límite

- **2 MiB por terminal** (ADR 0005). Con el máximo de 32 pestañas son 64 MiB
  como máximo, más los archivos de pestañas no visitadas de la misma sesión.
- El tope se aplica al cuerpo ANSI; si se alcanza, se descartan las líneas más
  antiguas y se marca `truncated`.

### Restauración perezosa

- Al arrancar se carga `session.json` como siempre, pero solo se arranca la
  pestaña activa de cada panel visible. Las pestañas ocultas no crean PTY.
- El PTY se abre con las dimensiones de la instantánea y el primer frame lo
  redimensiona al hueco real; el historial se refluye. Así un shell que pide la
  posición del cursor (DSR, habitual en WSL) recibe una fila dentro de su vista
  y dibuja el prompt en lugar de quedarse esperando.
- Al activar una pestaña (clic, teclado, paleta o foco de panel) se lee su
  instantánea en el ejecutor de fondo, se inyecta en el parser **antes** de
  consumir los bytes del PTY y se arranca el perfil guardado en la pestaña.
- Si no hay instantánea, la terminal se abre normal desde cero.
- Si la instantánea no se puede leer o validar, se ignora, se abre el **mismo
  perfil** desde cero y se muestra un aviso. Nunca se cambia de perfil en
  silencio: un perfil ausente conserva su pantalla de error accionable.

### `epoch`

- Cada ejecución usa un `epoch` guardado en `<config>/scrollback/epoch`.
- Si la sesión se recupera desde cero (archivo ausente o corrupto), se genera
  un `epoch` nuevo. Los archivos de la vida anterior no se restauran por error
  en una pestaña nueva con un `TerminalId` reutilizado y se podan al cerrar.
- Al cerrar se conservan los archivos de todas las pestañas del workspace,
  estén o no arrancadas: una pestaña sin visitar mantiene su historial. Los de
  pestañas cerradas se eliminan.

## Alternativas

- **Guardar el historial dentro de `session.json`**: supera el tope de 4 MiB,
  hace la sesión lenta de parsear y mezcla dos ciclos de vida distintos.
- **Reproducir los bytes crudos del PTY**: puede reactivar la pantalla
  alternativa, responder consultas DSR al shell nuevo y arrastrar OSC de la
  sesión anterior. La transcripción generada solo contiene SGR y texto.
- **Serializar celdas y escribirlas directamente en `Grid`**: rompe las
  invariantes internas de `Row` (`occ`) y obliga a depender de detalles de
  alacritty. Reproducir ANSI reutiliza el parser y el reflow existentes.
- **Guardar solo texto plano**: pierde colores y atributos que el usuario
  espera encontrar al volver.
- **Restaurar todos los PTYs al arrancar**: contradice el requisito de arranque
  y memoria de ADR 0005.

## Consecuencias

### Positivas

- Reabrir la app devuelve el contexto visual y el scrollback de cada pestaña.
- El coste de arranque no depende del número de pestañas guardadas.
- Un archivo corrupto no impide trabajar: fallback visible a terminal nueva.
- El formato es legible y testeable sin levantar una ventana.
- Un programa que dejó la terminal sin cursor no impide usar el prompt nuevo al
  reabrir: el cursor se normaliza durante la restauración.

### Negativas y riesgos

- El historial contiene el texto que se mostró en la terminal, incluidos
  posibles secretos. Se guarda bajo `%APPDATA%\axfterm\scrollback` con los
  permisos del usuario; no se cifra ni se sincroniza.
- Un cierre no ordenado pierde la última tanda de historial.
- Los enlaces, la selección y la posición de scroll no se recuperan.
- La transcripción puede ocupar hasta 64 MiB en disco; la poda por `epoch` y
  por pestaña cerrada mantiene el directorio acotado.

## Referencias

- ADR 0001 — motor VT/PTY.
- ADR 0004 — formato y recuperación de la sesión.
- ADR 0005 — límites de v0.1.
- ADR 0006 — perfiles y ausencia de fallback silencioso.
- `.memory/milestones/09-persistencia-release/tasks.md` — seguimiento.
