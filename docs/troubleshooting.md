# Solución de problemas

## La app no arranca

1. Ejecuta desde una consola con `AXFTERM_LOG=debug` para ver el motivo.
2. Revisa `%APPDATA%\axfterm\logs\crash-*.log` si hubo un panic.
3. Prueba con un directorio aislado:
   ```powershell
   $env:AXFTERM_CONFIG_DIR = "$env:TEMP\axfterm-diagnostico"
   axfterm
   ```
   Si arranca, el problema está en tu `config.toml` o `session.json`.

## `config.toml` corrupto o con un typo

Axfterm rechaza campos desconocidos y valores fuera de rango. Un archivo
inválido se renombra a `config.toml.bak-YYYYMMDD-HHMMSS` y se arranca con los
defaults; el aviso en la app incluye la ruta del backup.

- Restaura el backup y corrige el campo señalado (archivo, línea y columna).
- Un `schema_version` mayor que el de la build se rechaza sin tocar el archivo.
- Si el panel de ajustes detecta un cambio externo mientras editas, pide
  recargar o confirmar la sobrescritura. Nunca pisa en silencio.

## `session.json` corrupto

Se conserva una copia `session.json.bak-*` y se arranca con un workspace limpio.
Puedes recuperar proyectos y pestañas desde esa copia o volver a abrir las
carpetas.

## No se restaura el historial de una pestaña

- Solo se guarda en un cierre ordenado; un cierre forzado pierde la última
  tanda.
- Un `.snap` inválido se descarta y la misma pestaña abre su perfil desde cero
  con un aviso.
- El historial persistido tiene un tope de 2 MiB; si aparece «restaurado
  parcialmente», se conservaron las líneas más recientes.

## La terminal no aparece o el perfil falla

- Un perfil inexistente o un ejecutable que no está en `PATH` es un error
  visible en la pestaña; no se cae en silencio a otro shell.
- Revisa `[[profiles]]` y `terminal.default_profile` en `config.toml`.
- Para WSL, el perfil debe usar `wsl -d "Distro"` y el proyecto debe ser de esa
  distro.

## Abrir una carpeta desde Explorer

El MSI registra «Abrir con Axfterm» en el menú contextual de carpetas y del
fondo de carpeta. Si no aparece:

1. Comprueba que instalaste con el MSI (el zip portable no registra nada).
2. Reinicia Explorer o cierra y abre sesión.
3. Puedes usar `axfterm "C:\ruta\a\la\carpeta"` o arrastrar la carpeta.

Si ya hay una ventana abierta, la carpeta se envía a esa instancia y la segunda
sale. Si la ventana se está cerrando, vuelve a intentarlo.

## Teclado, AltGr e IME

- Con una terminal enfocada, una tecla sin binding se envía al PTY.
- AltGr y las dead keys se dejan al manejador de texto de Windows; no se
  escriben dos veces.
- Los atajos de app exigen Ctrl/Alt/Win. `Ctrl+C/D/Z`, Tab, Esc y las flechas
  están reservados a la terminal salvo que un binding de terminal lo cambie.
- Si un carácter CJK no aparece, prueba a pegar texto y revisa el aviso de la
  terminal.

## DPI y tamaños

El DPI del monitor se aplica aparte de `ui.scale`. Si cambias de monitor y algo
queda pequeño o grande, mueve la ventana al otro monitor y espera un frame; el
tema se recalcula. Los tamaños se validan al guardar.

## Rendimiento

- El output masivo se coalesce a un repintado por frame.
- Si el frame supera el presupuesto, revisa los benchmarks de
  [diagnostics.md](diagnostics.md) antes de tocar la configuración.
- Un scrollback muy alto con una ventana ancha se recorta por el tope de 32 MiB
  por terminal.

## Cómo pedir ayuda

Adjunta:

1. versión de Axfterm y de Windows;
2. `AXFTERM_LOG=debug` con el fragmento relevante (sin secretos);
3. el crash report si lo hubo;
4. los pasos exactos y si ocurre con un `AXFTERM_CONFIG_DIR` limpio.
