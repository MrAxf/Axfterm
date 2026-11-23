# ADR 0006 — Perfiles de terminal, comandos e iconos

- **Estado**: Aceptado
- **Fecha**: 2026-11-22
- **Decidores**: Juan Andrés

## Contexto

M1–M3 arrancan el PTY con una lista hardcodeada de candidatos (`pwsh`,
`powershell`, `cmd`). El usuario no elige la terminal, no puede añadir WSL o
Git Bash y la pestaña no recuerda con qué programa se abrió. v0.1 necesita
perfiles declarativos porque:

- Windows primero, pero el mismo modelo debe describir shells de otras
  plataformas sin código de dominio distinto.
- La configuración es la fuente de verdad y se edita a mano; el panel de ajustes
  (ADR 0007) escribirá en el mismo archivo.
- La sesión restaura pestañas; sin identidad de perfil, reiniciar una pestaña
  perdería el shell elegido.
- El icono debe representar al ejecutable real, no solo una marca predefinida.
- Nada de red ni telemetría: la detección es local y solo en el primer arranque.

## Decisión

### Esquema

`config.toml` sube a `schema_version = 2` y añade:

```toml
[terminal]
default_profile = "pwsh"

[[profiles]]
id = "pwsh"
name = "PowerShell 7"
command = 'pwsh -NoLogo'
# Ruta a la imagen del icono. Si se omite, se usa el icono del ejecutable.
# icon = "icons/powershell.png"
```

- `[[profiles]]` es un array de tablas: conserva el orden de menús y paleta,
  evita claves dinámicas y permite validar ids duplicados.
- `id` es un slug estable y único; `name` es solo presentación y se puede
  renombrar sin romper la sesión. El panel oculta el id.
- `default_profile` referencia un `id` existente. Un id ausente, duplicado,
  vacío o un `default_profile` que no apunte a ningún perfil es error de campo;
  no se corrige en silencio.
- `icon` es opcional. Una ruta relativa se resuelve contra el directorio de
  configuración; también admite rutas absolutas.
- Límites en ADR 0005. Los perfiles viven solo en config: la sesión guarda el
  `id`, no una copia del comando.
- Migración config v1 → v2 en memoria y, si faltan perfiles, descubrimiento y
  escritura atómica una sola vez.

### Línea de comandos

- `command` es una sola línea: el primer token es el ejecutable y el resto son
  argumentos.
- Tokenizer propio: espacios separan; comillas dobles y simples agrupan;
  `\"` y `\'` escapan dentro de su comilla; las barras invertidas son literales
  para no romper `C:\Program Files\...`; no hay variables de entorno ni
  operadores de shell (`|`, `>`, `&&`, `;` se pasan como argv).
- Un `command` vacío o sin ejecutable es error de validación. Un ejecutable
  inexistente en tiempo de arranque es error visible en la pestaña, con acción
  para corregir el perfil; nunca se cae a otro shell en silencio.
- `axf_terminal` deja de elegir shell: recibe `SpawnSpec { program, args }`.
  La detección de candidatos se mueve a config. Esto mantiene el crate de PTY
  sin conocimiento de producto.

### Iconos

- Si `icon` existe y la imagen carga, se usa. Formatos aceptados: los que
  soporta GPUI por contenido (PNG, JPG, ICO, WebP, GIF, BMP, TIFF) y SVG.
- Si `icon` falta, en Windows se extrae el icono del ejecutable con la API de
  shell (`SHGetFileInfoW`/`ExtractIconExW`), se convierte a PNG con alpha y se
  cachea en `<config>/icons/cache/<hash>.png`, con clave ejecutable + mtime.
  La caché es acotada y se puede borrar sin perder config.
- Si la extracción falla, no hay caché o la ruta no existe, se usa
  `icons/tabler/terminal-2.svg` sin interrumpir al usuario; un icono movido no
  invalida toda la configuración. El motivo queda en el log de diagnóstico.
  *(Revisado el 2026-11-22: antes se mostraba un aviso, pero no era accionable y
  competía con los avisos reales.)*
- En macOS/Linux la extracción nativa queda para post-v0.1; la ruta manual
  funciona igual y el fallback es el icono genérico.
- La extracción es un adaptador de plataforma en `axf_ui` detrás de `cfg`; el
  modelo de config solo guarda la ruta y el comando.
- Spike S8 valida HICON → PNG con alpha y DPI antes de fijar el render.

### Descubrimiento

- `axf_config::profiles::discover` no conoce GPUI ni abre PTYs: comprueba el
  PATH, rutas conocidas y (en Windows) distros WSL.
- Windows: `pwsh`, `powershell`, `cmd`, `nu` y Git Bash; además `wsl -l -q`
  para cada distro.
- Unix: `$SHELL`, `zsh`, `bash`, `fish`, `nu` y `sh`.
- Las comprobaciones rápidas ocurren al arrancar; WSL se consulta en segundo
  plano con timeout y salida UTF-16. La UI nunca espera a un proceso.
- Si no se detecta nada, se crea un perfil de respaldo por plataforma.
- El descubrimiento automático se ejecuta solo cuando no hay perfiles. Si el
  usuario borra todos, se vuelve a detectar; el estado "cero perfiles" no es
  válido porque siempre hace falta un perfil por defecto.
- El panel de ajustes expone «Detectar terminales» con vista previa y fusión sin
  duplicar comandos.

### Sesión y pestañas

- `Tab` gana `profile_id`; mover la pestaña entre paneles no lo cambia.
- `session.json` sube a `version = 2`. La migración v1 → v2 rellena el perfil por
  defecto de cada pestaña. Duplicar y reiniciar conservan el perfil.
- Si el perfil de una pestaña restaurada ya no existe, la pestaña se restaura
  como terminada y ofrece elegir otro perfil; no se arranca otro shell por
  sorpresa.
- El título inicial de la pestaña es el `name` del perfil. Un OSC 0/2 puede
  sustituirlo salvo que el usuario haya fijado un título manual.
- Hot reload de perfiles: las pestañas nuevas usan la config vigente; las
  abiertas conservan su `SpawnSpec`.

## Consecuencias

### Positivas

- Un solo modelo describe PowerShell, cmd, WSL, Git Bash y shells Unix.
- La sesión restaura el shell correcto y el usuario puede añadir terminales sin
  tocar código.
- El arranque deja de depender de una lista oculta y los errores son
  explicables.
- Los iconos son reales sin obligar a mantener un catálogo de marcas.

### Negativas y riesgos

- El tokenizer introduce reglas propias; Mitigación: documentarlas y cubrirlas
  con tests de rutas Windows, comillas y argumentos vacíos.
- La extracción de iconos usa APIs Win32 y puede variar con DPI o temas;
  Mitigación: spike S8 y fallback siempre disponible.
- El descubrimiento es heurístico y puede encontrar shells que no interesan;
  Mitigación: es editable y no se repite si ya hay perfiles.
- Autoescribir la config en la primera actualización puede sorprender;
  Mitigación: solo añade perfiles cuando no hay ninguno, conserva comentarios y
  es atómico.
- `wsl -l -q` puede tardar o fallar; Mitigación: segundo plano, timeout y
  ausencia de error fatal.

## Alternativas

- **`program` + `args` en TOML**: menos ambigüedad, pero el usuario pidió una
  línea de comandos y el panel tendría que tokenizar igualmente. Se descarta
  guardar dos campos y parsear en la UI; se puede añadir después sin romper.
- **Ejecutar la línea con un shell**: permitiría pipes y variables, pero
  duplica el shell, complica el cierre del árbol y abre inyección accidental.
- **Perfiles por proyecto**: aplaza el problema de clasificación y la sesión;
  post-v0.1.
- **Catálogo cerrado de iconos**: más simple, pero no representa ejecutables
  reales ni iconos propios.
- **Detección en cada arranque**: pisa decisiones del usuario y añade latencia.
- **WSL bloqueante al arrancar**: viola el presupuesto de arranque y el hilo de
  UI libre.

## Referencias

- `.memory/milestones/04-perfiles/tasks.md` — implementación y tests.
- ADR 0003 — modelo de pestañas y paneles.
- ADR 0004 — config, versionado y recuperación.
- ADR 0005 — límites de perfiles y tamaños.
- ADR 0007 — panel de ajustes y escritura de config.
- `AGENTS.md` — restricciones de plataforma, hilos y minimalismo.

## Nota de implementación (M4, 2026-11-22)

- Spike S8 completado: `ExtractIconExW` + `DrawIconEx` sobre un DIB de 64×64,
  normalización a RGBA y PNG con `png`. Si ningún píxel trae alpha, se vuelve
  opaco. La caché se limita a 64 archivos y se poda por `mtime`; la clave es
  ejecutable + `mtime` + tamaño.
- El tokenizer vive en `axf_config::profiles` y produce `CommandLine`; la app
  resuelve el ejecutable en `PATH`, lo pasa como ruta absoluta a
  `axf_terminal::SpawnSpec` y nunca invoca un shell intermedio.
- `Tab.auto_title` conserva el nombre del perfil para que quitar un rename no
  caiga al nombre del proyecto.
- WSL se descubre en segundo plano y se fusiona con `append_profiles`
  preservando comentarios y el orden; una config con perfiles propios no se
  redetecta.
- El perfil detectado de PowerShell 7 no añade `-NoProfile`: debe cargar el
  `$PROFILE` del usuario. Solo se pasa `-NoLogo`; desactivar el perfil es una
  decisión explícita del usuario en `command`.

