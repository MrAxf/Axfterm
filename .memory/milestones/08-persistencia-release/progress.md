# Progreso — M8 Persistencia y release

## 2026-11-22 — Restauración de historial (adelanto)

Implementado antes de M5 para cerrar la pérdida de contexto al reabrir. No se
adelantan las migraciones ni el empaquetado de M8; solo este bloque.

- `axf_terminal::snapshot`: transcripción ANSI versionada con checksum, epoch y
  tope de 2 MiB; conserva colores, atributos, caracteres anchos y combinantes.
- `ParserSetup::restore` reproduce el historial antes de consumir los bytes del
  PTY; `ParserHandle::snapshot` exporta sin bloquear la UI.
- `axf_app::HistoryStore`: carga en segundo plano, guardado atómico al cerrar,
  poda de pestañas cerradas y de epochs anteriores.
- Arranque perezoso: solo la pestaña activa de cada panel visible crea PTY; el
  resto se restaura al activarse. Si el archivo falla, se abre el mismo perfil
  desde cero con aviso.
- ADR 0008, límite en ADR 0005, nota en `AGENTS.md` y `docs/session.md`.
- Tests: roundtrip de texto/historial, colores/atributos/anchos, recorte,
  corrupción y almacén.
- Gate manual con `AXFTERM_CONFIG_DIR` aislado: cierre ordenado crea el `.snap`;
  con dos pestañas solo arranca la activa (1 `pwsh`), el historial de la activa
  se restaura (dos prompts) y el de la no visitada se conserva; un `.snap`
  corrompido cae a terminal nueva con el mismo perfil y se reescribe.
- Verificado con dos paneles visibles: dos PTY al arrancar y cada snapshot
  restaurado por separado.
- Pendiente: gate con WSL.

## 2026-11-22 — Avisos y fallback de icono

- Un icono de perfil ausente ya no genera un aviso: se usa el genérico y el
  motivo queda en `tracing::debug!`. El aviso competía con los errores
  accionables y no ofrecía ninguna acción.
- `Toast` pasa de `max_w` a un ancho fijo (`UiMetrics::toast_width`) con la
  columna de texto en `flex_1().min_w_0()`: el texto envuelve dentro de la caja
  en vez de salirse por el borde derecho.
- Verificado con captura de la ventana: aviso de historial corrupto en tres
  líneas dentro del panel, sin desbordar; sin aviso al arrancar con un
  `icon = "icons/missing.png"`.
- `check.ps1` en verde (200 tests) y `build --release` correcto.
