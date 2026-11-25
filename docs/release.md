# Release y empaquetado

## Versión y canal

- Versión actual: **0.2.0**.
- Canal: **estable local**. No hay auto-update ni canal beta separado en v0.1.
- El número de versión vive en `Cargo.toml` (`[workspace.package].version`) y se
  propaga al MSI, al zip, a `FileVersion`/`ProductVersion` del `.exe` y a los
  crash reports.

## Artefactos

`scripts/package.ps1` genera en `dist/`:

| Artefacto | Contenido |
| --- | --- |
| `Axfterm-<version>-win-x64.zip` | `axfterm.exe`, `axfterm.ico`, README, licencias, NOTICE y `docs/` |
| `Axfterm-<version>-win-x64.msi` | Instalación per-user, menú Inicio, `App Paths` y «Abrir con Axfterm»; instala también `axfterm.ico` junto al ejecutable |
| `SHA256SUMS.txt` | Hash de cada artefacto |

El icono de la app tiene una única fuente, `crates/app/assets/axfterm.ico`:
`build.rs` lo embebe en el `.exe` con `winresource` (recurso 1, el que GPUI usa
para la ventana y la taskbar), el zip lo adjunta y el MSI lo instala. El acceso
directo del menú Inicio usa el icono del propio ejecutable y
`ARPPRODUCTICON` se genera desde el mismo `.ico`.

`scripts/verify-exe-icon.ps1` comprueba que cada imagen del `.ico` aparece
íntegra dentro del `.exe`; `package.ps1` lo ejecuta antes de empaquetar y aborta
si no coincide. La build release necesita el Windows SDK (`rc.exe`): sin él
`build.rs` falla a propósito en vez de publicar un binario sin icono ni
metadatos. En debug solo avisa.

El MSI se construye con WiX 5 como herramienta local de dotnet
(`.config/dotnet-tools.json`). Se fija la 5 porque WiX 7 exige aceptar el EULA
del Open Source Maintenance Fee y no aporta nada al instalador de v0.1.
La primera vez:

```powershell
dotnet tool restore
pwsh -File scripts/package.ps1
```

Opciones: `-SkipBuild`, `-SkipZip`, `-SkipMsi`.

## Instalación per-user

- Destino: `%LOCALAPPDATA%\Programs\Axfterm`.
- Acceso directo en el menú Inicio, con el icono del ejecutable.
- `axfterm.ico` se instala junto al `.exe`; `ARPPRODUCTICON` usa ese mismo
  archivo tal como lo embebe el binario.
- Registro en `HKCU` de `App Paths` y de los verbos de Explorer.
- `MajorUpgrade` desinstala la versión anterior del mismo producto.
- El zip portable no escribe registro ni requiere permisos de administrador.

## Firma de código

**Decisión v0.1: no se firma.** No hay certificado de code signing en el
proyecto. Windows SmartScreen mostrará el aviso de editor desconocido. Firmar
queda para una release posterior, cuando exista certificado y proceso de
custodia; el script de empaquetado no depende de ello.

## Auto-update

**Decisión v0.1: fuera de alcance.** El usuario descarga el MSI o el zip y
actualiza manualmente. ADR 0005 ya lo excluía; añadir red, feed y rollback
excede el objetivo de estabilidad de v0.1. La actualización manual conserva
`%APPDATA%\axfterm` (config, sesión, temas, scrollback y logs).

## Checklist antes de publicar

1. `pwsh -File scripts/check.ps1` en verde.
2. `pwsh -File scripts/cargo-low.ps1 build --release` en verde; el icono y los
   metadatos se embeben en el `.exe` (requiere el Windows SDK).
3. `pwsh -File scripts/verify-exe-icon.ps1` en verde (lo ejecuta también
   `package.ps1`).
4. `pwsh -File scripts/package.ps1` genera zip y MSI con `axfterm.ico`.
5. Instalar el MSI en una cuenta limpia:
   - abrir desde el menú Inicio;
   - ejecutar `axfterm C:\ruta` desde Win+R/PowerShell;
   - «Abrir con Axfterm» sobre una carpeta;
   - comprobar que una segunda instancia reenvía la carpeta.
6. Desinstalar y verificar que `%APPDATA%\axfterm` se conserva.
7. Probar el zip portable en una carpeta con espacios y sin permisos de admin.
8. Matriz manual: Windows 10/11, DPI 100/150/200, teclado ES y CJK.
9. Actualizar `CHANGELOG.md` y esta página si cambia el canal.

## Notas de versión

Las notas públicas de cada versión viven en `CHANGELOG.md`. Las decisiones que
cambian el alcance (firma, auto-update, canales) se registran aquí y, si tocan
límites, en un ADR nuevo.
