#Requires -Version 5.1
<#
.SYNOPSIS
    Genera el zip portable y el MSI de Axfterm para Windows x64.

.DESCRIPTION
    Compila en release con el wrapper de cargo, prepara un zip portable y,
    si WiX está disponible, construye el MSI per-user. Todo se escribe en
    `dist/`. No firma el binario: la firma de código se evalúa post-v0.1.

    Uso:
        pwsh -File scripts/package.ps1
        pwsh -File scripts/package.ps1 -SkipBuild
        pwsh -File scripts/package.ps1 -SkipMsi
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$SkipMsi,
    [switch]$SkipZip
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $cargoToml = Get-Content (Join-Path $repoRoot 'Cargo.toml') -Raw
    $match = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) {
        throw 'No se pudo leer la versión del workspace en Cargo.toml.'
    }
    $version = $match.Groups[1].Value
    $exe = Join-Path $repoRoot 'target/release/axfterm.exe'
    $dist = Join-Path $repoRoot 'dist'
    $stem = "Axfterm-$version-win-x64"
    $zip = Join-Path $dist "$stem.zip"
    $msi = Join-Path $dist "$stem.msi"

    if (-not $SkipBuild) {
        & (Join-Path $PSScriptRoot 'cargo-low.ps1') build --release
        if ($LASTEXITCODE -ne 0) {
            throw "Fallo la build release (exit $LASTEXITCODE)."
        }
    }
    if (-not (Test-Path $exe)) {
        throw "No existe $exe. Ejecuta sin -SkipBuild."
    }

    New-Item -ItemType Directory -Path $dist -Force | Out-Null

    if (-not $SkipZip) {
        $staging = Join-Path $dist $stem
        if (Test-Path $staging) {
            Remove-Item -Recurse -Force $staging
        }
        New-Item -ItemType Directory -Path $staging -Force | Out-Null
        Copy-Item $exe (Join-Path $staging 'axfterm.exe')
        foreach ($name in @('README.md', 'NOTICE', 'LICENSE-MIT', 'LICENSE-APACHE')) {
            Copy-Item (Join-Path $repoRoot $name) $staging
        }
        Copy-Item (Join-Path $repoRoot 'docs') (Join-Path $staging 'docs') -Recurse
        if (Test-Path $zip) {
            Remove-Item -Force $zip
        }
        Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zip -CompressionLevel Optimal
        Remove-Item -Recurse -Force $staging
        Write-Host "Zip portable: $zip" -ForegroundColor Green
    }

    if (-not $SkipMsi) {
        $dotnet = Get-Command dotnet -ErrorAction SilentlyContinue
        if (-not $dotnet) {
            Write-Warning 'dotnet no está disponible; se omite el MSI.'
        }
        elseif (-not (Test-Path (Join-Path $repoRoot '.config/dotnet-tools.json'))) {
            Write-Warning 'Falta .config/dotnet-tools.json; se omite el MSI.'
        }
        else {
            & dotnet tool restore | Out-Host
            if ($LASTEXITCODE -ne 0) {
                throw "Fallo 'dotnet tool restore' (exit $LASTEXITCODE)."
            }
            $wxs = Join-Path $repoRoot 'installer/Axfterm.wxs'
            & dotnet tool run wix build $wxs `
                -d "ProjectRoot=$repoRoot" `
                -d "Version=$version" `
                -arch x64 `
                -o $msi
            if ($LASTEXITCODE -ne 0) {
                throw "Fallo 'wix build' (exit $LASTEXITCODE)."
            }
            # WiX deja un .wixpdb de símbolos junto al MSI; no se distribuye.
            Get-ChildItem -Path $dist -Filter '*.wixpdb' -ErrorAction SilentlyContinue |
                Remove-Item -Force -ErrorAction SilentlyContinue
            Write-Host "MSI: $msi" -ForegroundColor Green
        }
    }

    $artifacts = @()
    if (Test-Path $zip) { $artifacts += Get-Item $zip }
    if (Test-Path $msi) { $artifacts += Get-Item $msi }
    if ($artifacts.Count -gt 0) {
        $sums = foreach ($artifact in $artifacts) {
            $hash = (Get-FileHash -Algorithm SHA256 $artifact.FullName).Hash.ToLowerInvariant()
            "$hash  $($artifact.Name)"
        }
        Set-Content -Path (Join-Path $dist 'SHA256SUMS.txt') -Value $sums -Encoding ascii
        Write-Host "Hashes: $(Join-Path $dist 'SHA256SUMS.txt')" -ForegroundColor Green
    }
}
finally {
    Pop-Location
}
