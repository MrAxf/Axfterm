# Tareas — M2 Layout, proyectos y pestañas

## Modelo

- [ ] `Project`, `PaneTree`, `Pane`, `Tab`, `TerminalId`; IDs estables con serde
- [ ] Operaciones puras: split, close, move tab, focus, resize ratio
- [ ] Tests del árbol: colapso, ratios e invariantes

## Paneles

- [ ] Split horizontal/vertical con divisor arrastrable y doble clic para reset
- [ ] Foco por click y por teclado; borde de foco visible
- [ ] Cerrar panel con confirmación si hay procesos vivos
- [ ] Colapsar panel y zoom temporal
- [ ] Layout mínimo con 1 panel siempre presente

## Pestañas

- [ ] Tab strip por panel + botón de nueva pestaña
- [ ] Título dinámico (OSC) con fallback a shell/cwd
- [ ] Cerrar, renombrar, duplicar y mover entre paneles
- [ ] Drag-reorder con preview y auto-scroll
- [ ] Menú contextual de pestaña
- [ ] Overflow: ancho mínimo/máximo, rueda, chevrons, dropdown "todas"
- [ ] La pestaña activa siempre visible al cambiar de foco
- [ ] Indicadores: agente, atención y proceso en curso

## Proyectos

- [ ] Abrir carpeta (diálogo + argumento CLI), dedupe por path canónico
- [ ] Cambiar de proyecto conserva layout y terminales
- [ ] Nueva terminal en el cwd del proyecto
- [ ] Cerrar proyecto con confirmación
- [ ] Sidebar de proyectos simple (sin agentes todavía)

## Sesión

- [ ] Guardar/cargar `session.json`: proyectos, layout, pestañas y foco
- [ ] Versión de esquema + migración y fallback a layout limpio
- [ ] No restaurar procesos muertos: marcar pestaña como terminada

## Gate

- [ ] Estrés manual: 4 paneles / 20 pestañas / 3 proyectos
- [ ] Cerrar app y restaurar sin pérdida de layout
