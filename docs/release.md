# Release y empaquetado

## Versión y canal

- Versión actual: **0.1.0**.
- Canal: **estable local**. No hay auto-update ni canal beta separado en v0.1.
- El número de versión vive en `Cargo.toml` (`[workspace.package].version`) y se
  propaga al MSI, al zip, a `FileVersion`/`ProductVersion` del `.exe` y a los
  crash reports.

## Artefactos

`scripts/package.ps1` genera en `dist/`:

| Artefacto | Contenido |
| --- | --- |
| `Axfterm-<version>-win-x64.zip` | `axfterm.exe`, README, licencias, NOTICE y `docs/` |
| `Axfterm-<version>-win-x64.msi` | Instalación per-user, menú Inicio, `App Paths` y «Abrir con Axfterm» |
| `SHA256SUMS.txt` | Hash de cada artefacto |

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
- Acceso directo en el menú Inicio.
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
2. `pwsh -File scripts/cargo-low.ps1 build --release` en verde.
3. `pwsh -File scripts/package.ps1` genera zip y MSI.
4. Instalar el MSI en una cuenta limpia:
   - abrir desde el menú Inicio;
   - ejecutar `axfterm C:\ruta` desde Win+R/PowerShell;
   - «Abrir con Axfterm» sobre una carpeta;
   - comprobar que una segunda instancia reenvía la carpeta.
5. Desinstalar y verificar que `%APPDATA%\axfterm` se conserva.
6. Probar el zip portable en una carpeta con espacios y sin permisos de admin.
7. Matriz manual: Windows 10/11, DPI 100/150/200, teclado ES y CJK.
8. Actualizar `CHANGELOG.md` y esta página si cambia el canal.

## Notas de versión

Las notas públicas de cada versión viven en `CHANGELOG.md`. Las decisiones que
cambian el alcance (firma, auto-update, canales) se registran aquí y, si tocan
límites, en un ADR nuevo.
