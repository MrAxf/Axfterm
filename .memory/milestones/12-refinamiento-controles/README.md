# M12 — Refinamiento de controles y estados

Objetivo: dar a todos los controles una sola escala derivada del tamaño base de
la fuente y estados visibles y coherentes, sin rediseñar los recorridos ni tocar
el motor de terminal. Cierra la deuda que M10 dejó entre los componentes propios
y gpui-kit.

## Alcance

- Escala única `ControlSize` en `rem`: `Xs`, `Sm` (por defecto) y `Md`.
- Tokens de interacción: hover, pulsado, selección, foco, abierto y
  deshabilitado.
- Botones, icon buttons, campos de texto, selectores, switches, grupos de
  acción + menú y controles segmentados.
- Bienvenida, sidebar, tab strip, panel de ajustes, WSL, paleta y atajos.
- Galería del sistema de diseño y capturas de Studio.

## Fuera de alcance

- Rediseño de jerarquía o de distribución de superficies.
- Terminal, PTY, parser, agentes, persistencia o atajos nuevos.
- Migración completa de la app a los botones de gpui-kit.
- Temas nuevos o cambios de paleta fuera de los tokens de interacción.

## Criterio de hecho

- [x] Ningún control de producción usa alturas fuera de 1,25 / 1,5 / 2 rem.
- [x] Los controles que comparten fila usan el mismo nivel.
- [x] Hover, pulsado, foco, seleccionado y abierto se distinguen en temas claro,
      oscuro y personalizado.
- [x] Los menús muestran hover y selección de teclado sobre el popover.
- [x] Los switches encendido, apagado y deshabilitado se distinguen sin depender
      solo del color.
- [x] Los grupos acción + menú tienen una silueta, divisoria y estado abierto.
- [x] El foco no cambia la geometría de ningún control.
- [x] Campos y botones de una misma fila miden lo mismo.
- [x] Todos los controles accionables usan mano; menús y chrome conservan la
      flecha nativa y el divisor usa cursor de redimensionado.
- [x] Los segmentados, switches y radios comparten escala, hover y selección.
- [x] La sidebar crece con la fuente y no trunca sus acciones a escala 1,25.
- [x] La galería y las escenas de Studio reflejan el resultado.
- [x] Pasan `check.ps1`, build release, pruebas del wrapper y clippy de Studio.
- [ ] Gate manual final de hover, pulsado y menús abiertos con la instancia
      instalada.

## Orden de trabajo

Primero la escala y los tokens, porque arreglan los defectos más visibles con
poco riesgo. Después los componentes propios y los grupos. Al final se normaliza
cada superficie y se verifican capturas y teclado.
