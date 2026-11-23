# ADR 0002 — GPUI encapsulado en `axf_ui`

- **Estado**: Aceptado
- **Fecha**: 2026-11-21
- **Decidores**: Juan Andrés
- **Spikes**: Shell GPUI de M0, S4 (`grid_bench`), S5 (`keylog`), S7 (`icons`)

## Contexto

GPUI es pre-1.0 y su API cambia entre versiones. `gpui-kit` 0.10 aporta shell,
dock, componentes base, tema y test-support, pero fija `gpui-pre 0.3.12`; una
actualización de cualquiera de los dos puede romper el proyecto. Axfterm
necesita además:

- Un tema propio (tokens UI + paleta ANSI) que no dependa solo del tema de
  `gpui-kit`.
- Que `axf_terminal` use tipos de GPUI sin declarar `gpui-kit` directamente.
- Que `axf_workspace`, `axf_agents` y `axf_config` no conozcan GPUI.
- Un camino de escape realista si `gpui-kit` deja de mantenerse o rompe.

El código de M0 declaraba `gpui-kit` directamente en `app`, `ui` y en los
ejemplos, y `main.rs`/`shell.rs` lo importaban sin pasar por `axf_ui`. Eso
contradecía la decisión de encapsulamiento ya escrita en las tareas de M0.

## Decisión

### Frontera

- **Solo `crates/ui` declara `gpui-kit`.** El resto de crates, incluido el
  binario `app`, importa GPUI como `axf_ui::gpui`.
- `axf_ui` reexporta la superficie necesaria (`pub use gpui_kit as gpui`) y
  posee tokens, tema, métricas, iconos, assets y componentes propios.
- `axf_terminal` puede usar `axf_ui::gpui` para `TerminalElement`, pero nunca
  `gpui-kit` directo.
- `axf_workspace`, `axf_agents` y `axf_config` no dependen de GPUI en ninguna
  forma: su API es de dominio.
- Los ejemplos y tests usan la misma frontera que el código de producción. Un
  ejemplo que necesite GPUI lo obtiene de `axf_ui::gpui`.

### Versiones y actualizaciones

- `gpui-kit = "=0.10.0"` con pin exacto; su `Cargo.lock` fija `gpui-pre`.
- Una actualización es un cambio atómico: subir pin, compilar, ejecutar
  `scripts/check.ps1`, abrir la ventana smoke y probar teclado/IME y resize.
- Si una versión nueva rompe sin fix razonable, no se hace downgrade silencioso:
  se documenta el bloqueo y se evalúa el escape.
- `Cargo.lock` se versiona; el `NOTICE` se actualiza si cambian licencias.

### Plan de escape a `gpui-base`

El escape no es reescribir la app: es sustituir la capa de presentación.
Disparadores:

1. `gpui-kit` deja de recibir fixes o no compila con una versión de Rust que
   Axfterm necesita.
2. Un bug bloqueante de shell, dock o input sin workaround en `axf_ui`.
3. El pin de `gpui-pre` impide una feature necesaria (IME, DPI, accesibilidad).

Pasos:

1. Congelar features y mantener `axf_ui` como única superficie pública de UI.
2. Introducir `gpui-base` (o un backend propio) detrás de los mismos
   componentes/tokens de `axf_ui`.
3. Migrar primero la ventana y el shell, después `TerminalElement`, después
   dock/paneles.
4. Eliminar `gpui-kit` cuando no quede ningún import ni tipo en la API pública.
5. Registrar la sustitución en un ADR nuevo.

El coste aceptado es que `axf_ui` es un cuello de botella: toda primitiva nueva
de GPUI que necesite otro crate se reexporta aquí, no se importa directo.

## Consecuencias

### Positivas

- Un solo punto de cambio para actualizar GPUI o cambiar de base.
- Las reglas de producto (tema único, métricas, iconos) viven en un crate y no
  se pueden saltar desde el dominio.
- Los tests de UI pueden usar el test-support sin filtrar GPUI a otros crates.
- `app` deja de duplicar imports y el grafo de dependencias coincide con
  `AGENTS.md`.

### Negativas y riesgos

- `axf_ui` crece como fachada y puede quedarse corta; Mitigación: reexportar
  solo lo usado y añadir primitivas con tests.
- Los macros de GPUI tienen resolución de rutas propia. En concreto, el derive
  de `actions!` expande `gpui::Action`, así que el crate que declara acciones
  debe tener `gpui` en scope (en `app`: `use axf_ui::gpui;`). Queda documentado
  en el código y en este ADR; no se resuelve añadiendo `gpui-kit` directo.
- El escape a `gpui-base` sigue siendo trabajo real, no un cambio de una línea.
  Mitigación: mantener los componentes propios pequeños y sin lógica de dominio.

## Alternativas

- **`gpui-kit` directo en todos los crates**: menos indirección, pero rompe la
  encapsulación y hace imposible el escape sin tocar todo el workspace.
- **GPUI directo sin `gpui-kit`**: obliga a escribir shell, dock y componentes
  desde M0; demasiado coste antes de validar el producto.
- **egui/iced/tao+winit**: ecosistemas más estables, pero pierden el modelo de
  GPU, el test-support y la integración con el resto del stack elegido.

## Referencias

- `crates/ui/src/lib.rs` — reexportación de GPUI.
- `crates/app/src/main.rs`, `crates/app/src/shell.rs` — consumo vía `axf_ui`.
- `AGENTS.md` — dependencias permitidas y riesgos conocidos.
- ADR 0001 — tipos de GPUI que necesita `axf_terminal`.
