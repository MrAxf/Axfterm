<p align="center">
  <img src="crates/ui/assets/brand/axfterm-logo.svg" alt="Axfterm" width="520" />
</p>

<p align="center">
  Terminal workspace nativo, rápido y keyboard-first para Windows.
</p>

Axfterm reúne las terminales de cada proyecto en una barra lateral, con paneles
y pestañas en el área central, y muestra los agentes de IA (OpenCode y GitHub
Copilot) con su estado en la misma vista.

- **Windows primero**: ConPTY, grupos de procesos y apertura desde Explorer.
- **Proyectos, no ventanas**: una sidebar por carpeta, paneles divisibles y
  pestañas que se mueven sin recrear el PTY.
- **Keyboard-first**: acciones, paleta de comandos y atajos configurables; una
  tecla sin binding va a la terminal.
- **Tema compartido**: UI, terminal y paleta ANSI usan los mismos tokens.
- **Historial persistente**: cada terminal restaura su scrollback al activar la
  pestaña.
- **Agentes locales**: OSC 7501 y árbol de procesos; sin telemetría ni red.
- **Ajustes sobre el archivo**: panel con borrador, `config.toml` preservando
  comentarios y hot reload.

## Instalar

Requisitos: Windows 10 1809 o superior / Windows 11, x64.

- **MSI**: `Axfterm-0.1.1-win-x64.msi` instala por usuario en
  `%LOCALAPPDATA%\Programs\Axfterm`, con acceso directo en el menú Inicio,
  `App Paths` y «Abrir con Axfterm» en carpetas.
- **Zip portable**: descomprime `Axfterm-0.1.1-win-x64.zip` y ejecuta
  `axfterm.exe`. No requiere instalación.

También puedes lanzar una carpeta desde la terminal:

```powershell
axfterm C:\Proyectos\mi-app
```

Una segunda instancia reenvía la carpeta a la ventana abierta y sale.

## Desarrollo

Requisitos: Rust stable (MSRV 1.85), edition 2024 y Windows SDK para embeber el
icono y los metadatos del ejecutable.

```powershell
pwsh -File scripts/check.ps1          # fmt + clippy + tests
pwsh -File scripts/cargo-low.ps1 build --release
pwsh -File scripts/package.ps1        # zip portable + MSI
```

`scripts/check.ps1` también comprueba que ningún archivo Rust de producción
supere 500 líneas. Los benchmarks del motor están en
`crates/terminal/examples/engine_bench.rs` y `grid_bench.rs`.

## Configuración

La configuración vive en:

- Windows: `%APPDATA%\axfterm\config.toml`
- macOS: `~/Library/Application Support/axfterm/config.toml`
- Linux: `~/.config/axfterm/config.toml` o `$XDG_CONFIG_HOME/axfterm/`

El archivo es la fuente de verdad y se recarga en caliente. La sesión
(`session.json`), el scrollback (`scrollback/`) y los logs opcionales
(`logs/`) viven junto a él.

## Documentación

- [Configuración (`config.toml`)](docs/config.md)
- [Panel de ajustes](docs/settings.md)
- [Perfiles de terminal](docs/profiles.md)
- [Sesión e historial](docs/session.md)
- [Proyectos en WSL](docs/wsl.md)
- [Temas](docs/themes.md)
- [Atajos](docs/keymap.md)
- [Agentes](docs/agents.md)
- [Diagnóstico y logs](docs/diagnostics.md)
- [Solución de problemas](docs/troubleshooting.md)
- [Release y empaquetado](docs/release.md)

## Privacidad

Axfterm no usa telemetría ni red por defecto. Los logs son opt-in
(`AXFTERM_LOG`) y los crash reports se quedan en el perfil del usuario. El
historial de terminal contiene lo que se mostró en pantalla y no se cifra ni se
sincroniza.

## Licencia

MIT OR Apache-2.0. Ver [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)
y [NOTICE](NOTICE) para las dependencias y assets de terceros.
