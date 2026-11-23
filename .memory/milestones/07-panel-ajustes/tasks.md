# Tareas — M7 Panel de ajustes

## Superficie

- [x] Acción `OpenSettings` registrada y contexto `Settings` en el dispatch
- [x] Overlay con navegación por secciones: Apariencia, Terminal, Perfiles,
      Fuentes, Atajos, Agentes y Avanzado
- [x] Foco atrapado mientras está abierto; Esc cierra y devuelve el foco
- [x] Navegación completa por teclado: secciones, campos, botones y listas
- [x] Entrada desde paleta, menú y atajo por defecto
- [x] Registro extensible de secciones, con Agentes ya incluido (M5)

## Modelo y escritura

- [x] Borrador de `Config` independiente del activo, con validación inline
- [x] Preview en vivo de apariencia, tema y fuentes sin escribir en disco
- [x] Cancelar restaura el estado visual anterior
- [x] Escritura con `toml_edit` preservando comentarios y orden, sobre
      `write_atomic`; nunca se escribe parcialmente
- [x] Si la validación o la escritura fallan: aviso con campo y ruta, sin
      aplicar cambios
- [x] Conflicto si `config.toml` cambia por fuera con el panel abierto:
      recargar o descartar el borrador, nunca pisar en silencio
- [x] Tras guardar, una sola aplicación del tema; el watcher no duplica el
      trabajo

## Secciones

- [x] Apariencia: sistema/claro/oscuro, tema del catálogo con preview de variante,
      fuente de UI, tamaño y escala
- [x] Terminal: fuente, tamaño y altura de línea, con preview
- [x] Perfiles: alta, edición, duplicado, borrado protegido, orden, icono y
      perfil predeterminado
- [x] «Detectar terminales»: vista previa de lo encontrado y fusión sin duplicar
- [x] Fuentes: añadir y quitar orígenes, con selector de archivo o carpeta
- [x] Atajos: búsqueda, rebind, `unbind` y detección de conflictos y teclas
      reservadas contra el registro de M6
- [x] Agentes: activación, intervalo de escaneo, avisos y perfil preferido por
      agente
- [x] Avanzado: rutas de config y temas, abrir `config.toml`, versión y
      restauración de valores por defecto

## Integración

- [x] Reutilizar el marco de modal de M3 y los componentes de gpui-kit
- [x] Icono de ajustes en el menú del workspace (el de la barra de título ya
      existe)
- [x] La entrada de ajustes de la barra de título pasa de abrir tema y fuentes a
      abrir `OpenSettings`
- [x] El perfil en uso se conserva al editar perfiles; borrar uno en uso avisa
- [x] ADR 0007 y enmienda a ADR 0004
- [x] `docs/settings.md` con secciones, escritura y resolución de conflictos

## Tests

- [x] Round-trip de escritura conserva comentarios, orden y la plantilla
- [x] Errores de validación bloquean el guardado con campo accionable
- [x] Preview y Cancelar no dejan cambios en config ni en pantalla
- [x] Conflicto externo y recarga sin perder trabajo del usuario
- [x] Atajos: duplicados, reservadas y `unbind` desde el panel
- [x] Sección Agentes: validación, escritura y recarga sin duplicar el escaneo
- [ ] Gate: cambiar tema, fuente, perfil, atajo y agente desde el panel y
      reiniciar
