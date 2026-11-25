# Tareas — M10 Rediseño de experiencia UI/UX

## Referencia y auditoría

- [x] Ejecutar y revisar las 34 escenas actuales de Studio para establecer la
      referencia de workspace, onboarding, sidebar, agentes, paletas, ajustes,
      atajos y galería.
- [x] Aislar el inicio de PowerShell del fixture de Studio para evitar que
      `Import-Clixml`/`InvalidOperation` contaminen las capturas.
- [x] Mapear recorridos: primer arranque, abrir proyecto/terminal, cambiar
      pestaña o panel, encontrar un agente y atender su estado, buscar/ejecutar
      comandos y editar ajustes.
- [x] Registrar hallazgos con escena, estado, problema observado, impacto,
      frecuencia y cambio propuesto; priorizar antes de tocar UI de producción.

## Dirección visual

- [x] Definir la tarea principal, jerarquía y decisión de layout por superficie
      a partir de los hallazgos priorizados.
- [x] Revisar tokens semánticos, métricas y componentes donde exista un motivo
      comprobable; conservar el tema activo como fuente de colores y estilos.
- [x] Probar la dirección en temas integrados claros/oscuros y tema
      personalizado antes de propagarla.

## Recorridos y superficies

- [x] Revisar primer arranque, workspace vacío y descubrimiento/creación de
      terminales sin añadir selector o breadcrumb de proyecto al área central.
- [x] Revisar la lectura y operación del workspace: sidebar, proyectos,
      pestañas, paneles, foco, títulos y navegación entre terminales.
- [x] Revisar las superficies de comandos y orientación: paletas, búsqueda,
      atajos y feedback de acciones.
- [x] Revisar ajustes y superficies de estado: jerarquía de secciones, errores,
      agentes, avisos, confirmaciones, vacíos y estados deshabilitados.
- [x] Mantener acciones esenciales visibles o disponibles por teclado; validar
      foco, estados y recuperación en cada flujo cambiado.

## Verificación y cierre

- [x] Actualizar escenas de Studio para cada superficie o estado que cambie.
- [x] Comparar capturas antes/después en tema oscuro, claro y personalizado,
      con escalas/tamaños de ventana representativos.
- [x] Completar revisión de accesibilidad y diseño: teclado, orden/restauración
      de foco, nombres accesibles, contraste, estados no basados solo en color,
      expansión de texto y ventanas estrechas.
- [x] Ejecutar `pwsh -File scripts/check.ps1`,
      `pwsh -File scripts/cargo-low.ps1 build --release`,
      `pwsh -File scripts/test-cargo-low.ps1` y
      `pwsh -File scripts/cargo-low.ps1 clippy -p axfterm --features studio --all-targets -- -D warnings`.
- [x] Actualizar docs y `progress.md` con evidencia y pendientes restantes.
