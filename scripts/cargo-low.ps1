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
         el linker también corren ahí y la CPU favorece al escritorio (Normal).
      2. Inyecta `-j N` en los subcomandos de compilación. El valor por defecto
         es 1; `AXF_CARGO_JOBS` o `CARGO_BUILD_JOBS` lo cambian.
      3. Serializa las ejecuciones del wrapper para este workspace.

    Lo usan `scripts/check.ps1` y, por convención (AGENTS.md), cualquier cargo
    que ejecute un agente. `cargo fmt` no acepta `-j` y se ejecuta tal cual.
    Ejecutar desde el workspace para que Cargo encuentre su configuración.
    El bloqueo corresponde al workspace que contiene este script, dentro de la
    sesión de Windows; un Cargo directo o un proceso huérfano no queda bloqueado.

    Variables de entorno: `AXF_CARGO_JOBS` (1 por defecto), `AXF_CARGO_PRIORITY`
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

# Jobs explícitos de Cargo > AXF_CARGO_JOBS > CARGO_BUILD_JOBS > 1.
$jobs = 1
$jobsSetting = if ($env:AXF_CARGO_JOBS) { $env:AXF_CARGO_JOBS } else { $env:CARGO_BUILD_JOBS }
if ($jobsSetting) {
    $parsed = 0
    if ([int]::TryParse($jobsSetting, [ref]$parsed) -and $parsed -gt 0) {
        $jobs = $parsed
    }
    else {
        Write-Warning "El límite de jobs '$jobsSetting' no es un entero positivo; se usa $jobs."
    }
}

$explicitJobs = $false
foreach ($arg in $cargoArgs) {
    if ($arg -eq '--') {
        break
    }
    if ($arg -eq '-j' -or $arg -eq '--jobs' -or $arg -like '--jobs=*' -or $arg -match '^-j\d+$') {
        $explicitJobs = $true
        break
    }
}

# Algunas opciones globales consumen el argumento siguiente.
$subcommand = -1
for ($i = 0; $i -lt $cargoArgs.Count; $i++) {
    $arg = $cargoArgs[$i]
    if ($arg -eq '--') {
        break
    }
    if ($arg -in @('--config', '--color', '-Z', '-C')) {
        $i++
        continue
    }
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

$priority = 'BelowNormal'
if ($env:AXF_CARGO_PRIORITY) {
    $priority = $env:AXF_CARGO_PRIORITY
}
if ($env:AXF_CARGO_ECHO) {
    Write-Host "==> cargo $($cargoArgs -join ' ') (prioridad=$priority)" -ForegroundColor DarkGray
    exit 0
}

$root = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd('\').ToLowerInvariant()
$sha = [System.Security.Cryptography.SHA256]::Create()
try {
    $hash = [BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($root))).Replace('-', '')
}
finally {
    $sha.Dispose()
}
$lockName = "Local\AxftermCargo-$hash"
$previousLocks = $env:AXF_CARGO_ACTIVE_LOCKS
if ($previousLocks -and ($previousLocks -split ';') -contains $hash) {
    throw 'Una ejecución anidada del wrapper para este workspace provocaría un bloqueo.'
}

$mutex = [System.Threading.Mutex]::new($false, $lockName)
$locked = $false
$process = [System.Diagnostics.Process]::GetCurrentProcess()
$originalPriority = $process.PriorityClass
$originalAffinity = $process.ProcessorAffinity
try {
    try {
        $locked = $mutex.WaitOne(0)
        if (-not $locked) {
            Write-Host 'Esperando a que termine otra ejecución de cargo-low para este workspace...'
            $locked = $mutex.WaitOne()
        }
    }
    catch [System.Threading.AbandonedMutexException] {
        $locked = $true
        Write-Warning 'La ejecución anterior terminó inesperadamente; se recuperó el bloqueo.'
    }

    $env:AXF_CARGO_ACTIVE_LOCKS = (@($previousLocks, $hash) | Where-Object { $_ }) -join ';'
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
            if ($mask -le 0) { throw 'La máscara debe ser positiva.' }
            $process.ProcessorAffinity = [IntPtr]$mask
        }
        catch {
            Write-Warning "AXF_CARGO_AFFINITY='$($env:AXF_CARGO_AFFINITY)' no es una máscara válida; se usa la afinidad actual."
        }
    }

    & cargo @cargoArgs
    $cargoExitCode = $LASTEXITCODE
}
finally {
    $env:AXF_CARGO_ACTIVE_LOCKS = $previousLocks
    try {
        try {
            $process.ProcessorAffinity = $originalAffinity
        }
        finally {
            $process.PriorityClass = $originalPriority
        }
    }
    finally {
        if ($locked) { $mutex.ReleaseMutex() }
        $mutex.Dispose()
        $process.Dispose()
    }
}
exit $cargoExitCode
