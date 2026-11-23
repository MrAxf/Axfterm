# Tareas — M7 Panel de ajustes

## Superficie

- [ ] Acción `OpenSettings` registrada y contexto `Settings` en el dispatch
- [ ] Overlay con navegación por secciones: Apariencia, Terminal, Perfiles,
      Fuentes, Atajos, Agentes y Avanzado
- [ ] Foco atrapado mientras está abierto; Esc cierra y devuelve el foco
- [ ] Navegación completa por teclado: secciones, campos, botones y listas
- [ ] Entrada desde paleta, menú y atajo por defecto
- [ ] Registro extensible de secciones, con Agentes ya incluido (M5)

## Modelo y escritura

- [ ] Borrador de `Config` independiente del activo, con validación inline
- [ ] Preview en vivo de apariencia, tema y fuentes sin escribir en disco
- [ ] Cancelar restaura el estado visual anterior
- [ ] Escritura con `toml_edit` preservando comentarios y orden, sobre
      `write_atomic`; nunca se escribe parcialmente
- [ ] Si la validación o la escritura fallan: aviso con campo y ruta, sin
      aplicar cambios
- [ ] Conflicto si `config.toml` cambia por fuera con el panel abierto:
      recargar o descartar el borrador, nunca pisar en silencio
- [ ] Tras guardar, una sola aplicación del tema; el watcher no duplica el
      trabajo

## Secciones

- [ ] Apariencia: sistema/claro/oscuro, tema del catálogo con preview de variante,
      fuente de UI, tamaño y escala
- [ ] Terminal: fuente, tamaño y altura de línea, con preview
- [ ] Perfiles: alta, edición, duplicado, borrado protegido, orden, icono y
      perfil predeterminado
- [ ] «Detectar terminales»: vista previa de lo encontrado y fusión sin duplicar
- [ ] Fuentes: añadir y quitar orígenes, con selector de archivo o carpeta
- [ ] Atajos: búsqueda, rebind, `unbind` y detección de conflictos y teclas
      reservadas contra el registro de M6
- [ ] Agentes: activación, intervalo de escaneo, avisos y perfil preferido por
      agente
- [ ] Avanzado: rutas de config y temas, abrir `config.toml`, versión y
      restauración de valores por defecto

## Integración

- [ ] Reutilizar el marco de modal de M3 y los componentes de gpui-kit
- [ ] Icono de ajustes en el menú del workspace (el de la barra de título ya
      existe)
- [ ] La entrada de ajustes de la barra de título pasa de abrir tema y fuentes a
      abrir `OpenSettings`
- [ ] El perfil en uso se conserva al editar perfiles; borrar uno en uso avisa
- [ ] ADR 0007 y enmienda a ADR 0004
- [ ] `docs/settings.md` con secciones, escritura y resolución de conflictos

## Tests

- [ ] Round-trip de escritura conserva comentarios, orden y la plantilla
- [ ] Errores de validación bloquean el guardado con campo accionable
- [ ] Preview y Cancelar no dejan cambios en config ni en pantalla
- [ ] Conflicto externo y recarga sin perder trabajo del usuario
- [ ] Atajos: duplicados, reservadas y `unbind` desde el panel
- [ ] Sección Agentes: validación, escritura y recarga sin duplicar el escaneo
- [ ] Gate: cambiar tema, fuente, perfil, atajo y agente desde el panel y
      reiniciar
