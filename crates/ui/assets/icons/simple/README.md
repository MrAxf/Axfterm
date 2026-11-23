# Marcas simple-icons vendorizadas

Subset mínimo de [simple-icons](https://github.com/simple-icons/simple-icons)
para las marcas de agentes y del stack, vendorizado para que el binario no
dependa de la red.

- Licencia: CC0 1.0 (ver `LICENSE` en esta carpeta y `NOTICE` en la raíz).
- Origen: `https://cdn.jsdelivr.net/npm/simple-icons/icons/rust.svg`
- Fecha de descarga: 2026-11-21.
- Iconos: `rust`.

Estos SVG son de un solo path: GPUI los pinta con el color que recibe el
elemento `svg`. El color de marca no es un token del tema (identifica a un
producto externo); vive en `crates/ui/src/brand.rs`.

Al añadir o actualizar un icono, actualiza también el commit/fecha de origen y
`crates/ui/src/icons.rs`.
