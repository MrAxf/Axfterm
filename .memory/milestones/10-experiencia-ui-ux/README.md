# M10 — Rediseño de experiencia UI/UX

Objetivo: mejorar la experiencia completa de Axfterm mediante una revisión
basada en recorridos reales y capturas reproducibles. El milestone estaba
previsto después del release v0.1 (M9), pero se ejecutó antes por solicitud
explícita. Prioriza claridad y uso keyboard-first, y conserva los temas como
fuente visual de la aplicación.

## Alcance

- Workspace, sidebar, proyectos, pestañas, paneles y foco.
- Primer arranque, estados vacíos, búsqueda y creación de terminales.
- Estados de agentes, paletas de comandos/búsqueda y atajos.
- Ajustes, diálogos, avisos y estados de error.
- Revisión de tokens/componentes solo cuando la auditoría demuestre que hace
  falta; no es un cambio cosmético indiscriminado ni una reescritura de dominio.

Se pueden ajustar los tokens y temas integrados, pero la UI siempre responde al
tema activo. Se mantiene el soporte para variantes claras/oscuras y temas
personalizados.

## Criterio de hecho

- [x] Recorridos completos y fricciones priorizadas con evidencia visual y de
      interacción; cada cambio implementado responde a un hallazgo.
- [x] Axfterm Studio produce capturas reproducibles sin errores ajenos de
      inicialización del shell en el fixture.
- [x] El workspace y las superficies incluidas tienen jerarquía clara, estados
      comprensibles y acciones principales descubribles sin ratón.
- [x] El tema activo gobierna todas las superficies; los temas integrados,
      claro/oscuro y un tema personalizado se revisan después de los cambios.
- [x] Teclado, foco, accesibilidad, escalas/tamaños representativos y tamaños
      de ventana se verifican en las escenas afectadas.
- [x] Escenas, documentación y `progress.md` reflejan el estado final; pasan
      `scripts/check.ps1`, el clippy de Studio y los checks aplicables.

## Orden de trabajo

Primero estabilizar la referencia visual y documentar fricciones. Después
acordar la dirección/jerarquía y rediseñar los recorridos en cortes coherentes.
Cada corte se valida en Studio antes de extenderlo a otras superficies.
