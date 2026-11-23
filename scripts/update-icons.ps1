# Actualiza los SVG vendorizados de Axfterm.
#
# Uso:
#   pwsh -File scripts/update-icons.ps1
#
# Tabler se descarga de la rama `main` (outline, 24x24, stroke=currentColor) y
# simple-icons de `develop`. Después de actualizar, ejecutar:
#   cargo test -p axf_ui
# para comprobar que todos los assets embebidos cargan.

$ErrorActionPreference = 'Stop'

$tabler = @(
  'terminal-2', 'layout-sidebar', 'layout-columns', 'layout-rows', 'settings',
  'plus', 'x', 'minus', 'square', 'chevron-down', 'chevron-right', 'chevron-left', 'chevron-up',
  'folder', 'folder-open', 'pencil', 'copy', 'maximize', 'refresh', 'search',
  'sun', 'moon', 'palette', 'menu-2', 'keyboard', 'device-desktop',
  'external-link', 'file-import', 'file-export', 'check', 'circle',
  'alert-triangle', 'info-circle', 'player-play', 'player-pause',
  'player-stop', 'loader-2', 'bell', 'bell-off', 'brand-powershell'
)

$simple = @('rust', 'opencode', 'githubcopilot')

$root = Split-Path -Parent $PSScriptRoot
$tablerDir = Join-Path $root 'crates/ui/assets/icons/tabler'
$simpleDir = Join-Path $root 'crates/ui/assets/icons/simple'
New-Item -ItemType Directory -Force -Path $tablerDir, $simpleDir | Out-Null

foreach ($name in $tabler) {
  $url = "https://raw.githubusercontent.com/tabler/tabler-icons/main/icons/outline/$name.svg"
  Invoke-WebRequest -UseBasicParsing $url -TimeoutSec 30 |
    Select-Object -ExpandProperty Content |
    Set-Content -Path (Join-Path $tablerDir "$name.svg") -NoNewline -Encoding UTF8
  Write-Host "tabler/$name.svg"
}

foreach ($name in $simple) {
  $url = "https://raw.githubusercontent.com/simple-icons/simple-icons/develop/icons/$name.svg"
  Invoke-WebRequest -UseBasicParsing $url -TimeoutSec 30 |
    Select-Object -ExpandProperty Content |
    Set-Content -Path (Join-Path $simpleDir "$name.svg") -NoNewline -Encoding UTF8
  Write-Host "simple/$name.svg"
}

Write-Host 'Iconos actualizados. Ejecuta `cargo test -p axf_ui`.'
