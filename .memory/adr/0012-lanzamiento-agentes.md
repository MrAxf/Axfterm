# ADR 0012 — Lanzamiento de agentes desde la app

- **Estado**: Aceptado
- **Fecha**: 2026-11-25
- **Decidores**: Juan Andrés

## Contexto

Axfterm detecta OpenCode y Copilot dentro de las terminales y muestra su estado,
pero para trabajar con un agente hay que abrir una terminal y escribir el
comando. `[agents.preferred_profiles]` ya se edita y se valida, aunque hasta
ahora solo se persiste: la app no tenía un lanzador.

El producto quiere un acceso de un solo gesto en el pie de la sidebar y otro en
el desplegable del «+» de la pestaña, sin convertirlos en un gestor de sesiones
ni en un selector de agentes.

## Decisión

### Puntos de entrada

- Pie fijo de la sidebar, bajo la lista, con un acceso por agente reconocido
  (`AgentKind::ALL`: OpenCode y Copilot). Con ancho suficiente muestra marca y
  etiqueta; por debajo del umbral de acciones de la sidebar se reduce a iconos
  con tooltip. El tooltip incluye el atajo y el estado agregado del agente en el
  proyecto.
- Dos ítems en el desplegable del «+» de cada panel, junto a «Nueva terminal».
  Actúan sobre ese panel y su proyecto actual.
- Acciones únicas `agent.open_opencode` y `agent.open_copilot`, con atajos
  `Ctrl+Alt+1` y `Ctrl+Alt+2`. Se registran en los contextos `Workspace` y
  `Sidebar`, nunca en `Terminal`, y no ocupan teclas reservadas; con una terminal
  enfocada disparan como el resto de acciones de app, mientras AltGr e IME se
  entregan como texto. Alimentan pie, menú, paleta, ayuda y `[keymap]`.

### Pestaña nueva, siempre

Cada gesto abre una pestaña nueva; no se busca ni se enfoca una sesión existente
del mismo agente. Evitar duplicados exige decidir qué conversación «es» la del
proyecto y rompe el gesto predecible de «abrir otro agente». Si el proyecto ya
tiene sesiones, el pie lo indica en el tooltip, pero el clic siempre abre otra.

Se respetan los límites vigentes: sin proyecto se abre el flujo de «Abrir
carpeta» con aviso; con 32 pestañas se rechaza con aviso visible.

### Precedencia de perfil

1. `[agents.preferred_profiles]` del agente, si el perfil existe y encaja con el
   host del proyecto.
2. Perfil del proyecto (`Project.default_profile`).
3. Perfil global de `[terminal]`.

Compatibilidad de host:

- Proyecto local: el perfil preferido no puede ser un `wsl`; abriría fuera de la
  carpeta del proyecto.
- Proyecto WSL: el perfil preferido debe arrancar en la misma distro.

Un preferido inexistente o incompatible no impide lanzar: se usa el siguiente
perfil y se muestra un aviso con el motivo. Un perfil de proyecto declarado pero
inexistente es un error visible, nunca un fallback silencioso (ADR 0006/0011).
El perfil del proyecto no se revalida contra el host más allá de lo que ya hace
`resolve_project_profile`; es una elección explícita del usuario.

### Cómo arranca el agente

La pestaña se crea con el perfil resuelto y su shell habitual. Cuando el PTY
está listo se escribe una línea con el comando del agente, igual que la
reanudación actual (`send_command`). No se sustituye el programa del PTY.

- El comando base es `[profiles.agent_commands]` del perfil o, si falta,
  `opencode` / `copilot`. No se añaden argumentos ni se interpreta sintaxis de
  shell: la línea la ejecuta el shell del perfil.
- Al salir del agente se vuelve al shell, que es donde el usuario espera
  quedarse tras una TUI.
- El `--cd` de WSL ya lo aplica el lanzamiento normal de la terminal; no hay
  código nuevo de host.
- Un ejecutable ausente lo reporta el propio shell en la pestaña y se acompaña
  de un aviso accionable; no se intenta detectar en `PATH` desde la app.
- La inyección no bloquea el hilo de UI: usa el mismo temporizador breve que la
  reanudación y escribe cuando la terminal sigue viva.

## Consecuencias

### Positivas

- `[agents.preferred_profiles]` deja de ser configuración muerta.
- Un solo contrato sirve para el pie, el menú, la paleta y `[keymap]`.
- El cwd del proyecto y el host se heredan del spawn existente, incluido WSL.
- Los errores siguen siendo visibles y locales a la pestaña.

### Negativas y riesgos

- Cada pulsación puede duplicar sesiones del mismo agente en el proyecto; es una
  decisión deliberada y el coste lo asume el usuario.
- Enviar la línea tras el spawn depende de que el shell esté leyendo entrada;
  el buffer del PTY lo cubre, pero conviene el gate manual con PowerShell, cmd,
  Git Bash y WSL.
- La validación de host del preferido es más estricta que la de un perfil de
  proyecto elegido a mano; se documenta y se avisa.
- El estado mostrado es informativo y no altera el comportamiento del clic.

## Alternativas

- **Enfocar la sesión viva del agente**: menos pestañas, pero ambigua con varias
  conversaciones y contrario al gesto pedido. Se descarta.
- **Sustituir el programa del PTY por el agente**: ahorra un shell, pero pierde
  el retorno al prompt, complica WSL y rompe la reanudación por pestaña. Se
  descarta.
- **Un único «Abrir agente…» con submenú**: más pasos y más superficie para dos
  agentes estables. Se descarta hasta que exista un registro configurable.
- **No validar el preferido y dejar que falle el spawn**: convierte un error de
  configuración en una pestaña rota. Se descarta.

## Referencias

- `.memory/milestones/11-accesos-agentes/tasks.md`
- ADR 0006 — perfiles de terminal.
- ADR 0009 — detección y estado de agentes.
- ADR 0011 — proyectos WSL y hosts.
- `docs/agents.md`
