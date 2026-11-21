# Tareas — M5 Agentes y sidebar

## Sidebar

- [ ] `SidebarHost` izquierda/derecha, ancho persistido y colapso
- [ ] Trait `SidebarPanel` + registro de paneles
- [ ] Mover panel entre slots, ocultar y mostrar
- [ ] Placeholders de contenedores y puertos (sin lógica)

## UI de agentes

- [ ] Árbol proyecto -> agentes/terminales
- [ ] Badge de estado, duración y última actividad
- [ ] Click/Enter salta a la pestaña; acciones de menú contextual
- [ ] Filtro y búsqueda dentro de la sidebar
- [ ] Notificación de atención: badge, título de ventana y toast opcional

## Detección

- [ ] PID raíz por PTY + árbol `sysinfo` cada 1-2 s
- [ ] Matcher `opencode`/`copilot` por exe y argv, con exclusiones
- [ ] Binding agente <-> pestaña con PID y reattach tras cambiar de proyecto
- [ ] Máquina de estados: Idle, Working, NeedsAttention, Stopped
- [ ] Señales: bell, OSC 9/777/1337, tasa de salida, quiescencia y exit
- [ ] Latch de atención hasta foco/ack; override manual de estado

## Lanzar agentes

- [ ] Abrir terminal con agente en el cwd del proyecto
- [ ] Perfiles y argumentos por agente en config
- [ ] Detección de binario faltante y guía de instalación

## Tests

- [ ] CLIs falsos en PowerShell y sh para cada estado
- [ ] Tests deterministas de la máquina de estados
- [ ] Gate: 2 proyectos, opencode + copilot, salto por teclado
