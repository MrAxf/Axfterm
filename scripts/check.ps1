#Requires -Version 5.1
<#
.SYNOPSIS
    Ejecuta las comprobaciones obligatorias del workspace Axfterm.

.DESCRIPTION
    fmt + clippy (-D warnings) + test. Falla con código distinto de cero en el
    primer paso que no pase.

.PARAMETER SkipFmt
    Omite `cargo fmt --all -- --check`.

.PARAMETER SkipClippy
    Omite `cargo clippy --workspace --all-targets -- -D warnings`.

.PARAMETER SkipTests
    Omite `cargo test --workspace`.

.EXAMPLE
    pwsh -File scripts/check.ps1
#>
[CmdletBinding()]
param(
    [switch]$SkipFmt,
    [switch]$SkipClippy,
    [switch]$SkipTests
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

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
    & cargo @Arguments
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
