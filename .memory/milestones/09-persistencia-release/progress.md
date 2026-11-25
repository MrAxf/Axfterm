# Progreso — M9 Persistencia y release

## 2026-11-25 — Release 0.1.1

- Publicada la release 0.1.1 con los cambios posteriores a 0.1.0: comandos de
  agente por perfil, Shift+Enter, drag & drop de archivos con activación por
  hover y las correcciones de restauración de historial y de OpenCode 2.
- `Cargo.toml`, `CHANGELOG.md`, `README.md` y `docs/release.md` alineados a
  0.1.1; el changelog conserva la 0.1.0 ya publicada y añade la sección nueva.

## 2026-11-25 — Cursor y prompt al restaurar el historial

- Al reproducir una instantánea se fuerza `ESC[?25h` y el desplazamiento al
  final. Un programa que dejó el cursor oculto (por ejemplo, `docker compose`
  v5 con logs en streaming) conserva su historial, pero el prompt nuevo se
  dibuja con cursor visible y la terminal vuelve a ser usable.
- El PTY y el grid restaurado comparten desde el primer byte el tamaño de la
  instantánea; el primer frame lo ajusta a la ventana. Corrige las pestañas WSL
  que se quedaban sin prompt: el shell pedía DSR y recibía una fila fuera de su
  vista, así que esperaba indefinidamente.
- Tests en `axf_terminal`: snapshot con `?25l` que se conserva y recupera el
  cursor al restaurar, y tamaño inicial del PTY derivado de la instantánea.
- Docs: `docs/session.md`, `docs/troubleshooting.md`, ADR 0008 y CHANGELOG.

## 2026-11-25 — Cierre de M9: release, robustez y marca

Implementado el bloque que faltaba de M9. Quedan solo gates manuales de matriz
DPI, teclados ES/CJK y un día de uso.

- **Marca e icono**: `crates/ui/assets/brand/axfterm-mark.svg` (monocromo,
  tintado por el tema) y `axfterm-logo.svg`; `scripts/make-icon.ps1` genera el
  `.ico` multi-tamaño. La barra de título usa la marca y `build.rs` embebe icono,
  `ProductName`, `FileVersion`, copyright y `OriginalFilename` con
  `winresource`.
- **Scrollback configurable**: `terminal.scrollback_lines` (0–50 000, defecto
  10 000) con tope efectivo de 32 MiB por terminal calculado desde el ancho;
  al ensanchar se recorta el historial antiguo. Slider en Apariencia, límite en
  ADR 0005 y tests.
- **Logs y crash**: `AXFTERM_LOG` opt-in a `<config>/logs/axfterm.log` con
  rotación de 2 MiB; crash reports con backtrace acotados a 10 archivos. El
  primer frame deja `primer frame en N ms` para medir arranque.
- **Instancia única**: mutex local + named pipe con `PIPE_REJECT_REMOTE_CLIENTS`.
  La segunda instancia reenvía sus carpetas y sale; la primera abre cada carpeta
  y activa la ventana. Los archivos se resuelven a su carpeta (Explorer puede
  pasar un archivo).
- **Escritura de config**: `write_config_checked` compara el texto esperado y
  devuelve `ConfigError::Conflict` si cambió entre la comprobación y la
  escritura; la sobrescritura solo ocurre con confirmación explícita.
- **Empaquetado**: `scripts/package.ps1` genera zip portable y MSI per-user con
  WiX 5 (`.config/dotnet-tools.json`), menú Inicio, `App Paths`, «Abrir con
  Axfterm» en carpetas/fondo y hashes SHA-256.
- **Docs**: README, `docs/config.md`, `docs/diagnostics.md`,
  `docs/troubleshooting.md`, `docs/release.md`, `CHANGELOG.md` y NOTICE.

### Verificación

- `scripts/check.ps1` en verde (fmt, clippy con `-D warnings`, tests; el test de
  entorno del PTY se hizo robusto ante un frame completo vacío de ConPTY).
- `build --release` correcto; `VersionInfo` del `.exe` muestra 0.1.0 y el icono
  se extrae con `ExtractAssociatedIcon`.
- MSI instalado y desinstalado con `msiexec /qn`: exe, acceso directo,
  `App Paths` y verbos de Explorer correctos; `%APPDATA%\axfterm` se conserva.
- Zip portable generado con exe, README, licencias, NOTICE y `docs/`.
- Instancia única: segundo proceso con la misma carpeta sale con código 0 y no
  deja otro proceso; el primero sigue vivo.
- Benchmarks release:
  - arranque tibio hasta primer frame: 259, 273 y 267 ms (p95 < 300 ms);
    primer arranque con descubrimiento: 469 ms;
  - RAM en reposo con una terminal: 73,5–74,4 MB (< 150 MB);
  - motor: 76,9 MiB/s, resize 0,75 ms/ciclo, frame 0,11 ms;
  - grid 80×24: total p95 0,637 ms (completo) y 0,873 ms (incremental), < 8 ms;
  - grid 200×50: total p95 1,878 ms y 2,096 ms, < 16 ms;
  - fuzzy: 0,68 µs/comparación (2 000 000 en 1,36 s).
- Studio verificó visualmente la barra de título con la marca (`app.workspace`)
  y el slider `Historial (terminales nuevas)` en Apariencia
  (`app.settings.appearance`), sin recortes.
- Pendiente manual: Windows 10/11 con DPI 100/150/200, teclado ES y CJK/IME, y
  gate de un día completo.

## 2026-11-22 — Restauración de historial (adelanto)

Implementado antes de M5 para cerrar la pérdida de contexto al reabrir. No se
adelantan las migraciones ni el empaquetado de M9; solo este bloque.

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
