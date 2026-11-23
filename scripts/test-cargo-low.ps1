#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("axfterm-cargo-test-" + [guid]::NewGuid())
$bin = Join-Path $temp 'bin'
$workspace = Join-Path $temp 'workspace'
$scripts = Join-Path $workspace 'scripts'
New-Item -ItemType Directory -Path $bin, $scripts -Force | Out-Null
$wrapper = Join-Path $scripts 'cargo-low.ps1'
Copy-Item (Join-Path $PSScriptRoot 'cargo-low.ps1') $wrapper
$log = Join-Path $temp 'calls.jsonl'
$previousPath = $env:PATH
$first = $null
$second = $null
$variables = @('AXF_CARGO_JOBS', 'CARGO_BUILD_JOBS', 'AXF_CARGO_ECHO', 'AXF_CARGO_PRIORITY',
    'AXF_CARGO_AFFINITY', 'AXF_CARGO_ACTIVE_LOCKS', 'MOCK_LOG', 'MOCK_ACTION', 'MOCK_WRAPPER')
$previous = @{}
foreach ($name in $variables) {
    $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    [Environment]::SetEnvironmentVariable($name, $null, 'Process')
}

@'
@echo off
set "MOCK_ARGS=%*"
pwsh -NoProfile -File "%~dp0fake-cargo.ps1"
exit /b %errorlevel%
'@ | Set-Content -Encoding Ascii (Join-Path $bin 'cargo.cmd')
@'
$record = @{ event = 'start'; arguments = @($env:MOCK_ARGS -split ' '); pid = $PID;
    priority = (Get-Process -Id $PID).PriorityClass.ToString() } | ConvertTo-Json -Compress
Add-Content -LiteralPath $env:MOCK_LOG -Value $record
if ($env:MOCK_ACTION -eq 'wait') { Start-Sleep -Seconds 2 }
if ($env:MOCK_ACTION -eq 'nested') {
    & pwsh -NoProfile -File $env:MOCK_WRAPPER build 2>&1 | Out-Null
    exit $LASTEXITCODE
}
if ($env:MOCK_ACTION -eq 'fail') { exit 17 }
Add-Content -LiteralPath $env:MOCK_LOG -Value (@{ event = 'end'; pid = $PID } | ConvertTo-Json -Compress)
exit 0
'@ | Set-Content -Encoding UTF8 (Join-Path $bin 'fake-cargo.ps1')

function Assert-True([bool]$condition, [string]$message) {
    if (-not $condition) { throw $message }
}
function Read-Calls {
    if (Test-Path $log) {
        return @(Get-Content $log | ForEach-Object { $_ | ConvertFrom-Json })
    }
    return @()
}
function Assert-Arguments([string[]]$expected) {
    $calls = @(Read-Calls)
    $actual = @($calls[-2].arguments)
    Assert-True (($actual -join '|') -ceq ($expected -join '|')) "Argumentos: $actual; esperados: $expected"
    Assert-True ($calls[-2].priority -eq 'BelowNormal') "Prioridad incorrecta: $($calls[-2].priority)"
}

try {
    $env:PATH = "$bin;$previousPath"
    $env:MOCK_LOG = $log
    $env:MOCK_WRAPPER = $wrapper
    $originalPriority = (Get-Process -Id $PID).PriorityClass
    $originalAffinity = (Get-Process -Id $PID).ProcessorAffinity

    $arguments = @('test', '--workspace', '--', '--jobs', '9')
    & $wrapper @arguments
    Assert-True ($LASTEXITCODE -eq 0) 'Falló test simulado.'
    Assert-Arguments @('test', '-j', '1', '--workspace', '--', '--jobs', '9')
    Assert-True ((Get-Process -Id $PID).PriorityClass -eq $originalPriority) 'No se restauró la prioridad.'
    Assert-True ((Get-Process -Id $PID).ProcessorAffinity -eq $originalAffinity) 'No se restauró la afinidad.'
    $mask = [int64]$originalAffinity
    $env:AXF_CARGO_AFFINITY = [string]($mask -band (-$mask))
    & $wrapper check
    Assert-True ((Get-Process -Id $PID).ProcessorAffinity -eq $originalAffinity) 'No se restauró la afinidad explícita.'
    $env:AXF_CARGO_AFFINITY = $null
    $arguments = @('fmt', '--all', '--', '--check')
    & $wrapper @arguments
    Assert-Arguments @('fmt', '--all', '--', '--check')
    & $wrapper --config 'build.jobs=3' build --jobs=2
    Assert-Arguments @('--config', 'build.jobs=3', 'build', '--jobs=2')
    $env:CARGO_BUILD_JOBS = '3'
    & $wrapper check
    Assert-Arguments @('check', '-j', '3')
    $env:AXF_CARGO_JOBS = '2'
    & $wrapper clippy
    Assert-Arguments @('clippy', '-j', '2')
    $env:AXF_CARGO_JOBS = $null
    $env:CARGO_BUILD_JOBS = $null

    $env:AXF_CARGO_ECHO = '1'
    $count = @(Read-Calls).Count
    & $wrapper build
    Assert-True (@(Read-Calls).Count -eq $count) 'El modo de inspección ejecutó Cargo.'
    Assert-True ((Get-Process -Id $PID).PriorityClass -eq $originalPriority) 'El modo de inspección cambió la prioridad.'
    $env:AXF_CARGO_ECHO = $null

    $env:MOCK_ACTION = 'fail'
    & $wrapper build
    Assert-True ($LASTEXITCODE -eq 17) 'No se propagó el código de salida.'
    Assert-True ((Get-Process -Id $PID).PriorityClass -eq $originalPriority) 'No se restauró la prioridad tras un fallo.'
    $env:MOCK_ACTION = 'nested'
    & $wrapper build
    Assert-True ($LASTEXITCODE -ne 0) 'El wrapper anidado debería fallar sin esperar.'

    Remove-Item -LiteralPath $log
    $env:MOCK_ACTION = 'wait'
    $pwsh = (Get-Process -Id $PID).Path
    $arguments = @('-NoProfile', '-File', "`"$wrapper`"", 'build')
    $first = Start-Process -FilePath $pwsh -ArgumentList $arguments -PassThru -RedirectStandardOutput (Join-Path $temp 'first.log')
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 100
        $calls = @(Read-Calls)
    } while ($calls.Count -lt 1 -and [DateTime]::UtcNow -lt $deadline)
    Assert-True ($calls.Count -ge 1) 'No arrancó la primera ejecución.'
    $second = Start-Process -FilePath $pwsh -ArgumentList $arguments -PassThru -RedirectStandardOutput (Join-Path $temp 'second.log')
    Assert-True ($first.WaitForExit(15000)) 'La primera ejecución no terminó.'
    Assert-True ($second.WaitForExit(15000)) 'La segunda ejecución no terminó.'
    Assert-True ($first.ExitCode -eq 0 -and $second.ExitCode -eq 0) 'Fallaron ejecuciones concurrentes.'
    $calls = @(Read-Calls)
    Assert-True (($calls.event -join ',') -eq 'start,end,start,end') 'Las ejecuciones no se serializaron.'
    Assert-True ((Get-Content (Join-Path $temp 'second.log') -Raw) -match 'Esperando') 'No se avisó de la espera.'

    $otherScripts = Join-Path $temp 'other\scripts'
    New-Item -ItemType Directory -Path $otherScripts -Force | Out-Null
    $otherWrapper = Join-Path $otherScripts 'cargo-low.ps1'
    Copy-Item $wrapper $otherWrapper
    Remove-Item -LiteralPath $log
    $first = Start-Process -FilePath $pwsh -ArgumentList $arguments -PassThru -RedirectStandardOutput (Join-Path $temp 'other-first.log')
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 100
        $calls = @(Read-Calls)
    } while ($calls.Count -lt 1 -and [DateTime]::UtcNow -lt $deadline)
    Assert-True ($calls.Count -ge 1) 'No arrancó la ejecución para comparar workspaces.'
    $otherArguments = @('-NoProfile', '-File', "`"$otherWrapper`"", 'build')
    $second = Start-Process -FilePath $pwsh -ArgumentList $otherArguments -PassThru -RedirectStandardOutput (Join-Path $temp 'other-second.log')
    Assert-True ($first.WaitForExit(15000) -and $second.WaitForExit(15000)) 'No terminaron los workspaces independientes.'
    $calls = @(Read-Calls)
    Assert-True (($calls.event -join ',') -eq 'start,start,end,end') 'Workspaces distintos deberían poder ejecutarse en paralelo.'

    Remove-Item -LiteralPath $log
    $first = Start-Process -FilePath $pwsh -ArgumentList $arguments -PassThru -RedirectStandardOutput (Join-Path $temp 'abandoned.log')
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 100
        $calls = @(Read-Calls)
    } while ($calls.Count -lt 1 -and [DateTime]::UtcNow -lt $deadline)
    Assert-True ($calls.Count -ge 1) 'No arrancó la ejecución que se va a interrumpir.'
    Stop-Process -Id $first.Id -Force
    $first.WaitForExit()
    # El Cargo simulado puede sobrevivir a su padre; dejar que termine.
    Start-Sleep -Seconds 3
    $env:MOCK_ACTION = $null
    & $wrapper build
    Assert-True ($LASTEXITCODE -eq 0) 'No se recuperó el bloqueo abandonado.'
    Write-Host 'OK: pruebas de cargo-low.'
}
finally {
    foreach ($child in @($first, $second)) {
        if ($child -and -not $child.HasExited) { Stop-Process -Id $child.Id -Force }
    }
    $env:PATH = $previousPath
    foreach ($name in $variables) {
        [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process')
    }
    Remove-Item -LiteralPath $temp -Recurse -Force
}
