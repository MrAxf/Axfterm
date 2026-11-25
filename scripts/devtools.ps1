#Requires -Version 5.1
<#
.SYNOPSIS
    Cliente de Axfterm Studio.

.DESCRIPTION
    Envía un comando JSONL al servidor local de Studio y muestra la respuesta.
    Requiere que la app se haya lanzado con `--studio` (o con studio.ps1).

.EXAMPLE
    pwsh scripts/devtools.ps1 state
    pwsh scripts/devtools.ps1 open app.settings.terminal
    pwsh scripts/devtools.ps1 shot settings-terminal
    pwsh scripts/devtools.ps1 shot gallery-actions -Clip 0,0,1280,800
#>
[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Command = 'state'
$Rest = @()
$Clip = ''
$Dir = ''
$positional = @()
for ($index = 0; $index -lt $Arguments.Count; $index++) {
    switch -Regex ($Arguments[$index]) {
        '^(?i)-dir$' {
            if ($index + 1 -lt $Arguments.Count) {
                $index++
                $Dir = $Arguments[$index]
            }
            continue
        }
        '^(?i)-clip$' {
            if ($index + 1 -lt $Arguments.Count) {
                $index++
                $Clip = $Arguments[$index]
            }
            continue
        }
        default { $positional += $Arguments[$index] }
    }
}
if ($positional.Count -gt 0) {
    $Command = $positional[0]
}
if ($positional.Count -gt 1) {
    $Rest = @($positional[1..($positional.Count - 1)])
}

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $Dir) {
    $Dir = Join-Path $repoRoot 'target/studio'
}
$sessionPath = Join-Path $Dir 'session.json'
if (-not (Test-Path $sessionPath)) {
    throw "No hay sesión de Studio en $sessionPath. Ejecuta scripts/studio.ps1 -Serve."
}
$session = Get-Content $sessionPath -Raw | ConvertFrom-Json

switch ($Command.ToLowerInvariant()) {
    'state' { $cmd = @{ type = 'state' } }
    'surfaces' { $cmd = @{ type = 'surfaces' } }
    'open' {
        if ($Rest.Count -lt 1) { throw 'open requiere una superficie.' }
        $cmd = @{ type = 'open'; surface = $Rest[0] }
    }
    'action' {
        if ($Rest.Count -lt 1) { throw 'action requiere un id.' }
        $cmd = @{ type = 'action'; id = $Rest[0] }
    }
    'theme' {
        if ($Rest.Count -lt 1) { throw 'theme requiere un nombre.' }
        $cmd = @{ type = 'theme'; name = ($Rest -join ' ') }
    }
    'variant' {
        if ($Rest.Count -lt 1) { throw 'variant requiere dark o light.' }
        $cmd = @{ type = 'variant'; variant = $Rest[0] }
    }
    'scale' {
        if ($Rest.Count -lt 1) { throw 'scale requiere un número.' }
        $cmd = @{ type = 'scale'; value = [double]$Rest[0] }
    }
    'size' {
        if ($Rest.Count -lt 1 -or $Rest[0] -notmatch '^(\d+)x(\d+)$') {
            throw 'size requiere ancho x alto, por ejemplo 1280x800.'
        }
        $cmd = @{ type = 'size'; width = [double]$Matches[1]; height = [double]$Matches[2] }
    }
    'focus' {
        if ($Rest.Count -lt 1) { throw 'focus requiere un destino.' }
        $cmd = @{ type = 'focus'; target = $Rest[0] }
    }
    'key' {
        if ($Rest.Count -lt 1) { throw 'key requiere una tecla permitida.' }
        $cmd = @{ type = 'key'; key = $Rest[0].ToLowerInvariant() }
    }
    'key' {
        if ($Rest.Count -lt 1) { throw 'key requiere una tecla permitida.' }
        $cmd = @{ type = 'key'; key = $Rest[0].ToLowerInvariant() }
    }
    'workspace' {
        if ($Rest.Count -lt 1) { throw 'workspace requiere empty o demo.' }
        $cmd = @{ type = 'workspace'; fixture = $Rest[0] }
    }
    'modal' {
        if ($Rest.Count -lt 1) { throw 'modal requiere open o close.' }
        $cmd = @{ type = 'modal'; open = ($Rest[0] -eq 'open') }
    }
    'wait' {
        if ($Rest.Count -lt 1) { throw 'wait requiere milisegundos.' }
        $cmd = @{ type = 'wait'; ms = [int]$Rest[0] }
    }
    'shot' {
        if ($Rest.Count -lt 1) { throw 'shot requiere un nombre.' }
        $cmd = @{ type = 'shot'; name = $Rest[0] }
        if ($Clip) {
            $parts = $Clip -split ','
            if ($parts.Count -ne 4) {
                throw 'Clip debe ser x,y,ancho,alto en píxeles lógicos.'
            }
            $cmd.clip = @{
                x = [double]$parts[0]
                y = [double]$parts[1]
                width = [double]$parts[2]
                height = [double]$parts[3]
            }
        }
    }
    'quit' { $cmd = @{ type = 'quit' } }
    default { throw "Comando desconocido: $Command" }
}

$request = @{
    id = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    token = $session.token
    cmd = $cmd
}

$client = New-Object System.Net.Sockets.TcpClient
try {
    $client.Connect('127.0.0.1', [int]$session.port)
    $stream = $client.GetStream()
    $writer = New-Object System.IO.StreamWriter($stream, [System.Text.UTF8Encoding]::new($false))
    $writer.AutoFlush = $true
    $reader = New-Object System.IO.StreamReader($stream, [System.Text.UTF8Encoding]::new($false))
    $writer.WriteLine(($request | ConvertTo-Json -Compress -Depth 10))
    $line = $reader.ReadLine()
    if ($null -eq $line) {
        throw 'El servidor cerró la conexión sin respuesta.'
    }
    $response = $line | ConvertFrom-Json
    $response | ConvertTo-Json -Depth 10
    if (-not $response.ok) {
        exit 1
    }
}
finally {
    $client.Dispose()
}
