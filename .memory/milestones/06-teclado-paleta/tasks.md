# Tareas — M6 Teclado y paleta

## Registro de acciones

- [ ] `ActionMeta`: id, label, categoría, contexto, chord por defecto, enabled
- [ ] Acciones de app, proyecto, panel, pestaña, perfil, tema y agente
- [ ] Acciones de perfil: nueva pestaña con cada perfil y con el predeterminado
- [ ] Menús generados desde el registro, con iconos de perfil
- [ ] Documentación de atajos autogenerada

## Keybindings

- [ ] Parser TOML: `chord = accion`, contextos y `unbind`
- [ ] Validación: duplicados, teclas reservadas, chords inválidos
- [ ] Contextos GPUI y orden de dispatch, con `Settings` reservado para M7
- [ ] Regla de foco: sin binding -> PTY
- [ ] Recarga en caliente y errores accionables
- [ ] Aviso al usuario si un atajo nuevo pisa una tecla de terminal

## Command palette

- [ ] Fuzzy matcher con ranking, resaltado y soporte de acentos
- [ ] Fuentes: comandos, pestañas, paneles, proyectos (revela su fila en la
      sidebar), perfiles, agentes en atención, temas y fuentes
- [ ] Salto al agente que requiere atención desde la paleta
- [ ] Nueva pestaña con el perfil elegido desde la paleta, con su icono
- [ ] Recientes y frecuencia de uso
- [ ] Navegación por teclado, preview y ejecución
- [ ] Punto de extensión para fuentes futuras (contenedores y puertos de
      99-futuro)

## Tests

- [ ] Dispatch por contexto (terminal no pierde Ctrl+C)
- [ ] Conflictos, unbind y recarga de config
- [ ] Fuzzy con casos ES/acentos y ranking estable
- [ ] Salto al agente en atención desde la paleta
- [ ] Gate manual: sesión completa sin ratón, incluido abrir terminales de
      varios perfiles
