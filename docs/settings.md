# Panel de ajustes

El panel edita toda la configuración vigente sin dejar de ser una vista del
archivo: `config.toml` sigue siendo la fuente de verdad. Se abre con el icono de
la barra de título, con el menú del panel (`Ajustes…`) o con `Ctrl+Alt+T`, y se
cierra con Esc devolviendo el foco a la terminal o superficie anterior.

## Secciones

| Sección | Qué edita |
| --- | --- |
| Apariencia | Sistema/claro/oscuro, tema del catálogo, fuente de la UI, tamaño base y escala. |
| Terminal | Fuente monoespaciada, tamaño de celda y altura de línea, con muestra. |
| Perfiles | Alta, edición, duplicado, borrado protegido, orden, icono, predeterminado y «Detectar terminales». |
| Fuentes | Orígenes `.ttf`/`.otf`/`.ttc`: archivos o carpetas, máximo 16. |
| Atajos | Rebindeo, `unbind`, restauración y conflictos contra el registro de acciones. |
| Agentes | Activación, intervalo de escaneo, avisos y perfil preferido por agente. |
| Avanzado | Rutas, versión de esquema, apertura del archivo y valores por defecto. |

Todo el panel es operable con teclado:

- `Tab` / `Shift+Tab` recorren secciones, campos y botones en orden visual.
- `Enter` o `Espacio` activan la fila enfocada.
- `Ctrl+Tab` / `Ctrl+Shift+Tab` cambian de sección.
- En un valor numérico, `←`/`→` lo disminuyen o aumentan; los botones `−`/`+`
  hacen lo mismo con el ratón.
- `Ctrl+S` guarda; `Esc` descarta el borrador (o cancela una captura/edición
  inline antes de cerrar).

## Borrador, preview y guardado

- El panel copia la config activa a un borrador. Nada se escribe hasta pulsar
  **Guardar**.
- Apariencia, tema, fuentes y métricas se previsualizan en vivo. **Descartar**
  restaura el estado visual anterior sin tocar `config.toml`.
- Al guardar se valida el borrador completo con las mismas reglas que el
  arranque, se edita el documento actual con `toml_edit` y se vuelve a parsear
  antes de escribir de forma atómica.
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

- El `id` es estable y no cambia al renombrar; la sesión guarda solo ese id.
- No se puede borrar el último perfil ni el predeterminado sin elegir otro.
- Borrar un perfil que usan pestañas abiertas pide confirmación: esas pestañas
  no podrán arrancar de nuevo hasta reiniciarlas con otro perfil.
- «Detectar terminales» busca shells en `PATH` en segundo plano, muestra solo lo
  que no existe y fusiona sin duplicar id ni comando.
- El icono propio es opcional; si falta, Windows usa el del ejecutable y, en
  último término, el icono Tabler del perfil.

## Fuentes

- Las rutas dentro del directorio de configuración se guardan relativas.
- Añadir o quitar un origen relee e instala las fuentes en caliente durante la
  previsualización; si el archivo no es válido, se muestra el error y se usa la
  cadena de fallback.
- El límite de v0.1 es 16 orígenes (ADR 0005).

## Atajos

- La lista sale del registro de acciones de M6; la búsqueda filtra por nombre,
  id y palabras clave.
- `Enter` sobre una acción captura la siguiente pulsación. `Supr`/`Backspace`
  quita el atajo y **Restaurar** recupera el de por defecto.
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
id de `[[profiles]]`. Se valida que el perfil exista. En v0.1 solo se guarda: lo
consumirá el lanzador de agentes de v0.2. Los valores de detección y avisos se
aplican en caliente al guardar.

## Avanzado y valores por defecto

- Muestra la ruta de `config.toml`, la de `themes/` y la versión de esquema.
- **Abrir config.toml** y **Abrir temas** usan la aplicación del sistema.
- **Restaurar valores por defecto** pide confirmación y restaura apariencia,
  terminal, fuentes, atajos y agentes. Los perfiles se conservan y el
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
- `cargo test -p axfterm` cubre perfiles, rebindeo/`unbind`, conflictos de
  atajos, perfiles preferidos de agentes, restauración de defaults y política de
  conflicto externo.

Gate manual recomendado:

1. Abre el panel con `Ctrl+Alt+T` y recórrelo solo con teclado, incluidos
   `Ctrl+Tab`, `Ctrl+S` y Esc.
2. Cambia tema y fuente de la terminal; comprueba la preview, pulsa `Descartar`
   y verifica que todo vuelve y que `config.toml` no cambió.
3. Edita a mano `config.toml` con el panel abierto y el borrador limpio: debe
   recargarse con aviso. Repite con cambios sin guardar: debe pedir decisión.
4. Crea, duplica, reordena y borra un perfil; intenta borrar el predeterminado y
   uno en uso. Reinicia y comprueba que el orden y los ids se conservan.
5. Rebinde a `Ctrl+Alt+Shift+N`, intenta robar `Ctrl+D` y el atajo de otra
   acción, y recupera el valor por defecto con Restaurar.
6. Activa/desactiva agentes, cambia el intervalo y el perfil preferido; reinicia
   y comprueba que se mantienen.
