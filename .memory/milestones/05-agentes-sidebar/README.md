# M5 — Agentes y sidebar

Objetivo: detectar los agentes que corren en cada terminal (`opencode` y su
alias `opencode2`, y `copilot`), mantener una sesión por proceso, mostrar su
estado en la sidebar y en el icono de la terminal, y avisar cuando terminen,
fallen o necesiten atención. La entrada en la paleta llega en M6 y el panel de
ajustes en M7.

## Criterio de hecho

- [x] `opencode`/`opencode2` y `copilot` detectados por árbol de procesos y por
      OSC 7501, con atribución a su pestaña.
- [x] Varias sesiones por pestaña, cada una con tipo, estado, mensaje, progreso,
      PID e instantes de inicio/actividad.
- [x] La sidebar se divide en `Terminales` y `Agentes`; Agentes agrupa las
      sesiones por proyecto y las ordena por prioridad.
- [x] Cada terminal con agentes muestra debajo de su fila un chip con icono,
      número de sesiones y estado agregado.
- [x] El icono de la terminal y de la pestaña usa la marca del agente dominante
      y lleva un círculo de estado superpuesto: verde/check al terminar,
      ámbar/! si requiere atención, neutro/spinner si trabaja y rojo/alerta si
      falla.
- [x] Clic o Enter en una sesión enfoca su pestaña, la selecciona y restaura la
      terminal si hace falta.
- [x] Se notifica en la app y en el título al terminar, fallar o requerir
      atención, sin duplicados ni avisos si la pestaña ya está enfocada.
- [x] Prioridad `NeedsAttention > Working > Error > Finished > Idle > Stopped`;
      atención, terminado y error se limpian al enfocar.
- [x] El escaneo, el parser y las notificaciones no bloquean el hilo de UI; no
      quedan estados de agente obsoletos tras reiniciar.
- [x] Escenarios con snapshots de procesos y CLIs falsos pasan de forma
      determinista.

Pendiente fuera del criterio: gate manual interactivo con `opencode2` y
`copilot` reales y los elementos de sidebar estructural/lanzar agentes, movidos
a `99-futuro`.
