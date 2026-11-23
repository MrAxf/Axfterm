#!/bin/sh
# CLI falso para probar la detección de agentes en Unix/WSL.
# Emite OSC 7501, OSC 9 y BEL sin depender de opencode ni Copilot.
emit() {
    printf '\033]%s\033\\' "$1"
}

emit '7501;state=working:app=opencode:progress=10:msg=QnVpbGRpbmc='
sleep 0.25

emit '7501;state=blocked:kind=question:app=opencode:progress=10:msg=T3ZlcndyaXRlIGNvbmZpZy50b21sPw=='
printf '\007'
sleep 0.25

emit '7501;state=done:app=opencode:msg=RG9jcyBidWlsdA=='
emit '9;Fake agent finished'
sleep 0.25
