# ADR 0005 — Límites y no objetivos de v0.1

- **Estado**: Aceptado
- **Fecha**: 2026-11-21
- **Decidores**: Juan Andrés

## Contexto

v0.1 debe ser una app usable a diario, no una demo con todas las features a
medio hacer. Cada límite tiene que ser explícito para que producto, tests y
documentación midan lo mismo, y para que una petición futura no se cuele como
"detalle" en un milestone ya cerrado.

Este ADR fija el alcance operativo. No sustituye a `AGENTS.md`: lo concreta con
números y con el comportamiento esperado al alcanzar cada límite.

## Decisión

### Plataforma

- **Windows 10 build 17763 (1809) o superior y Windows 11, x64.**
  - 1809 es la primera versión con ConPTY estable; es el mínimo real de
    `portable-pty`.
  - ARM64, macOS y Linux no se empaquetan en v0.1; el código se mantiene
    portable tras `cfg`, pero no se prueba en release.
- **Una ventana y un proceso.** Una segunda instancia reenvía la carpeta pedida
  a la existente y sale (M9). No hay pestañas en ventanas separadas.
- DPI 100/150/200 y teclados ES/CJK son requisito de release (M9).

### Límites operativos

| Recurso | Límite v0.1 | Comportamiento al alcanzarlo |
| --- | --- | --- |
| Terminales abiertas | 32 | La acción de nueva terminal se deshabilita con aviso accionable; no se crean PTYs extra |
| Perfiles de terminal | 32 | La validación rechaza el perfil nuevo con campo y límite; los existentes siguen funcionando |
| Nombre de perfil | 64 caracteres | Error de campo al guardar el perfil |
| Línea de comandos de perfil | 1024 caracteres | Error de campo; no se trunca |
| Icono de perfil | 2 MiB por imagen | Se ignora el icono y se usa el genérico sin aviso (fallback silencioso); la config sigue siendo válida |
| Scrollback por terminal | 10 000 líneas por defecto; configurable 0–50 000 | Se descartan las líneas más antiguas; además, tope efectivo de 32 MiB por terminal |
| Historial persistido por terminal | 2 MiB por archivo ANSI (máximo 32 pestañas) | Se descartan las líneas más antiguas y se avisa al restaurar (ADR 0008) |
| Texto pegado | 1 MiB | Se rechaza con aviso; nunca se trunca en silencio |
| `config.toml` | 1 MiB | Se trata como corrupto: backup + defaults + aviso |
| `session.json` | 4 MiB | Backup + workspace limpio |
| Escaneo de agentes | Cada 1,5 s, solo subárbol del PTY | Sin terminales no se escanea; nunca bloquea la UI |
| Frame de UI | < 8 ms a 80×24; < 16 ms a 200×50 | Presupuesto de CPU de prepaint+paint, no FPS |
| Arranque | p95 < 300 ms hasta primer frame (release, caliente, workspace vacío) | Se mide en M9; si no se cumple, se investiga antes de release |
| RAM en reposo | < 150 MB con una terminal | Se mide en M9; incluye cachés de fuente |
| Logs | Opt-in, rotación acotada (M9) | Sin telemetría ni envío de red |

- El tope de 32 MiB por terminal existe porque el coste de una celda depende del
  ancho: 50 000 líneas a 200 columnas no caben en el presupuesto de memoria.
  Se aplica el primero de los dos límites.
- Los límites de terminales, perfiles y pegado son fijos en v0.1, no
  configurables. Si un usuario los necesita, se documenta como caso de
  post-v0.1 con medición.
- Un límite alcanzado siempre produce un mensaje; nunca una pérdida silenciosa
  de datos.

### Dentro de alcance

- Terminales reales con PTY, VT, colores, resize, scrollback, selección, copiar
  y pegar.
- Paneles divisibles, pestañas, drag & drop, proyectos en la sidebar y
  restauración de sesión.
- Temas built-in y de usuario, fuentes separadas de UI y terminal, iconos.
- Perfiles de terminal con nombre, comando, icono y perfil por defecto, con
  detección local en el primer arranque.
- Keybindings configurables, command palette y panel de ajustes que escribe en
  `config.toml`.
- Detección local de `opencode` y `copilot` con estados explicables.
- Config TOML con hot reload y recuperación.
- Instalador MSI y zip portable para Windows x64.

### Fuera de alcance

Cada punto vive además en `.memory/milestones/99-futuro/tasks.md`:

- SSH/remoto con reconexión.
- Plugins de terceros.
- Contenedores, puertos y sidebar de Docker/Podman.
- Split dentro de una pestaña.
- Ligaduras y font features en terminal.
- Kitty graphics, sixel e imágenes inline.
- Búsqueda en scrollback.
- Daemon de sesiones que sobreviva al cierre de la app.
- Worktrees por proyecto.
- Sincronización de config/temas.
- Auto-update y firma de código (evaluar después de v0.1).
- Empaquetado macOS/Linux.
- Perfiles por proyecto y variables de entorno por perfil.
- Extracción nativa de iconos de ejecutable en macOS/Linux.
- Shell integration (OSC 133, marcas de comando).
- Múltiples ventanas y pestañas desacoplables.
- Soporte ARM64.
- Eventos del servidor/plugin de OpenCode (detección heurística local solamente).

### Revisión

Estos límites se revisan al cerrar v0.1 con las mediciones de M9. Cambiarlos
requiere un ADR nuevo que cite la medición que lo justifica; no se editan a
mitad de milestone.

## Consecuencias

### Positivas

- El alcance de v0.1 cabe en los milestones y se puede decir "no" con una razón.
- Los presupuestos de rendimiento y memoria son medibles y comparables.
- Los límites duros evitan que un caso patológico bloquee la app o agote RAM.
- La documentación de producto y los tests pueden citar los mismos números.

### Negativas y riesgos

- 32 terminales y 50 000 líneas pueden quedarse cortos para usuarios intensivos;
  Mitigación: mensajes claros y revisión post-v0.1 con datos.
- El tope de 32 MiB por terminal hace que el número de líneas efectivo dependa
  del ancho; debe explicarse en la documentación.
- El pegado de más de 1 MiB se rechaza, lo que puede sorprender al pegar logs;
  Mitigación: aviso con tamaño y sugerencia de usar un archivo.
- Los objetivos de arranque/RAM son aspiraciones hasta M9; si no se cumplen, el
  release se retrasa, no se bajan los números sin ADR.

## Alternativas

- **Sin límites duros**: más simple de implementar, pero un scrollback enorme o
  cientos de PTYs pueden agotar memoria y volver la app impredecible.
- **Límites configurables desde v0.1**: añade validación y superficies de error
  sin evidencia de uso; se pospone.
- **Solo Windows 11**: reduce la matriz, pero deja fuera equipos con Windows 10
  que ConPTY soporta desde 1809.
- **Multi-ventana desde v0.1**: multiplica el modelo de foco, sesión y
  persistencia; fuera del objetivo del producto.

## Referencias

- `AGENTS.md` — restricciones de producto y "Fuera de alcance v0.1".
- `.memory/milestones/99-futuro/tasks.md` — tareas post-v0.1.
- `.memory/milestones/09-persistencia-release/tasks.md` — límites y mediciones.
- ADR 0001 — rendimiento medido de PTY/VT.
- ADR 0004 — recuperación ante archivos por encima del límite.
