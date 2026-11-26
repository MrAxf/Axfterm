//! Axfterm Studio: control local y capturas para desarrollo.
//!
//! Solo se compila con la feature `studio`. No forma parte del binario de
//! release y nunca escribe en la configuración real del usuario: el lanzador
//! apunta `AXFTERM_CONFIG_DIR` a `target/studio/home`.

mod capture;
mod protocol;
mod server;

use std::{path::PathBuf, sync::mpsc::SyncSender};

use axf_ui::gallery::{Gallery, GallerySection};
use axf_ui::gpui::{
    App, AppContext as _, Context, Entity, InputEvent, IntoElement, KeyDownEvent, KeyUpEvent,
    Keystroke, ParentElement, Styled, Window, div, px, size,
};
use axf_ui::theme;

use crate::Shell;

use self::protocol::{Command, Reply, Request, Snapshot};
use self::server::{Incoming, ServerHandle};

/// Opciones de arranque de Studio.
#[derive(Clone, Debug, Default)]
pub(crate) struct Launch {
    /// Abre directamente la galería.
    pub gallery_only: bool,
    /// Mantiene la ventana oculta.
    pub hidden: bool,
    /// Carga el workspace de demostración.
    pub fixture: Option<String>,
    /// Directorio de estado y capturas.
    pub dir: PathBuf,
    /// Puerto fijo; 0 elige uno libre.
    pub port: u16,
}

/// Superficie activa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    App,
    Gallery(GallerySection),
}

/// Estado de Studio dentro del shell.
pub(crate) struct StudioState {
    pub(crate) surface: Surface,
    pub(crate) app_surface: String,
    pub(crate) gallery: Entity<Gallery>,
    pub(crate) shots_dir: PathBuf,
    pub(crate) fixture: Option<String>,
    pub(crate) agents_frozen: bool,
    pub(crate) _server: Option<ServerHandle>,
}

impl StudioState {
    /// `true` si la galería ocupa la ventana.
    pub(crate) fn is_gallery(&self) -> bool {
        matches!(self.surface, Surface::Gallery(_))
    }

    /// Identificador de la superficie activa.
    pub(crate) fn surface_id(&self) -> String {
        match self.surface {
            Surface::App => self.app_surface.clone(),
            Surface::Gallery(section) => format!("gallery.{}", section.id()),
        }
    }
}

/// Interpreta los argumentos de Studio y los quita de `args`.
pub(crate) fn parse_args(args: &mut Vec<PathBuf>) -> Option<Launch> {
    let mut launch = Launch {
        dir: std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("target")
            .join("studio"),
        ..Launch::default()
    };
    let mut enabled = false;
    let mut index = 0;
    while index < args.len() {
        let value = args[index].to_string_lossy().to_string();
        match value.as_str() {
            "--studio" => {
                enabled = true;
                args.remove(index);
            }
            "--studio-gallery" => {
                enabled = true;
                launch.gallery_only = true;
                args.remove(index);
            }
            "--studio-hidden" => {
                enabled = true;
                launch.hidden = true;
                args.remove(index);
            }
            "--studio-dir" => {
                enabled = true;
                if let Some(path) = take_value(args, index) {
                    launch.dir = PathBuf::from(path);
                }
            }
            "--studio-port" => {
                enabled = true;
                if let Some(port) = take_value(args, index).and_then(|value| value.parse().ok()) {
                    launch.port = port;
                }
            }
            "--studio-fixture" => {
                enabled = true;
                launch.fixture = take_value(args, index);
            }
            _ => index += 1,
        }
    }
    enabled.then_some(launch)
}

fn take_value(args: &mut Vec<PathBuf>, index: usize) -> Option<String> {
    if index + 1 >= args.len() {
        return None;
    }
    let value = args.remove(index + 1);
    args.remove(index);
    Some(value.to_string_lossy().to_string())
}

/// Crea la galería, arranca el servidor y enlaza el bucle de comandos.
pub(crate) fn attach(
    launch: &Launch,
    window: &mut Window,
    cx: &mut Context<'_, Shell>,
) -> Result<StudioState, String> {
    let gallery = cx.new(|cx| Gallery::new(window, cx));
    let surface = if launch.gallery_only {
        Surface::Gallery(GallerySection::Foundations)
    } else {
        Surface::App
    };
    if let Surface::Gallery(section) = surface {
        gallery.update(cx, |gallery, _| gallery.set_section(section));
    }
    let (tx, rx) = async_channel::unbounded::<Incoming>();
    let server = server::start(&launch.dir, launch.port, tx).map_err(|error| {
        format!(
            "no se pudo abrir el control de Studio en {}: {error}",
            launch.dir.display()
        )
    })?;
    let shots_dir = launch.dir.join("shots");
    let shots_dir_loop = shots_dir.clone();
    cx.spawn_in(window, async move |this, cx| {
        while let Ok(incoming) = rx.recv().await {
            let Request { id, token, cmd } = incoming.request;
            let reply = incoming.reply;
            let result = match cmd {
                Command::Wait { ms } => {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(ms.min(15_000)))
                        .await;
                    let _ = reply.send(serialize(&Reply::done(id)));
                    Ok(())
                }
                Command::Shot { name, clip } => {
                    let shots_dir = shots_dir_loop.clone();
                    // Deja que los PTY y los efectos pendientes (restauración
                    // de historial, layout) terminen antes de pintar.
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(1_200))
                        .await;
                    let response = cx
                        .update(move |window, cx| {
                            window.refresh();
                            let _ = window.draw(cx);
                            let ui_scale = axf_ui::theme::metrics(cx).scale;
                            capture::capture(window, &shots_dir, &name, clip, ui_scale)
                        })
                        .unwrap_or_else(|error| {
                            Err(anyhow::anyhow!("ventana no disponible: {error}"))
                        });
                    let response = match response {
                        Ok(shot) => Reply {
                            id,
                            ok: true,
                            state: None,
                            surfaces: None,
                            shot: Some(shot),
                            error: None,
                        },
                        Err(error) => Reply::error(id, format!("no se pudo capturar: {error}")),
                    };
                    let _ = reply.send(serialize(&response));
                    Ok(())
                }
                Command::Key { key } => {
                    let result = cx
                        .update(|window, cx| dispatch_key(&key, window, cx))
                        .unwrap_or_else(|error| Err(format!("ventana no disponible: {error}")));
                    let response = match result {
                        Ok(()) => this
                            .update_in(cx, |shell, window, cx| {
                                Reply::ok(id, snapshot(shell, window, cx))
                            })
                            .unwrap_or_else(|error| {
                                Reply::error(id, format!("ventana no disponible: {error}"))
                            }),
                        Err(error) => Reply::error(id, error),
                    };
                    let _ = reply.send(serialize(&response));
                    Ok(())
                }
                cmd => this.update_in(cx, |shell, window, cx| {
                    execute(shell, Request { id, token, cmd }, reply, window, cx);
                }),
            };
            let _ = result;
        }
    })
    .detach();
    Ok(StudioState {
        surface,
        app_surface: "app.workspace".into(),
        gallery,
        shots_dir,
        fixture: launch.fixture.clone(),
        agents_frozen: launch.fixture.as_deref() == Some("demo"),
        _server: Some(server),
    })
}

fn execute(
    shell: &mut Shell,
    request: Request,
    reply: SyncSender<String>,
    window: &mut Window,
    cx: &mut Context<'_, Shell>,
) {
    let id = request.id;
    let result = match request.cmd {
        Command::State => Ok(()),
        Command::Surfaces => {
            let response = Reply {
                id,
                ok: true,
                state: Some(snapshot(shell, window, cx)),
                surfaces: Some(
                    protocol::SURFACES
                        .iter()
                        .map(|surface| (*surface).to_owned())
                        .collect(),
                ),
                shot: None,
                error: None,
            };
            let _ = reply.send(serialize(&response));
            return;
        }
        Command::Open { surface } => open(shell, &surface, window, cx),
        Command::Action { id: action } => shell.studio_dispatch_action(&action, window, cx),
        Command::Theme { name } => shell.studio_theme(&name, cx),
        Command::Variant { variant } => shell.studio_variant(&variant, cx),
        Command::Scale { value } => shell.studio_scale(value, cx),
        Command::Size { width, height } => {
            window.resize(size(px(width.max(320.0)), px(height.max(240.0))));
            cx.notify();
            Ok(())
        }
        Command::Focus { target } => shell.studio_focus(&target, window, cx),
        Command::Key { .. } => Err("la tecla se atiende fuera de la actualización".into()),
        Command::Workspace { fixture } => shell.studio_workspace(&fixture, window, cx),
        Command::Modal { open } => {
            let Some(studio) = shell.studio.as_ref() else {
                return reply_error(id, reply, "Studio no está activo");
            };
            let gallery = studio.gallery.clone();
            if open {
                shell.studio_set_gallery_section(GallerySection::Overlays, window, cx);
            }
            gallery.update(cx, |gallery, cx| {
                gallery.set_modal(open);
                cx.notify();
            });
            Ok(())
        }
        Command::Wait { .. } => Ok(()),
        Command::Shot { .. } => Err("la captura se atiende fuera de la actualización".into()),
        Command::Quit => {
            shell.studio_quit(cx);
            let _ = reply.send(serialize(&Reply::ok(id, snapshot(shell, window, cx))));
            return;
        }
    };
    let response = match result {
        Ok(()) => Reply::ok(id, snapshot(shell, window, cx)),
        Err(error) => Reply::error(id, error),
    };
    let _ = reply.send(serialize(&response));
}

fn dispatch_key(key: &str, window: &mut Window, cx: &mut App) -> Result<(), String> {
    if !protocol::is_allowed_key(key) {
        return Err(format!("tecla no permitida en Studio: {key}"));
    }
    let keystroke = Keystroke::parse_with_os(key, cx.platform_info().os)
        .map_err(|error| format!("tecla inválida: {error}"))?;
    window.dispatch_event(
        KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(KeyUpEvent { keystroke }.to_platform_input(), cx);
    Ok(())
}

fn open(
    shell: &mut Shell,
    surface: &str,
    window: &mut Window,
    cx: &mut Context<'_, Shell>,
) -> Result<(), String> {
    if let Some(section) = surface.strip_prefix("gallery.") {
        let Some(section) = GallerySection::from_id(section) else {
            return Err(format!("sección de galería desconocida: {section}"));
        };
        shell.studio_set_gallery_section(section, window, cx);
        return Ok(());
    }
    if surface == "gallery" {
        shell.studio_set_gallery_section(GallerySection::Foundations, window, cx);
        return Ok(());
    }
    shell.studio_open_app_surface(surface, window, cx)
}

fn snapshot(shell: &mut Shell, window: &Window, cx: &mut Context<'_, Shell>) -> Snapshot {
    let state = theme::state(cx);
    let (projects, tabs, panes) = shell.studio_workspace_counts();
    Snapshot {
        surface: shell
            .studio
            .as_ref()
            .map(StudioState::surface_id)
            .unwrap_or_else(|| "app.workspace".into()),
        theme: state.theme.meta.name.clone(),
        variant: match state.theme.variant() {
            axf_ui::theme::ThemeVariant::Dark => "dark".into(),
            axf_ui::theme::ThemeVariant::Light => "light".into(),
        },
        width: window.viewport_size().width / px(1.0),
        height: window.viewport_size().height / px(1.0),
        ui_scale: state.metrics.scale,
        display_scale: window.scale_factor(),
        projects,
        tabs,
        panes,
        gallery_modal: shell
            .studio
            .as_ref()
            .map(|studio| studio.gallery.read(cx).modal_open())
            .unwrap_or(false),
        shots_dir: shell
            .studio
            .as_ref()
            .map(|studio| studio.shots_dir.display().to_string())
            .unwrap_or_default(),
    }
}

fn reply_error(id: u64, reply: SyncSender<String>, message: impl Into<String>) {
    let _ = reply.send(serialize(&Reply::error(id, message)));
}

fn serialize(reply: &Reply) -> String {
    serde_json::to_string(reply).unwrap_or_else(|error| {
        format!(
            "{{\"id\":{},\"ok\":false,\"error\":\"serialize: {error}\"}}",
            reply.id
        )
    })
}

/// Renderiza la galería como superficie completa.
pub(crate) fn render_gallery(
    state: &StudioState,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    div().size_full().child(state.gallery.clone())
}

// Mantiene el import de `Context`/`App` usados por las firmas de otras partes.
const _: fn(&mut Context<'_, Shell>) = |_| {};
