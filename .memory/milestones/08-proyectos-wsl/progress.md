# Progreso — M8 Proyectos en WSL

Lo más reciente arriba.

- 2026-11-24: la sidebar recupera el botón de carpeta junto a «Nueva terminal»;
  el caret del desplegable queda a su derecha con «Carpeta local…» y «Proyecto
  en WSL…». Capturas `app-sidebar-terminals{,-light}.png`; `check.ps1` en verde.
- 2026-11-24: gate end-to-end en Studio con Ubuntu real: abrir el modal desde
  `app.wsl.live`, `Ctrl+Enter` sobre `/home/juanandres`, crear el proyecto,
  arrancar el prompt WSL, nueva pestaña, split, restaurar tras reiniciar y
  cerrar la pestaña (el `bash` de la sesión desapareció). Capturas en
  `target/studio/shots/app-wsl*.png`; tema claro validado. `check.ps1`, clippy
  con Studio, release y `test-cargo-low.ps1` en verde.
- 2026-11-24: implementación completa de la base: `ProjectHost` y sesión v3,
  adaptador WSL con `--cd` y navegador de carpetas, botón partido en la sidebar
  y onboarding, modal con distro/perfil/ruta/lista, perfil fijo por proyecto,
  badge y salud de proyectos, y documentación. Verificado con Ubuntu real:
  `realpath`, listado de `/home`, ConPTY con `wsl --cd`, y capturas de Studio
  `app.wsl` y `app.wsl.live`. Quedan los gates manuales de abrir desde el modal,
  reinicio, cierre del árbol y DPI/tema claro.
- 2026-11-24: abierto el milestone y el ADR 0011. Renumerados los milestones
  anteriores: persistencia/release pasa a M9 y el rediseño UI/UX a M10. La
  implementación empieza por el modelo y el adaptador WSL.
