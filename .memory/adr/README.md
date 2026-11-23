# Decisiones de arquitectura (ADR)

Registro de las decisiones que no se deducen del código y que condicionan
milestones futuros. Un ADR aceptado no se reescribe: si cambia la decisión, se
añade uno nuevo que lo sustituya y se actualiza el estado del anterior.

## Formato

Cada ADR usa MADR-lite en español:

- **Estado**: `Propuesto`, `Aceptado`, `Sustituido por NNNN` o `Rechazado`.
- **Fecha** y **decidores**.
- **Contexto**: el problema y las restricciones reales.
- **Decisión**: qué se hace, en términos verificables.
- **Consecuencias**: lo que se gana, lo que cuesta y los riesgos con su
  mitigación.
- **Alternativas**: qué se descartó y por qué.
- **Referencias**: spikes, otros ADR, `AGENTS.md` y tareas.

## Índice

| ADR | Decisión | Estado | Fecha |
| --- | --- | --- | --- |
| [0001](0001-motor-vt-pty.md) | Motor VT y PTY: `alacritty_terminal` + `portable-pty` | Aceptado | 2026-11-21 |
| [0002](0002-gpui-kit-escape.md) | GPUI encapsulado en `axf_ui`, escape a `gpui-base` | Aceptado | 2026-11-21 |
| [0003](0003-workspace-layout.md) | Workspace global, árbol binario y proyectos como clasificación | Aceptado | 2026-11-21 |
| [0004](0004-config-rutas-versionado.md) | Config TOML, rutas, versionado y recuperación | Aceptado | 2026-11-21 |
| [0005](0005-limites-v01.md) | Límites y no objetivos de v0.1 | Aceptado | 2026-11-21 |
| [0006](0006-perfiles-terminal.md) | Perfiles de terminal, comandos e iconos | Aceptado | 2026-11-22 |
| [0007](0007-panel-ajustes.md) | Panel de ajustes y escritura de config | Aceptado | 2026-11-22 |
| [0008](0008-restauracion-historial.md) | Restauración perezosa del historial de terminal | Aceptado | 2026-11-22 |
| [0009](0009-agentes-deteccion-estado.md) | Detección y estado de agentes con OSC 7501 y árbol de procesos | Aceptado | 2026-11-22 |

## Cuándo escribir uno

- Se elige una dependencia estructural o se fija una versión.
- Se define un modelo de datos que la sesión persiste.
- Se decide un límite de producto, rendimiento o plataforma.
- Se escoge una solución que cierra una alternativa que alguien intentará
  reabrir.

No hace falta ADR para detalles locales de implementación, nombres o formato de
código: eso vive en `AGENTS.md` y en los tests.
