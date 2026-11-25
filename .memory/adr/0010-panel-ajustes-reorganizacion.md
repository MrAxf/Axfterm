# ADR 0010 — Reorganización del panel de ajustes

- **Estado**: Aceptado
- **Fecha**: 2026-11-24
- **Decidores**: Juan Andrés

## Contexto

ADR 0007 fijó un panel con siete secciones (Apariencia, Terminal, Perfiles,
Fuentes, Atajos, Agentes y Avanzado). El uso real y las capturas de M10 mostraron
fricciones:

- La tipografía de la aplicación y la de la terminal están separadas aunque se
  configuran juntas.
- La sección Fuentes solo servía para añadir archivos `.ttf/.otf/.ttc`; las
  familias del sistema (incluidas las Nerd Fonts instaladas) ya aparecen sin
  ella.
- Avanzado existía para dos rutas y una restauración, acciones que encajan en el
  pie del modal.
- Las acciones del perfil seleccionado quedaban debajo de una lista larga y las
  opciones se perdían al hacer scroll.
- La edición de atajos se iniciaba al pulsar la fila, sin una acción explícita.

Las reglas de borrador, validación, escritura atómica, conflicto externo y hot
reload de ADR 0007 siguen vigentes.

## Decisión

- El panel queda en **cuatro secciones**: Apariencia, Perfiles, Atajos y
  Agentes.
- **Apariencia** agrupa tema, tipografía de la aplicación y de la terminal,
  tamaños, altura de línea y escala. Las tarjetas de tema se compactan para que
  la tipografía quede visible en ventanas normales.
- Los tamaños y la escala se editan con **sliders** con los mismos rangos que
  `Config::validate`; el valor se actualiza al arrastrar y la previsualización
  completa se aplica al soltar.
- Los selectores de fuente muestran **solo familias instaladas**, permiten
  buscar y muestran siempre la familia activa o la de respaldo.
- **`[fonts].sources` deja de editarse desde el panel** y se conserva tal cual.
  Restaurar valores por defecto también lo conserva, igual que los perfiles.
- **Perfiles**: cada fila contiene sus acciones (editar, duplicar, mover,
  predeterminado, eliminar). Nuevo y Editar abren un modal; un perfil nuevo solo
  entra en el borrador al aplicar.
- **Atajos**: la lista se agrupa por categoría; Editar inicia una captura inline
  en la fila, con quitar, restaurar y cancelar.
- **Avanzado desaparece**: abrir `config.toml`, abrir la carpeta de temas y
  restaurar valores viven en un menú «Más» del pie. La ruta del archivo se muestra
  como subtítulo del elemento; restaurar pide confirmación en un modal.
- El modal no muestra la versión de esquema ni la ruta de `config.toml` en la
  cabecera. Las acciones del pie se alinean a la derecha.

## Consecuencias

### Positivas

- La configuración visual queda en un único sitio y se alcanza sin scroll en
  ventanas habituales.
- Los perfiles y atajos se editan sin perder de vista la lista, y el teclado
  mantiene acciones explícitas.
- Se elimina una sección que solo duplicaba lo que el sistema ya ofrece.
- El pie concentra las acciones poco frecuentes sin competir con Guardar.

### Negativas y riesgos

- Quien añadía fuentes desde la UI pierde ese atajo; Mitigación: se documenta
  que las familias instaladas se usan directamente y `[fonts].sources` sigue
  disponible en el archivo.
- Los modales anidados añaden capas de foco y Esc; Mitigación: el orden de cierre
  es captura → perfil → confirmación → panel, con foco atrapado por capa.
- Restaurar defaults ahora conserva más estado del que el texto antiguo
  indicaba; Mitigación: la confirmación enumera qué se conserva.
- Los sliders pueden recompilar el tema al soltar; Mitigación: no se re-enumeran
  fuentes si familias y orígenes no cambian, y no se previsualiza por frame.

## Alternativas

- **Mantener siete secciones y solo retocar estilos**: no resuelve la duplicación
  ni el scroll de acciones por perfil; rechazado.
- **Sección Fuentes como modal independiente**: conserva una función que el
  sistema operativo ya cubre y añade otra capa; rechazado.
- **Restaurar valores con un botón directo en el pie**: demasiado destructivo sin
  confirmación; rechazado.
- **Editar atajos con un modal por acción**: más pasos que la captura inline;
  rechazado.

## Referencias

- ADR 0007 — panel de ajustes y escritura de config (reglas de borrador y
  conflicto, aún vigentes).
- ADR 0005 — límites de v0.1.
- `.memory/milestones/07-panel-ajustes/` — implementación original.
- `.memory/milestones/10-experiencia-ui-ux/audit.md` — hallazgos de uso.
- `docs/settings.md` — comportamiento resultante.
