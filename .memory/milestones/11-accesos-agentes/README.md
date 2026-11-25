# M11 — Accesos rápidos a agentes

Objetivo: abrir OpenCode o Copilot en una pestaña nueva del proyecto actual con
un solo gesto, desde el pie de la sidebar y desde el desplegable del «+» de la
pestaña. El agente arranca directamente en esa pestaña, sin escribir el comando
a mano, y la app consume por fin `[agents.preferred_profiles]`, que hasta ahora
solo se persistía. Objetivo de entrega: v0.2.

## Alcance

- Pie fijo de la sidebar bajo la lista, visible tanto en `Todas` como en
  `Agentes`, con un acceso por agente reconocido (OpenCode y Copilot). Muestra
  marca, etiqueta y atajo, y se reduce a iconos con tooltip cuando el ancho no
  permite la etiqueta.
- Ítems «Abrir OpenCode» y «Abrir Copilot» en el desplegable del «+» de la
  pestaña, actuando sobre el panel y el proyecto activos.
- Acciones registradas una sola vez (`agent.open_opencode` y
  `agent.open_copilot`) que alimentan pie, menú, paleta, ayuda de atajos y
  `[keymap]`.
- Resolución de perfil y comando, apertura en el cwd del proyecto (incluido
  WSL), avisos de error y límites existentes.
- Referencia visual y verificación en Axfterm Studio.

No se busca reutilizar una sesión viva: cada pulsación abre una pestaña nueva.
Si ya hay un agente del mismo tipo corriendo en el proyecto, el pie puede
mostrarlo como estado informativo, pero el clic siempre abre otra pestaña.

## Fuera de alcance

- Enfocar o evitar duplicados de sesiones existentes.
- Selector configurable de agentes, agentes de terceros o lista dinámica.
- Argumentos extra, variables de entorno y perfiles por proyecto.
- Detección del binario en `PATH` antes de lanzar: el error del shell es la
  señal visible y se acompaña de un aviso in-app.
- Eventos ACP, notificaciones nativas y apertura remota por SSH.

## Criterio de hecho

- [x] El pie de la sidebar ofrece un acceso por agente con icono de marca,
      tooltip (atajo y estado en el proyecto) y una fila que puede crecer con
      más agentes; debajo resume versión, proyectos, terminales y agentes
      activos.
- [x] El desplegable del «+» incluye «Abrir OpenCode» y «Abrir Copilot» y actúa
      sobre el panel y el proyecto activos.
- [x] Ambos gestos crean una pestaña nueva con el perfil resuelto y escriben el
      comando del agente en su shell en cuanto el PTY está listo: el agente se
      ve arrancar en la pestaña y, al salir, se vuelve al shell.
- [x] Precedencia de perfil: `[agents.preferred_profiles]` si es compatible con
      el host del proyecto; si no, el perfil del proyecto;
      `[profiles.agent_commands]` sobrescribe siempre el ejecutable. Una
      preferencia inválida es un aviso accionable y nunca un cambio silencioso.
- [x] `agent.open_opencode` y `agent.open_copilot` están registradas una sola
      vez, aparecen en paleta y ayuda, y usan `Ctrl+Alt+1` / `Ctrl+Alt+2` sin
      ocupar teclas reservadas del terminal.
- [x] Sin proyecto, sin perfiles, preferido de otro host, binario ausente y
      límite de 32 pestañas producen estados o avisos explícitos; nada cae a
      otro shell ni a otra carpeta en silencio.
- [x] La apertura no bloquea el hilo de UI y el estado mostrado sigue viniendo
      de las sesiones en runtime.
- [ ] Escenas de Studio del pie en tema oscuro, claro y personalizado, con
      sidebar normal; tests deterministas verdes (la escena existe y los tests
      pasan; falta capturarla con la instancia instalada cerrada).
- [x] `scripts/check.ps1`, el build release y las pruebas del wrapper pasan;
      docs, textos del panel de ajustes y `progress.md` actualizados.

## Orden de trabajo

Primero documentar la semántica en un ADR y fijar la referencia visual en
Studio. Después el helper de lanzamiento y las acciones; luego el pie y el menú.
Cada superficie se valida en Studio antes de extenderla y el cierre exige el
gate completo con `opencode` y `copilot` reales, en local y en WSL.
