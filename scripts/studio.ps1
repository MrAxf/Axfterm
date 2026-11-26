#Requires -Version 5.1
<#
.SYNOPSIS
    Lanza Axfterm Studio y captura escenas del sistema de diseño.

.DESCRIPTION
    Compila el binario con la feature `studio`, lo arranca con un perfil aislado
    en `target/studio/home`, reproduce las escenas de `studio/scenes` y escribe
    PNGs y un manifiesto en `target/studio`.

.EXAMPLE
    pwsh scripts/studio.ps1 -All
    pwsh scripts/studio.ps1 -Scene gallery.actions
    pwsh scripts/studio.ps1 -Serve
#>
[CmdletBinding()]
param(
    [string[]]$Scene = @(),
    [switch]$All,
    [switch]$Serve,
    [switch]$Hidden,
    [switch]$NoBuild,
    [string]$Dir = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $Dir) {
    $Dir = Join-Path $repoRoot 'target/studio'
}
$Dir = [System.IO.Path]::GetFullPath($Dir)
$homeDir = Join-Path $Dir 'home'
$shotsDir = Join-Path $Dir 'shots'
$scenesDir = Join-Path $repoRoot 'studio/scenes'
$sessionPath = Join-Path $Dir 'session.json'
New-Item -ItemType Directory -Force -Path $Dir, $homeDir, $shotsDir | Out-Null
Remove-Item -Force -ErrorAction SilentlyContinue $sessionPath, (Join-Path $homeDir 'session.json')
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue (Join-Path $homeDir 'scrollback')
$themesDir = Join-Path $homeDir 'themes'
$themeFixture = Join-Path $repoRoot 'studio/fixtures/themes/axfterm-studio-fixture.toml'
$themeFixtureTarget = Join-Path $themesDir 'axfterm-studio-fixture.toml'
New-Item -ItemType Directory -Force -Path $themesDir | Out-Null
if (Test-Path $themeFixtureTarget) {
    $firstLine = Get-Content -Path $themeFixtureTarget -TotalCount 1
    if ($firstLine -ne '# Axfterm Studio fixture') {
        throw "El archivo de tema de Studio ya existe y no es una fixture: $themeFixtureTarget"
    }
}
Copy-Item -LiteralPath $themeFixture -Destination $themeFixtureTarget -Force

if (-not $NoBuild) {
    Write-Host '==> compilando axfterm con --features studio' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'cargo-low.ps1') build -p axfterm --features studio
    if ($LASTEXITCODE -ne 0) {
        throw "Falló la compilación (exit code $LASTEXITCODE)."
    }
}

$exe = Join-Path $repoRoot 'target/debug/axfterm.exe'
if (-not (Test-Path $exe)) {
    throw "No se encontró $exe. Ejecuta sin -NoBuild."
}

$env:AXFTERM_CONFIG_DIR = $homeDir
$arguments = @(
    '--studio',
    '--studio-dir', "`"$Dir`"",
    '--studio-fixture', 'demo'
)
if ($Hidden) {
    $arguments += '--studio-hidden'
}
$stderr = Join-Path $Dir 'stderr.log'
$stdout = Join-Path $Dir 'stdout.log'
Write-Host "==> iniciando Studio ($exe)" -ForegroundColor Cyan
$process = Start-Process -FilePath $exe -ArgumentList ($arguments -join ' ') `
    -PassThru `
    -RedirectStandardError $stderr -RedirectStandardOutput $stdout

try {
    $deadline = (Get-Date).AddSeconds(30)
    while (-not (Test-Path $sessionPath)) {
        if ($process.HasExited) {
            $detail = if (Test-Path $stderr) { Get-Content $stderr -Raw } else { '' }
            throw "Studio terminó antes de abrir el control.`n$detail"
        }
        if ((Get-Date) -gt $deadline) {
            throw "Studio no escribió $sessionPath en 30 segundos."
        }
        Start-Sleep -Milliseconds 100
    }
    $session = Get-Content $sessionPath -Raw | ConvertFrom-Json
    Write-Host "==> control en 127.0.0.1:$($session.port)" -ForegroundColor Green

    if ($Serve) {
        Write-Host "Studio visible. Capturas en $shotsDir" -ForegroundColor Green
        Write-Host 'Cuando termines, ejecuta: pwsh scripts/devtools.ps1 quit'
        return
    }

    $selectAll = [bool]$All
    if (-not $selectAll -and $Scene.Count -eq 0) {
        $selectAll = $true
    }
    $files = @(Get-ChildItem -Path $scenesDir -Filter '*.json' | Sort-Object Name)
    if ($selectAll) {
        $selected = $files
    }
    else {
        $selected = @()
        foreach ($name in $Scene) {
            $slug = $name -replace '\.', '-'
            $match = $files | Where-Object {
                $_.BaseName -eq $name -or $_.BaseName -eq $slug -or
                $_.BaseName -eq "gallery-$slug" -or $_.BaseName -eq "app-$slug"
            }
            if (-not $match) {
                throw "No existe la escena '$name' en $scenesDir."
            }
            $selected += $match
        }
    }
    if ($selected.Count -eq 0) {
        throw "No hay escenas en $scenesDir."
    }

    $devtools = Join-Path $PSScriptRoot 'devtools.ps1'
    $manifest = @()
    foreach ($file in $selected) {
        $sceneData = Get-Content $file.FullName -Raw | ConvertFrom-Json
        $sceneName = if ($sceneData.PSObject.Properties['name']) {
            $sceneData.PSObject.Properties['name'].Value
        }
        else {
            $file.BaseName
        }
        $steps = @($sceneData.PSObject.Properties['steps'].Value)
        Write-Host "==> escena $sceneName" -ForegroundColor Cyan
        # La escala de UI es estado global: cada escena parte de 1.0 salvo que
        # pida otra cosa en sus pasos.
        & $devtools scale 1 | Out-Null
        foreach ($step in $steps) {
            $arguments = @($step.PSObject.Properties['cmd'].Value)
            $stepArgs = $step.PSObject.Properties['args']
            if ($stepArgs -and $stepArgs.Value) { $arguments += @($stepArgs.Value) }
            $stepClip = $step.PSObject.Properties['clip']
            if ($stepClip -and $stepClip.Value) { $arguments += @('-Clip', $stepClip.Value) }
            $arguments += @('-Dir', $Dir)
            $raw = (& $devtools @arguments) -join "`n"
            $result = $raw | ConvertFrom-Json
            if (-not $result.ok) {
                throw "La escena '$sceneName' falló en '$($step.PSObject.Properties['cmd'].Value)': $raw"
            }
            if ($result.PSObject.Properties.Name -contains 'shot') {
                $manifest += [pscustomobject]@{
                    scene = $sceneName
                    path = $result.shot.path
                    width = $result.shot.width
                    height = $result.shot.height
                    scale = $result.shot.scale
                    ui_scale = $result.shot.ui_scale
                }
            }
        }
    }
    $manifestPath = Join-Path $Dir 'manifest.json'
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -Path $manifestPath -Encoding UTF8
    Write-Host "==> $($manifest.Count) capturas en $shotsDir" -ForegroundColor Green
    Write-Host "==> manifiesto $manifestPath" -ForegroundColor Green
}
finally {
    if (-not $Serve) {
        try {
            if (Test-Path $sessionPath) {
                & (Join-Path $PSScriptRoot 'devtools.ps1') -Command quit -Dir $Dir | Out-Null
            }
        }
        catch {
            # La app ya puede estar cerrada; el kill de abajo es el respaldo.
        }
        if (-not $process.HasExited) {
            $process.WaitForExit(5000) | Out-Null
        }
        if (-not $process.HasExited) {
            $process.Kill()
        }
        Remove-Item -Force -ErrorAction SilentlyContinue $sessionPath
    }
}
