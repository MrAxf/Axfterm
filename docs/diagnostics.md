# Diagnóstico, logs y benchmarks

Axfterm no envía datos a ninguna red. Todo el diagnóstico se escribe en
`<config>/logs` y es opcional salvo los crash reports.

## Logs normales (opt-in)

Activa el subscriber al arrancar con `AXFTERM_LOG`:

| Valor | Nivel |
| --- | --- |
| `off`, `none`, `0` o vacío | Sin logs (por defecto) |
| `error` | Solo errores |
| `warn` | Avisos y errores |
| `info` | Arranque, primer frame y operaciones relevantes |
| `debug` | Detalle de configuración, agentes y WSL |
| `trace` | Todo el detalle disponible |

```powershell
$env:AXFTERM_LOG = "info"
axfterm
```

El archivo es `<config>/logs/axfterm.log`. Al superar 2 MiB se conserva una
sola copia en `axfterm.log.1`; no hay rotación por tiempo ni envío remoto.

El primer frame registra `primer frame en N ms`, que sirve para comparar el
presupuesto de arranque (p95 < 300 ms en release en caliente con workspace
vacío).

## Crash reports

Un panic escribe `logs/crash-YYYYMMDD-HHMMSS-<pid>.log` con versión, instante,
hilo, lugar, mensaje y backtrace de Rust. Se conservan los 10 más recientes; los
demás se eliminan al arrancar o al registrar un crash nuevo.

Los crash reports se escriben siempre porque son la única evidencia local de un
fallo. Pueden incluir rutas del proyecto; revisa el texto antes de compartirlo.
No se suben a ningún sitio automáticamente.

## Benchmarks

Motor VT/PTY, sin ventana:

```powershell
cargo run -p axf_terminal --release --example engine_bench
cargo run -p axf_terminal --release --example grid_bench -- 200 50 120
cargo run -p axf_terminal --release --example grid_bench -- 80 24 120
```

Presupuestos de ADR 0005:

- frame de UI: < 8 ms a 80×24 y < 16 ms a 200×50 (CPU de prepaint+paint);
- arranque: p95 < 300 ms hasta el primer frame en release en caliente;
- RAM en reposo: < 150 MB con una terminal.

El matcher difuso de la paleta tiene un benchmark manual ignorado por defecto:

```powershell
cargo test -p axfterm --release fuzzy_bench -- --ignored --nocapture
```

## Límites

- Scrollback: 10 000 líneas por defecto, configurable 0–50 000, con tope
  efectivo de 32 MiB por terminal. Al ensanchar la ventana se recorta el
  historial antiguo para respetar el tope.
- Historial persistido: 2 MiB por terminal (ADR 0008).
- Terminales: 32; la acción de nueva terminal avisa y no crea PTYs extra.
- `config.toml`: 1 MiB; `session.json`: 4 MiB. Un archivo por encima se trata
  como corrupto.
