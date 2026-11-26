# Tareas — M12 Refinamiento de controles y estados

## Decisión y fundaciones

- [x] Escribir el ADR 0013: tres alturas `rem`, tokens de interacción y
      proyección a gpui-kit.
- [x] Fijar `ControlSize::{Xs, Sm, Md}` con `Sm` por defecto y retirar `Lg`.
- [x] Alinear altura, texto, icono y padding con `Size` de gpui-kit.
- [x] Añadir `surface_hover`, `surface_active`, `accent_hover`, `accent_active`
      y `switch_off` a `UiTokens`, al TOML opcional y a la exportación.
- [x] Derivar hover/pressed de cada tono semántico sin colores literales.
- [x] Proyectar `accent`, `secondary`, `list`, `table`, `sidebar`, `switch` y
      `link` a los tokens nuevos.
- [x] Tests de escala, contraste y tokens distintos por built-in.

## Componentes

- [x] `Button` e `IconButton`: tamaño por defecto, hover, pulsado, foco sin
      desplazamiento, borde de secundario y `disabled` en icon buttons.
- [x] `TextField`: `Xs/Sm/Md` mapeados a `XSmall/Small/Medium` y ancho completo.
- [x] `SelectField`: selector con escala y etiqueta accesible.
- [x] `SplitButton` con dos mitades de gpui-kit, divisoria y etiqueta de menú.
- [x] Retirar el `Toggle` propio; la galería usa el `Switch` de producción.
- [x] Switches de ajustes con `accessibility_label`.
- [x] Menú presentacional de la galería alineado con el hover nuevo.

## Superficies

- [x] Bienvenida: grupo `Md`, «Nueva terminal» `Md` y fila que envuelve.
- [x] Sidebar: acciones `Md`, carpeta + menú como grupo, búsqueda y segmentos
      `Sm`, cierres `Sm`.
- [x] Tab strip: «+» + menú del panel como grupo `Sm`.
- [x] Ajustes: campos, selects, stepper y pie `Sm`; rail `Md`.
- [x] WSL: selects y campo `Sm`; acciones de ruta con superficie visible.
- [x] Paleta y atajos: búsqueda y segmentos `Sm`.
- [x] Barra de título: se mantienen los 24 px del chrome del sistema.

## Segunda pasada — cursor, hover y `rem` (2026-11-26)

- [x] Cursor de mano en botones, filas, pestañas, segmentados, switches y
      tarjetas; flecha en menús/chrome y resize en el divisor.
- [x] `SegmentedControl` único para sidebar, paleta y Apariencia, con foco por
      Tab/Enter/Espacio y selección visible.
- [x] `SwitchField` en `rem`, con hover y pulsado, en ajustes y galería.
- [x] Hover `surface_hover` y pulsado `surface_active` en filas, paletas,
      tarjetas, rail, steppers y acciones de perfil.
- [x] `Button::align_start` y `padding_x` para alinear la acción de la sidebar
      con la búsqueda.
- [x] Ancho de la sidebar proyectado desde la base de 14 px con tope del 45 %
      del viewport.
- [x] `SplitButton` con dos mitades de kit, mano, esquinas y divisoria propias.
- [x] Proyectar `button*` al tema de gpui-kit y reforzar el hover de `outline`.
- [x] Bienvenida con proyecto sin terminales: misma familia y altura; escenas
      `app.onboarding.project` y `.compact`.
- [x] Studio: reset de escala por escena y `ui_scale` en el manifiesto.
- [x] 38 escenas reproducidas y revisadas con los nuevos estados.
- [ ] Gate manual de hover, pulsado, foco, menús, grupos y switches.

## Galería y verificación

- [x] Galería: tres tamaños, switches reales, tokens y métricas nuevas.
- [x] Reproducir las 38 escenas de Studio y revisar One Dark, One Light y tema
      personalizado a 1440×900 y 1024×720 (compacto con escala 1,25).
- [ ] Gate manual de hover, pulsado, foco, menús, grupos y switches.
- [x] Pasar `pwsh -File scripts/check.ps1`,
      `pwsh -File scripts/cargo-low.ps1 build --release`,
      `pwsh -File scripts/test-cargo-low.ps1` y el clippy de Studio.
- [x] Actualizar capturas y `progress.md` con los resultados finales.
