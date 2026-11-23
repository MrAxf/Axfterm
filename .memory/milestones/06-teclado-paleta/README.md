# M6 — Teclado y command palette

Objetivo: operar todo el producto sin ratón, con atajos configurables por
contexto, sin robar teclas a la terminal, y una paleta que ejecute acciones y
salte a terminales, perfiles, proyectos y agentes, revelando proyectos en la
sidebar.

## Criterio de hecho

- [x] Todo el flujo diario es operable solo con teclado, incluido abrir una
      terminal con cualquier perfil.
- [x] `Ctrl+C` y demás teclas de terminal siguen llegando al PTY.
- [x] Conflictos y teclas reservadas se detectan y explican.
- [x] La paleta de comandos ejecuta acciones y la de búsqueda salta a
      pestañas, paneles, proyectos, perfiles y agentes; temas y fuentes viven
      en el modal correspondiente.
- [x] Existe una pantalla de atajos generada desde el registro de acciones.
