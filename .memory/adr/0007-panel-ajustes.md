# ADR 0007 — Panel de ajustes y escritura de config

- **Estado**: Aceptado
- **Fecha**: 2026-11-22
- **Decidores**: Juan Andrés
- **Revisión 2026-11-24**: la lista de secciones, la ubicación de las acciones
  avanzadas y la edición de perfiles se sustituyen por ADR 0010. Las reglas de
  borrador, guardado, conflicto y hot reload de este ADR siguen vigentes.

## Contexto

ADR 0004 fijó que, en v0.1, `config.toml` era la única interfaz y no había GUI
de ajustes. El producto necesita ahora editar apariencia, fuentes, perfiles y
atajos desde la app sin perder la garantía de que el archivo sigue siendo la
fuente de verdad, se puede editar a mano y se recarga en caliente.

Restricciones reales:

- El archivo tiene comentarios y orden manual que el usuario espera conservar.
- Un guardado a medias puede dejar la app sin tema o sin perfiles.
- El hot reload ya observa el archivo; la GUI no puede competir con él.
- El teclado manda: el panel debe ser operable sin ratón.
- Los atajos solo se pueden validar contra el registro de acciones de M6.
- Sin telemetría ni secretos en config.

## Decisión

### Superficie

- El panel es un overlay dentro de la ventana principal, no una ventana ni una
  app aparte.
- Se abre con la acción `OpenSettings`, disponible en el contexto `Settings` y
  desde el menú, la paleta y el icono de la barra de título. (El icono nació en
  la antigua barra de estado, retirada en M3; la barra de título es su sitio.)
- Secciones: Apariencia, Terminal, Perfiles, Fuentes, Atajos y Avanzado. El
  registro de secciones es extensible; Agentes llega de M5.
- Foco atrapado mientras está abierto; Esc cierra y devuelve el foco a la
  terminal o superficie anterior. Toda la navegación y edición se hace con
  teclado.

### Edición y guardado

- El panel trabaja sobre un borrador de `Config`; no modifica el estado activo
  hasta guardar.
- Apariencia, tema y fuentes pueden previsualizarse en vivo. Cancelar restaura
  el estado visual anterior.
- Guardar: validar el borrador completo, serializar con `toml_edit` sobre el
  archivo actual para conservar comentarios y orden, volver a parsear el
  resultado y escribirlo con `write_atomic`.
- Solo se tocan las secciones modificadas. Un error de validación o de escritura
  bloquea el guardado y muestra campo y ruta; no se aplica nada a medias.
- Tras guardar, la app aplica el archivo resultante. El evento del watcher
  provocado por la propia escritura se reconoce por contenido/mtime y no se
  aplica dos veces.
- No hay auto-guardado por pulsación ni almacén paralelo: el archivo es la
  fuente y el borrador solo vive mientras el panel está abierto.

### Conflictos con el exterior

- Si `config.toml` cambia en disco mientras el panel está abierto y el borrador
  está limpio, se recarga el borrador y se avisa.
- Si el borrador tiene cambios, se muestra un conflicto: recargar (descarta el
  borrador) o seguir editando (guardar requiere confirmación explícita porque
  sobrescribe el cambio externo). Nunca se pisa en silencio.
- Un error de hot reload con el panel abierto conserva la última config válida y
  se muestra en el propio panel.

### Secciones y validación

- Los campos usan los mismos rangos y errores tipados que `Config::validate`;
  el panel no duplica reglas.
- Perfiles: alta, edición, duplicado, borrado protegido (no el último, no el
  predeterminado sin elegir otro), orden, icono, perfil por defecto y
  «Detectar terminales» con fusión sin duplicados.
- Atajos: se editan contra `ActionMeta` del registro de M6; duplicados, teclas
  reservadas y `unbind` se validan antes de escribir.
- Fuentes: orígenes con selector de archivo o carpeta; rutas relativas al
  directorio de config.
- Avanzado: rutas de config y temas, abrir `config.toml`, versión y restauración
  de valores por defecto con confirmación.

## Consecuencias

### Positivas

- El usuario deja de necesitar editar TOML para tareas cotidianas sin perder la
  posibilidad de hacerlo.
- El archivo conserva comentarios y orden, así que sigue siendo legible y
  versionable.
- La validación es única: la misma que usa el arranque y el hot reload.
- El panel es keyboard-first y extensible a agentes y futuras opciones.

### Negativas y riesgos

- `toml_edit` añade una dependencia (MIT/Apache, del mismo ecosistema que
  `toml`); Mitigación: se usa solo en config y se cubre con round-trip tests.
- La semántica de "solo secciones modificadas" puede perder formato si un
  usuario reordena a mano; Mitigación: no reordenar lo no tocado y testear con
  la plantilla completa.
- Dos fuentes de cambio (panel y editor externo) exigen una política de
  conflicto; Mitigación: borrador + aviso + confirmación, nunca merge automático.
- La preview en vivo puede dejar el estado visual distinto si la app cae con el
  panel abierto; Mitigación: el archivo no se escribe hasta Guardar y al
  arrancar se aplica el archivo.

## Alternativas

- **Regenerar el TOML desde los defaults**: simple, pero borra comentarios,
  orden y documentación de la plantilla; rechazado.
- **Archivo `settings.json` aparte**: dos fuentes de verdad y hot reload doble;
  rechazado.
- **Guardar en cada cambio**: multiplica escrituras, watcher y estados a medias;
  rechazado.
- **Ventana de ajustes separada**: rompe la ventana única y el modelo de foco;
  rechazado.
- **Panel solo de lectura**: no cumple el objetivo de configurar la app desde la
  UI; rechazado.

## Nota de cierre M9 (2026-11-25)

- `axf_config::write_config_checked(path, expected, base, draft)` cierra la
  carrera entre la comprobación del borrador y la escritura: relee el archivo y
  devuelve `ConfigError::Conflict` si el texto ya no coincide con el que el panel
  guardó como baseline. La única vía que ignora esa comprobación es la
  sobrescritura explícita tras el conflicto.
- La UI muestra el conflicto como error de guardado y conserva tanto el borrador
  como el archivo externo; no hay merge automático.

## Referencias

- `.memory/milestones/07-panel-ajustes/tasks.md` — implementación y tests.
- ADR 0004 — config TOML, versionado, hot reload y recuperación.
- ADR 0006 — perfiles de terminal.
- `.memory/milestones/06-teclado-paleta/tasks.md` — registro de acciones y
  validación de atajos.
- `AGENTS.md` — teclado, hilos y minimalismo.
