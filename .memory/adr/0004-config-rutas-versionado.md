# ADR 0004 — Configuración TOML, rutas y versionado

- **Estado**: Aceptado
- **Fecha**: 2026-11-21
- **Decidores**: Juan Andrés

## Contexto

La configuración es un contrato con el usuario: se edita a mano, se recarga en
caliente y no puede tumbar la app ni perder trabajo. La sesión, en cambio, es un
archivo que escribe Axfterm y que debe sobrevivir a cierres, corrupción y
actualizaciones. Ambos viven en el mismo directorio y comparten la necesidad de
versionado, escritura atómica y recuperación, pero tienen políticas distintas.

Restricciones:

- Windows primero; macOS/Linux después, con rutas nativas.
- Sin telemetry ni red; la config no sale de la máquina.
- Sin GUI de ajustes en el primer corte de v0.1: el archivo es la interfaz. M7
  añade un panel que escribe en el mismo archivo (ver enmienda al final).
- `toml` 1.x y `notify` 9 ya están en el stack; no se añade un crate de rutas si
  `cfg` + variables de entorno bastan.
- Un error de config debe ser accionable (archivo, campo) y nunca un panic.

## Decisión

### Rutas

| Plataforma | Directorio |
| --- | --- |
| Windows | `%APPDATA%\axfterm\` |
| macOS | `~/Library/Application Support/axfterm/` |
| Linux/BSD | `$XDG_CONFIG_HOME/axfterm/` o `~/.config/axfterm/` |

- La resolución vive en `axf_config::paths` con `cfg(target_os)`; no se usa el
  crate `directories` para no añadir una dependencia por tres rutas.
- `AXFTERM_CONFIG_DIR` (ruta absoluta) permite tests y diagnóstico. No es un
  modo portable de producto en v0.1.
- Archivos: `config.toml` (usuario), `themes/*.toml` (usuario) y `session.json`
  (app). El directorio se crea con los permisos por defecto del usuario.
- Si la variable de entorno o el directorio base no existen, se devuelve un
  error tipado y la app arranca con defaults en memoria; no se escribe fuera de
  la ruta esperada.

### Esquema

`config.toml` lleva un entero `schema_version` en la raíz:

```toml
schema_version = 1

[app]
# ...

[ui]
# ...

[terminal]
# ...

[keybindings]
# ...

[agents]
# ...
```

- `schema_version` ausente se interpreta como `1` para no romper configs escritas
  a mano durante v0.1.
- Las claves concretas de cada sección se fijan en M3 (tema/fuentes), M4
  (perfiles) y M6 (keybindings); este ADR fija la raíz versionada, las secciones
  estables y la política de cambio.
- Los temas son archivos independientes en `themes/`: `[meta]` con nombre y
  variante, tokens de UI, paleta ANSI de 16 colores, cursor y selección.
- `config.toml` **rechaza campos desconocidos** (`deny_unknown_fields`): un typo
  debe dar error, no ignorarse en silencio. `session.json` **ignora campos
  desconocidos** para tolerar versiones nuevas al leer.
- Los valores se validan con rangos explícitos (tamaños de fuente, scrollback,
  intervalos). Un valor fuera de rango es error con ruta de campo; no se hace
  clamp silencioso.

### Carga y errores

- **Primer arranque**: si `config.toml` no existe, se escribe una plantilla
  mínima comentada (mejor esfuerzo). Si falla la escritura, se usan defaults sin
  bloquear.
- **Arranque con config corrupta o inválida**: se renombra a
  `config.toml.bak-YYYYMMDD-HHMMSS` (con sufijo si ya existe), se arranca con
  defaults y se muestra un aviso accionable con la ruta del backup.
- **Hot reload con error**: se conserva la última configuración válida y se
  muestra el error con archivo y línea/columna de `toml` o ruta del campo. La app
  sigue funcionando.
- **Migración**: `schema_version` mayor que el soportado es un error explícito;
  no se degrada ni se reescribe el archivo. Versiones anteriores se migran en
  memoria mediante una cadena `migrate(from -> from + 1)` sobre
  `toml::Value`; el archivo se reescribe solo cuando la app es dueña de la
  escritura (sesión) o el usuario guarda.
- **Escritura**: archivo temporal en el mismo directorio, `flush` + `sync`,
  `rename` atómico sobre el destino. Nunca se escribe directamente sobre el
  archivo vivo.
- **Tamaño**: los límites de ADR 0005 se comprueban antes de parsear; un archivo
  por encima del límite se trata como corrupto.

### Hot reload

- `notify` 9 observa el archivo de config y el directorio `themes/` (no todo el
  perfil).
- Debounce de 250 ms para absorber escrituras de editores; se ignoran archivos
  temporales y de swap.
- Se relee y valida el archivo completo; si es válido, se aplica de forma
  atómica. Los cambios de tema, fuente o tamaño base recalculan métricas y
  limpian caches de render sin reiniciar. Los cambios de keybindings se validan
  (duplicados, teclas reservadas, `unbind`) antes de registrarse.
- Un fallo de aplicación no deja la config a medias: se aplica todo o nada.

### Sesión

- `session.json` es de la app, no se recarga en caliente y se escribe al cerrar
  y en puntos de guardado.
- Lleva `version: 1`; misma política de migración y rechazo de versiones futuras
  que la config.
- Corrupta o ilegible: backup fechado + workspace limpio; nunca se pierde el
  archivo original.
- No se restauran procesos muertos: la pestaña se marca como terminada.
- El esquema de datos es el de ADR 0003; el formato exacto y las migraciones
  v1→vN se implementan en M2/M8.

### Seguridad y privacidad

- Sin telemetría, red ni secretos en la config. Las variables de entorno del
  shell se heredan, no se serializan salvo que el usuario las escriba.
- Los logs son opt-in (M8) y viven fuera de `config.toml`.

## Consecuencias

### Positivas

- Un solo lugar para rutas y versionado; tests con directorios temporales.
- La app nunca arranca con una config a medias ni pierde la última válida.
- Los typos de config se detectan en vez de ignorarse.
- La migración es explícita y reversible por backup.
- El hot reload no requiere reiniciar ni puede dejar estado inconsistente.

### Negativas y riesgos

- Rechazar campos desconocidos puede molestar al copiar una config de una
  versión futura; el error y el backup hacen el coste visible y recuperable.
- `notify` tiene diferencias entre plataformas (eventos duplicados, rename);
  Mitigación: debounce, relectura completa y tests de reload.
- La plantilla de primer arranque debe mantenerse sincronizada con el esquema;
  Mitigación: generarla desde los defaults y testear que parsea.
- Las rutas `cfg` no cubren todos los casos raros (OneDrive, perfiles
  redirigidos); Mitigación: validar la ruta y reportar error, no improvisar.

## Alternativas

- **JSON para todo**: sin comentarios y menos legible para config de usuario.
- **RON**: más expresivo, pero menos familiar y con menos herramientas.
- **Un único archivo config + sesión**: el hot reload competiría con las
  escrituras de sesión y un error arrastraría ambos.
- **Crate `directories`/`dirs`**: correcto, pero añade dependencia y superficie
  para tres rutas que caben en un módulo con tests.
- **Sin `schema_version`**: imposible migrar sin heurísticas; rechazado.

## Referencias

- `AGENTS.md` — persistencia, hot reload y convenciones.
- `.memory/milestones/08-persistencia-release/tasks.md` — migraciones y
  recuperación.
- ADR 0003 — esquema de la sesión.
- ADR 0005 — límites de tamaño y comportamiento al alcanzarlos.

## Nota de implementación (M3)

`notify` 9 exige Rust 1.91 y rompería la MSRV 1.85. M3 implementó el hot reload
con `notify 8.2.1`, la misma versión que ya usa gpui-kit, manteniendo el
comportamiento descrito aquí: watch de `config.toml` y `themes/`, debounce de
250 ms, relectura completa y aplicación atómica.

## Nota de enmienda (M4/M7, 2026-11-22)

- `schema_version` sube a 2 con `[[profiles]]` y `[terminal] default_profile`
  (ADR 0006). La migración v1 → v2 se hace en memoria y, si faltan perfiles, se
  completa con el descubrimiento y una escritura atómica única.
- M7 añade el panel de ajustes (ADR 0007). El archivo sigue siendo la fuente de
  verdad: el panel escribe con `toml_edit` preservando comentarios, valida antes
  de escribir y no mantiene un almacén paralelo. La edición manual y el hot
  reload siguen siendo contratos de primera clase.
- El panel no hereda el hot reload para escrituras propias: reconoce el cambio
  que él mismo provocó para no aplicar dos veces la misma config.

## Enmienda M7 (2026-11-23) — escritura desde el panel

- `axf_config::write_config(path, base, draft)` edita el documento actual con
  `toml_edit`: solo toca los campos cuyo valor difiere entre `base` y `draft`,
  conserva comentarios y orden (incluidos los de `[keymap]` y `[[profiles]]`) y
  vuelve a parsear el resultado antes de escribir con `write_atomic`.
- El borrador del panel no modifica la config activa: la apariencia y las
  fuentes se aplican como previsualización en `ThemeHost` y `Descartar` las
  revierte. Al guardar se aplica una sola vez y el evento del watcher producido
  por la escritura se ignora comparando el contenido con la huella guardada.
- Conflicto externo: con el borrador limpio se recarga; con cambios sin guardar
  el panel pide recargar o seguir editando, y guardar exige confirmación
  explícita para sobrescribir. Nunca se pisa en silencio.
- `[agents]` añade `preferred_profiles` (agente → id de perfil), validado contra
  `[[profiles]]`; el lanzador de agentes de v0.2 lo consumirá.
- `theme.info` queda como id legado que abre el panel; los atajos existentes
  siguen funcionando, pero la ayuda y la paleta muestran `settings.open`.
