# Tareas — M4 Teclado y paleta

## Registro de acciones

- [ ] `ActionMeta`: id, label, categoría, contexto, chord por defecto, enabled
- [ ] Acciones de app, proyecto, panel, pestaña, tema y agente
- [ ] Menús generados desde el registro
- [ ] Documentación de atajos autogenerada

## Keybindings

- [ ] Parser TOML: `chord = accion`, contextos y `unbind`
- [ ] Validación: duplicados, teclas reservadas, chords inválidos
- [ ] Contextos GPUI y orden de dispatch
- [ ] Regla de foco: sin binding -> PTY
- [ ] Recarga en caliente y errores accionables
- [ ] Aviso al usuario si un atajo nuevo pisa una tecla de terminal

## Command palette

- [ ] Fuzzy matcher con ranking, resaltado y soporte de acentos
- [ ] Fuentes: comandos, pestañas, paneles, proyectos, temas y fuentes
- [ ] Recientes y frecuencia de uso
- [ ] Navegación por teclado, preview y ejecución
- [ ] Paleta "ir a atención" para agentes que requieren intervención

## Tests

- [ ] Dispatch por contexto (terminal no pierde Ctrl+C)
- [ ] Conflictos, unbind y recarga de config
- [ ] Fuzzy con casos ES/acentos y ranking estable
- [ ] Gate manual: sesión completa sin ratón
