# M7 — Panel de ajustes

Objetivo: editar toda la configuración vigente desde la app (apariencia, tema,
fuentes, terminal, perfiles, atajos y agentes) sin abandonar el teclado,
escribiendo siempre en `config.toml` con validación y sin dejar estados a medias.

## Criterio de hecho

- [ ] Todas las opciones vigentes son editables desde el panel.
- [ ] La escritura preserva comentarios y es atómica; si falla no se aplica nada.
- [ ] El preview de tema y fuentes se revierte con Cancelar.
- [ ] Un cambio externo de `config.toml` con el panel abierto se explica y nunca
      se pisa en silencio.
- [ ] Todo el panel es operable solo con teclado y devuelve el foco al salir.
- [ ] Perfiles: alta, edición, duplicado, orden, icono, predeterminado y
      «Detectar terminales».
- [ ] Atajos: rebind, `unbind` y conflictos validados contra el registro de M6.
- [ ] Agentes: activación, intervalo de escaneo, avisos y perfil preferido por
      agente.
