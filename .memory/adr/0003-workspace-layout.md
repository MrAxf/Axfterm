# ADR 0003 — Workspace global, layout y proyectos

- **Estado**: Aceptado
- **Fecha**: 2026-11-21
- **Decidores**: Juan Andrés

## Contexto

La restricción de producto es explícita: **los proyectos son una clasificación
de la sidebar**, no contenedores de layout. Un panel puede mezclar pestañas de
proyectos distintos, una pestaña conserva su proyecto al moverse y el proyecto
actual se deriva de la pestaña enfocada. No puede existir un `active_project`.

Al mismo tiempo, M2 exige paneles divisibles, pestañas reordenables, cierre con
confirmación, estados vacíos accionables y restauración de sesión sin recrear
PTYs. El modelo debe hacer imposibles los estados inválidos (pestaña en dos
paneles, foco a una pestaña cerrada, pestaña sin proyecto) y ser barato de
serializar.

## Decisión

### Modelo

```text
WindowState {
  projects: Vec<Project>,
  layout: PaneTree,
  tabs: HashMap<TabId, Tab>,
  panes: HashMap<PaneId, Pane>,
  focus: Focus,
  sidebars: SidebarState,
}

Project    { id, root: PathBuf, name }
PaneTree   = Split { axis, ratio, first, second } | Leaf(PaneId)
Pane       { tabs: Vec<TabId>, active: Option<TabId> }
Tab        { project_id, terminal_id, title, agent: Option<AgentBinding> }
Focus      { pane: PaneId, tab: Option<TabId> }
SidebarState { visible, width, expanded: Set<ProjectId>, filter }
```

- `ProjectId`, `PaneId`, `TabId` y `TerminalId` son newtypes sobre `u64`,
  serializables y estables durante la sesión. Un contador persistido evita
  reutilizar IDs al restaurar.
- `PaneTree` es un **árbol binario**: cada split tiene eje (`horizontal` /
  `vertical`), dos hijos y un ratio en `(0, 1)`. Las hojas son `PaneId`.
- El ratio vive en el split, no en el panel. Un panel no tiene geometría propia;
  su tamaño se deriva del camino desde la raíz.
- `Pane::active` y `Focus::tab` son `Option` para representar paneles y
  workspace vacíos sin IDs centinela.

### Invariantes

1. Cada `TabId` aparece como máximo en un panel y, si está en `WindowState.tabs`,
   exactamente en uno.
2. `active` es `None` o una pestaña del propio panel.
3. `Focus.tab` es `None` o la pestaña activa del panel enfocado.
4. Todo `PaneId` es alcanzable desde la raíz exactamente una vez; un split tiene
   dos hijos no vacíos.
5. `0 < min_ratio <= ratio <= 1 - min_ratio`; el mínimo se deriva de las
   métricas (ancho/alto mínimo usable) y se recalcula al cambiar de fuente.
6. El orden de `projects` es el orden de la sidebar; no hay IDs duplicados ni
   dos proyectos con el mismo `root` canónico.
7. `Tab.project_id` existe en `projects`; `Tab.terminal_id` pertenece a esa
   pestaña y a ninguna otra.
8. Siempre existe al menos un panel (la raíz). Un panel vacío solo es válido si
   es el último panel; cerrar la última pestaña de un panel no raíz colapsa ese
   panel y enfoca el vecino.

### Operaciones

Las operaciones del modelo son funciones puras que devuelven `Result` con error
tipado: `split_pane`, `close_pane`, `close_tab`, `move_tab`, `reorder_tab`,
`focus_tab`, `focus_pane`, `resize_split`, `register_project`,
`remove_project`, `reorder_projects`, `set_project_expanded`.

- **Split**: divide el panel enfocado en el eje pedido; el nuevo panel nace con
  una pestaña nueva o vacío según la acción, y recibe el foco.
- **Mover pestaña**: cambia el `PaneId` de pertenencia; no toca `terminal_id`,
  `project_id` ni el PTY.
- **Cerrar pestaña**: si era la activa, enfoca la vecina derecha y luego la
  izquierda del mismo panel. Si el panel queda vacío y no es la raíz, se
  colapsa y se enfoca el panel hermano.
- **Cerrar panel**: con procesos vivos pide confirmación; al cerrar, las
  pestañas se destruyen con sus PTYs y el split se sustituye por el hermano,
  conservando el ratio global del padre.
- **Quitar proyecto**: cierra todas sus pestañas (con confirmación si hay
  procesos vivos), repara el foco y elimina el proyecto aunque quede vacío el
  workspace.
- **Registrar proyecto**: canonicaliza la ruta, deduplica por `root` y añade al
  final; abrir una carpeta sin pestañas crea la primera terminal, con pestañas
  solo registra y expande.

### Foco y proyecto actual

- `focus` apunta a un panel y, si existe, a su pestaña activa.
- El proyecto actual es `tabs[focus.tab].project_id`. Si no hay pestaña
  enfocada, no hay proyecto actual; la UI muestra el estado vacío.
- La sidebar agrupa por proyecto y deriva sus filas de `layout`/`tabs`; no
  guarda una copia del árbol.
- Al cerrar o mover la pestaña enfocada, el foco se repara en la misma
  operación; nunca queda apuntando a un ID eliminado.

### Persistencia

`session.json` (versión 1) guarda: `projects` con orden, `expanded` y proyectos
vacíos; el árbol con `PaneId` y ratios; cada pestaña con `project_id`,
`terminal_id`, título y estado de agente; `focus`; y el contador de IDs. Al
restaurar no se recrean PTYs: una pestaña cuyo proceso ya no existe se marca
como terminada. La migración y recuperación se detallan en ADR 0004.

## Consecuencias

### Positivas

- Mezclar proyectos en un panel es trivial y no requiere mover terminales.
- El proyecto actual no se puede desincronizar: se deriva del foco.
- Los invariantes se testean sin GPUI ni PTY, con operaciones puras.
- El árbol binario hace simples el split, el colapso y el cálculo de ratios.
- Los IDs estables permiten restaurar layout y pestañas sin ambigüedad.

### Negativas y riesgos

- Hay dos niveles de identidad (panel y pestaña) y reparar el foco al cerrar
  requiere tests exhaustivos.
- El árbol binario puede generar splits visualmente "torcidos" si el usuario
  divide mucho; el mínimo de ratio y el reset a 0.5 lo contienen.
- La sesión depende de `PaneId`; cambiar el esquema exige migración.
- La sidebar debe recalcular filas al mover pestañas; se resuelve con consultas
  derivadas y tests, no con estado duplicado.

## Alternativas

- **Layout por proyecto** (cada proyecto con su árbol): contradice la mezcla de
  pestañas y obliga a un selector de proyecto que el producto prohíbe.
- **Árbol n-ario / grid**: más fácil para cuadrículas iguales, mucho más
  complejo para splits arbitrarios, colapso y ratios estables.
- **La pestaña dueña de su rectángulo**: simplifica el render, pero rompe el
  resize proporcional y el colapso.
- **`active_project` cacheado**: evita derivar, pero se desincroniza en cuanto
  una pestaña se mueve o se cierra.

## Referencias

- `AGENTS.md` — modelo, restricciones de producto y flujo de cierre.
- `.memory/milestones/02-layout-proyectos/tasks.md` — operaciones y gate.
- ADR 0001 — el PTY se ata a `TerminalId`.
- ADR 0004 — formato y migración de `session.json`.

## Nota de implementación (M2/M5, 2026-11-22)

- `canonicalize` en Windows devuelve la forma verbatim (`\\?\C:\...`). Ese
  prefijo no se puede mapear a una unidad: usado como CWD, PowerShell muestra
  `Microsoft.PowerShell.Core\FileSystem::\\?\C:\...` en el prompt.
- El `root` se guarda normalizado con `axf_terminal::paths::normalize`: se quita
  `\\?\` de unidades y de `\\?\UNC\…`, y se conserva solo cuando la ruta supera
  `MAX_PATH`, donde `CreateProcessW` lo necesita para arrancar el shell.
  `repair()` normaliza los roots de sesiones antiguas antes de deduplicar y
  `usable_cwd` repite la normalización al arrancar el PTY como red de seguridad.

## Nota de enmienda (M5, 2026-11-22)

- `Tab.agent: Option<AgentBinding>` pasa a `Tab.agents: Vec<AgentSession>`.
  El estado de los agentes es runtime-only y no se serializa en la sesión; una
  pestaña puede tener varias sesiones. Ver ADR 0009.

