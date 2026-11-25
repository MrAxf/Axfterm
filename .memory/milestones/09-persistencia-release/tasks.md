# Tareas — M9 Persistencia y release

## Restauración de historial (adelanto, 2026-11-22)

- [x] Formato ANSI versionado, checksum, `epoch` y tope de 2 MiB por terminal
- [x] Instantánea desde el parser y reproducción antes de la salida del shell
- [x] Guardado en cierre ordenado y poda de archivos huérfanos
- [x] Restauración perezosa de la pestaña activa de cada panel visible
- [x] Fallback al mismo perfil desde cero con aviso accionable
- [x] Tests de roundtrip, atributos, recorte, corrupción y almacén
- [x] Gate manual: dos pestañas, solo arranca la activa, restaura su historial y
      conserva el de la no visitada; un `.snap` corrupto cae a terminal nueva
- [x] Gate manual: dos paneles visibles arrancan una terminal cada uno y
      restauran su historial

## Robustez

- [x] Migraciones de config y sesión v1 -> vN, incluidos proyectos vacíos,
      orden, plegado, `project_id` por pestaña, `profile_id` y foco
- [x] Verificar en actualización real config v1 → v2 y sesión v1 → v2 (M4)
- [x] Perfiles vacíos o ausentes: detección de respaldo sin bloquear el arranque
- [x] Escritura del panel de ajustes: conflicto, error y recuperación sin perder
      la última config válida (`write_config_checked`, conflicto optimista)
- [x] Recuperación ante TOML/JSON corruptos: backup + defaults + aviso
- [x] Límites de scrollback y memoria por terminal (0–50 000 líneas + 32 MiB)
- [x] Límite de terminales abiertas con aviso (32)
- [x] Crash log y `tracing` opt-in (`AXFTERM_LOG`, rotación 2 MiB)
- [x] Instancia única y abrir carpeta desde CLI/Explorer (mutex + named pipe)

## Release

- [x] Icono de app y metadatos de Windows (embeber `.ico` y sustituir el glifo
      provisional de la barra de título)
- [x] Build release + MSI y zip portable (validado instalar/desinstalar)
- [x] Firma de código: evaluar (decisión v0.1: no firmar; ver `docs/release.md`)
- [x] Notas de versión y canal de actualización (`CHANGELOG.md`, `docs/release.md`)
- [x] Auto-update: decidir dentro o fuera de v0.1 (fuera de v0.1)

## Calidad

- [x] Benchmarks finales: arranque, RAM, frame y fuzzy
- [ ] Matriz Windows 10/11 + DPI 100/150/200 (manual pendiente)
- [ ] Pruebas con teclados ES y CJK, IME chino/japonés (manual pendiente)
- [x] Docs: README, config, profiles, settings, keymap, themes y troubleshooting
- [ ] Gate v0.1: instalar, usar un día completo y cerrar sin crashes (manual)
