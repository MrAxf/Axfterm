# M9 — Persistencia, robustez y release v0.1

Objetivo: convertir el prototipo en una app instalable y fiable para uso diario,
con recuperación ante errores, empaquetado Windows y documentación.

## Criterio de hecho

- [x] Config y sesión migran y se recuperan ante corrupción, incluidas las
      migraciones v1→v2 de perfiles.
- [x] El historial de cada terminal se restaura de forma perezosa al activar su
      pestaña, con fallback visible si el archivo no es válido.
- [x] La escritura desde el panel de ajustes nunca pierde el archivo válido.
- [~] Instalador MSI y zip portable funcionan en Windows 10/11 (instalar,
      registrar y desinstalar validado en Windows 11; falta Windows 10).
- [ ] Matriz DPI y teclados ES/CJK validada (gate manual pendiente).
- [x] Presupuestos de arranque, RAM y frame medidos y dentro de rango.
- [x] Documentación completa y sin crashes conocidos.
- [ ] Uso real durante un día completo sin bloqueos (gate manual pendiente).
