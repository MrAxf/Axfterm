# ADR 0011 — Proyectos WSL y hosts de proyecto

- **Estado**: Aceptado
- **Fecha**: 2026-11-24
- **Decidores**: Juan Andrés

## Contexto

Axfterm nace como terminal local de Windows. `Project.root` es una ruta local,
se canonicaliza con `std::fs::canonicalize`, se exige `is_dir()` y se pasa como
CWD al PTY. Los perfiles ya descubren `wsl -d <distro>`, pero un proyecto WSL
necesita además una ruta POSIX, un CWD Linux y un perfil de terminal fijo.

El producto quiere abrir proyectos que viven dentro de una distro WSL y, más
adelante, reutilizar la misma idea para SSH y contenedores. No se quiere tocar
`axf_terminal` para cada host ni introducir un “modo remoto” genérico antes de
tener una segunda implementación real.

## Decisión

### Host en el proyecto

```rust
pub enum ProjectHost {
    Local,
    Wsl { distro: String },
}

pub struct Project {
    pub id: ProjectId,
    pub root: PathBuf,          // ruta en el dialecto del host
    pub host: ProjectHost,
    pub default_profile: Option<ProfileId>,
    pub name: String,
}
```

- `root` es una ruta local para `Local` y una ruta POSIX para `Wsl`.
- `default_profile` fija la terminal del proyecto. En WSL siempre está presente;
  un proyecto local lo deja en `None` y sigue usando el perfil global.
- Los proyectos se deduplican por `(host, root)`. Solo `Local` se canonicaliza
  con el sistema de archivos; WSL se canonicaliza con `realpath`/`pwd` en la
  distro antes de llamar al modelo.
- `repair()` normaliza raíces locales y conserva las POSIX. Una sesión v2 se
  migra a v3 como proyectos `Local`.

### Lanzamiento por host

La app resuelve un `SpawnTarget { host, path }` y lo convierte en:

- `Local`: `SpawnSpec` del perfil + `cwd` local, como hasta ahora.
- `Wsl`: perfil cuyo ejecutable es `wsl` y cuya distro coincide; se inyecta
  `--cd <ruta>` antes de `--`, `-e` o `--exec`. El CWD del proceso Windows queda
  vacío y `axf_terminal` sigue recibiendo solo programa, argumentos y CWD.

Un perfil incompatible o inexistente es un error accionable; nunca se cae al
perfil global. `Project.default_profile` evita abrir PowerShell dentro de un
proyecto Linux.

### Navegador y validación

- La lista de distros sale de `wsl -l -q` (UTF-16) en segundo plano.
- Las subcarpetas se listan con `find -L ... -printf '%f\0'` y fallback a
  `ls -1p`, siempre como argv y sin shell.
- La validación usa `realpath -e` o `test -d` + `pwd -P`. No se usa el código de
  salida de `pwd` como única prueba: con un `--cd` inválido WSL imprime el error
  pero puede ejecutar `pwd` desde `/` con código 0.
- Todo proceso WSL se lanza con `CREATE_NO_WINDOW`, timeout y sin bloquear el
  hilo de UI.

### UI

- Junto a «Nueva terminal» hay un botón partido con el icono de carpeta: el
  icono abre el selector nativo y el caret de su derecha muestra las fuentes
  disponibles (local y WSL).
- El modal de WSL elige distro, perfil y carpeta. La carpeta se navega con una
  barra de ruta editable y una lista virtualizada de directorios.
- El proyecto muestra un badge de host y conserva su perfil al restaurar.
- Para WSL se ignora el cwd vivo de OSC 7 en la primera versión; duplicar usa la
  ruta del proyecto. Una tarea de pulido lo hace host-aware.

### Futuro

SSH y Docker añaden variantes a `ProjectHost` y a la resolución de lanzamiento.
El motor de terminal no cambia. Autenticación, reconexión, ciclo de vida de
contenedores y puertos quedan fuera de este ADR.

## Consecuencias

### Positivas

- Un proyecto declara dónde vive y con qué terminal se abre; el resto del modelo
  no necesita saber si es WSL.
- La sesión expresa el host sin introducir un `active_project`.
- El mismo patrón sirve para SSH/Docker sin reescribir el PTY.
- Los errores de distro, ruta y perfil son visibles y accionables.

### Negativas y riesgos

- Se sube la sesión a v3; una build anterior rechaza el archivo y arranca limpia.
- `--cd` depende de la versión de WSL; hace falta un mensaje claro y un gate
  manual en Windows 10/11 y WSL1/WSL2.
- `taskkill /T /F` sobre `wsl.exe` puede dejar procesos Linux vivos; se mide y se
  documenta.
- La navegación de carpetas añade latencia de arranque en frío; la UI debe
  mostrar progreso y descartar respuestas viejas.
- El fallback de procesos de agentes no ve Linux; OSC 7501 sigue siendo la
  fuente principal.

## Alternativas

- **Ruta UNC `\\wsl.localhost\...`**: mantiene `PathBuf` local, pero bloquea en
  render, no sirve como CWD y no generaliza a SSH. Se descarta.
- **Un campo `distro` opcional sin host**: mezcla clasificación y lanzamiento y
  deja ambiguo qué perfil puede usar cada pestaña.
- **Trait de transporte remoto ahora**: una sola implementación no justifica la
  abstracción; se extraerá cuando exista SSH o Docker.
- **Resolver el perfil en cada spawn por heurística**: más frágil que guardar el
  perfil elegido en el proyecto.

## Referencias

- `.memory/milestones/08-proyectos-wsl/tasks.md`
- ADR 0003 — modelo de proyectos y pestañas.
- ADR 0006 — perfiles de terminal.
- ADR 0009 — agentes y límite del fallback en WSL.
- `docs/wsl.md`
