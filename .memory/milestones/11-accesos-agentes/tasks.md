# Tareas — M11 Accesos rápidos a agentes

## Decisión y contrato

- [x] Escribir el ADR 0012 «Lanzamiento de agentes»: precedencia de perfil por
      host, inyección del comando tras el spawn frente a sustituir el programa
      del PTY, pestaña nueva por pulsación, semántica de errores y atajos por
      defecto.
- [ ] Capturar en Studio el pie de la sidebar (oscuro, claro, personalizado y
      sidebar normal). La escena `app.sidebar.agent-actions` existe; la captura
      quedó bloqueada porque la instancia instalada de Axfterm ocupaba el mutex
      de instancia única.

## Dominio, acciones y lanzamiento

- [x] Exponer una iteración estable de los agentes reconocidos
      (`AgentKind::ALL` en `axf_agents`, orden OpenCode, Copilot) para pie, menú
      y acciones.
- [x] Registrar `agent.open_opencode` y `agent.open_copilot` en
      `actions_parts/01_actioncategory.rs` con categoría Agente, contextos
      `Workspace` y `Sidebar`, `Ctrl+Alt+1` / `Ctrl+Alt+2`, keywords, acción y
      dispatch.
- [x] Implementar `Shell::launch_agent(kind, pane)`: resolver el proyecto (foco
      actual, selección de sidebar, primer proyecto) y el panel; sin proyecto,
      abrir el flujo de «Abrir carpeta» con aviso accionable.
- [x] Resolver el perfil: `[agents.preferred_profiles]` del agente si existe y
      es compatible con el host del proyecto; si no, el perfil del proyecto; si
      tampoco, el global. Un preferido ausente o de otro host avisa y cae al
      siguiente; un perfil de proyecto inexistente es error visible.
- [x] Crear la pestaña con el perfil resuelto y, cuando el PTY está listo,
      escribir el comando del agente (override de `[profiles.agent_commands]` o
      el base) con `send_command`, reutilizando el patrón de la reanudación y
      sin bloquear el hilo de UI.
- [x] Acotar los avisos: no proyecto, preferido incompatible, límite de 32
      pestañas y shell que no arranca; la deduplicación existente evita apilar
      toasts idénticos.
- [x] Tests unitarios de resolución de perfil y comando: local, WSL compatible,
      WSL incompatible, preferido ausente y override de agente.

## Superficies

- [x] Pie de la sidebar bajo la lista: dos accesos de icono con tooltip (atajo
      y estado del agente en el proyecto), en una fila que puede crecer con más
      agentes; debajo, versión y resumen de proyectos, terminales y agentes
      vivos.
- [x] Desplegable del «+» del panel: ítems «Abrir OpenCode» y «Abrir Copilot»
      con icono y atajo, separados de perfiles y de dividir; mismo helper y
      mismos avisos.
- [x] Teclado y foco: alcanzable por atajo, paleta y tabulación; sin ocupar
      teclas reservadas del terminal y respetando AltGr/IME; el foco vuelve a la
      terminal nueva.
- [x] Estados y copy: sin proyecto, preferido incompatible, límite alcanzado y
      error de arranque con texto accionable y sin sorpresas.

## Verificación y cierre

- [ ] Escena de Studio del pie; falta comparar capturas en tres temas y con
      sidebar normal y compacta (bloqueado por la instancia abierta).
- [x] Tests deterministas de lanzamiento, resolución de perfil, comando,
      recuento de agentes vivos y resumen del pie.
- [x] Pasar `pwsh -File scripts/check.ps1`,
      `pwsh -File scripts/cargo-low.ps1 build --release`,
      `pwsh -File scripts/test-cargo-low.ps1` y el clippy de Studio.
- [x] Actualizar `docs/agents.md` (el lanzador deja de ser futuro y describe el
      flujo), `docs/keymap.md` (acciones y atajos), `docs/settings.md` (el
      perfil preferido ya se consume), `docs/studio.md`, `CHANGELOG.md` y
      `progress.md`.
- [ ] Gate manual con `opencode` y `copilot` reales, en local y en WSL:
      apertura, cwd correcto, ejecutable ausente y salida del agente al shell.
