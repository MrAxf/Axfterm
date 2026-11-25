# Configuración (`config.toml`)

`config.toml` es la fuente de verdad de la configuración y se recarga en
caliente. Se puede editar a mano o desde el panel de ajustes; ambos caminos
validan con las mismas reglas.

## Ubicación

- Windows: `%APPDATA%\axfterm\config.toml`
- macOS: `~/Library/Application Support/axfterm/config.toml`
- Linux: `~/.config/axfterm/config.toml` o `$XDG_CONFIG_HOME/axfterm/config.toml`

`AXFTERM_CONFIG_DIR` (ruta absoluta) cambia el directorio para tests y
diagnóstico. No es un modo portable de producto.

## Esquema

```toml
schema_version = 2
appearance = "system"          # system | light | dark

[ui]
theme = "One Dark"
font_size = 14.0               # 11–24
scale = 1.0                    # 0.8–2.0

[terminal]
font_size = 13.0               # 8–32
line_height = 1.0              # 1.0–2.5
scrollback_lines = 10000       # 0–50000; tope efectivo de 32 MiB
default_profile = "pwsh"

[fonts]
sources = []                   # archivos/directorios; relativos a este archivo

[agents]
enabled = true
scan_interval_ms = 1500        # 1000–2000
notify_finished = true
notify_error = true
notify_attention = true

[keymap]
# "ctrl-alt-q" = "tab.new"
# unbind = ["app.quit"]

[[profiles]]
id = "pwsh"
name = "PowerShell"
command = "pwsh -NoLogo"
# icon = "icons/pwsh.png"
```

- Los campos desconocidos son un error: un typo no se ignora en silencio.
- Los valores fuera de rango señalan el campo y el límite; no hay clamp.
- `schema_version` ausente se interpreta como 1. Una versión futura se rechaza
  sin modificar el archivo.
- `terminal.default_profile` debe existir en `[[profiles]]`.
- El scrollback se aplica a terminales nuevas; al ensanchar una existente se
  recorta el historial antiguo para no superar 32 MiB.

Cada sección se documenta en detalle:

- [Perfiles](profiles.md)
- [Panel de ajustes](settings.md)
- [Atajos](keymap.md)
- [Temas](themes.md)
- [Agentes](agents.md)

## Recuperación

- **Archivo corrupto o por encima de 1 MiB**: se renombra a
  `config.toml.bak-YYYYMMDD-HHMMSS`, se arranca con defaults y se muestra un
  aviso con la ruta del backup.
- **Error de hot reload**: se conserva la última configuración válida y se
  muestra el error con archivo, línea y columna.
- **Escritura del panel**: se escribe con un temporal, `flush`, `sync` y
  `rename` atómico. Si el archivo cambió entre la comprobación y la escritura,
  no se escribe nada y se pide recargar o confirmar.
- **Migración v1 → v2**: se completa con el descubrimiento local una sola vez y
  se conserva el texto, los comentarios y el orden.

## Perfiles ausentes o vacíos

Una config v2 sin `[[profiles]]` o con `profiles = []` se completa con el
descubrimiento local en el primer arranque y se reescribe una sola vez. Después
manda el archivo: no se vuelve a detectar salvo que se pida desde el panel.

Si el perfil por defecto no existe, se usa el primero de la lista; sin ningún
perfil se escribe el fallback (`cmd /Q`) para que el estado «cero perfiles» no
sea válido.
