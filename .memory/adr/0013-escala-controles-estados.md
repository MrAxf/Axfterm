# ADR 0013 — Escala de controles y proyección de estados

- **Estado**: Aceptado
- **Fecha**: 2026-11-25
- **Actualizado**: 2026-11-26
- **Decidores**: Juan Andrés

## Contexto

M10 revisó recorridos y jerarquía, pero dejó dos sistemas de control conviviendo:

- Los componentes propios usaban `ControlSize` con alturas de 1,25, 1,5, 1,75 y
  2,0 rem.
- gpui-kit usa `Size::{XSmall, Small, Medium, Large}` con 1,25, 1,5, 2,0 y 2,0
  rem.

El mismo nombre de tamaño describía alturas distintas según el componente. En
la bienvenida, la sidebar, los formularios y la galería aparecían botones de
24,5, 21 y 28 px en la misma fila. `TextField::Md` se proyectaba a `Input::Small`
(21), mientras un `Button::Md` medía 24,5.

Además, la proyección del tema anulaba estados visibles:

- `accent` y `popover` apuntaban a `surface_raised`, así que el hover y la
  selección de teclado de los menús usaban el mismo color que el menú.
- `primary_hover`, `primary_active`, `danger_hover` y equivalentes repetían el
  color base. Los botones rellenos no cambiaban al pasar el ratón.
- `switch` apuntaba al acento, igual que el estado encendido del switch.
- El foco de los botones propios añadía un borde solo al enfocar y desplazaba
  la geometría.

## Decisión

1. **Tres alturas de control**, derivadas del tamaño base de la fuente:
   - `Xs` = 1,25 rem, equivalente a `Size::XSmall`.
   - `Sm` = 1,5 rem, equivalente a `Size::Small` y tamaño por defecto.
   - `Md` = 2 rem, equivalente a `Size::Medium`, reservado a la acción principal
     de una página.
   - `ControlSize::Lg` desaparece. Si en el futuro hace falta una altura mayor,
     se añade con su equivalente explícito en gpui-kit; no se inventa un valor
     intermedio.
2. La tabla de `ControlSize` define juntos altura, tipografía, icono, padding y
   radio. Ningún componente ajusta solo la caja exterior.
3. **Tokens de interacción semánticos** en `UiTokens`:
   - `surface_hover` y `surface_active` para superficies neutras;
   - `accent_hover` y `accent_active` para acciones primarias;
   - `switch_off` para la pista apagada.
   Son opcionales en el TOML y se derivan por mezcla con el texto del tema.
4. La proyección a gpui-kit usa esos tokens:
   - `accent` → `surface_hover`;
   - `secondary` → `surface_raised`;
   - `secondary_hover` → `surface_hover`;
   - `primary/danger/warning/success/info_hover` → su variante derivada;
   - `switch` → `switch_off`;
   - `list_hover`, `table_hover` y `sidebar_accent` → `surface_hover`.
5. Los controles propios mantienen hover, pulsado, foco, seleccionado, abierto y
   deshabilitado. El foco reserva siempre un borde de 1 px transparente para no
   mover el contenido.
6. Los grupos de acción + menú usan una sola silueta, una divisoria interior y
   el estado abierto de gpui-kit (`SplitButton` sobre `DropdownButton`).

## Consecuencias

### Positivas

- Un control propio y uno de gpui-kit con el mismo nivel miden exactamente lo
  mismo a cualquier zoom.
- Los estados de hover, pulsado, foco y selección son visibles en temas claros,
  oscuros y personalizados.
- Los switches distinguen encendido, apagado y deshabilitado por posición y
  superficie.
- La bienvenida, la sidebar, las pestañas, WSL y los formularios dejan de mezclar
  alturas.
- El tema exporta los tokens nuevos y los temas antiguos siguen cargando con
  valores derivados.

### Negativas y riesgos

- Los temas personalizados ganan cinco claves opcionales; exportarlas hace que
  una versión antigua de la app las rechace si el archivo se comparte.
  Mitigación: son opcionales al leer y se documentan en `docs/themes.md`.
- Retirar `ControlSize::Lg` es un cambio interno de API; solo lo usaban la
  galería y un icono de ajustes. Mitigación: se actualiza la galería y las
  pruebas de métricas.
- Cambiar la tipografía de los botones propios de 10,5 a 12,25 px puede apretar
  etiquetas largas en la sidebar estrecha. Mitigación: las escenas compactas y
  el truncado con tooltip.

## Alternativas

- **Mantener cuatro tamaños y forzar `.h()` en gpui-kit**: rompe la escala del
  componente y su tipografía; rechazado.
- **Migrar toda la app a `Button` de gpui-kit**: resuelve estados pero pierde el
  atajo embebido y obliga a reescribir todas las superficies de una vez;
  rechazado para este hito.
- **Añadir un color literal en cada hover**: rompe los temas y el contraste;
  rechazado.

## Actualización 2026-11-26 — cursor, segmentados y `rem`

La primera pasada dejó tres huecos que se veían en uso real:

1. El cursor era mano en los componentes propios y flecha en los de gpui-kit.
2. Los estados de hover de las filas usaban `surface_raised.opacity(...)` y los
   botones `outline` de kit pintaban su hover al 20 %, casi invisible.
3. El switch de gpui-kit mide 28×16 px fijos y la sidebar conservaba 240 px
   aunque la fuente creciera; a escala 1,25 el texto se truncaba.

Se fija lo siguiente:

- **Cursor**: mano en toda superficie accionable construida por Axfterm
  (botones, icon-buttons, filas, pestañas, segmentados, switches, tarjetas,
  steppers y acciones de perfil). Los menús desplegables, el chrome de ventana
  y los campos de texto conservan la flecha o el cursor de texto porque
  gpui-kit fija su cursor internamente. El divisor de paneles usa
  `cursor_col_resize` / `cursor_row_resize`.
- **Hover y pulsado**: las filas y tarjetas usan `surface_hover` y
  `surface_active`, nunca una opacidad de `surface_raised`. Los botones de kit
  se proyectan con `button`, `button_hover`, `button_active` y sus variantes;
  el `outline` de kit se refuerza con una superficie de hover propia.
- **Hueco icono‑texto**: `ControlSize` define `0.25 rem` en `Xs`/`Sm` y
  `0.5 rem` en `Md`, igual que gpui-kit. Los botones propios usan la misma
  altura de línea relativa (`1.25`).
- **Segmentados**: un solo `SegmentedControl` con opciones propias, selección
  visible, hover, pulsado y foco por Tab/Enter/Espacio. Sustituye al segmentado
  de la sidebar, a los toggles de la paleta y a los radios de Apariencia.
- **Switch**: `SwitchField` deriva pista, pulgar y recorrido del tamaño base de
  la fuente, con hover y pulsado propios.
- **Sidebar**: el ancho guardado en sesión se interpreta en la base histórica
  de 14 px y se proyecta con el tamaño de fuente activo, con un tope del 45 %
  del viewport. La barra crece en `rem` y deja de truncar «Nueva terminal».
- **Studio**: cada escena parte de escala 1,0 y el manifiesto guarda
  `ui_scale` además del DPI, para que una captura no se compare con otra de
  escala distinta.

## Referencias

- ADR 0002 — encapsulación de GPUI/gpui-kit.
- ADR 0010 — panel de ajustes.
- `.memory/milestones/10-experiencia-ui-ux/` — auditoría previa.
- `.memory/milestones/12-refinamiento-controles/` — implementación.
