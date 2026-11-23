# M8 — Persistencia, robustez y release v0.1

Objetivo: convertir el prototipo en una app instalable y fiable para uso diario,
con recuperación ante errores, empaquetado Windows y documentación.

## Criterio de hecho

- [ ] Config y sesión migran y se recuperan ante corrupción, incluidas las
      migraciones v1→v2 de perfiles.
- [ ] El historial de cada terminal se restaura de forma perezosa al activar su
      pestaña, con fallback visible si el archivo no es válido.
- [ ] La escritura desde el panel de ajustes nunca pierde el archivo válido.
- [ ] Instalador MSI y zip portable funcionan en Windows 10/11.
- [ ] Matriz DPI y teclados ES/CJK validada.
- [ ] Presupuestos de arranque, RAM y frame medidos y dentro de rango.
- [ ] Documentación completa y sin crashes conocidos.
- [ ] Uso real durante un día completo sin bloqueos.
