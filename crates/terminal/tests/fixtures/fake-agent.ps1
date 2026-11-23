# CLI falso para probar la detección de agentes en Windows.
# No depende de opencode ni de Copilot: solo emite OSC 7501, OSC 9 y BEL.
$ErrorActionPreference = 'Stop'
$esc = [char]27
$st = "$esc\"

function Emit-Osc([string]$body) {
    [Console]::Out.Write("$esc]$body$st")
    [Console]::Out.Flush()
}

Emit-Osc '7501;state=working:app=opencode:progress=10:msg=QnVpbGRpbmc='
Start-Sleep -Milliseconds 250

Emit-Osc '7501;state=blocked:kind=question:app=opencode:progress=10:msg=T3ZlcndyaXRlIGNvbmZpZy50b21sPw=='
[Console]::Out.Write("$([char]7)")
Start-Sleep -Milliseconds 250

Emit-Osc '7501;state=done:app=opencode:msg=RG9jcyBidWlsdA=='
Emit-Osc '9;Fake agent finished'
Start-Sleep -Milliseconds 250
