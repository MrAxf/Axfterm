# Atajos de teclado

<!-- Generado desde el registro de acciones; no editar a mano. -->

Todos los atajos se pueden cambiar en la sección `[keymap]` de
`config.toml`. Un atajo por defecto se desactiva con `unbind`.

## Aplicación

| Acción | Atajo | Id |
| --- | --- | --- |
| Salir de Axfterm | `ctrl-q` | `app.quit` |
| Abrir la paleta de comandos | `ctrl-shift-k` | `palette.open` |
| Abrir la búsqueda | `ctrl-p` | `palette.search` |
| Alternar la barra lateral | `ctrl-b` | `sidebar.toggle` |
| Enfocar la barra lateral | `ctrl-alt-b` | `sidebar.focus` |
| Alternar el filtro de agentes | `ctrl-alt-a` | `sidebar.toggle_agents` |
| Abrir los ajustes | `ctrl-alt-t` | `settings.open` |

## Proyectos

| Acción | Atajo | Id |
| --- | --- | --- |
| Abrir una carpeta como proyecto | `ctrl-shift-o` | `project.open` |
| Abrir un proyecto en WSL | `ctrl-alt-shift-o` | `project.open_wsl` |

## Paneles

| Acción | Atajo | Id |
| --- | --- | --- |
| Dividir en columnas | `ctrl-shift-e` | `pane.split_horizontal` |
| Dividir en filas | `ctrl-shift-d` | `pane.split_vertical` |
| Cerrar el panel | `ctrl-shift-x` | `pane.close` |
| Enfocar el panel siguiente | `ctrl-alt-down` | `pane.focus_next` |
| Enfocar el panel anterior | `ctrl-alt-up` | `pane.focus_previous` |
| Alternar el zoom temporal del panel | `ctrl-shift-z` | `pane.toggle_zoom` |

## Pestañas

| Acción | Atajo | Id |
| --- | --- | --- |
| Nueva terminal | `ctrl-shift-t` | `tab.new` |
| Cerrar la pestaña | `ctrl-shift-w` | `tab.close` |
| Pestaña siguiente | `ctrl-alt-right` | `tab.next` |
| Pestaña anterior | `ctrl-alt-left` | `tab.previous` |
| Renombrar la pestaña | `ctrl-shift-r` | `tab.rename` |
| Duplicar la pestaña | `ctrl-shift-u` | `tab.duplicate` |
| Reiniciar la terminal | `ctrl-shift-p` | `tab.restart` |
| Mover la pestaña al panel siguiente | `ctrl-shift-m` | `tab.move_next_pane` |
| Copiar la selección | `ctrl-shift-c` | `terminal.copy` |
| Pegar en la terminal | `ctrl-shift-v` | `terminal.paste` |

## Tema y fuentes

| Acción | Atajo | Id |
| --- | --- | --- |
| Previsualizar la variante clara/oscura | `ctrl-alt-v` | `theme.toggle_variant` |
| Tema siguiente | `ctrl-alt-n` | `theme.next` |
| Tema anterior | `ctrl-alt-p` | `theme.previous` |
| Recargar temas desde disco | `ctrl-alt-shift-t` | `theme.reload` |
| Abrir la carpeta de temas | `ctrl-alt-o` | `theme.open_folder` |
| Importar un tema | `ctrl-alt-i` | `theme.import` |
| Exportar el tema activo | `ctrl-alt-e` | `theme.export` |

## Agentes

| Acción | Atajo | Id |
| --- | --- | --- |
| Ir al siguiente agente que requiere atención | `ctrl-alt-j` | `agent.next_attention` |

## Ayuda

| Acción | Atajo | Id |
| --- | --- | --- |
| Ver todos los atajos | `ctrl-alt-k` | `help.shortcuts` |

## Configurar atajos

```toml
[keymap]
# Añade un atajo alternativo; el de por defecto sigue activo.
"ctrl-alt-shift-n" = "tab.new"
# Quita todos los atajos por defecto de una acción.
unbind = ["app.quit"]

[keymap.terminal]
"ctrl-shift-c" = "terminal.copy"
```

Los contextos disponibles son `workspace`, `sidebar` y `terminal`.
Los atajos de app requieren modificador; `Ctrl+C`, `Ctrl+D`, `Ctrl+Z`,
`Tab`, `Esc` y las flechas quedan siempre para la terminal.
