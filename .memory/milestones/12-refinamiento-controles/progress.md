# Progreso — M12 Refinamiento de controles y estados

## 2026-11-25 — Implementación inicial

- Escala única en `rem`: `Xs` 1,25, `Sm` 1,5 (por defecto) y `Md` 2,0. Se
  retiró `ControlSize::Lg`; la barra de título conserva sus 24 px de chrome del
  sistema.
- `Button`, `IconButton`, `TextField` y `SelectField` comparten altura,
  tipografía, icono y padding con `Size` de gpui-kit.
- Tokens nuevos `surface_hover`, `surface_active`, `accent_hover`,
  `accent_active` y `switch_off`, con derivación automática y TOML opcional.
- La proyección a gpui-kit corrige el hover de menús y botones primarios, el
  fondo apagado del switch y el hover de listas y sidebar.
- `Button` e `IconButton` ganan hover, pulsado y foco sin desplazamiento;
  `IconButton` admite `disabled`.
- Nuevo `SplitButton` para carpeta + menú en la sidebar, «+» + menú del panel y
  «Abrir carpeta» de la bienvenida.
- Los formularios de ajustes, paleta y WSL usan `Sm`; la bienvenida y la sidebar
  usan `Md` para sus acciones principales.
- La galería muestra los tres tamaños, el `Switch` real y los tokens nuevos.
- Los botones de WSL pasan a `KitButton` para que Tab/Enter los alcance, y los
  switches de ajustes anuncian su etiqueta accesible.
- Pasan `scripts/check.ps1` (límite de 500 líneas, fmt, clippy y 363 tests, 3
  ignorados por requieren WSL/bench), `cargo-low.ps1 build --release` y
  `test-cargo-low.ps1`; también el clippy de `axf_ui --features gallery` y de
  `axfterm --features studio`.
- Se reprodujeron las 36 escenas de Studio en One Dark, One Light, Cobalt2,
  Monokai y el tema personalizado. La revisión visual confirma:
  - la bienvenida y la sidebar muestran acciones `Md` alineadas y el grupo
    carpeta + menú como una sola silueta;
  - el tab strip agrupa «+» y el menú del panel;
  - los formularios, selects, steppers y WSL comparten altura `Sm`;
  - los switches apagados son grises y los encendidos azules en la galería y
    los ajustes;
  - WSL usa botones outline visibles junto al campo de carpeta.
- La escena compacta a 1024×720 con escala 1,25 conserva las alturas y la
  jerarquía; la etiqueta «Nueva terminal» se recorta con tooltip cuando la
  sidebar fija de 240 px no deja sitio. Gate manual de hover/pulsado pendiente.

## 2026-11-26 — Cursor, segmentados y escala real

- El cursor de mano cubre botones propios y de kit, filas, pestañas,
  segmentados, switches, tarjetas y steppers. Menús, campos y chrome de ventana
  conservan la flecha nativa; el divisor usa cursor de redimensionado.
- `SegmentedControl` sustituye los tres segmentados anteriores: sidebar,
  paleta y Apariencia. Tiene selección visible, hover, pulsado y foco por
  Tab/Enter/Espacio.
- `SwitchField` dibuja pista, pulgar y recorrido en `rem`, con hover y pulsado.
- Las filas, tarjetas, rail, steppers y acciones de perfil usan
  `surface_hover` y `surface_active`; ya no queda hover por opacidad.
- `Button` incorpora `align_start` y `padding_x`; «Nueva terminal» queda
  alineada con «Buscar…» y deja de centrarse.
- El ancho de la sidebar se proyecta desde la base de 14 px y se limita al 45 %
  del viewport. A escala 1,25 ya no trunca «Nueva terminal».
- `SplitButton` usa dos mitades de gpui-kit con esquinas, divisoria y hover de
  grupo propios; ambas tienen cursor de mano.
- La bienvenida con proyecto sin terminales usa dos botones de kit `Md` y una
  fila con `items_center`. Las escenas `app.onboarding.project` y
  `app.onboarding.project.compact` lo verifican.
- `colors.button*` se proyecta al tema de gpui-kit y el `outline` de WSL
  refuerza su hover con una superficie propia.
- Studio reinicia la escala a 1,0 por escena y el manifiesto añade `ui_scale`.
  Las 38 escenas se reprodujeron; las capturas «normales» posteriores a
  `.scaled` ya no heredan 1,25.
- `check.ps1` verde con 48 tests de `axf_ui` y 111 de la app (3 ignorados).
  Gate manual de hover/pulsado y menús abiertos sigue pendiente.
- Menús desplegables y elementos de menú conservan la flecha de gpui-kit; para
  mano dentro del menú haría falta sustituir `PopupMenu`.
