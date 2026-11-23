#Requires -Version 5.1
<#
.SYNOPSIS
    Ejecuta cargo con prioridad baja y paralelismo acotado.

.DESCRIPTION
    Este portátil tiene 8 núcleos / 16 hilos y 15 GB de RAM: un `cargo test
    --workspace` o `cargo clippy --workspace --all-targets` sin límites levanta
    16 `rustc` y deja el escritorio congelado. El wrapper:

      1. Baja este proceso a `BelowNormal` antes de lanzar cargo. En Windows la
         clase de prioridad se hereda al crear procesos, así que cargo, rustc y
         el linker también corren ahí y el escritorio (Normal) siempre gana.
      2. Inyecta `-j N` en los subcomandos de compilación. El valor por defecto
         es 4; `AXF_CARGO_JOBS` lo cambia sin editar nada.

    Lo usan `scripts/check.ps1` y, por convención (AGENTS.md), cualquier cargo
    que ejecute un agente. `cargo fmt` no acepta `-j` y se ejecuta tal cual.

    Variables de entorno: `AXF_CARGO_JOBS` (4 por defecto), `AXF_CARGO_PRIORITY`
    (`BelowNormal`), `AXF_CARGO_AFFINITY` (máscara opcional) y
    `AXF_CARGO_ECHO=1` (imprime el comando final sin ejecutarlo).

.PARAMETER args
    Argumentos para cargo, sin transformar.

.EXAMPLE
    pwsh -File scripts/cargo-low.ps1 clippy --workspace --all-targets -- -D warnings

.EXAMPLE
    $env:AXF_CARGO_JOBS = 8; pwsh -File scripts/cargo-low.ps1 test --workspace
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$buildCommands = @(
    'build', 'check', 'clippy', 'test', 'run', 'bench', 'doc', 'rustc', 'fix'
)

$cargoArgs = @($args)
if ($cargoArgs.Count -eq 0) {
    throw 'Uso: pwsh -File scripts/cargo-low.ps1 <subcomando de cargo> [argumentos...]'
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'No se encontró `cargo` en PATH.'
}

# Jobs: AXF_CARGO_JOBS > 4. Un `-j`/`--jobs` explícito en la línea de comandos
# manda sobre todo lo demás, así que no se inyecta nada en ese caso.
$jobs = 4
if ($env:AXF_CARGO_JOBS) {
    $parsed = 0
    if ([int]::TryParse($env:AXF_CARGO_JOBS, [ref]$parsed) -and $parsed -gt 0) {
        $jobs = $parsed
    }
    else {
        Write-Warning "AXF_CARGO_JOBS='$($env:AXF_CARGO_JOBS)' no es un entero positivo; se usa $jobs."
    }
}

$explicitJobs = $false
foreach ($arg in $cargoArgs) {
    if ($arg -eq '-j' -or $arg -eq '--jobs' -or $arg -like '--jobs=*' -or $arg -match '^-j\d+$') {
        $explicitJobs = $true
        break
    }
}

# Subcomando = primer argumento que no es un flag global (`--locked`) ni un
# override de toolchain (`+nightly`).
$subcommand = -1
for ($i = 0; $i -lt $cargoArgs.Count; $i++) {
    $arg = $cargoArgs[$i]
    if (-not $arg.StartsWith('-') -and -not $arg.StartsWith('+')) {
        $subcommand = $i
        break
    }
}

if (-not $explicitJobs -and $subcommand -ge 0 -and $buildCommands -contains $cargoArgs[$subcommand]) {
    $before = @($cargoArgs[0..$subcommand])
    $after = @()
    if ($subcommand -lt ($cargoArgs.Count - 1)) {
        $after = @($cargoArgs[($subcommand + 1)..($cargoArgs.Count - 1)])
    }
    $cargoArgs = $before + @('-j', "$jobs") + $after
}

# Prioridad y, opcionalmente, afinidad. Ambos se heredan a los hijos.
$process = [System.Diagnostics.Process]::GetCurrentProcess()

$priority = 'BelowNormal'
if ($env:AXF_CARGO_PRIORITY) {
    $priority = $env:AXF_CARGO_PRIORITY
}
try {
    $process.PriorityClass = [System.Diagnostics.ProcessPriorityClass]$priority
}
catch {
    Write-Warning "Prioridad '$priority' no válida; se usa BelowNormal."
    $process.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::BelowNormal
}

if ($env:AXF_CARGO_AFFINITY) {
    try {
        $mask = [int64]$env:AXF_CARGO_AFFINITY
        if ($mask -gt 0) {
            $process.ProcessorAffinity = [IntPtr]$mask
        }
    }
    catch {
        Write-Warning "AXF_CARGO_AFFINITY='$($env:AXF_CARGO_AFFINITY)' no es una máscara válida; se usa la afinidad actual."
    }
}

if ($env:AXF_CARGO_ECHO) {
    Write-Host "==> cargo $($cargoArgs -join ' ') (prioridad=$priority)" -ForegroundColor DarkGray
    exit 0
}

try {
    & cargo @cargoArgs
    exit $LASTEXITCODE
}
finally {
    # La comprobación no debe heredar la prioridad del build al terminar.
    $process.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::Normal
}
