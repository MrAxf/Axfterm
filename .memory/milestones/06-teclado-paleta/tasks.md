# Tareas — M6 Teclado y paleta

## Registro de acciones

- [x] `ActionMeta`: id, label, categoría, contexto, chord por defecto, enabled
- [x] Acciones de app, proyecto, panel, pestaña, perfil, tema y agente
- [x] Acciones de perfil: nueva pestaña con cada perfil y con el predeterminado
- [x] Menús generados desde el registro, con iconos de perfil
- [x] Documentación de atajos autogenerada

## Keybindings

- [x] Parser TOML: `chord = accion`, contextos y `unbind`
- [x] Validación: duplicados, teclas reservadas, chords inválidos
- [x] Contextos GPUI y orden de dispatch, con `Settings` reservado para M7
- [x] Regla de foco: sin binding -> PTY
- [x] Recarga en caliente y errores accionables
- [x] Aviso al usuario si un atajo nuevo pisa una tecla de terminal

## Paletas

- [x] Fuzzy matcher con ranking, resaltado y soporte de acentos
- [x] Paleta de comandos (`Ctrl+Shift+K`): solo acciones del registro, sin las
      duplicadas por la búsqueda o el modal de tema
- [x] Paleta de búsqueda (`Ctrl+P`): pestañas, paneles, proyectos, perfiles y
      agentes en atención
- [x] Cambio de modo dentro de la paleta (`Ctrl+Tab`) con control segmentado y
      filtro por tipo (`Ctrl+1–6`), conservando consulta y selección
- [x] Salto al agente que requiere atención desde la paleta
- [x] Nueva pestaña con el perfil elegido desde la búsqueda, con su icono
- [x] Recientes y frecuencia de uso
- [x] Navegación por teclado, preview y ejecución
- [x] Punto de extensión para fuentes futuras (contenedores y puertos de
      99-futuro)
- [x] Temas y fuentes en el modal «Tema y fuentes»: catálogo, variante y dos
      selectores de familia con preview en memoria

## Tests

- [x] Dispatch por contexto (terminal no pierde Ctrl+C)
- [x] Conflictos, unbind y recarga de config
- [x] Fuzzy con casos ES/acentos y ranking estable
- [x] Salto al agente en atención desde la paleta
- [x] Gate manual: sesión completa sin ratón, incluido abrir terminales de
      varios perfiles
