# Tareas — M2 Layout, proyectos y pestañas

## Modelo

- [x] `Workspace`, `Project`, `PaneTree`, `Pane`, `Tab` con `project_id` y
      `Focus`; IDs estables con serde
- [x] Operaciones puras: split, close, move tab, focus, resize ratio y orden de
      proyectos
- [x] Tests del árbol: colapso, ratios e invariantes
- [x] Tests de proyectos: mover una pestaña conserva `project_id` y el foco
      deriva el proyecto actual

## Paneles

- [x] Split horizontal/vertical con divisor arrastrable y doble clic para reset
- [x] Foco por click y por teclado; borde de foco visible
- [x] Cerrar panel con confirmación si hay procesos vivos
- [x] Colapsar panel y zoom temporal
- [x] Layout mínimo con 1 panel siempre presente

## Pestañas

- [x] Tab strip por panel + botón de nueva pestaña
- [x] Título dinámico (OSC) con fallback a shell/cwd
- [x] Cerrar, renombrar, duplicar y mover entre paneles
- [x] Drag-reorder con preview y auto-scroll
- [x] Menú contextual de pestaña
- [x] Overflow: ancho mínimo/máximo, rueda, chevrons, dropdown "todas"
- [x] La pestaña activa siempre visible al cambiar de foco
- [x] Indicadores: agente, atención y proceso en curso

## Proyectos (solo sidebar)

- [x] Registrar proyecto (diálogo, CLI y drop de carpeta), dedupe por path
      canónico
- [x] Sidebar: cabecera de proyecto con plegado y pestañas agrupadas debajo
- [x] Nueva pestaña hereda el proyecto del foco (o de la fila invocada) y abre
      en su raíz
- [x] Abrir carpeta sin pestañas crea la primera terminal; con pestañas ya
      abiertas solo registra y expande
- [x] Click en una fila enfoca su pestaña; la sidebar revela y resalta el foco
- [x] Quitar proyecto cierra todas sus pestañas, con confirmación si hay
      procesos vivos
- [x] Un panel puede mezclar pestañas de varios proyectos; mover una pestaña
      conserva su proyecto
- [x] Estados: proyecto vacío, carpeta inexistente y workspace sin proyectos
      (onboarding "Abrir carpeta")
- [x] Navegación completa por teclado en contexto `Sidebar`

## Sesión

- [x] Guardar/cargar `session.json`: proyectos (incluidos los vacíos), orden,
      plegado de la sidebar, layout, pestañas con `project_id` y foco
- [x] Versión de esquema + migración y fallback a layout limpio
- [x] No restaurar procesos muertos: marcar pestaña como terminada

## Gate

- [x] Estrés manual: 4 paneles / 20 pestañas / 3 proyectos en la sidebar
- [x] Cerrar app y restaurar sin pérdida de layout ni de proyectos vacíos
