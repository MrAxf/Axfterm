# Progreso — M2 Layout, proyectos y pestañas

## 2026-11-22 — Root de proyecto sin prefijo verbatim

- `canonicalize` devolvía `\\?\C:\...` y ese valor se usaba como CWD del PTY;
  PowerShell 7 mostraba `Microsoft.PowerShell.Core\FileSystem::\\?\C:\...` en
  el prompt.
- `Project.root` se guarda normalizado (`axf_terminal::paths::normalize`), la
  sesión se sanea en `repair()` antes de deduplicar y el CWD se normaliza otra
  vez al arrancar el PTY. Tests de normalización, registro y reparación.

## 2026-11-21 — M2 cerrado

Implementado sobre el motor de M1 sin reescribir PTY/VT. El modelo es puro y la
UI de `axfterm` solo conecta el modelo con entidades de terminal.

### Modelo (`axf_workspace`)

- `ProjectId`, `PaneId`, `TabId` y `TerminalId` como newtypes serde con
  contadores persistidos (`IdCounters`); reparación de contadores al cargar.
- `Project`, `SidebarState` (`visible`, `width`, `expanded`, `filter`),
  `PaneTree` binario (`Horizontal`/`Vertical`, ratio clamp 0.1–0.9), `Pane`,
  `Tab` (`project_id`, `terminal_id`, `title`, `custom_title`, `agent`) y
  `Focus`.
- Operaciones puras: registrar/abrir/quitar/reordenar proyectos, crear,
  duplicar, cerrar, mover y reordenar pestañas, dividir/cerrar paneles, enfocar,
  redimensionar splits y actualizar título/agente.
- Invariantes: una pestaña en un panel, foco siempre vivo, un panel mínimo,
  hojas únicas, dedupe por root canónico y proyecto actual derivado del foco.
- `repair()` idempotente para sesiones corruptas o manipuladas.

### Sesión

- `session.json` v1 con proyectos (incluidos vacíos), orden, plegado, árbol,
  pestañas, foco y contadores.
- Escritura atómica (temporal + `sync` + rename), tope de 4 MiB, rechazo de
  versiones futuras sin tocar el archivo, backup fechado (`YYYYMMDD-HHMMSS`) y
  arranque limpio si está corrupto.
- `axf_config::paths` resuelve `%APPDATA%`/macOS/XDG y `AXFTERM_CONFIG_DIR`.
- La app guarda con debounce y espera la escritura antes de salir; cerrar la
  ventana usa el mismo camino que Ctrl+Q.

### UI (`axfterm`)

- Sidebar con cabeceras plegables, conteo, filas de pestaña, resaltado del foco,
  estados de proyecto vacío/carpeta inexistente/workspace vacío, botón y diálogo
  de “Abrir carpeta”, drop de carpeta y CLI.
- Navegación por teclado en contexto `Sidebar` (↑/↓, Home/End, ←/→, Enter,
  Esc) y acción `FocusSidebar` (`Ctrl+Alt+B`).
- Árbol de paneles recursivo con divisores arrastrables, doble clic a 0.5,
  borde de foco, zoom temporal, cierre con colapso y confirmación si hay
  procesos vivos.
- Tab strip por panel con título dinámico (OSC → cwd → proyecto), renombrado
  inline, cierre, duplicado, menú contextual, arrastre con preview y auto-scroll,
  drop en pestaña o cuerpo del panel, overflow con rueda, chevrons y dropdown
  “todas”.
- Indicadores de pestaña: proceso en curso y agente (contrato de agente;
  detección real en M5).
- Atajos de M2: `Ctrl+Shift+T`, `Ctrl+Shift+W`, `Ctrl+Alt+←/→`,
  `Ctrl+Shift+M/E/D/X/R/U/Z/P`, `Ctrl+Alt+↑/↓`, además de los menús.
- Placeholder de sesión restaurada: el PTY anterior no se recrea y la pestaña
  se puede reiniciar explícitamente.

### Verificación

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace` y `cargo build --release` en verde.
- 32 tests de workspace (árbol, proyectos, foco, reparación y sesión), 2 de
  rutas, 50 de terminal, 17 fixtures VT, 4 de assets y 1 de claves de split.
- Estrés automatizado: 4 paneles, 20 pestañas, 3 proyectos + proyecto vacío;
  guardado/carga conserva hojas, pestañas, proyectos y foco.
- Estrés visual real: sesión con 4 paneles/20 pestañas/3 proyectos restaurada en
  la ventana; sidebar, cuatro tab strips, overflow y placeholders correctos.
- Cierre por `WM_CLOSE` y reapertura verificados: la sesión se guarda y el
  layout/pestañas/proyectos se restauran sin recrear PTYs.
- Nota: el smoke test de UI con `gpui-kit/test-support` no se pudo activar con
  el pin actual: la feature arrastra `gpui-fast` y choca con `wgpu =30.0.1` de
  `gpui-pre` (`wgpu 30.0.2` ya fijado). Se cubre la lógica con tests puros y la
  ventana con arranque real + captura.

### Pendiente fuera de M2

- Pasada manual de teclado real con los nuevos atajos (se solapa con M6), y
  detección real de agentes (M5).
- Sustituir la captura de pantalla manual por un smoke de UI cuando el pin de
  gpui-kit permita `test-support` sin arrastrar `gpui-fast`.
