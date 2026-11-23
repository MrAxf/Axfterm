#Requires -Version 5.1
<#
.SYNOPSIS
    Ejecuta las comprobaciones obligatorias del workspace Axfterm.

.DESCRIPTION
    fmt + clippy (-D warnings) + test. Falla con código distinto de cero en el
    primer paso que no pase. Todo pasa por `cargo-low.ps1`: prioridad
    BelowNormal y `-j 1`, para reducir la carga sobre el escritorio.

.PARAMETER SkipFmt
    Omite `cargo fmt --all -- --check`.

.PARAMETER SkipClippy
    Omite `cargo clippy --workspace --all-targets -- -D warnings`.

.PARAMETER SkipTests
    Omite `cargo test --workspace`.

.PARAMETER Jobs
    Jobs de cargo para esta ejecución. 0 (por defecto) respeta
    `AXF_CARGO_JOBS` o `CARGO_BUILD_JOBS`, y si no están definidos usa 1.

.EXAMPLE
    pwsh -File scripts/check.ps1
#>
[CmdletBinding()]
param(
    [switch]$SkipFmt,
    [switch]$SkipClippy,
    [switch]$SkipTests,

    [ValidateRange(0, 64)]
    [int]$Jobs = 0
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($Jobs -gt 0) {
    $env:AXF_CARGO_JOBS = "$Jobs"
}

$cargoLow = Join-Path $PSScriptRoot 'cargo-low.ps1'
if (-not (Test-Path $cargoLow)) {
    throw "No se encontró $cargoLow."
}

function Invoke-Cargo {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Label,

        # Array explícito: un `--` suelto como argumento de función lo consume
        # el binder de PowerShell y cargo no lo recibe.
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    Write-Host "==> cargo $($Arguments -join ' ')" -ForegroundColor Cyan
    & $cargoLow @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Fallo '$Label' (exit code $LASTEXITCODE)."
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $repoRoot 'Cargo.toml'))) {
    throw "No se encontró Cargo.toml en $repoRoot."
}

Push-Location $repoRoot
try {
    $lineLimit = 500
    $oversizedRustFiles = @(
        Get-ChildItem -Path (Join-Path $repoRoot 'crates') -Directory |
            ForEach-Object {
                $srcPath = Join-Path $_.FullName 'src'
                if (Test-Path $srcPath) {
                    Get-ChildItem -Path $srcPath -Recurse -Filter '*.rs' |
                        ForEach-Object {
                            $lineCount = [System.IO.File]::ReadAllLines($_.FullName).Count
                            if ($lineCount -gt $lineLimit) {
                                [pscustomobject]@{
                                    Path = $_.FullName.Substring($repoRoot.Length + 1)
                                    Lines = $lineCount
                                }
                            }
                        }
                }
            }
    )
    if ($oversizedRustFiles.Count -gt 0) {
        $details = $oversizedRustFiles |
            ForEach-Object { "  $($_.Path): $($_.Lines) líneas (máximo $lineLimit)" }
        throw "Hay archivos Rust de producción que superan $lineLimit líneas:`n$($details -join "`n")"
    }
    Write-Host "OK: archivos Rust de producción dentro del límite de $lineLimit líneas." -ForegroundColor Green

    if (-not $SkipFmt) {
        Invoke-Cargo -Label 'fmt' -Arguments @('fmt', '--all', '--', '--check')
    }

    if (-not $SkipClippy) {
        Invoke-Cargo -Label 'clippy' -Arguments @(
            'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'
        )
    }

    if (-not $SkipTests) {
        Invoke-Cargo -Label 'test' -Arguments @('test', '--workspace')
    }

    Write-Host 'OK: todas las comprobaciones pasaron.' -ForegroundColor Green
}
finally {
    Pop-Location
}
