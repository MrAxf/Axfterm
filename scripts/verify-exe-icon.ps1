#Requires -Version 5.1
<#
.SYNOPSIS
    Comprueba que un ejecutable lleva embebido el icono de Axfterm.

.DESCRIPTION
    Lee el `.ico` multi-tamaño y verifica que cada una de sus imágenes aparece
    íntegra dentro del PE. `rc.exe` copia los datos de cada `RT_ICON` tal cual,
    así que la comprobación demuestra que el icono asignado al ejecutable es el
    mismo archivo que se empaqueta. No necesita herramientas externas: usa
    .NET y un helper C# en memoria.

    Si el ejecutable no lleva el icono (build release sin Windows SDK) el
    script falla con un mensaje accionable; `package.ps1` lo llama antes de
    generar zip y MSI.

.PARAMETER Exe
    Ejecutable a comprobar. Por defecto `target/release/axfterm.exe`.

.PARAMETER Icon
    Icono de referencia. Por defecto `crates/app/assets/axfterm.ico`.

.EXAMPLE
    pwsh -File scripts/verify-exe-icon.ps1

.EXAMPLE
    pwsh -File scripts/verify-exe-icon.ps1 -Exe 'C:\ruta\axfterm.exe'
#>
[CmdletBinding()]
param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target/release/axfterm.exe'),
    [string]$Icon = (Join-Path (Split-Path -Parent $PSScriptRoot) 'crates/app/assets/axfterm.ico')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $Exe)) {
    throw "No existe el ejecutable a comprobar: $Exe"
}
if (-not (Test-Path -LiteralPath $Icon)) {
    throw "No existe el icono de referencia: $Icon"
}

if (-not ('Axf.IconProbe' -as [type])) {
    Add-Type -TypeDefinition @'
namespace Axf {
    public static class IconProbe {
        public static int IndexOf(byte[] haystack, byte[] needle) {
            if (needle == null || needle.Length == 0 || haystack == null || haystack.Length < needle.Length) {
                return -1;
            }
            int limit = haystack.Length - needle.Length;
            byte first = needle[0];
            for (int i = 0; i <= limit; i++) {
                if (haystack[i] != first) {
                    continue;
                }
                int j = 1;
                while (j < needle.Length && haystack[i + j] == needle[j]) {
                    j++;
                }
                if (j == needle.Length) {
                    return i;
                }
            }
            return -1;
        }
    }
}
'@
}

$exePath = (Resolve-Path -LiteralPath $Exe).ProviderPath
$iconPath = (Resolve-Path -LiteralPath $Icon).ProviderPath
$exeBytes = [System.IO.File]::ReadAllBytes($exePath)
$iconBytes = [System.IO.File]::ReadAllBytes($iconPath)

if ($iconBytes.Length -lt 6) {
    throw "El icono está truncado: $iconPath"
}
$reserved = [System.BitConverter]::ToUInt16($iconBytes, 0)
$type = [System.BitConverter]::ToUInt16($iconBytes, 2)
$count = [System.BitConverter]::ToUInt16($iconBytes, 4)
if ($reserved -ne 0 -or $type -ne 1 -or $count -eq 0) {
    throw "No es un .ico válido: $iconPath"
}

$missing = New-Object System.Collections.Generic.List[string]
for ($i = 0; $i -lt $count; $i++) {
    $entry = 6 + 16 * $i
    if (($entry + 16) -gt $iconBytes.Length) {
        throw "La entrada $i del icono sale del archivo: $iconPath"
    }
    $width = [int]$iconBytes[$entry]
    $dimension = if ($width -eq 0) { 256 } else { $width }
    $length = [int][System.BitConverter]::ToUInt32($iconBytes, $entry + 8)
    $offset = [int][System.BitConverter]::ToUInt32($iconBytes, $entry + 12)
    if ($length -le 0 -or $offset -lt 0 -or ($offset + $length) -gt $iconBytes.Length) {
        throw "La imagen $dimension px del icono sale del archivo: $iconPath"
    }
    $frame = New-Object byte[] $length
    [System.Array]::Copy($iconBytes, $offset, $frame, 0, $length)
    if ([Axf.IconProbe]::IndexOf($exeBytes, $frame) -lt 0) {
        $missing.Add("$dimension px")
    }
}

if ($missing.Count -gt 0) {
    $detail = $missing -join ', '
    throw ("No se puede empaquetar: '$exePath' no contiene el icono de Axfterm " +
        "(faltan $detail de $iconPath). Reconstruye con " +
        "'scripts/cargo-low.ps1 build --release' y el Windows SDK (rc.exe) instalado.")
}

Write-Host "OK: el ejecutable contiene el icono de la app ($count tamaños)." -ForegroundColor Green
