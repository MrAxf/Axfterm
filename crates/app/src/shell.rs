//! Shell de la ventana: sidebar de proyectos, árbol de paneles, pestañas y
//! terminales.
//!
//! El modelo (`axf_workspace`) es la fuente de verdad. Este módulo traduce sus
//! operaciones a entidades `TerminalView`, guarda el mapa `TerminalId -> PTY` y
//! persiste la sesión sin bloquear el hilo de UI.
#![expect(
    clippy::let_unit_value,
    reason = "WeakEntity::update devuelve () en la versión fijada de gpui-pre"
)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use axf_agents::{
    AgentKind, AgentReport, AgentSession, AgentSessionId, AgentState, AgentTransition,
    AttentionKind, DetectedAgent, ProcessScanner, ProcessSnapshot,
};
use axf_config::ProfileId;
use axf_terminal::{
    ProgramState, ProgramStatus, TerminalSignal, TerminalSnapshot, TerminalStyle, TerminalView,
};

use crate::history::HistoryStore;
use axf_ui::components::{
    Button, ControlSize, EmptyState, Icon, IconButton, IconSize, Menu, MenuItem, Modal,
    ProfileIcon, ProfileIconSource, Tab as TabView, TabStatus, TextField, Toast, Toggle,
    text_tooltip,
};
use axf_ui::gpui::component::button::{Button as KitButton, ButtonVariants as _};
use axf_ui::gpui::component::input::{InputEvent, InputState};
use axf_ui::gpui::component::menu::{
    ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem,
};
use axf_ui::gpui::component::scroll::ScrollableElement as _;
use axf_ui::gpui::component::{ActiveTheme, ElementExt as _, Sizable as _, WindowExt as _};
use axf_ui::gpui::prelude::FluentBuilder as _;
#[cfg(target_os = "windows")]
use axf_ui::gpui::svg;
use axf_ui::gpui::{
    AnyElement, AppContext, Bounds, ClickEvent, Context, Entity, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels,
    Point, Render, ScrollDelta, ScrollEvent, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, WindowControlArea, div, point,
    px, relative, transparent_black,
};
use axf_ui::icons;
use axf_ui::metrics::UiMetrics;
use axf_ui::theme;
use axf_workspace::{
    Branch, PaneId, PaneTree, ProjectId, SessionFile, SidebarSection, SplitAxis, TabId, TerminalId,
    Workspace,
};

use crate::theme_host::ThemeHost;
use crate::{
    ClosePane, CloseTab, DuplicateTab, ExportTheme, FocusNextPane, FocusPreviousPane, FocusSidebar,
    ImportTheme, MoveTabNextPane, NewTab, NextTab, NextTheme, OpenCommandPalette, OpenProject,
    OpenThemesFolder, PreviousTab, PreviousTheme, Quit, ReloadThemes, RenameTab, RestartTab,
    SIDEBAR_CONTEXT, ShowThemeInfo, SplitHorizontal, SplitVertical, ToggleSidebar,
    ToggleSidebarAgents, ToggleThemeVariant, ToggleZoom, WORKSPACE_CONTEXT, keys,
};

/// Tope de terminales de v0.1 (ADR 0005).
const MAX_TABS: usize = 32;

/// Nombre visible de la app en la barra de título.
const APP_NAME: &str = "AxfTerm";

/// Avisos simultáneos en pantalla.
const MAX_NOTICES: usize = 3;
/// Ventana en la que un aviso de agente con la misma clave no se repite.
const NOTICE_COOLDOWN: Duration = Duration::from_secs(10);
/// Silencio de salida que cierra un `working` sin OSC 7501.
const OUTPUT_QUIET: Duration = Duration::from_millis(2_500);

/// Fila navegable de la sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarRow {
    Project(ProjectId),
    Tab(TabId),
}

/// Proyecto visible en la sidebar con sus pestañas ya filtradas.
struct SidebarGroup {
    project: ProjectId,
    tabs: Vec<TabId>,
    /// `false` cuando el proyecto está plegado.
    show_tabs: bool,
}

/// Calcula los grupos de terminales sin duplicar lógica entre render y teclado.
///
/// Con `SidebarSection::Agents` solo se incluyen proyectos y terminales con una
/// sesión de agente activa, ordenando sus pestañas por prioridad y actividad.
fn sidebar_view(workspace: &Workspace, section: SidebarSection) -> Vec<SidebarGroup> {
    let expanded = workspace.expanded_projects();
    let agents_only = section == SidebarSection::Agents;
    let mut groups = Vec::new();
    for project in workspace.projects() {
        let mut tabs = workspace.project_tabs(project.id);
        if agents_only {
            tabs.retain(|tab| {
                workspace
                    .tab(*tab)
                    .is_some_and(axf_workspace::Tab::has_agents)
            });
            if tabs.is_empty() {
                continue;
            }
            tabs.sort_by(|left, right| {
                let left_agg = workspace
                    .tab(*left)
                    .and_then(axf_workspace::Tab::agent_aggregate);
                let right_agg = workspace
                    .tab(*right)
                    .and_then(axf_workspace::Tab::agent_aggregate);
                let left_prio = left_agg.as_ref().map_or(0, |agg| agg.state.priority());
                let right_prio = right_agg.as_ref().map_or(0, |agg| agg.state.priority());
                let left_act = left_agg.as_ref().map(|agg| agg.dominant.last_activity);
                let right_act = right_agg.as_ref().map(|agg| agg.dominant.last_activity);
                right_prio
                    .cmp(&left_prio)
                    .then(right_act.cmp(&left_act))
                    .then(left.get().cmp(&right.get()))
            });
        }
        groups.push(SidebarGroup {
            project: project.id,
            tabs,
            show_tabs: agents_only || expanded.contains(&project.id),
        });
    }
    groups
}

/// Convierte los grupos visibles de terminales en filas navegables.
fn sidebar_rows_from_groups(groups: &[SidebarGroup]) -> Vec<SidebarRow> {
    let mut rows = Vec::new();
    for group in groups {
        rows.push(SidebarRow::Project(group.project));
        if group.show_tabs {
            rows.extend(group.tabs.iter().copied().map(SidebarRow::Tab));
        }
    }
    rows
}

/// Contexto de la terminal relativo al proyecto.
///
/// Dentro del proyecto se muestra la ruta relativa; fuera se usa la última
/// carpeta con elipsis. La raíz del proyecto devuelve `None` para no repetir el
/// nombre que ya está en la cabecera del grupo.
fn context_path(root: &std::path::Path, cwd: &std::path::Path) -> Option<String> {
    if let Ok(relative) = cwd.strip_prefix(root) {
        if relative.as_os_str().is_empty() {
            return None;
        }
        return Some(relative.display().to_string());
    }
    cwd.file_name()
        .map(|name| format!("…{}{}", std::path::MAIN_SEPARATOR, name.to_string_lossy()))
}

/// Confirmación pendiente al cerrar algo con procesos vivos.
#[derive(Clone, Copy, Debug)]
enum PendingConfirm {
    ClosePane(PaneId),
    RemoveProject(ProjectId),
}

struct TerminalEntry {
    view: Entity<TerminalView>,
    _subscription: Subscription,
}

/// Cómo arrancar una terminal: historial y si debe tomar el foco.
struct SpawnOptions {
    restore: Option<TerminalSnapshot>,
    focus: bool,
}

struct TabEdit {
    tab: TabId,
    input: Entity<InputState>,
    original: String,
    focused: bool,
    _subscription: Subscription,
}

/// Datos que viajan con el drag de una pestaña.
#[derive(Clone)]
struct TabDrag {
    tab: TabId,
    title: SharedString,
    icon: ProfileIconSource,
    status: Option<TabStatus>,
}

/// Zona de un panel sobre la que se suelta una pestaña.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabDropZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

impl TabDropZone {
    /// Eje del split que crea esta zona; `None` para mover al panel.
    fn axis(self) -> Option<SplitAxis> {
        match self {
            Self::Center => None,
            Self::Left | Self::Right => Some(SplitAxis::Horizontal),
            Self::Top | Self::Bottom => Some(SplitAxis::Vertical),
        }
    }

    /// Lado del split en el que nace el panel nuevo.
    fn side(self) -> Branch {
        match self {
            Self::Center | Self::Right | Self::Bottom => Branch::Second,
            Self::Left | Self::Top => Branch::First,
        }
    }
}

/// Destino de una pestaña durante el drag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TabDropTarget {
    pane: PaneId,
    zone: TabDropZone,
}

/// Calcula la zona a partir de la posición y los límites del cuerpo del panel.
///
/// Gana el borde más cercano cuando está dentro de `edge_ratio`; si no, la
/// pestaña se mueve al panel.
fn tab_drop_zone(position: Point<Pixels>, bounds: Bounds<Pixels>, edge_ratio: f32) -> TabDropZone {
    let width = f32::from(bounds.size.width).max(1.0);
    let height = f32::from(bounds.size.height).max(1.0);
    let x = (f32::from(position.x - bounds.left()) / width).clamp(0.0, 1.0);
    let y = (f32::from(position.y - bounds.top()) / height).clamp(0.0, 1.0);
    let distances = [x, 1.0 - x, y, 1.0 - y];
    let mut nearest = 0;
    let mut distance = distances[0];
    for (index, candidate) in distances.iter().enumerate().skip(1) {
        if *candidate < distance {
            nearest = index;
            distance = *candidate;
        }
    }
    if distance > edge_ratio {
        return TabDropZone::Center;
    }
    match nearest {
        0 => TabDropZone::Left,
        1 => TabDropZone::Right,
        2 => TabDropZone::Top,
        _ => TabDropZone::Bottom,
    }
}

fn bounds_contain(bounds: Bounds<Pixels>, position: Point<Pixels>) -> bool {
    position.x >= bounds.left()
        && position.x <= bounds.right()
        && position.y >= bounds.top()
        && position.y <= bounds.bottom()
}

/// Ajusta un índice de inserción visible al que espera `move_tab`, que quita
/// la pestaña antes de insertarla.
fn adjusted_insert_index(source: Option<usize>, index: usize) -> usize {
    match source {
        Some(from) if from < index => index.saturating_sub(1),
        _ => index,
    }
}

/// Ancho uniforme de cada pestaña, ya comprimido contra la strip.
///
/// Los chips no tienen hueco entre sí y comparten base y mínimos, así que se
/// contraen a la vez: el ancho es el que cabe dividido entre cuántas hay,
/// acotado entre el mínimo y el objetivo.
fn tab_slot_width(strip_width: Pixels, count: usize, min_width: Pixels, max_width: Pixels) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let fit = f32::from(strip_width) / count as f32;
    fit.clamp(f32::from(min_width), f32::from(max_width))
}

/// Índice de inserción para una posición local dentro del contenido de la strip.
///
/// La pestaña cambia de lado al cruzar su punto medio, igual que el indicador.
fn insert_index_at(local_x: f32, tab_width: f32, count: usize) -> usize {
    if count == 0 || tab_width <= 0.0 {
        return 0;
    }
    let slot = (local_x / tab_width + 0.5).floor();
    (slot.max(0.0) as usize).min(count)
}

/// Un paso de auto-scroll de la strip según la posición del puntero.
///
/// Es una función pura para garantizar que cada movimiento aplica exactamente
/// un `nudge`, nunca uno por pestaña.
fn tab_autoscroll_offset(
    offset: Pixels,
    max: Pixels,
    pointer_x: Pixels,
    left: Pixels,
    right: Pixels,
    edge: Pixels,
    nudge: Pixels,
) -> Pixels {
    if max <= px(0.0) {
        return offset;
    }
    if pointer_x < left + edge {
        (offset + nudge).min(px(0.0))
    } else if pointer_x > right - edge {
        (offset - nudge).max(-max)
    } else {
        offset
    }
}

/// Datos que viajan con el drag de un divisor.
#[derive(Clone)]
struct SplitDrag {
    path: Vec<Branch>,
    axis: SplitAxis,
    key: String,
}

/// Aviso visible como toast flotante.
#[derive(Clone, Debug)]
struct Notice {
    text: String,
    detail: Option<String>,
    error: bool,
    /// Clave de deduplicación; los avisos de agente la usan.
    key: Option<String>,
    seq: u64,
}

/// Cambios detectados por el escáner para una pestaña.
type AgentTransitions = Vec<(AgentSessionId, AgentTransition)>;

struct AgentTabUpdate {
    tab: TabId,
    changed: bool,
    transitions: AgentTransitions,
}

/// Estado runtime de la sidebar y los agentes.
#[derive(Default)]
struct AgentUiState {
    /// Identificador de la siguiente sesión detectada.
    seq: u64,
    /// Cooldown de avisos por clave.
    notice_cooldowns: HashMap<String, Instant>,
}

/// Vista raíz de la ventana.
pub(crate) struct Shell {
    workspace: Workspace,
    session_path: Option<PathBuf>,
    focus: FocusHandle,
    sidebar_focus: FocusHandle,
    theme_modal_focus: FocusHandle,
    terminals: HashMap<TerminalId, TerminalEntry>,
    pending_terminals: HashSet<TerminalId>,
    spawn_errors: HashMap<TerminalId, String>,
    history: HistoryStore,
    pane_scroll: HashMap<PaneId, ScrollHandle>,
    body_bounds: Rc<RefCell<HashMap<PaneId, Bounds<Pixels>>>>,
    split_bounds: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    revealed: HashMap<PaneId, TabId>,
    last_focused_pane: Option<PaneId>,
    tab_drop: Option<TabDropTarget>,
    tab_insert: Option<(PaneId, usize)>,
    zoomed: Option<PaneId>,
    sidebar_cursor: Option<SidebarRow>,
    sidebar_section: SidebarSection,
    editing: Option<TabEdit>,
    pending_focus: Option<TabId>,
    notices: Vec<Notice>,
    notice_seq: u64,
    agents: AgentUiState,
    palette_hint: bool,
    palette_hint_seq: u64,
    theme_host: ThemeHost,
    theme_modal: bool,
    style_revision: u64,
    appearance_subscription: Option<Subscription>,
    window_title: String,
    save_generation: u64,
    quitting: bool,
    close_hook_registered: bool,
}

impl Shell {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        startup: Option<PathBuf>,
        theme_host: ThemeHost,
    ) -> Self {
        let focus = cx.focus_handle();
        let sidebar_focus = cx.focus_handle();
        let theme_modal_focus = cx.focus_handle();
        window.focus(&focus, cx);

        let session_path = match axf_config::session_path() {
            Ok(path) => Some(path),
            Err(error) => {
                // Seguimos sin sesión: el workspace funciona en memoria.
                tracing::warn!("no se pudo resolver session.json: {error}");
                let _ = axf_config::ensure_config_dir();
                None
            }
        };
        let load = session_path
            .as_deref()
            .map(|path| {
                axf_workspace::load_or_recover(path, &theme_host.settings.terminal.default_profile)
            })
            .unwrap_or_default();
        let startup_warning = load.warning.or_else(|| {
            session_path.is_none().then(|| {
                "No se pudo resolver el directorio de configuración; la sesión no se guardará."
                    .to_owned()
            })
        });
        let fresh_session = load.session.is_none();
        let workspace = load
            .session
            .map(|session| session.workspace)
            .unwrap_or_default();
        let history = HistoryStore::from_config(fresh_session);
        let theme_notes = theme_host.notes.clone();
        let sidebar_section = workspace.sidebar().section;

        let mut shell = Self {
            workspace,
            session_path,
            focus,
            sidebar_focus,
            theme_modal_focus,
            terminals: HashMap::new(),
            pending_terminals: HashSet::new(),
            spawn_errors: HashMap::new(),
            history,
            pane_scroll: HashMap::new(),
            body_bounds: Rc::new(RefCell::new(HashMap::new())),
            split_bounds: Rc::new(RefCell::new(HashMap::new())),
            revealed: HashMap::new(),
            last_focused_pane: None,
            tab_drop: None,
            tab_insert: None,
            zoomed: None,
            sidebar_cursor: None,
            sidebar_section,
            editing: None,
            pending_focus: None,
            notices: Vec::new(),
            notice_seq: 0,
            agents: AgentUiState::default(),
            palette_hint: false,
            palette_hint_seq: 0,
            theme_host,
            // Diagnóstico visual: abre el modal de tema al arrancar.
            theme_modal: std::env::var_os("AXFTERM_SHOW_THEME_MODAL").is_some(),
            style_revision: 0,
            appearance_subscription: None,
            window_title: "Axfterm".into(),
            save_generation: 0,
            quitting: false,
            close_hook_registered: false,
        };

        if let Some(path) = startup {
            shell.open_project_path(path, window, cx);
        }
        if let Some(tab) = shell.workspace.focus().tab {
            shell.pending_focus = Some(tab);
        }
        shell.restore_visible_terminals(cx);
        shell.reveal_focus();
        shell.watch_config(cx);
        shell.spawn_wsl_discovery(cx);
        shell.spawn_agent_scanner(cx);

        let this = cx.entity().downgrade();
        shell.appearance_subscription = Some(window.observe_window_appearance(move |_, cx| {
            if let Some(this) = this.upgrade() {
                let _ = this.update(cx, |shell, cx| {
                    if shell.theme_host.settings.appearance == axf_config::Appearance::System {
                        let messages = shell.theme_host.apply_current(cx);
                        for message in messages {
                            shell.set_notice(message, false, cx);
                        }
                        cx.notify();
                    }
                });
            }
        }));

        let mut warnings = theme_notes;
        if let Some(warning) = startup_warning {
            warnings.push(warning);
        }
        if !warnings.is_empty() {
            shell.set_notice(warnings.join(" "), true, cx);
        }

        shell
    }

    /// Detecta WSL en segundo plano solo cuando los perfiles vienen del
    /// descubrimiento inicial; una config con perfiles propios no se toca.
    fn spawn_wsl_discovery(&mut self, cx: &mut Context<'_, Self>) {
        if !self.theme_host.profiles_detected {
            return;
        }
        self.theme_host.profiles_detected = false;
        cx.spawn(async move |this, cx| {
            let discovered = cx
                .background_executor()
                .spawn(async { axf_config::discover_wsl() })
                .await;
            if discovered.is_empty() {
                return;
            }
            this.update(cx, |shell, cx| {
                match shell.theme_host.merge_wsl_profiles(discovered, cx) {
                    Ok(added) if !added.is_empty() => {
                        let names: Vec<String> = added
                            .iter()
                            .filter_map(|id| shell.theme_host.profile_name(id))
                            .collect();
                        shell.set_notice(
                            format!("Terminales WSL añadidas: {}.", names.join(", ")),
                            false,
                            cx,
                        );
                        shell.persist(cx);
                        cx.notify();
                    }
                    Ok(_) => {}
                    Err(error) => shell.set_notice(error, true, cx),
                }
            })
            .ok();
        })
        .detach();
    }

    // --- Agentes --------------------------------------------------------

    /// Escaneo de procesos en segundo plano; nunca bloquea el hilo de UI.
    fn spawn_agent_scanner(&mut self, cx: &mut Context<'_, Self>) {
        let scanner = Arc::new(Mutex::new(ProcessScanner::new()));
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(settings) =
                    this.update(cx, |shell, _| shell.theme_host.settings.agents.clone())
                else {
                    break;
                };
                if !settings.enabled {
                    let _ = this.update(cx, |shell, cx| {
                        if shell.workspace.clear_agents() {
                            cx.notify();
                        }
                    });
                    cx.background_executor()
                        .timer(Duration::from_millis(1_000))
                        .await;
                    continue;
                }

                let roots = this
                    .update(cx, |shell, cx| shell.terminal_roots(cx))
                    .unwrap_or_default();
                if !roots.is_empty() {
                    let scanner = Arc::clone(&scanner);
                    let snapshot = cx
                        .background_executor()
                        .spawn(async move {
                            match scanner.lock() {
                                Ok(mut scanner) => scanner.scan(),
                                Err(poisoned) => poisoned.into_inner().scan(),
                            }
                        })
                        .await;
                    let alive = this
                        .update(cx, |shell, cx| shell.apply_agent_snapshot(&snapshot, cx))
                        .is_ok();
                    if !alive {
                        break;
                    }
                }

                cx.background_executor()
                    .timer(Duration::from_millis(settings.scan_interval_ms))
                    .await;
            }
        })
        .detach();
    }

    /// PID raíz de cada terminal viva.
    fn terminal_roots(&self, cx: &mut Context<'_, Self>) -> Vec<(TerminalId, u32)> {
        self.terminals
            .iter()
            .filter_map(|(terminal, entry)| entry.view.read(cx).pid().map(|pid| (*terminal, pid)))
            .collect()
    }

    /// Reconcilia las sesiones de cada pestaña con un snapshot de procesos.
    fn apply_agent_snapshot(&mut self, snapshot: &ProcessSnapshot, cx: &mut Context<'_, Self>) {
        let now = SystemTime::now();
        let mut next_id = self.agents.seq;
        let mut updates: Vec<AgentTabUpdate> = Vec::new();

        for (terminal, entry) in &self.terminals {
            let Some(tab) = self.workspace.tab_for_terminal(*terminal) else {
                continue;
            };
            let view = entry.view.read(cx);
            let Some(pid) = view.pid() else {
                continue;
            };
            let running = view.is_running();
            let exit = view.exit_info();
            let recent_output = view
                .last_output_at()
                .is_some_and(|at| at.elapsed() < OUTPUT_QUIET);
            let detected = snapshot.agents_for(pid);
            let mut scan_next_id = next_id;
            let outcome = self.workspace.update_tab_agents(tab, |sessions| {
                reconcile_agents(
                    sessions,
                    &detected,
                    running,
                    exit,
                    recent_output,
                    now,
                    &mut scan_next_id,
                )
            });
            next_id = scan_next_id;
            if let Some((changed, transitions)) = outcome {
                updates.push(AgentTabUpdate {
                    tab,
                    changed,
                    transitions,
                });
            }
        }
        self.agents.seq = next_id;

        let mut changed = false;
        let mut persist = false;
        for update in updates {
            changed |= update.changed;
            let resume = self
                .workspace
                .tab_agents(update.tab)
                .and_then(|sessions| sessions.iter().find(|session| session.alive))
                .map(|session| session.kind.as_str().to_owned());
            if self.workspace.set_tab_resume_agent(update.tab, resume) {
                persist = true;
            }
            for (session, transition) in update.transitions {
                self.notify_agent_transition(update.tab, session, transition, cx);
            }
        }
        if persist {
            self.persist(cx);
        }
        if changed {
            cx.notify();
        }
    }

    /// Procesa las señales OSC de una terminal para la máquina de agentes.
    fn drain_terminal_signals(&mut self, terminal: TerminalId, cx: &mut Context<'_, Self>) {
        let Some(entry) = self.terminals.get(&terminal) else {
            return;
        };
        let signals = entry.view.update(cx, |view, _| view.take_signals());
        if signals.is_empty() {
            return;
        }
        let Some(tab) = self.workspace.tab_for_terminal(terminal) else {
            return;
        };

        let now = SystemTime::now();
        let mut next_id = self.agents.seq;
        let mut transitions: AgentTransitions = Vec::new();
        let mut changed = false;

        for signal in signals {
            match signal {
                TerminalSignal::ProgramStatus(status) => {
                    let report = agent_report_from_status(&status);
                    let outcome = self.workspace.update_tab_agents(tab, |sessions| {
                        let hint = report.app.as_deref().and_then(AgentKind::from_app);
                        sessions.truncate(1);
                        let id = match sessions.first_mut() {
                            Some(session) => {
                                if let Some(kind) = hint {
                                    session.kind = kind;
                                }
                                session.id
                            }
                            None => {
                                let kind = hint?;
                                let id = next_id;
                                next_id = next_id.wrapping_add(1);
                                sessions.push(AgentSession::new(id, kind, 0, now));
                                id
                            }
                        };
                        let session = sessions.first_mut()?;
                        Some((id, session.apply_report(&report, now)))
                    });
                    if let Some(outcome) = outcome.flatten() {
                        transitions.push(outcome);
                    }
                }
                TerminalSignal::Attention(message) => {
                    let message = message.or_else(|| Some("Señal de atención".into()));
                    let outcome = self.workspace.update_tab_agents(tab, |sessions| {
                        let session = sessions
                            .iter_mut()
                            .max_by_key(|session| (session.last_activity, session.id))?;
                        let id = session.id;
                        Some((id, session.apply_attention(None, message.clone(), now)))
                    });
                    if let Some(outcome) = outcome.flatten() {
                        transitions.push(outcome);
                    }
                }
                TerminalSignal::Exited(_) => {}
            }
        }

        self.agents.seq = next_id;
        let resume = self
            .workspace
            .tab_agents(tab)
            .and_then(|sessions| sessions.iter().find(|session| session.alive))
            .map(|session| session.kind.as_str().to_owned());
        let persist = self.workspace.set_tab_resume_agent(tab, resume);
        for (session, transition) in transitions {
            changed = true;
            self.notify_agent_transition(tab, session, transition, cx);
        }
        if persist {
            self.persist(cx);
        }
        if changed {
            cx.notify();
        }
    }

    /// Aviso in-app y marca de título para un cambio de estado de agente.
    fn notify_agent_transition(
        &mut self,
        tab: TabId,
        session_id: AgentSessionId,
        transition: AgentTransition,
        cx: &mut Context<'_, Self>,
    ) {
        if !transition.notify {
            return;
        }
        let settings = &self.theme_host.settings.agents;
        let enabled = match transition.to {
            AgentState::Finished => settings.notify_finished,
            AgentState::Error => settings.notify_error,
            AgentState::NeedsAttention => settings.notify_attention,
            _ => false,
        };
        if !enabled || self.workspace.focus().tab == Some(tab) {
            return;
        }
        let Some(session) = self
            .workspace
            .tab_agents(tab)
            .and_then(|sessions| sessions.iter().find(|session| session.id == session_id))
            .cloned()
        else {
            return;
        };
        let title = self.tab_title(tab, cx);
        let (text, error) = match transition.to {
            AgentState::Finished => (format!("«{title}» terminó"), false),
            AgentState::Error => (format!("«{title}» falló"), true),
            AgentState::NeedsAttention => (format!("«{title}» requiere atención"), false),
            _ => return,
        };
        let detail = session.message.clone().or_else(|| {
            session
                .attention
                .map(|kind| format!("Motivo: {}", kind.label()))
        });
        let key = format!(
            "agent:{}:{}:{}",
            tab.get(),
            session.kind.as_str(),
            transition.to.label()
        );
        self.push_notice(text, error, Some(key), detail, cx);
    }

    fn watch_config(&mut self, cx: &mut Context<'_, Self>) {
        let Some(watcher) = self.theme_host.watcher.as_ref() else {
            return;
        };
        let events = watcher.events.clone();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let applied = this.update(cx, |shell, cx| {
                    let report = shell.theme_host.reload(event, cx);
                    if report.applied {
                        cx.notify();
                    }
                    let had = !report.errors.is_empty() || !report.messages.is_empty();
                    let has_error = !report.errors.is_empty();
                    let mut parts = report.errors;
                    parts.extend(report.messages);
                    if !parts.is_empty() {
                        shell.set_notice(parts.join(" "), has_error, cx);
                    }
                    if !had {
                        let message = match (event.config, event.themes) {
                            (true, true) => "Configuración y temas recargados.",
                            (true, false) => "Configuración recargada.",
                            (false, true) => "Temas recargados.",
                            (false, false) => "",
                        };
                        if !message.is_empty() {
                            shell.set_notice(message, false, cx);
                        }
                    }
                });
                if applied.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    // --- Sesión ---------------------------------------------------------

    fn sync_terminal_meta(&mut self, cx: &mut Context<'_, Self>) {
        let mut updates = Vec::new();
        for (terminal, entry) in &self.terminals {
            let program = entry.view.read(cx).program().map(str::to_owned);
            if let Some(title) = entry.view.read(cx).title()
                && !title_is_shell_path(title, program.as_deref())
                && let Some(tab) = self.workspace.tab_for_terminal(*terminal)
            {
                updates.push((tab, title.to_owned()));
            }
        }
        for (tab, title) in updates {
            self.workspace.set_tab_title(tab, title);
        }
    }

    fn persist(&mut self, cx: &mut Context<'_, Self>) {
        if self.quitting {
            return;
        }
        self.sync_terminal_meta(cx);
        let Some(path) = self.session_path.clone() else {
            return;
        };
        let document = SessionFile::current(self.workspace.clone());
        self.save_generation = self.save_generation.wrapping_add(1);
        let generation = self.save_generation;

        cx.spawn(async move |this, cx| {
            let timer = cx.background_executor().timer(Duration::from_millis(250));
            timer.await;
            let latest = this
                .update(cx, |shell, _| shell.save_generation == generation)
                .unwrap_or(false);
            if latest {
                cx.background_executor()
                    .spawn(async move {
                        if let Err(error) = axf_workspace::save_session(&path, &document) {
                            tracing::warn!("no se pudo guardar la sesión: {error}");
                        }
                    })
                    .detach();
            }
        })
        .detach();
    }

    fn request_quit(&mut self, cx: &mut Context<'_, Self>) {
        if self.quitting {
            return;
        }
        self.quitting = true;
        self.sync_terminal_meta(cx);
        let path = self.session_path.clone();
        let document = SessionFile::current(self.workspace.clone());
        let history = self.history.clone();
        let keep: HashSet<TerminalId> = self.workspace.terminal_ids().collect();
        let requests: Vec<(TerminalId, axf_terminal::SnapshotRequest)> = self
            .terminals
            .iter()
            .filter_map(|(terminal, entry)| {
                entry
                    .view
                    .read(cx)
                    .request_snapshot()
                    .map(|request| (*terminal, request))
            })
            .collect();
        cx.spawn(async move |_, cx| {
            let writer = cx.background_executor().spawn(async move {
                for (terminal, request) in requests {
                    if let Some(snapshot) = request.wait().await
                        && let Err(error) = history.save(terminal, &snapshot)
                    {
                        tracing::warn!(
                            "no se pudo guardar el historial de la terminal {}: {error}",
                            terminal.get()
                        );
                    }
                }
                history.prune(&keep);
            });
            let _ = writer.await;
            if let Some(path) = path {
                let _ = cx
                    .background_executor()
                    .spawn(async move { axf_workspace::save_session(&path, &document) })
                    .await;
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }

    // --- Acciones -------------------------------------------------------

    fn quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.request_quit(cx);
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<'_, Self>) {
        let visible = self.workspace.sidebar().visible;
        self.set_sidebar_visible(!visible, cx);
    }

    /// Cambia la visibilidad de la sidebar y la persiste.
    fn set_sidebar_visible(&mut self, visible: bool, cx: &mut Context<'_, Self>) {
        if self.workspace.sidebar().visible == visible {
            return;
        }
        self.workspace.sidebar_mut().visible = visible;
        self.persist(cx);
        cx.notify();
    }

    fn open_project(&mut self, _: &OpenProject, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.prompt_open_project(cx);
    }

    fn focus_sidebar(&mut self, _: &FocusSidebar, window: &mut Window, cx: &mut Context<'_, Self>) {
        if !self.workspace.sidebar().visible {
            self.workspace.sidebar_mut().visible = true;
            self.persist(cx);
        }
        let rows = self.sidebar_rows();
        let current = self
            .sidebar_cursor
            .filter(|cursor| rows.contains(cursor))
            .or_else(|| {
                self.workspace
                    .current_project()
                    .map(SidebarRow::Project)
                    .filter(|row| rows.contains(row))
            });
        self.sidebar_cursor = current.or_else(|| rows.first().copied());
        window.focus(&self.sidebar_focus, cx);
        cx.notify();
    }

    fn toggle_sidebar_agents(
        &mut self,
        _: &ToggleSidebarAgents,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let section = self.sidebar_section.toggled();
        self.set_sidebar_section(section, cx);
    }

    fn set_sidebar_section(&mut self, section: SidebarSection, cx: &mut Context<'_, Self>) {
        self.palette_hint = false;
        if !self.workspace.sidebar().visible {
            self.workspace.sidebar_mut().visible = true;
        }
        self.workspace.sidebar_mut().section = section;
        self.sidebar_section = section;
        // El cursor anterior puede pertenecer a la otra sección.
        self.sidebar_cursor = self.sidebar_rows().first().copied();
        self.persist(cx);
        cx.notify();
    }

    fn open_command_palette(
        &mut self,
        _: &OpenCommandPalette,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.show_palette_placeholder(cx);
    }

    fn show_palette_placeholder(&mut self, cx: &mut Context<'_, Self>) {
        if !self.workspace.sidebar().visible {
            self.set_sidebar_visible(true, cx);
        }
        self.palette_hint = true;
        self.palette_hint_seq = self.palette_hint_seq.wrapping_add(1);
        let seq = self.palette_hint_seq;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            this.update(cx, |shell, cx| {
                if shell.palette_hint_seq == seq {
                    shell.palette_hint = false;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    // --- Tema y fuentes ------------------------------------------------

    fn show_theme_info(&mut self, _: &ShowThemeInfo, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.theme_modal = !self.theme_modal;
        cx.notify();
    }

    /// Abre el modal de tema y fuentes con el foco dentro.
    ///
    /// Hasta M7 es la superficie de ajustes; cuando exista `OpenSettings`, este
    /// método será el que abra el panel completo.
    fn open_theme_modal(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.theme_modal = true;
        window.focus(&self.theme_modal_focus, cx);
        cx.notify();
    }

    fn toggle_theme_variant(
        &mut self,
        _: &ToggleThemeVariant,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.theme_host.toggle_preview_variant(cx);
        let variant = theme::state(cx).theme.variant().label().to_lowercase();
        self.set_notice(format!("Previsualizando variante {variant}."), false, cx);
    }

    fn next_theme(&mut self, _: &NextTheme, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.cycle_theme(1, cx);
    }

    fn previous_theme(&mut self, _: &PreviousTheme, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.cycle_theme(-1, cx);
    }

    fn cycle_theme(&mut self, delta: isize, cx: &mut Context<'_, Self>) {
        let names = self.theme_host.theme_names();
        if names.is_empty() {
            return;
        }
        let current = names
            .iter()
            .position(|(name, _)| name == &self.theme_host.selected)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(names.len() as isize) as usize;
        let name = names[next].0.clone();
        if self.theme_host.select_theme(&name, cx) {
            self.set_notice(format!("Tema: {name}"), false, cx);
        }
    }

    fn reload_themes(&mut self, _: &ReloadThemes, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.reload_themes_impl(cx);
    }

    fn open_themes_folder(
        &mut self,
        _: &OpenThemesFolder,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_themes_folder_impl(cx);
    }

    fn import_theme(&mut self, _: &ImportTheme, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.import_theme_impl(cx);
    }

    fn export_theme(&mut self, _: &ExportTheme, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.export_theme_impl(cx);
    }

    fn close_theme_modal(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.theme_modal {
            self.theme_modal = false;
            self.restore_workspace_focus(window, cx);
            cx.notify();
        }
    }

    fn restore_workspace_focus(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab
            && let Some(entry) = self
                .workspace
                .tab(tab)
                .and_then(|tab| self.terminals.get(&tab.terminal_id))
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        } else {
            window.focus(&self.focus, cx);
        }
    }

    fn prompt_open_project(&mut self, cx: &mut Context<'_, Self>) {
        let receiver = cx.prompt_for_paths(axf_ui::gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Abrir carpeta".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                this.update_in(cx, |shell, window, cx| {
                    for path in paths {
                        shell.open_project_path(path, window, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn open_project_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((profile, title)) = self.default_profile() else {
            self.set_notice(
                "No hay un perfil por defecto válido en config.toml.",
                true,
                cx,
            );
            return;
        };
        match self.workspace.open_project(&path, &profile, &title) {
            Ok(opened) => {
                if let Some(new_tab) = opened.tab {
                    self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                }
                self.reveal_project(opened.project);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.new_tab_for(None, None, window, cx);
    }

    fn new_tab_for(
        &mut self,
        project: Option<ProjectId>,
        profile: Option<ProfileId>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let project = project
            .or_else(|| self.workspace.current_project())
            .or_else(|| self.sidebar_cursor_project())
            .or_else(|| self.workspace.projects().first().map(|project| project.id));
        let Some(project) = project else {
            self.prompt_open_project(cx);
            return;
        };
        let requested =
            profile.unwrap_or_else(|| self.theme_host.settings.terminal.default_profile.clone());
        let Some(title) = self.profile_title(&requested) else {
            self.set_notice(
                format!("El perfil `{requested}` no existe en config.toml."),
                true,
                cx,
            );
            return;
        };
        let pane = self.workspace.focus().pane;
        match self.workspace.new_tab(pane, project, &requested, &title) {
            Ok(new_tab) => {
                self.reveal_project(project);
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab {
            self.close_tab_id(tab, cx);
        }
    }

    fn close_tab_id(&mut self, tab: TabId, cx: &mut Context<'_, Self>) {
        let terminal = self.workspace.tab(tab).map(|tab| tab.terminal_id);
        if let Err(error) = self.workspace.close_tab(tab) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        if let Some(terminal) = terminal {
            self.terminals.remove(&terminal);
            self.pending_terminals.remove(&terminal);
            self.spawn_errors.remove(&terminal);
            self.history.remove(terminal);
        }
        if self.editing.as_ref().is_some_and(|edit| edit.tab == tab) {
            self.editing = None;
        }
        if let Some(focused) = self.workspace.focus().tab {
            self.pending_focus = Some(focused);
            self.ensure_terminal(focused, cx);
        }
        self.persist(cx);
        cx.notify();
    }

    fn next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.cycle_tab(1, window, cx);
    }

    fn previous_tab(&mut self, _: &PreviousTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.cycle_tab(-1, window, cx);
    }

    fn cycle_tab(&mut self, delta: isize, window: &mut Window, cx: &mut Context<'_, Self>) {
        let pane_id = self.workspace.focus().pane;
        let Some(pane) = self.workspace.pane(pane_id) else {
            return;
        };
        if pane.tabs.is_empty() {
            return;
        }
        let current = pane
            .active
            .and_then(|active| pane.tabs.iter().position(|tab| *tab == active))
            .unwrap_or(0) as isize;
        let len = pane.tabs.len() as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        let tab = pane.tabs[next];
        self.focus_tab(tab, window, cx);
    }

    fn move_tab_next_pane(
        &mut self,
        _: &MoveTabNextPane,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(tab) = self.workspace.focus().tab else {
            return;
        };
        let leaves = self.workspace.layout().leaves();
        if leaves.len() < 2 {
            return;
        }
        let current = self
            .workspace
            .pane_of_tab(tab)
            .and_then(|pane| leaves.iter().position(|id| *id == pane))
            .unwrap_or(0);
        let target = leaves[(current + 1) % leaves.len()];
        if let Err(error) = self.workspace.move_tab(tab, target, usize::MAX) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        self.focus_tab(tab, window, cx);
    }

    fn split_horizontal(
        &mut self,
        _: &SplitHorizontal,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.split(SplitAxis::Horizontal, window, cx);
    }

    fn split_vertical(
        &mut self,
        _: &SplitVertical,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.split(SplitAxis::Vertical, window, cx);
    }

    fn split(&mut self, axis: SplitAxis, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let project = self
            .workspace
            .current_project()
            .or_else(|| self.workspace.projects().first().map(|project| project.id));
        let Some(project) = project else {
            self.prompt_open_project(cx);
            return;
        };
        let pane = self.workspace.focus().pane;
        let Some((profile, title)) = self.default_profile() else {
            self.set_notice(
                "No hay un perfil por defecto válido en config.toml.",
                true,
                cx,
            );
            return;
        };
        let new_pane = match self.workspace.split_pane(pane, axis) {
            Ok(pane) => pane,
            Err(error) => {
                self.set_notice(error.to_string(), true, cx);
                return;
            }
        };
        match self.workspace.new_tab(new_pane, project, &profile, &title) {
            Ok(new_tab) => {
                self.reveal_project(project);
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn close_pane(&mut self, _: &ClosePane, window: &mut Window, cx: &mut Context<'_, Self>) {
        let pane = self.workspace.focus().pane;
        if self.pane_has_live_processes(pane, cx) {
            self.confirm(
                window,
                "Cerrar panel",
                "Hay procesos vivos en este panel. Cerrarlo terminará sus terminales.",
                PendingConfirm::ClosePane(pane),
                cx,
            );
        } else {
            self.close_pane_impl(pane, cx);
        }
    }

    fn close_pane_impl(&mut self, pane: PaneId, cx: &mut Context<'_, Self>) {
        let terminals: Vec<TerminalId> = self
            .workspace
            .pane(pane)
            .map(|pane| {
                pane.tabs
                    .iter()
                    .filter_map(|tab| self.workspace.tab(*tab).map(|tab| tab.terminal_id))
                    .collect()
            })
            .unwrap_or_default();
        match self.workspace.close_pane(pane) {
            Ok(_tabs) => {
                for terminal in terminals {
                    self.terminals.remove(&terminal);
                    self.pending_terminals.remove(&terminal);
                    self.spawn_errors.remove(&terminal);
                    self.history.remove(terminal);
                }
                if self.zoomed == Some(pane) {
                    self.zoomed = None;
                }
                if let Some(tab) = self.workspace.focus().tab {
                    self.pending_focus = Some(tab);
                    self.ensure_terminal(tab, cx);
                }
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn focus_next_pane(
        &mut self,
        _: &FocusNextPane,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.cycle_pane(1, window, cx);
    }

    fn focus_previous_pane(
        &mut self,
        _: &FocusPreviousPane,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.cycle_pane(-1, window, cx);
    }

    fn cycle_pane(&mut self, delta: isize, window: &mut Window, cx: &mut Context<'_, Self>) {
        let leaves = self.workspace.layout().leaves();
        if leaves.is_empty() {
            return;
        }
        let current = leaves
            .iter()
            .position(|id| *id == self.workspace.focus().pane)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(leaves.len() as isize) as usize;
        let pane = leaves[next];
        if let Err(error) = self.workspace.focus_pane(pane) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        if let Some(tab) = self.workspace.focus().tab {
            self.focus_tab(tab, window, cx);
        } else {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    fn rename_tab(&mut self, _: &RenameTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab {
            self.begin_rename(tab, window, cx);
        }
    }

    fn begin_rename(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let original = self.tab_title(tab, cx);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(original.clone()));
        let subscription =
            cx.subscribe(&input, |shell, input, event: &InputEvent, cx| match event {
                InputEvent::PressEnter { .. } => shell.finish_rename(input.clone(), true, cx),
                // Perder el foco equivale a confirmar: el clic que lo provoca
                // decide dónde queda el foco, así que aquí no se fuerza.
                InputEvent::Blur => shell.finish_rename(input.clone(), false, cx),
                InputEvent::Change | InputEvent::Focus => {}
            });
        self.editing = Some(TabEdit {
            tab,
            input,
            original,
            focused: false,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Cierra el editor inline y, si el texto cambió, renombra la pestaña.
    ///
    /// `refocus` solo se activa al confirmar con Enter: en un blur manda el
    /// destino del clic y robarle el foco sería un error.
    fn finish_rename(
        &mut self,
        input: Entity<InputState>,
        refocus: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        let value = input.read(cx).value();
        let title = value.as_ref().trim();
        // Un doble clic seguido de clic fuera no debe congelar el título
        // automático ni marcar la pestaña como renombrada.
        if title != edit.original.trim() {
            match self.workspace.rename_tab(edit.tab, title) {
                Ok(()) => self.persist(cx),
                Err(error) => self.set_notice(error.to_string(), true, cx),
            }
        }
        if refocus {
            self.pending_focus = self.workspace.focus().tab;
        }
        cx.notify();
    }

    fn cancel_rename(&mut self, cx: &mut Context<'_, Self>) {
        if self.editing.take().is_some() {
            self.pending_focus = self.workspace.focus().tab;
            cx.notify();
        }
    }

    fn duplicate_tab(&mut self, _: &DuplicateTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab {
            self.duplicate_tab_with_window(tab, window, cx);
        }
    }

    fn toggle_zoom(&mut self, _: &ToggleZoom, _: &mut Window, cx: &mut Context<'_, Self>) {
        let pane = self.workspace.focus().pane;
        self.zoomed = if self.zoomed == Some(pane) {
            None
        } else {
            Some(pane)
        };
        cx.notify();
    }

    fn restart_tab(&mut self, _: &RestartTab, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.workspace.focus().tab {
            self.restart_tab_id(tab, window, cx);
        }
    }

    fn restart_tab_id(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(model_tab) = self.workspace.tab(tab) else {
            return;
        };
        let terminal = model_tab.terminal_id;
        if self
            .terminals
            .get(&terminal)
            .is_some_and(|entry| entry.view.read(cx).is_running())
        {
            self.set_notice(
                "La terminal sigue viva; ciérrala o espera a que termine.",
                false,
                cx,
            );
            return;
        }
        self.terminals.remove(&terminal);
        self.pending_terminals.remove(&terminal);
        self.spawn_errors.remove(&terminal);
        self.spawn_terminal(tab, terminal, window, cx);
        self.persist(cx);
        cx.notify();
    }

    /// Cambia una pestaña sin perfil al predeterminado tras una elección
    /// explícita del usuario.
    fn use_default_profile(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some((profile, title)) = self.default_profile() else {
            self.set_notice(
                "No hay un perfil por defecto válido en config.toml.",
                true,
                cx,
            );
            return;
        };
        if let Err(error) = self.workspace.set_tab_profile(tab, profile, &title) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        let Some(terminal) = self.workspace.tab(tab).map(|tab| tab.terminal_id) else {
            return;
        };
        self.terminals.remove(&terminal);
        self.spawn_errors.remove(&terminal);
        self.spawn_terminal(tab, terminal, window, cx);
        self.persist(cx);
        cx.notify();
    }

    fn confirm(
        &mut self,
        window: &mut Window,
        title: &'static str,
        description: &'static str,
        action: PendingConfirm,
        cx: &mut Context<'_, Self>,
    ) {
        let this = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let this = this.clone();
            alert
                .title(title)
                .description(description)
                .confirm()
                .button_props(
                    axf_ui::gpui::component::dialog::DialogButtonProps::default()
                        .ok_text("Cerrar")
                        .cancel_text("Cancelar"),
                )
                .on_ok(move |_, _, cx| {
                    if let Some(this) = this.upgrade() {
                        let _ = this.update(cx, |shell, cx| {
                            shell.apply_confirm(action, cx);
                        });
                    }
                    true
                })
        });
    }

    fn apply_confirm(&mut self, action: PendingConfirm, cx: &mut Context<'_, Self>) {
        match action {
            PendingConfirm::ClosePane(pane) => self.close_pane_impl(pane, cx),
            PendingConfirm::RemoveProject(project) => {
                self.remove_project_impl(project, cx);
            }
        }
    }

    fn remove_project_checked(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let live_tabs = self.project_live_terminals(project, cx);
        if !live_tabs.is_empty() {
            self.confirm(
                window,
                "Quitar proyecto",
                "Hay procesos vivos en este proyecto. Quitarlo cerrará sus terminales.",
                PendingConfirm::RemoveProject(project),
                cx,
            );
            return;
        }
        self.remove_project_impl(project, cx);
    }

    fn remove_project_impl(&mut self, project: ProjectId, cx: &mut Context<'_, Self>) {
        let terminals: Vec<TerminalId> = self
            .workspace
            .project_tabs(project)
            .into_iter()
            .filter_map(|tab| self.workspace.tab(tab).map(|tab| tab.terminal_id))
            .collect();
        match self.workspace.remove_project(project) {
            Ok(_tabs) => {
                for terminal in terminals {
                    self.terminals.remove(&terminal);
                    self.pending_terminals.remove(&terminal);
                    self.spawn_errors.remove(&terminal);
                    self.history.remove(terminal);
                }
                if let Some(tab) = self.workspace.focus().tab {
                    self.pending_focus = Some(tab);
                    self.ensure_terminal(tab, cx);
                }
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    // --- Helpers de modelo/terminal -------------------------------------

    fn refresh_terminal_styles(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.terminals.is_empty() {
            return;
        }
        let style = terminal_style(cx);
        let views: Vec<Entity<TerminalView>> = self
            .terminals
            .values()
            .map(|entry| entry.view.clone())
            .collect();
        for view in views {
            view.update(cx, |view, cx| view.update_style(window, cx, style.clone()));
        }
    }

    fn spawn_terminal(
        &mut self,
        tab: TabId,
        terminal: TerminalId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let cwd = self
            .workspace
            .tab(tab)
            .and_then(|tab| self.workspace.project(tab.project_id))
            .map(|project| project.root.clone());
        self.spawn_terminal_with_cwd(
            tab,
            terminal,
            cwd,
            SpawnOptions {
                restore: None,
                focus: true,
            },
            window,
            cx,
        );
    }

    /// Restaura las terminales de las pestañas activas de cada panel visible.
    ///
    /// Las pestañas ocultas no arrancan hasta que se activan; así reabrir un
    /// workspace con muchas terminales no lanza todos los PTYs de golpe.
    fn restore_visible_terminals(&mut self, cx: &mut Context<'_, Self>) {
        let tabs: Vec<TabId> = self
            .workspace
            .layout()
            .leaves()
            .into_iter()
            .filter_map(|pane| self.workspace.pane(pane).and_then(|pane| pane.active))
            .collect();
        for tab in tabs {
            self.ensure_terminal(tab, cx);
        }
    }

    /// Arranca una pestaña restaurada, cargando su historial en segundo plano.
    fn ensure_terminal(&mut self, tab: TabId, cx: &mut Context<'_, Self>) {
        let Some(model_tab) = self.workspace.tab(tab) else {
            return;
        };
        let terminal = model_tab.terminal_id;
        let profile = model_tab.profile_id.clone();
        if self.terminals.contains_key(&terminal)
            || self.pending_terminals.contains(&terminal)
            || self.spawn_errors.contains_key(&terminal)
        {
            return;
        }
        // Un perfil ausente tiene su propia pantalla accionable; no se lanza
        // otro shell por sorpresa.
        if self.theme_host.profile(&profile).is_none() {
            return;
        }

        self.pending_terminals.insert(terminal);
        let history = self.history.clone();
        cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move { history.load(terminal) })
                .await;
            let _ = this.update_in(cx, |shell, window, cx| {
                shell.pending_terminals.remove(&terminal);
                if shell.workspace.tab(tab).is_none() || shell.terminals.contains_key(&terminal) {
                    return;
                }
                let (restore, restore_error, truncated) = match loaded {
                    Ok(Some(snapshot)) if !snapshot.is_empty() => {
                        let truncated = snapshot.truncated();
                        (Some(snapshot), None, truncated)
                    }
                    Ok(Some(_)) => {
                        shell.history.remove(terminal);
                        (None, None, false)
                    }
                    Ok(None) => (None, None, false),
                    Err(error) => {
                        if error.discardable() {
                            shell.history.remove(terminal);
                        }
                        (None, Some(error.to_string()), false)
                    }
                };
                let title = shell.tab_title(tab, cx);
                let cwd = shell
                    .workspace
                    .tab(tab)
                    .and_then(|tab| shell.workspace.project(tab.project_id))
                    .map(|project| project.root.clone());
                let focus = shell.workspace.focus().tab == Some(tab);
                shell.spawn_terminal_with_cwd(
                    tab,
                    terminal,
                    cwd,
                    SpawnOptions { restore, focus },
                    window,
                    cx,
                );
                // Si el shell tampoco arrancó, manda su error, no el del
                // historial.
                if !shell.terminals.contains_key(&terminal) {
                    return;
                }
                if let Some(error) = restore_error {
                    shell.set_notice(
                        format!(
                            "No se pudo restaurar el historial de «{title}»: {error}. \
                             Se abrió una terminal nueva con el mismo perfil."
                        ),
                        true,
                        cx,
                    );
                } else if truncated {
                    shell.set_notice(
                        format!(
                            "Historial de «{title}» restaurado parcialmente \
                             (límite de 2 MiB por terminal)."
                        ),
                        false,
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    fn spawn_terminal_with_cwd(
        &mut self,
        tab: TabId,
        terminal: TerminalId,
        cwd: Option<PathBuf>,
        options: SpawnOptions,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let SpawnOptions { restore, focus } = options;
        if self.terminals.contains_key(&terminal) {
            return;
        }
        let Some(cwd) = cwd else {
            self.set_notice("No se pudo resolver la carpeta del proyecto.", true, cx);
            return;
        };
        if !cwd.is_dir() {
            self.set_notice(
                format!("La carpeta del proyecto no existe: {}", cwd.display()),
                true,
                cx,
            );
            return;
        }
        let Some(profile_id) = self.workspace.tab(tab).map(|tab| tab.profile_id.clone()) else {
            self.set_notice("La pestaña ya no existe.", true, cx);
            return;
        };
        let prepared = match self.theme_host.prepare_profile(&profile_id) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.spawn_errors.insert(terminal, error.clone());
                self.set_notice(error, true, cx);
                if focus {
                    self.pending_focus = Some(tab);
                }
                cx.notify();
                return;
            }
        };
        self.spawn_errors.remove(&terminal);
        let style = terminal_style(cx);
        let view =
            cx.new(|cx| TerminalView::new(window, cx, style, cwd, &prepared.spec, restore, focus));
        let terminal_for_meta = terminal;
        let subscription = cx.observe(&view, move |shell, _, cx| {
            shell.sync_terminal_meta(cx);
            shell.drain_terminal_signals(terminal_for_meta, cx);
            cx.notify();
        });
        self.terminals.insert(
            terminal,
            TerminalEntry {
                view: view.clone(),
                _subscription: subscription,
            },
        );
        if let Some(resume) = self
            .workspace
            .take_tab_resume_agent(tab)
            .as_deref()
            .and_then(AgentKind::from_name)
        {
            let command = resume.resume_command();
            cx.spawn(async move |_, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let _ = view.update(cx, |view, _| view.send_command(command));
            })
            .detach();
            self.persist(cx);
        }
        if focus {
            self.pending_focus = Some(tab);
        }
    }

    fn default_profile(&self) -> Option<(ProfileId, String)> {
        self.theme_host
            .default_profile()
            .map(|profile| (profile.id.clone(), profile.name.clone()))
    }

    fn profile_title(&self, id: &ProfileId) -> Option<String> {
        self.theme_host.profile_name(id)
    }

    /// Icono de la pestaña: marca del agente dominante o perfil con fallback.
    fn tab_icon(&mut self, tab: TabId) -> ProfileIconSource {
        if let Some(aggregate) = self
            .workspace
            .tab(tab)
            .and_then(|tab| tab.agent_aggregate())
        {
            return ProfileIconSource::Svg(agent_icon(aggregate.dominant.kind));
        }
        let Some(profile_id) = self.workspace.tab(tab).map(|tab| tab.profile_id.clone()) else {
            return ProfileIconSource::fallback();
        };
        let source = self.theme_host.profile_icon(&profile_id);
        match source {
            ProfileIconSource::Image(path) => ProfileIconSource::Image(path),
            ProfileIconSource::Svg(_) => {
                let stem = self
                    .theme_host
                    .profile(&profile_id)
                    .map(|profile| profile.program_stem())
                    .filter(|stem| !stem.is_empty());
                ProfileIconSource::Svg(terminal_tab_icon(stem.as_deref()))
            }
        }
    }

    fn tab_title(&self, tab: TabId, cx: &mut Context<'_, Self>) -> String {
        let Some(model_tab) = self.workspace.tab(tab) else {
            return String::new();
        };
        let entry = self.terminals.get(&model_tab.terminal_id);
        let program = entry.and_then(|entry| entry.view.read(cx).program().map(str::to_owned));
        let live_title = entry.and_then(|entry| entry.view.read(cx).title().map(str::to_owned));
        let project = self
            .workspace
            .project(model_tab.project_id)
            .map(|project| project.name.as_str());
        resolve_tab_title(
            model_tab,
            live_title.as_deref(),
            program.as_deref(),
            project,
        )
    }

    fn terminal_cwd(&self, tab: TabId, cx: &mut Context<'_, Self>) -> Option<PathBuf> {
        let model_tab = self.workspace.tab(tab)?;
        if let Some(entry) = self.terminals.get(&model_tab.terminal_id)
            && let Some(cwd) = entry.view.read(cx).cwd()
        {
            return Some(cwd.clone());
        }
        self.workspace
            .project(model_tab.project_id)
            .map(|project| project.root.clone())
    }

    fn pane_has_live_processes(&self, pane: PaneId, cx: &mut Context<'_, Self>) -> bool {
        self.workspace
            .pane(pane)
            .map(|pane| {
                pane.tabs.iter().any(|tab| {
                    self.workspace
                        .tab(*tab)
                        .and_then(|tab| self.terminals.get(&tab.terminal_id))
                        .is_some_and(|entry| entry.view.read(cx).is_running())
                })
            })
            .unwrap_or(false)
    }

    fn project_live_terminals(&self, project: ProjectId, cx: &mut Context<'_, Self>) -> Vec<TabId> {
        self.workspace
            .project_tabs(project)
            .into_iter()
            .filter(|tab| {
                self.workspace
                    .tab(*tab)
                    .and_then(|tab| self.terminals.get(&tab.terminal_id))
                    .is_some_and(|entry| entry.view.read(cx).is_running())
            })
            .collect()
    }

    fn focus_tab(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Err(error) = self.workspace.focus_tab(tab) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        // Enfocar limpia atención/terminado/error y retira sesiones sin proceso.
        let _ = self
            .workspace
            .acknowledge_tab_agents(tab, SystemTime::now());
        self.reveal_focus();
        self.ensure_terminal(tab, cx);
        if let Some(model_tab) = self.workspace.tab(tab)
            && let Some(entry) = self.terminals.get(&model_tab.terminal_id)
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        } else {
            window.focus(&self.focus, cx);
        }
        self.persist(cx);
        cx.notify();
    }

    fn reveal_focus(&mut self) {
        if let Some(project) = self.workspace.current_project() {
            let _ = self.workspace.set_project_expanded(project, true);
        }
    }

    fn reveal_project(&mut self, project: ProjectId) {
        let _ = self.workspace.set_project_expanded(project, true);
    }

    fn sidebar_cursor_project(&self) -> Option<ProjectId> {
        match self.sidebar_cursor {
            Some(SidebarRow::Project(project)) => Some(project),
            Some(SidebarRow::Tab(tab)) => self.workspace.tab(tab).map(|tab| tab.project_id),
            None => None,
        }
    }

    fn set_notice(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<'_, Self>) {
        self.push_notice(text.into(), error, None, None, cx);
    }

    /// Añade un aviso con deduplicación opcional y cola acotada.
    fn push_notice(
        &mut self,
        text: String,
        error: bool,
        key: Option<String>,
        detail: Option<String>,
        cx: &mut Context<'_, Self>,
    ) {
        let now = Instant::now();
        if let Some(key) = &key {
            self.agents
                .notice_cooldowns
                .retain(|_, at| now.duration_since(*at) < NOTICE_COOLDOWN);
            if self.agents.notice_cooldowns.contains_key(key) {
                return;
            }
            self.agents.notice_cooldowns.insert(key.clone(), now);
            self.notices
                .retain(|notice| notice.key.as_ref() != Some(key));
        }

        self.notice_seq = self.notice_seq.wrapping_add(1);
        let seq = self.notice_seq;
        self.notices.push(Notice {
            text,
            detail,
            error,
            key,
            seq,
        });
        while self.notices.len() > MAX_NOTICES {
            self.notices.remove(0);
        }

        let seconds = if error { 8 } else { 4 };
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(seconds))
                .await;
            this.update(cx, |shell, cx| {
                let before = shell.notices.len();
                shell.notices.retain(|notice| notice.seq != seq);
                if shell.notices.len() != before {
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

/// Reconcilia las sesiones de una pestaña con el árbol de procesos.
fn reconcile_agents(
    sessions: &mut Vec<AgentSession>,
    detected: &[DetectedAgent],
    running: bool,
    exit: Option<axf_terminal::ExitInfo>,
    recent_output: bool,
    now: SystemTime,
    next_id: &mut AgentSessionId,
) -> (bool, AgentTransitions) {
    let before = sessions.clone();
    let mut transitions = Vec::new();

    if !running {
        let success = exit.is_some_and(|info| info.success);
        for session in sessions.iter_mut() {
            let transition = session.shell_exited(success, now);
            if transition.notify {
                transitions.push((session.id, transition));
            }
        }
        return (before != *sessions, transitions);
    }

    for session in sessions.iter_mut() {
        session.alive = false;
    }
    if detected.is_empty() {
        sessions.retain(|session| session.pid != 0);
    } else if let Some(agent) = detected.first() {
        sessions.truncate(1);
        if let Some(session) = sessions.first_mut() {
            if session.kind != agent.kind {
                let id = *next_id;
                *next_id = next_id.wrapping_add(1);
                let mut replacement = AgentSession::new(id, agent.kind, agent.pid, now);
                replacement.alive = true;
                if recent_output {
                    replacement.state = AgentState::Working;
                }
                *session = replacement;
            } else {
                session.pid = agent.pid;
                session.alive = true;
            }
        } else {
            let id = *next_id;
            *next_id = next_id.wrapping_add(1);
            let mut session = AgentSession::new(id, agent.kind, agent.pid, now);
            session.alive = true;
            if recent_output {
                session.state = AgentState::Working;
            }
            sessions.push(session);
        }
    }

    for session in sessions.iter_mut() {
        if session.pid == 0 {
            continue;
        }
        let transition = if !session.alive {
            session.process_gone(None, now)
        } else if !session.reported {
            if recent_output {
                session.apply_output_activity(now)
            } else {
                session.apply_quiescence(now)
            }
        } else {
            continue;
        };
        if transition.notify {
            transitions.push((session.id, transition));
        }
    }

    (before != *sessions, transitions)
}

/// Normaliza un reporte OSC 7501 al modelo de `axf_agents`.
fn agent_report_from_status(status: &ProgramStatus) -> AgentReport {
    let state = match status.state {
        ProgramState::Idle => AgentState::Idle,
        ProgramState::Working => AgentState::Working,
        ProgramState::Blocked => AgentState::NeedsAttention,
        ProgramState::Done => AgentState::Finished,
        ProgramState::Error => AgentState::Error,
        ProgramState::Clear => AgentState::Idle,
    };
    let mut report = if status.state == ProgramState::Clear {
        AgentReport::clear()
    } else {
        AgentReport::new(state)
    };
    report.attention = status
        .kind
        .as_deref()
        .and_then(AttentionKind::from_protocol);
    report.progress = status.progress;
    report.app = status.app.clone();
    report.title = status.title.clone();
    report.message = status.message.clone();
    report.id = status.id.clone();
    report
}

fn terminal_style(cx: &mut Context<'_, Shell>) -> TerminalStyle {
    let state = theme::state(cx);
    TerminalStyle {
        font_family: state.fonts.mono.clone(),
        fallbacks: state.fonts.mono_fallbacks.clone(),
        font_size: state.mono_font_size,
        line_height: (state.terminal_line_height > px(0.0)).then_some(state.terminal_line_height),
        palette: state.theme.terminal,
        background: state.theme.ui.background,
        // El borde del panel lo dibuja el shell según el foco; la terminal solo
        // usa este color para el flash de campana.
        border: None,
        // Todo tamaño deriva del tamaño base de la fuente (AGENTS.md).
        padding: state.metrics.panel_padding(),
    }
}

fn tab_status_from_state(state: AgentState) -> TabStatus {
    match state {
        AgentState::NeedsAttention => TabStatus::Attention,
        AgentState::Working => TabStatus::Working,
        AgentState::Error => TabStatus::Error,
        AgentState::Finished => TabStatus::Finished,
        AgentState::Idle => TabStatus::Idle,
        AgentState::Stopped => TabStatus::Stopped,
    }
}

/// Marca del agente para iconos de pestaña y sidebar.
fn agent_icon(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::OpenCode => icons::BRAND_OPENCODE,
        AgentKind::Copilot => icons::BRAND_COPILOT,
    }
}

/// Estado de agente más urgente de un conjunto, según la prioridad de presentación.
fn most_urgent_agent_state(states: impl IntoIterator<Item = AgentState>) -> Option<AgentState> {
    states.into_iter().max_by_key(|state| state.priority())
}

/// Duración compacta para la sección `Agentes`.
fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {:02}m", seconds / 3_600, (seconds % 3_600) / 60)
    }
}

fn terminal_tab_icon(program: Option<&str>) -> &'static str {
    let Some(program) = program else {
        return icons::TERMINAL;
    };
    let name = program_stem(program).to_ascii_lowercase();
    match name.as_str() {
        "pwsh" | "powershell" => icons::BRAND_POWERSHELL,
        _ => icons::TERMINAL,
    }
}

/// Nombre del ejecutable sin ruta ni extensión.
fn program_stem(program: &str) -> &str {
    std::path::Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
}

/// `true` cuando el título OSC es solo el nombre o la ruta del shell.
///
/// `pwsh` en Windows anuncia su ruta completa como título al arrancar; mostrarla
/// en la pestaña no aporta nada y tapa el nombre real de la terminal. Se mira el
/// último segmento para tolerar prefijos como `Administrador: `.
fn title_is_shell_path(title: &str, program: Option<&str>) -> bool {
    let Some(program) = program else {
        return false;
    };
    let segment = title
        .trim()
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .trim();
    program_stem(segment).eq_ignore_ascii_case(program_stem(program))
}

/// Resuelve el nombre visible de una pestaña.
///
/// El nombre manual manda sobre OSC y sobre el nombre del perfil; sin él, el
/// título vivo del PTY gana cuando aporta información, y después se cae al
/// nombre del perfil (`auto_title`), al título persistido o al proyecto.
fn resolve_tab_title(
    tab: &axf_workspace::Tab,
    live_title: Option<&str>,
    program: Option<&str>,
    project_name: Option<&str>,
) -> String {
    if tab.custom_title && !tab.title.is_empty() {
        return tab.title.clone();
    }
    if let Some(title) = live_title
        && !title.trim().is_empty()
        && !title_is_shell_path(title, program)
    {
        return title.to_owned();
    }
    if !tab.auto_title.is_empty() {
        return tab.auto_title.clone();
    }
    if !tab.title.is_empty() {
        return tab.title.clone();
    }
    project_name.unwrap_or("Terminal").to_owned()
}

fn sanitize_file_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "tema".into()
    } else {
        trimmed.to_string()
    }
}

fn tab_scroll_offset(current: Pixels, max: Pixels, delta: Pixels) -> Pixels {
    (current + delta).min(px(0.0)).max(-max)
}

/// Geometría del título central de la barra.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TitleBarTitle {
    max_width: Pixels,
    padding_left: Pixels,
    padding_right: Pixels,
}

/// Centra el título en la ventana, no en el hueco entre sidebar y controles.
///
/// La zona de arrastre empieza tras la columna de la sidebar y termina antes de
/// los controles de ventana. Como esas dos franjas no miden lo mismo, su centro
/// está `(sidebar - controles) / 2` a la derecha del centro real. El padding
/// derecho (o izquierdo, si algún día los controles fueran más anchos que la
/// sidebar) desplaza el contenido justo esa diferencia.
///
/// Si la sidebar ocupa más de media ventana, el centro real caería sobre su
/// chrome; en ese caso el título se centra en el hueco libre para no taparlo.
fn title_bar_title_geometry(
    viewport_width: Pixels,
    sidebar_width: Pixels,
    controls_width: Pixels,
    max_width: Pixels,
    padding: Pixels,
) -> TitleBarTitle {
    let free_width = viewport_width - sidebar_width - controls_width;
    let center = viewport_width * 0.5;
    let left_room = center - sidebar_width - padding;
    let right_room = viewport_width - controls_width - center - padding;

    if left_room > px(0.0) && right_room > px(0.0) {
        let room = left_room.min(right_room);
        let shift = sidebar_width - controls_width;
        let max_width = (room * 2.0).min(max_width);
        if shift >= px(0.0) {
            TitleBarTitle {
                max_width,
                padding_left: px(0.0),
                padding_right: shift,
            }
        } else {
            TitleBarTitle {
                max_width,
                padding_left: -shift,
                padding_right: px(0.0),
            }
        }
    } else {
        TitleBarTitle {
            max_width: (free_width - padding * 2.0).max(px(0.0)).min(max_width),
            padding_left: px(0.0),
            padding_right: px(0.0),
        }
    }
}

/// Ancho que ocupan los controles nativos de ventana en la barra.
fn window_controls_width(metrics: UiMetrics) -> Pixels {
    #[cfg(target_os = "windows")]
    {
        metrics.titlebar_control_width() * 3.0
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = metrics;
        px(0.0)
    }
}

fn split_key(path: &[Branch]) -> String {
    let mut key = String::with_capacity(path.len());
    for branch in path {
        key.push(match branch {
            Branch::First => '0',
            Branch::Second => '1',
        });
    }
    key
}

/// Controles de ventana en el lado y orden del SO.
///
/// En Windows van a la derecha: minimizar, maximizar/restaurar y cerrar. En
/// macOS los semáforos nativos ocupan ese papel; Linux queda fuera de v0.1.
fn window_controls(metrics: UiMetrics, ui: theme::UiTokens, maximized: bool) -> AnyElement {
    #[cfg(target_os = "windows")]
    {
        let maximize = if maximized {
            // `copy` (dos cuadrados superpuestos) lee como restaurar.
            icons::DUPLICATE
        } else {
            icons::WINDOW_MAXIMIZE
        };
        div()
            .flex()
            .items_center()
            .h_full()
            .flex_shrink_0()
            .child(window_control(
                metrics,
                ui,
                "title-minimize",
                icons::WINDOW_MINIMIZE,
                WindowControlArea::Min,
                false,
            ))
            .child(window_control(
                metrics,
                ui,
                "title-maximize",
                maximize,
                WindowControlArea::Max,
                false,
            ))
            .child(window_control(
                metrics,
                ui,
                "title-close",
                icons::CLOSE,
                WindowControlArea::Close,
                true,
            ))
            .into_any_element()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (metrics, ui, maximized);
        div().flex_shrink_0().into_any_element()
    }
}

/// Un control de ventana que delega en el hit-testing nativo.
#[cfg(target_os = "windows")]
fn window_control(
    metrics: UiMetrics,
    ui: theme::UiTokens,
    id: &'static str,
    icon: &'static str,
    area: WindowControlArea,
    destructive: bool,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(metrics.titlebar_control_width())
        .h_full()
        .flex_shrink_0()
        .window_control_area(area)
        .hover(move |this| {
            if destructive {
                // Tinte de peligro sin cambiar el icono: el SVG no hereda el
                // color del padre, y así funciona igual en temas claros y
                // oscuros.
                this.bg(ui.danger.opacity(0.2))
            } else {
                this.bg(ui.surface_raised)
            }
        })
        .child(
            svg()
                .path(icon)
                .size(metrics.titlebar_control_icon())
                .text_color(ui.muted),
        )
        .into_any_element()
}

// --- Render -------------------------------------------------------------

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self
            .theme_host
            .sync_display_scale(window.scale_factor(), cx)
        {
            cx.notify();
        }
        if self.style_revision != self.theme_host.revision {
            self.style_revision = self.theme_host.revision;
            self.refresh_terminal_styles(window, cx);
        }
        if self.theme_modal && !self.theme_modal_focus.is_focused(window) {
            window.focus(&self.theme_modal_focus, cx);
        }
        // Un drag cancelado o soltado fuera de un destino no debe dejar el
        // resaltado de drop pegado en pantalla.
        if !cx.has_active_drag() && (self.tab_drop.is_some() || self.tab_insert.is_some()) {
            self.tab_drop = None;
            self.tab_insert = None;
        }
        // Los paneles cerrados no deben dejar límites fantasma para el drag.
        self.body_bounds
            .borrow_mut()
            .retain(|pane, _| self.workspace.pane(*pane).is_some());
        if !self.close_hook_registered {
            self.close_hook_registered = true;
            let this = cx.entity().downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                if let Some(this) = this.upgrade() {
                    let _ = this.update(cx, |shell, cx| shell.request_quit(cx));
                }
                // La app cierra el proceso después de guardar.
                false
            });
        }

        if let Some(tab) = self.pending_focus.take()
            && let Some(model_tab) = self.workspace.tab(tab)
            && let Some(entry) = self.terminals.get(&model_tab.terminal_id)
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        }

        if let Some(pane) = self.zoomed
            && (!self.workspace.layout().contains(pane) || self.workspace.pane(pane).is_none())
        {
            self.zoomed = None;
        }

        if let Some(edit) = &mut self.editing
            && !edit.focused
        {
            let input = edit.input.clone();
            let handle = input.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
            input.update(cx, |input, cx| input.select_all(window, cx));
            edit.focused = true;
        }

        // El título de ventana sigue a la pestaña enfocada y marca la atención
        // pendiente de cualquier agente.
        let base_title = self
            .workspace
            .focus()
            .tab
            .map(|tab| format!("{} — Axfterm", self.tab_title(tab, cx)))
            .unwrap_or_else(|| "Axfterm".into());
        let title = if self.workspace.has_pending_attention() {
            format!("! {base_title}")
        } else {
            base_title
        };
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }

        let (background, foreground) = {
            let theme = cx.theme();
            (theme.background, theme.foreground)
        };

        let sidebar = self
            .workspace
            .sidebar()
            .visible
            .then(|| self.render_sidebar(window, cx));

        let content = if self.workspace.tab_count() == 0 {
            self.render_onboarding(window, cx)
        } else if let Some(pane) = self.zoomed {
            div()
                .size_full()
                .child(self.render_pane(pane, cx))
                .into_any_element()
        } else {
            let layout = self.workspace.layout().clone();
            self.render_layout(&layout, &[], cx)
        };

        let toast = self.render_toast(cx);
        let modal = self.theme_modal.then(|| self.render_theme_modal(cx));

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .key_context(WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::open_project))
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::move_tab_next_pane))
            .on_action(cx.listener(Self::split_horizontal))
            .on_action(cx.listener(Self::split_vertical))
            .on_action(cx.listener(Self::close_pane))
            .on_action(cx.listener(Self::focus_next_pane))
            .on_action(cx.listener(Self::focus_previous_pane))
            .on_action(cx.listener(Self::rename_tab))
            .on_action(cx.listener(Self::duplicate_tab))
            .on_action(cx.listener(Self::toggle_zoom))
            .on_action(cx.listener(Self::restart_tab))
            .on_action(cx.listener(Self::focus_sidebar))
            .on_action(cx.listener(Self::open_command_palette))
            .on_action(cx.listener(Self::toggle_sidebar_agents))
            .on_action(cx.listener(Self::show_theme_info))
            .on_action(cx.listener(Self::toggle_theme_variant))
            .on_action(cx.listener(Self::next_theme))
            .on_action(cx.listener(Self::previous_theme))
            .on_action(cx.listener(Self::reload_themes))
            .on_action(cx.listener(Self::open_themes_folder))
            .on_action(cx.listener(Self::import_theme))
            .on_action(cx.listener(Self::export_theme))
            .on_drop::<axf_ui::gpui::ExternalPaths>(cx.listener(
                |shell, paths: &axf_ui::gpui::ExternalPaths, window, cx| {
                    for path in &paths.0 {
                        if path.is_dir() {
                            shell.open_project_path(path.clone(), window, cx);
                        }
                    }
                },
            ))
            .child(self.render_title_bar(window, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .overflow_hidden()
                    .children(sidebar)
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .w_full()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .on_drag_move(cx.listener(
                                |shell, event: &axf_ui::gpui::DragMoveEvent<TabDrag>, _, cx| {
                                    shell.update_tab_drag_target(event, cx);
                                },
                            ))
                            .child(content),
                    ),
            )
            .children(toast)
            .children(modal)
    }
}

impl Shell {
    /// Barra de título propia.
    ///
    /// La marca y la franja central son zonas de arrastre nativas; el toggle
    /// de la sidebar queda fuera de ellas para que su clic no mueva la
    /// ventana. Los controles usan `WindowControlArea`, así que minimizar,
    /// maximizar, cerrar, el doble clic y los snap layouts siguen siendo los
    /// del sistema.
    fn render_title_bar(&mut self, window: &Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let sidebar = self.workspace.sidebar();
        let sidebar_visible = sidebar.visible;
        let sidebar_width = sidebar.width;
        let title_label = self.workspace.focus().tab.and_then(|tab_id| {
            let project_id = self.workspace.tab(tab_id)?.project_id;
            let project = self.workspace.project(project_id)?;
            let terminal = self.tab_title(tab_id, cx);
            Some(format!("{}/{}", project.name, terminal))
        });
        let title = title_bar_title_geometry(
            window.viewport_size().width,
            px(sidebar_width),
            window_controls_width(metrics),
            metrics.titlebar_title_max(),
            metrics.titlebar_padding(),
        );

        // Provisional hasta M8: el .exe aún no embebe un icono propio. Cuando
        // exista, este bloque pasa a mostrar el icono extraído del ejecutable.
        let brand = div()
            .id("title-brand")
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(metrics.titlebar_gap())
            .h_full()
            .window_control_area(WindowControlArea::Drag)
            // El arrastre llega como caption nativo; sin esto, el texto podría
            // iniciar una selección que sigue al puntero tras soltar.
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                axf_ui::gpui::base::GlobalState::suppress_text_selection(cx);
            })
            .child(
                Icon::new(icons::TERMINAL)
                    .size(IconSize::Custom(metrics.titlebar_icon()))
                    .color(ui.accent),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(metrics.titlebar_font_size())
                    .text_color(ui.text)
                    .child(APP_NAME),
            );

        let toggle = IconButton::new("title-sidebar", icons::SIDEBAR)
            .size_px(metrics.titlebar_button())
            .icon_size(IconSize::Custom(metrics.titlebar_button_icon()))
            .tooltip(if sidebar_visible {
                "Ocultar barra lateral (Ctrl+B)"
            } else {
                "Mostrar barra lateral (Ctrl+B)"
            })
            .on_click(cx.listener(|shell, _, _, cx| {
                let visible = shell.workspace.sidebar().visible;
                shell.set_sidebar_visible(!visible, cx);
            }));

        // Entrada provisional a ajustes: hasta M7 abre el modal de tema y
        // fuentes. Va pegado al toggle y dentro de la columna de la sidebar,
        // así que no invade la separación con el área central.
        let settings = IconButton::new("title-settings", icons::SETTINGS)
            .size_px(metrics.titlebar_button())
            .icon_size(IconSize::Custom(metrics.titlebar_button_icon()))
            .tooltip("Ajustes de tema (Ctrl+Alt+T)")
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_theme_modal(window, cx);
            }));

        // La sidebar y el toggle comparten siempre el mismo borde: si la
        // sidebar se oculta, el icono no se mueve de sitio.
        let left = div()
            .flex()
            .items_center()
            .gap(metrics.spacing_xs())
            .h_full()
            .flex_shrink_0()
            .px(metrics.titlebar_padding())
            .w(px(sidebar_width))
            .child(brand)
            .child(settings)
            .child(toggle);

        let drag = div()
            .id("title-drag")
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .h_full()
            .min_w_0()
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                axf_ui::gpui::base::GlobalState::suppress_text_selection(cx);
            })
            // El título se centra en la ventana, no en el hueco: la franja de
            // arrastre compensa la diferencia entre la columna de la sidebar
            // y los controles de ventana.
            .when(title.padding_left > px(0.0), |this| {
                this.pl(title.padding_left)
            })
            .when(title.padding_right > px(0.0), |this| {
                this.pr(title.padding_right)
            })
            .when_some(title_label, |this, label| {
                this.child(
                    div()
                        .min_w_0()
                        .max_w(title.max_width)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(metrics.titlebar_font_size())
                        .text_color(ui.muted)
                        .child(label),
                )
            });

        let controls = window_controls(metrics, ui, window.is_maximized());

        div()
            .flex()
            .items_center()
            .h(metrics.titlebar_height())
            .w_full()
            .flex_shrink_0()
            .bg(ui.surface)
            .border_b_1()
            .border_color(ui.border)
            .child(left)
            .child(drag)
            .child(controls)
            .into_any_element()
    }

    fn render_onboarding(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let metrics = theme::metrics(cx);
        let empty_project = self.workspace.projects().is_empty();
        let message = if empty_project {
            "Sin proyectos todavía"
        } else {
            "Sin terminales abiertas"
        };
        let detail = if empty_project {
            "Registra una carpeta para agrupar sus terminales en la sidebar."
        } else {
            "Abre una terminal en el proyecto actual o registra otra carpeta."
        };

        EmptyState::new(icons::FOLDER, message, detail)
            .action(
                div()
                    .flex()
                    .gap(metrics.spacing_sm())
                    .child(
                        Button::new("onboarding-open", "Abrir carpeta")
                            .primary()
                            .icon(icons::FOLDER_OPEN)
                            .on_click(cx.listener(|shell, _, window, cx| {
                                shell.open_project_path_dialog(window, cx);
                            })),
                    )
                    .when(!empty_project, |this| {
                        this.child(
                            Button::new("onboarding-new", "Nueva terminal")
                                .icon(icons::PLUS)
                                .on_click(cx.listener(|shell, _, window, cx| {
                                    shell.new_tab_for(None, None, window, cx);
                                })),
                        )
                    }),
            )
            .into_any_element()
    }

    fn open_project_path_dialog(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        self.prompt_open_project(cx);
    }

    fn render_layout(
        &mut self,
        node: &PaneTree,
        path: &[Branch],
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        match node {
            PaneTree::Leaf(pane) => self.render_pane(*pane, cx),
            PaneTree::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let axis = *axis;
                let ratio = *ratio;
                let key = split_key(path);
                let mut first_path = path.to_vec();
                first_path.push(Branch::First);
                let mut second_path = path.to_vec();
                second_path.push(Branch::Second);
                let first_el = self.render_layout(first, &first_path, cx);
                let second_el = self.render_layout(second, &second_path, cx);

                let bounds = Rc::clone(&self.split_bounds);
                let prepaint_key = key.clone();
                let divider_key = key.clone();
                let divider_path = path.to_vec();
                let first_wrap = div()
                    .map(|this| match axis {
                        SplitAxis::Horizontal => this.w(relative(ratio)).flex_shrink_0(),
                        SplitAxis::Vertical => this.h(relative(ratio)).flex_shrink_0(),
                    })
                    .min_w_0()
                    .min_h_0()
                    .child(first_el);
                let second_wrap = div().flex_1().min_w_0().min_h_0().child(second_el);

                let splitter = UiMetrics::new(cx.theme().font_size).splitter();
                let divider = div()
                    .id(format!("split-{divider_key}"))
                    .flex_shrink_0()
                    .map(|this| match axis {
                        SplitAxis::Horizontal => this.w(splitter).h_full(),
                        SplitAxis::Vertical => this.h(splitter).w_full(),
                    })
                    .bg(cx.theme().border)
                    .hover(|this| this.bg(cx.theme().primary))
                    .cursor_pointer()
                    .on_drag(
                        SplitDrag {
                            path: divider_path.clone(),
                            axis,
                            key: divider_key.clone(),
                        },
                        |_, _, _, cx| cx.new(|_| DragGhost::new("Redimensionar")),
                    )
                    .on_drag_move(cx.listener(
                        move |shell, event: &axf_ui::gpui::DragMoveEvent<SplitDrag>, _, cx| {
                            let bounds = shell
                                .split_bounds
                                .borrow()
                                .get(&event.drag(cx).key)
                                .copied();
                            if let Some(bounds) = bounds {
                                let ratio = match event.drag(cx).axis {
                                    SplitAxis::Horizontal => {
                                        let width: f32 = bounds.size.width.into();
                                        if width > 0.0 {
                                            (event.event.position.x - bounds.left()) / px(width)
                                        } else {
                                            0.5
                                        }
                                    }
                                    SplitAxis::Vertical => {
                                        let height: f32 = bounds.size.height.into();
                                        if height > 0.0 {
                                            (event.event.position.y - bounds.top()) / px(height)
                                        } else {
                                            0.5
                                        }
                                    }
                                };
                                if shell.workspace.resize_split(&event.drag(cx).path, ratio) {
                                    cx.notify();
                                }
                            }
                        },
                    ))
                    .on_click(cx.listener({
                        let path = path.to_vec();
                        move |shell, event: &ClickEvent, _, cx| {
                            if event.click_count() == 2 && shell.workspace.resize_split(&path, 0.5)
                            {
                                shell.persist(cx);
                                cx.notify();
                            }
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|shell, _, _, cx| shell.persist(cx)),
                    );

                let container = div()
                    .flex()
                    .size_full()
                    .map(|this| match axis {
                        SplitAxis::Horizontal => this.flex_row(),
                        SplitAxis::Vertical => this.flex_col(),
                    })
                    .on_prepaint(move |bounds_value, _, _| {
                        bounds
                            .borrow_mut()
                            .insert(prepaint_key.clone(), bounds_value);
                    })
                    .child(first_wrap)
                    .child(divider)
                    .child(second_wrap);
                container.into_any_element()
            }
        }
    }

    fn sidebar_rows(&self) -> Vec<SidebarRow> {
        sidebar_rows_from_groups(&sidebar_view(&self.workspace, self.sidebar_section))
    }

    fn render_sidebar(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let width = self.workspace.sidebar().width;
        let cursor = self.sidebar_cursor;
        let sidebar_focused = self.sidebar_focus.is_focused(window);
        let groups = sidebar_view(&self.workspace, self.sidebar_section);
        let no_projects = self.workspace.projects().is_empty();
        let empty = groups.is_empty();

        let at_limit = self.workspace.tab_count() >= MAX_TABS;
        let mut new_terminal = Button::new("sidebar-new-terminal", "Nueva terminal")
            .icon(icons::PLUS)
            .secondary()
            .full_width()
            .disabled(at_limit)
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.new_tab_for(None, None, window, cx);
            }));
        if at_limit {
            new_terminal = new_terminal.tooltip("Límite de 32 terminales; cierra una pestaña.");
        }
        if width >= f32::from(metrics.sidebar_actions_compact_width()) {
            new_terminal = new_terminal.shortcut(keys::format_chord(keys::NEW_TAB));
        }

        let open_project = IconButton::new("sidebar-open-project", icons::FOLDER_OPEN)
            .size(ControlSize::Md)
            .icon_size(IconSize::Md)
            .secondary()
            .tooltip(format!(
                "Abrir proyecto… ({})",
                keys::format_chord(keys::OPEN_PROJECT)
            ))
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.prompt_open_project(cx);
            }));

        let actions = div()
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .child(div().flex_1().min_w_0().child(new_terminal))
            .child(open_project);

        let search = div()
            .id("sidebar-search")
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .h(metrics.control_height_md())
            .px(metrics.spacing_sm())
            .rounded(metrics.radius_md())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface_raised.opacity(0.5))
            .cursor_pointer()
            .hover(move |this| this.bg(ui.surface_raised))
            .tooltip(text_tooltip(format!(
                "Buscar o ejecutar un comando ({})",
                keys::format_chord(keys::COMMAND_PALETTE)
            )))
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.show_palette_placeholder(cx);
            }))
            .child(Icon::new(icons::SEARCH).size(IconSize::Sm).color(ui.muted))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(metrics.font_size * 0.75)
                    .child("Buscar o ejecutar un comando…"),
            );

        let palette_hint = self.palette_hint.then(|| {
            div()
                .flex()
                .items_center()
                .gap(metrics.spacing_xs())
                .px(metrics.spacing_xs())
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child(
                    Icon::new(icons::INFO_CIRCLE)
                        .size(IconSize::Xs)
                        .color(ui.info),
                )
                .child("Paleta de comandos: próximamente.")
        });

        let sections = div()
            .flex()
            .items_center()
            .h(metrics.control_height_md())
            .p(metrics.hairline())
            .rounded(metrics.radius_md())
            .border_1()
            .border_color(ui.border)
            .bg(ui.background)
            .child(self.render_sidebar_section_segment(0, "Todas", SidebarSection::All, cx))
            .child(self.render_sidebar_section_segment(1, "Agentes", SidebarSection::Agents, cx));

        let top = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_sm())
            .p(metrics.spacing_sm())
            .child(actions)
            .child(search)
            .when_some(palette_hint, |this, hint| this.child(hint))
            .child(sections);

        let mut blocks = Vec::with_capacity(groups.len());
        for group in &groups {
            blocks.push(self.render_sidebar_project(group, cursor, sidebar_focused, cx));
        }
        let sidebar_empty = empty.then(|| self.render_sidebar_empty(no_projects, cx));

        let list = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .px(metrics.spacing_sm())
            .pt(metrics.spacing_xs())
            .pb(metrics.spacing_sm())
            .gap(metrics.spacing_sm())
            .children(blocks)
            .when_some(sidebar_empty, |this, empty| this.child(empty));

        let list_section = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .mb(metrics.spacing_xs())
            .border_t_1()
            .border_b_1()
            .border_color(ui.border)
            .child(list);

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(width))
            .flex_shrink_0()
            .border_r_1()
            .border_color(ui.border)
            .bg(ui.surface)
            .text_color(ui.text)
            .key_context(SIDEBAR_CONTEXT)
            .track_focus(&self.sidebar_focus)
            .on_key_down(cx.listener(Self::sidebar_key))
            .child(top)
            .child(list_section)
            .into_any_element()
    }

    fn render_sidebar_section_segment(
        &self,
        index: usize,
        label: &'static str,
        section: SidebarSection,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let selected = self.sidebar_section == section;
        let radius = metrics.radius_sm();
        let tooltip = (section == SidebarSection::Agents).then(|| {
            format!(
                "Sesiones de agente ({})",
                keys::format_chord(keys::TOGGLE_SIDEBAR_AGENTS)
            )
        });

        let mut segment = div()
            .id(("sidebar-section", index))
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .h_full()
            .cursor_pointer()
            .text_size(metrics.font_size * 0.75)
            .text_color(if selected { ui.text } else { ui.muted })
            .when(selected, |this| this.bg(ui.surface_raised))
            .when(!selected, move |this| {
                this.hover(move |this| this.text_color(ui.text))
            })
            .when_some(tooltip, |this, tooltip| this.tooltip(text_tooltip(tooltip)))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.set_sidebar_section(section, cx);
            }))
            .child(label);

        if index == 0 {
            segment = segment.rounded_tl(radius).rounded_bl(radius);
        } else {
            segment = segment.rounded_tr(radius).rounded_br(radius);
        }
        segment.into_any_element()
    }

    fn render_sidebar_empty(&self, no_projects: bool, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let (icon, title, detail) = if no_projects {
            (
                icons::FOLDER,
                "Sin proyectos",
                "Abre una carpeta para agrupar sus terminales.",
            )
        } else {
            (
                icons::PLAYER_PLAY,
                "Sin agentes activos",
                "Las terminales con opencode o Copilot aparecerán aquí.",
            )
        };

        let mut empty = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(metrics.spacing_sm())
            .px(metrics.spacing_sm())
            .py(metrics.spacing_xl())
            .text_size(metrics.font_size * 0.75)
            .text_color(ui.muted)
            .child(Icon::new(icon).size(IconSize::Lg).color(ui.muted))
            .child(
                div()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(ui.text)
                    .child(title),
            )
            .child(div().text_center().child(detail));

        if no_projects {
            empty = empty.child(
                Button::new("sidebar-empty-open", "Abrir proyecto…")
                    .small()
                    .primary()
                    .icon(icons::FOLDER_OPEN)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.prompt_open_project(cx);
                    })),
            );
        }
        empty.into_any_element()
    }

    fn render_sidebar_project(
        &mut self,
        group: &SidebarGroup,
        cursor: Option<SidebarRow>,
        sidebar_focused: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(project) = self.workspace.project(group.project).cloned() else {
            return div().into_any_element();
        };
        let project_id = project.id;
        let missing = !project.root.is_dir();
        let selected = matches!(cursor, Some(SidebarRow::Project(id)) if id == project_id);
        let can_collapse = self.sidebar_section == SidebarSection::All;
        let expanded = self.workspace.expanded_projects().contains(&project_id);
        let toggle_label = if expanded { "Contraer" } else { "Expandir" };
        let chevron = can_collapse.then_some(if group.show_tabs {
            icons::CHEVRON_DOWN
        } else {
            icons::CHEVRON_RIGHT
        });
        let menu_this = cx.entity().downgrade();
        let indent = metrics.spacing_xs() + metrics.sidebar_icon_lane() + metrics.spacing_sm();
        // Con el proyecto plegado sus terminales no se ven: el encabezado
        // muestra el estado más urgente de sus agentes para que una petición
        // de atención no pase desapercibida.
        let collapsed_status = (!group.show_tabs)
            .then(|| {
                most_urgent_agent_state(group.tabs.iter().filter_map(|tab| {
                    self.workspace
                        .tab(*tab)?
                        .agent_aggregate()
                        .map(|aggregate| aggregate.state)
                }))
            })
            .flatten()
            .map(tab_status_from_state);
        let collapsed_glyph = collapsed_status.map(|status| {
            Icon::new(status.icon())
                .size(IconSize::Sm)
                .color(status.tone().color(ui))
                .spin(status.is_spinning())
        });

        let mut header = div()
            .id(("project", project_id.get()))
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .h(metrics.row_height())
            .px(metrics.spacing_xs())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(transparent_black())
            .cursor_pointer()
            .text_color(if missing { ui.danger } else { ui.muted })
            .tooltip(text_tooltip(project.root.display().to_string()))
            .child(
                div()
                    .w(metrics.sidebar_icon_lane())
                    .flex()
                    .justify_center()
                    .when_some(chevron, |this, chevron| {
                        this.child(Icon::new(chevron).size(IconSize::Sm).color(ui.muted))
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(metrics.font_size * 0.75)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(project.name.clone()),
            )
            .when_some(collapsed_glyph, |this, glyph| this.child(glyph))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(metrics.control_height_xs())
                    .px(metrics.spacing_xs())
                    .rounded(metrics.radius_full())
                    .bg(ui.surface_raised)
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(group.tabs.len().to_string()),
            )
            .child(
                IconButton::new(format!("project-new-{}", project_id.get()), icons::PLUS)
                    .size(ControlSize::Sm)
                    .icon_size(IconSize::Sm)
                    .tooltip("Nueva terminal")
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        cx.stop_propagation();
                        shell.new_tab_for(Some(project_id), None, window, cx);
                    })),
            );

        if selected && sidebar_focused {
            header = header.bg(ui.surface_raised).border_color(ui.focus);
        } else {
            header = header.hover(move |this| this.bg(ui.surface_raised.opacity(0.55)));
        }

        let header = header
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.activate_project_row(project_id, window, cx);
            }))
            .context_menu(move |menu, _, _| {
                let menu_this = menu_this.clone();
                let menu = menu.item(PopupMenuItem::new("Nueva terminal").on_click({
                    let this = menu_this.clone();
                    move |_, window, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                shell.new_tab_for(Some(project_id), None, window, cx);
                            });
                        }
                    }
                }));
                let menu = if can_collapse {
                    menu.item(PopupMenuItem::new(toggle_label).on_click({
                        let this = menu_this.clone();
                        move |_, _, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    let _ =
                                        shell.workspace.set_project_expanded(project_id, !expanded);
                                    shell.persist(cx);
                                    cx.notify();
                                });
                            }
                        }
                    }))
                } else {
                    menu
                };
                menu.separator()
                    .item(PopupMenuItem::new("Quitar proyecto").on_click({
                        let this = menu_this.clone();
                        move |_, window, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    shell.remove_project_checked(project_id, window, cx);
                                });
                            }
                        }
                    }))
            });

        let mut block = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_xxs())
            .child(header);
        if group.show_tabs {
            if missing {
                block = block.child(
                    div()
                        .pl(indent)
                        .pr(metrics.spacing_xs())
                        .py(metrics.spacing_xs())
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.danger)
                        .child("Carpeta no encontrada"),
                );
            } else if group.tabs.is_empty() {
                block = block.child(
                    div()
                        .pl(indent)
                        .pr(metrics.spacing_xs())
                        .py(metrics.spacing_xs())
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child("Sin terminales"),
                );
            }
            for tab in &group.tabs {
                block = block.child(self.render_sidebar_tab(
                    *tab,
                    project_id,
                    cursor,
                    sidebar_focused,
                    cx,
                ));
            }
        }
        block.into_any_element()
    }

    fn render_sidebar_tab(
        &mut self,
        tab: TabId,
        project: ProjectId,
        cursor: Option<SidebarRow>,
        sidebar_focused: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(model_tab) = self.workspace.tab(tab).cloned() else {
            return div().into_any_element();
        };
        let title = self.tab_title(tab, cx);
        let focused = self.workspace.focus().tab == Some(tab);
        let selected = cursor == Some(SidebarRow::Tab(tab));
        let missing = self
            .workspace
            .project(project)
            .is_some_and(|project| !project.root.is_dir());
        let aggregate = self
            .workspace
            .tab(tab)
            .and_then(|tab| tab.agent_aggregate());
        let agent_status = aggregate
            .as_ref()
            .map(|aggregate| tab_status_from_state(aggregate.state));
        let icon = self.tab_icon(tab);
        let profile = self
            .profile_title(&model_tab.profile_id)
            .unwrap_or_else(|| model_tab.profile_id.to_string());
        let live_cwd = self
            .terminals
            .get(&model_tab.terminal_id)
            .and_then(|entry| entry.view.read(cx).cwd().cloned());
        let project_root = self
            .workspace
            .project(project)
            .map(|project| project.root.clone());
        let context = match (project_root.as_deref(), live_cwd.as_deref()) {
            (Some(root), Some(cwd)) => context_path(root, cwd),
            _ => None,
        };
        let running = self
            .terminals
            .get(&model_tab.terminal_id)
            .is_some_and(|entry| entry.view.read(cx).is_running());
        let pending = self.pending_terminals.contains(&model_tab.terminal_id);
        let spawn_error = self.spawn_errors.contains_key(&model_tab.terminal_id);
        let state_label = if pending {
            "arrancando"
        } else if running {
            "en ejecución"
        } else {
            "detenido"
        };
        let mut meta_parts = Vec::new();
        if spawn_error {
            if profile != title {
                meta_parts.push(profile.clone());
            }
            meta_parts.push("error de arranque".to_owned());
        } else if let Some(aggregate) = &aggregate {
            let session = &aggregate.dominant;
            let status = tab_status_from_state(session.state);
            meta_parts.push(session.kind.label().to_owned());
            if let Some(context) = context {
                meta_parts.push(context);
            }
            if let Some(progress) = session.progress {
                meta_parts.push(format!("{progress}%"));
            }
            if let Some(message) = &session.message {
                meta_parts.push(message.clone());
            } else {
                meta_parts.push(status.label().to_ascii_lowercase());
            }
        } else {
            if profile != title {
                meta_parts.push(profile.clone());
            }
            if let Some(context) = context {
                meta_parts.push(context);
            } else {
                meta_parts.push(state_label.to_owned());
            }
        }
        let meta = meta_parts.join(" · ");
        let status = if spawn_error {
            Some((
                icons::ALERT_TRIANGLE,
                ui.danger,
                "Error de arranque".to_owned(),
            ))
        } else if aggregate.is_some() {
            // El agente se muestra como glifo en el hueco de estado (a la
            // derecha), así que el icono de la terminal queda sin superponer.
            None
        } else if pending {
            Some((icons::LOADER, ui.info, "Arrancando".to_owned()))
        } else if !running {
            Some((icons::STOP, ui.muted, "Detenido".to_owned()))
        } else {
            None
        };
        let title_color = if missing { ui.danger } else { ui.text };
        let meta_color = if missing || spawn_error {
            ui.danger
        } else {
            ui.muted
        };
        let icon_color = if focused || (selected && sidebar_focused) {
            ui.accent
        } else if aggregate.is_some() {
            ui.text
        } else {
            ui.muted
        };
        let row_tooltip = aggregate.as_ref().map(|aggregate| {
            let session = &aggregate.dominant;
            let status = tab_status_from_state(session.state);
            let now = SystemTime::now();
            let detail = session.message.as_deref().unwrap_or_else(|| status.label());
            format!(
                "{} · {} · {}\n{}",
                session.kind.label(),
                status.label(),
                format_duration(session.duration(now)),
                detail
            )
        });
        let menu_this = cx.entity().downgrade();
        let hover_group = format!("sidebar-tab-{}", tab.get());

        let mut row = div()
            .id(("sidebar-tab", tab.get()))
            .group(hover_group.clone())
            .relative()
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .h(metrics.sidebar_tab_height())
            .px(metrics.spacing_xs())
            .rounded(metrics.radius_md())
            .border_1()
            .border_color(transparent_black())
            .cursor_pointer()
            .when_some(row_tooltip, |this, tooltip| {
                this.tooltip(text_tooltip(tooltip))
            })
            .child(
                div()
                    .w(metrics.sidebar_icon_lane())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(metrics.sidebar_tab_icon())
                            .child(
                                ProfileIcon::new(icon)
                                    .size(IconSize::Custom(metrics.sidebar_tab_icon()))
                                    .color(icon_color),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.875)
                            .text_color(title_color)
                            .child(title),
                    )
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(meta_color)
                            .child(meta),
                    ),
            );

        let mut slot = div()
            .w(metrics.control_height_sm())
            .h(metrics.control_height_sm())
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0();

        if let Some(agent) = agent_status {
            slot = slot.child(
                div()
                    .id(format!("sidebar-agent-status-{}", tab.get()))
                    .group_hover(hover_group.clone(), |this| this.opacity(0.0))
                    .tooltip(text_tooltip(agent.label()))
                    .child(
                        Icon::new(agent.icon())
                            .size(IconSize::Md)
                            .color(agent.tone().color(ui))
                            .spin(agent.is_spinning()),
                    ),
            );
        }
        if let Some((status_icon, status_color, status_label)) = status {
            slot = slot.child(
                div()
                    .id(format!("sidebar-status-{}", tab.get()))
                    .group_hover(hover_group.clone(), |this| this.opacity(0.0))
                    .tooltip(text_tooltip(status_label))
                    .child(
                        Icon::new(status_icon)
                            .size(IconSize::Md)
                            .color(status_color),
                    ),
            );
        }

        slot = slot.child(
            div()
                .opacity(0.0)
                .group_hover(hover_group, |this| this.opacity(1.0))
                .child(
                    IconButton::new(format!("sidebar-close-{}", tab.get()), icons::CLOSE)
                        .size(ControlSize::Xs)
                        .icon_size(IconSize::Sm)
                        .tooltip("Cerrar terminal")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            cx.stop_propagation();
                            shell.close_tab_id(tab, cx);
                        })),
                ),
        );
        row = row.child(slot);
        if focused {
            row = row.bg(ui.surface_raised).child(
                div()
                    .absolute()
                    .left_0()
                    .top(metrics.spacing_xs())
                    .bottom(metrics.spacing_xs())
                    .w(metrics.tab_indicator())
                    .rounded(metrics.radius_full())
                    .bg(ui.accent),
            );
        }
        if selected && sidebar_focused {
            row = row.bg(ui.surface_raised).border_color(ui.focus);
        }
        if !focused && !(selected && sidebar_focused) {
            row = row.hover(move |this| this.bg(ui.surface_raised.opacity(0.55)));
        }

        let has_agent = aggregate.is_some();
        let row = row
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.activate_sidebar_tab(tab, window, cx);
            }))
            .context_menu(move |menu, _, _| {
                let menu = menu
                    .item(PopupMenuItem::new("Renombrar").on_click({
                        let this = menu_this.clone();
                        move |_, window, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    shell.begin_rename(tab, window, cx);
                                });
                            }
                        }
                    }))
                    .item(PopupMenuItem::new("Duplicar").on_click({
                        let this = menu_this.clone();
                        move |_, window, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| {
                                    shell.duplicate_tab_with_window(tab, window, cx);
                                });
                            }
                        }
                    }));
                let menu = if has_agent {
                    menu.separator()
                        .item(PopupMenuItem::new("Reconocer agente").on_click({
                            let this = menu_this.clone();
                            move |_, _, cx| {
                                if let Some(this) = this.upgrade() {
                                    let _ = this.update(cx, |shell, cx| {
                                        if shell
                                            .workspace
                                            .acknowledge_tab_agents(tab, SystemTime::now())
                                        {
                                            shell.persist(cx);
                                            cx.notify();
                                        }
                                    });
                                }
                            }
                        }))
                        .item(PopupMenuItem::new("Marcar atención").on_click({
                            let this = menu_this.clone();
                            move |_, _, cx| {
                                if let Some(this) = this.upgrade() {
                                    let _ = this.update(cx, |shell, cx| {
                                        let now = SystemTime::now();
                                        shell.workspace.update_tab_agents(tab, |sessions| {
                                            if let Some(session) = sessions.first_mut() {
                                                session.apply_attention(
                                                    Some(AttentionKind::Permission),
                                                    Some("Marcado manualmente".into()),
                                                    now,
                                                );
                                            }
                                        });
                                        cx.notify();
                                    });
                                }
                            }
                        }))
                } else {
                    menu
                };
                menu.separator()
                    .item(PopupMenuItem::new("Cerrar pestaña").on_click({
                        let this = menu_this.clone();
                        move |_, _, cx| {
                            if let Some(this) = this.upgrade() {
                                let _ = this.update(cx, |shell, cx| shell.close_tab_id(tab, cx));
                            }
                        }
                    }))
            });

        row.into_any_element()
    }

    fn activate_project_row(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.sidebar_focus, cx);
        self.palette_hint = false;
        self.sidebar_cursor = Some(SidebarRow::Project(project));
        if self.sidebar_section == SidebarSection::All {
            let expanded = self.workspace.expanded_projects().contains(&project);
            let _ = self.workspace.set_project_expanded(project, !expanded);
            self.persist(cx);
        }
        cx.notify();
    }

    fn activate_sidebar_tab(
        &mut self,
        tab: TabId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.sidebar_focus, cx);
        self.palette_hint = false;
        self.sidebar_cursor = Some(SidebarRow::Tab(tab));
        self.focus_tab(tab, window, cx);
    }

    fn sidebar_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let rows = self.sidebar_rows();
        if rows.is_empty() {
            return;
        }
        let current = self
            .sidebar_cursor
            .and_then(|cursor| rows.iter().position(|row| *row == cursor))
            .unwrap_or(0);
        let mut handled = true;
        match event.keystroke.key.as_str() {
            "down" => {
                let next = (current + 1).min(rows.len() - 1);
                self.sidebar_cursor = Some(rows[next]);
            }
            "up" => {
                let next = current.saturating_sub(1);
                self.sidebar_cursor = Some(rows[next]);
            }
            "home" => self.sidebar_cursor = Some(rows[0]),
            "end" => self.sidebar_cursor = Some(rows[rows.len() - 1]),
            "right" => match rows[current] {
                SidebarRow::Project(project) => {
                    let expanded = self.workspace.expanded_projects().contains(&project);
                    if self.sidebar_section == SidebarSection::All && !expanded {
                        let _ = self.workspace.set_project_expanded(project, true);
                        self.persist(cx);
                    } else if current + 1 < rows.len() {
                        self.sidebar_cursor = Some(rows[current + 1]);
                    }
                }
                SidebarRow::Tab(_) => {}
            },
            "left" => match rows[current] {
                SidebarRow::Project(project) => {
                    if self.sidebar_section == SidebarSection::All {
                        let _ = self.workspace.set_project_expanded(project, false);
                        self.persist(cx);
                    }
                }
                SidebarRow::Tab(tab) => {
                    if let Some(project) = self.workspace.tab(tab).map(|tab| tab.project_id)
                        && let Some(index) = rows
                            .iter()
                            .position(|row| *row == SidebarRow::Project(project))
                    {
                        self.sidebar_cursor = Some(rows[index]);
                    }
                }
            },
            "enter" => match rows[current] {
                SidebarRow::Project(project) => {
                    self.activate_project_row(project, window, cx);
                }
                SidebarRow::Tab(tab) => {
                    self.activate_sidebar_tab(tab, window, cx);
                }
            },
            "escape" => {
                window.focus(&self.focus, cx);
                if let Some(tab) = self.workspace.focus().tab
                    && let Some(entry) = self
                        .workspace
                        .tab(tab)
                        .and_then(|tab| self.terminals.get(&tab.terminal_id))
                {
                    entry.view.update(cx, |view, cx| view.focus(window, cx));
                }
            }
            _ => handled = false,
        }
        if handled {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn render_pane(&mut self, pane_id: PaneId, cx: &mut Context<'_, Self>) -> AnyElement {
        let Some(pane) = self.workspace.pane(pane_id).cloned() else {
            return div().into_any_element();
        };
        let focused = self.workspace.focus().pane == pane_id;
        let (border, background) = {
            let theme = cx.theme();
            (theme.border, theme.background)
        };
        let strip = self.render_tab_strip(pane_id, &pane, focused, cx);
        let body = match pane.active {
            Some(tab) => self.render_tab_content(tab, cx),
            None => self.render_empty_pane(pane_id, cx),
        };
        let metrics = theme::metrics(cx);
        let accent = theme::state(cx).theme.ui.accent;
        let drop_preview = self.tab_drop.filter(|target| target.pane == pane_id);
        let preview = drop_preview.map(|target| {
            let overlay = div()
                .absolute()
                .bg(accent.opacity(0.14))
                .border_1()
                .border_color(accent)
                .rounded(metrics.radius_sm())
                .on_drop::<TabDrag>(cx.listener(move |shell, drag: &TabDrag, window, cx| {
                    shell.drop_tab_target(drag, target, window, cx);
                }));
            match target.zone {
                TabDropZone::Center => overlay
                    .top(metrics.spacing_xs())
                    .left(metrics.spacing_xs())
                    .right(metrics.spacing_xs())
                    .bottom(metrics.spacing_xs()),
                TabDropZone::Left => overlay.left_0().top_0().bottom_0().w(relative(0.5)),
                TabDropZone::Right => overlay.right_0().top_0().bottom_0().w(relative(0.5)),
                TabDropZone::Top => overlay.top_0().left_0().right_0().h(relative(0.5)),
                TabDropZone::Bottom => overlay.bottom_0().left_0().right_0().h(relative(0.5)),
            }
            .into_any_element()
        });
        let body_bounds = Rc::clone(&self.body_bounds);

        div()
            .id(("pane", pane_id.get()))
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(background)
            .border_1()
            // Se conserva el grosor para no alterar el layout ni el reflow del
            // PTY al cambiar el foco; solo cambia el color.
            .border_color(if focused { background } else { border })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _, _, cx| {
                    if shell.workspace.focus().pane != pane_id {
                        let _ = shell.workspace.focus_pane(pane_id);
                        cx.notify();
                    }
                }),
            )
            .child(strip)
            .child(
                div()
                    .id(("pane-body", pane_id.get()))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .on_prepaint(move |bounds, _, _| {
                        body_bounds.borrow_mut().insert(pane_id, bounds);
                    })
                    .child(body)
                    .children(preview)
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.focus_pane_click(pane_id, window, cx);
                    }))
                    .on_drop::<TabDrag>(cx.listener(move |shell, drag: &TabDrag, window, cx| {
                        let target = shell.tab_drop.filter(|target| target.pane == pane_id);
                        if let Some(target) = target {
                            shell.drop_tab_target(drag, target, window, cx);
                        } else {
                            shell.drop_tab(drag, pane_id, usize::MAX, window, cx);
                        }
                    })),
            )
            .into_any_element()
    }

    fn focus_pane_click(&mut self, pane: PaneId, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Err(error) = self.workspace.focus_pane(pane) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        self.reveal_focus();
        if let Some(tab) = self.workspace.focus().tab {
            let _ = self
                .workspace
                .acknowledge_tab_agents(tab, SystemTime::now());
            self.ensure_terminal(tab, cx);
            if let Some(entry) = self
                .workspace
                .tab(tab)
                .and_then(|tab| self.terminals.get(&tab.terminal_id))
            {
                entry.view.update(cx, |view, cx| view.focus(window, cx));
            } else {
                window.focus(&self.focus, cx);
            }
            self.pending_focus = Some(tab);
        }
        self.persist(cx);
        cx.notify();
    }

    fn duplicate_tab_with_window(
        &mut self,
        tab: TabId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let cwd = self.terminal_cwd(tab, cx);
        match self.workspace.duplicate_tab(tab) {
            Ok(new_tab) => {
                self.spawn_terminal_with_cwd(
                    new_tab.tab,
                    new_tab.terminal,
                    cwd,
                    SpawnOptions {
                        restore: None,
                        focus: true,
                    },
                    window,
                    cx,
                );
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn render_tab_strip(
        &mut self,
        pane_id: PaneId,
        pane: &axf_workspace::Pane,
        focused: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let metrics = theme::metrics(cx);
        let (border, tab_bar) = {
            let theme = cx.theme();
            (theme.border, theme.tab_bar)
        };
        let scroll = self.pane_scroll.entry(pane_id).or_default().clone();
        let tabs = pane.tabs.clone();
        let active = pane.active;
        let focus_changed = focused && self.last_focused_pane != Some(pane_id);
        if focus_changed {
            self.last_focused_pane = Some(pane_id);
        }
        if let Some(active) = active {
            let needs_reveal = self.revealed.get(&pane_id) != Some(&active) || focus_changed;
            if needs_reveal && let Some(index) = pane.active_index() {
                if scroll.bounds_for_item(index).is_some() {
                    scroll.scroll_to_item(index);
                    self.revealed.insert(pane_id, active);
                } else {
                    // La pestaña aún no se ha medido: pide otro frame para
                    // revelarla en cuanto el layout la conozca.
                    cx.notify();
                }
            }
        }
        let overflow = scroll.max_offset().x > px(0.0);
        let scroll_step = metrics.tab_scroll_step();
        let shell_entity = cx.entity().entity_id();
        let this = cx.entity().downgrade();

        let mut tab_elements = Vec::with_capacity(tabs.len());
        for (index, tab) in tabs.iter().copied().enumerate() {
            tab_elements.push(self.render_tab(pane_id, tab, index, focused, cx));
        }

        let left_scroll = scroll.clone();
        let right_scroll = scroll.clone();
        let pane_menu_this = this.clone();
        let default_profile = self.theme_host.settings.terminal.default_profile.clone();
        let profile_ids: Vec<ProfileId> = self
            .theme_host
            .profiles()
            .iter()
            .map(|profile| profile.id.clone())
            .collect();
        let profile_items: Vec<(ProfileId, String, bool, ProfileIconSource)> = profile_ids
            .into_iter()
            .filter_map(|id| {
                let name = self.theme_host.profile_name(&id)?;
                let icon = self.theme_host.profile_icon(&id);
                Some((id.clone(), name, id == default_profile, icon))
            })
            .collect();

        div()
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .px(metrics.spacing_sm())
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .h(metrics.tab_height())
            .flex_shrink_0()
            .border_b_1()
            .border_color(border)
            .bg(tab_bar)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _, window, cx| {
                    shell.focus_pane_click(pane_id, window, cx);
                }),
            )
            .when(overflow, |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            IconButton::new(
                                format!("tabs-left-{}", pane_id.get()),
                                icons::CHEVRON_LEFT,
                            )
                            .size(ControlSize::Sm)
                            .icon_size(IconSize::Custom(metrics.tab_icon()))
                            .tooltip("Desplazar pestañas")
                            .on_click({
                                let scroll = left_scroll.clone();
                                let entity = shell_entity;
                                move |_, _, cx| {
                                    let offset = scroll.offset();
                                    let max = scroll.max_offset().x;
                                    let x = tab_scroll_offset(offset.x, max, scroll_step);
                                    if x != offset.x {
                                        scroll.set_offset(point(x, offset.y));
                                        cx.notify(entity);
                                    }
                                }
                            }),
                        ),
                )
            })
            .child(
                div()
                    .id(("tabs-scroll", pane_id.get()))
                    .relative()
                    .flex()
                    .flex_grow_0()
                    .flex_shrink_1()
                    .min_w_0()
                    .h_full()
                    .overflow_x_scroll()
                    .track_scroll(&scroll)
                    .on_scroll(cx.listener(move |shell, event: &ScrollEvent, _, cx| {
                        shell.scroll_tabs(pane_id, event, cx);
                    }))
                    .on_drop::<TabDrag>(cx.listener(move |shell, drag: &TabDrag, window, cx| {
                        let index = shell
                            .tab_insert
                            .filter(|(pane, _)| *pane == pane_id)
                            .map(|(_, index)| index)
                            .unwrap_or(usize::MAX);
                        shell.drop_tab(drag, pane_id, index, window, cx);
                    }))
                    .children(tab_elements),
            )
            .when(overflow, |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            IconButton::new(
                                format!("tabs-right-{}", pane_id.get()),
                                icons::CHEVRON_RIGHT,
                            )
                            .size(ControlSize::Sm)
                            .icon_size(IconSize::Custom(metrics.tab_icon()))
                            .tooltip("Desplazar pestañas")
                            .on_click({
                                let scroll = right_scroll.clone();
                                let entity = shell_entity;
                                move |_, _, cx| {
                                    let offset = scroll.offset();
                                    let max = scroll.max_offset().x;
                                    let x = tab_scroll_offset(offset.x, max, -scroll_step);
                                    if x != offset.x {
                                        scroll.set_offset(point(x, offset.y));
                                        cx.notify(entity);
                                    }
                                }
                            }),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_xs())
                    .flex_shrink_0()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        IconButton::new(format!("tabs-new-{}", pane_id.get()), icons::PLUS)
                            .size(ControlSize::Sm)
                            .icon_size(IconSize::Custom(metrics.tab_icon()))
                            .tooltip("Nueva terminal")
                            .on_click(cx.listener(move |shell, _, window, cx| {
                                shell.new_tab_in_pane(pane_id, window, cx);
                            })),
                    )
                    .child(
                        KitButton::new(("pane-menu", pane_id.get()))
                            .small()
                            .ghost()
                            .dropdown_caret(true)
                            .dropdown_menu(move |menu, window, cx| {
                                let this = pane_menu_this.clone();
                                let profile_menu = {
                                    let items = profile_items.clone();
                                    let this = this.clone();
                                    PopupMenu::build(window, cx, move |mut submenu, _, _| {
                                        for (id, name, is_default, icon) in items {
                                            let this = this.clone();
                                            submenu = submenu.item(
                                                PopupMenuItem::element(move |_, cx| {
                                                    let metrics = theme::metrics(cx);
                                                    let color = theme::state(cx).theme.ui.text;
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(metrics.spacing_xs())
                                                        .child(
                                                            ProfileIcon::new(icon.clone())
                                                                .size(IconSize::Sm)
                                                                .color(color),
                                                        )
                                                        .child(name.clone())
                                                        .into_any_element()
                                                })
                                                .checked(is_default)
                                                .on_click(move |_, window, cx| {
                                                    if let Some(this) = this.upgrade() {
                                                        let _ = this.update(cx, |shell, cx| {
                                                            shell.new_tab_in_pane_with_profile(
                                                                pane_id,
                                                                Some(id.clone()),
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                            );
                                        }
                                        submenu
                                    })
                                };
                                menu.item(PopupMenuItem::new("Nueva terminal").on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.new_tab_in_pane(pane_id, window, cx);
                                            });
                                        }
                                    }
                                }))
                                .when(!profile_items.is_empty(), |menu| {
                                    menu.item(PopupMenuItem::submenu("Con perfil", profile_menu))
                                })
                                .item(PopupMenuItem::new("Dividir a la derecha").on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.split_pane_action(
                                                    pane_id,
                                                    SplitAxis::Horizontal,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                }))
                                .item(PopupMenuItem::new("Dividir abajo").on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.split_pane_action(
                                                    pane_id,
                                                    SplitAxis::Vertical,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                }))
                                .item(PopupMenuItem::new("Zoom temporal").on_click({
                                    let this = this.clone();
                                    move |_, _, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.zoomed = if shell.zoomed == Some(pane_id) {
                                                    None
                                                } else {
                                                    Some(pane_id)
                                                };
                                                cx.notify();
                                            });
                                        }
                                    }
                                }))
                                .separator()
                                .item(
                                    PopupMenuItem::new("Cerrar panel").on_click({
                                        let this = this.clone();
                                        move |_, window, cx| {
                                            if let Some(this) = this.upgrade() {
                                                let _ = this.update(cx, |shell, cx| {
                                                    if shell.pane_has_live_processes(pane_id, cx) {
                                                        shell.confirm(
                                                            window,
                                                            "Cerrar panel",
                                                            "Hay procesos vivos en este panel.",
                                                            PendingConfirm::ClosePane(pane_id),
                                                            cx,
                                                        );
                                                    } else {
                                                        shell.close_pane_impl(pane_id, cx);
                                                    }
                                                });
                                            }
                                        }
                                    }),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_tab(
        &mut self,
        pane_id: PaneId,
        tab: TabId,
        index: usize,
        focused: bool,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        let active = self.workspace.pane(pane_id).and_then(|pane| pane.active) == Some(tab);
        let title = self.tab_title(tab, cx);
        let editing = self.editing.as_ref().is_some_and(|edit| edit.tab == tab);
        let aggregate = self
            .workspace
            .tab(tab)
            .and_then(|tab| tab.agent_aggregate());
        let status = aggregate
            .as_ref()
            .map(|aggregate| tab_status_from_state(aggregate.state));
        let icon = self.tab_icon(tab);
        let menu_this = cx.entity().downgrade();
        let drag_title = SharedString::from(title.clone());
        let insert = self
            .tab_insert
            .filter(|(pane, _)| *pane == pane_id)
            .map(|(_, index)| index);
        let tab_count = self
            .workspace
            .pane(pane_id)
            .map_or(0, |pane| pane.tabs.len());
        // El indicador vive dentro del chip: en su borde izquierdo o, para el
        // final de la lista, en el derecho del último. Así no aporta tamaño al
        // contenido scrollable y no inventa overflow.
        let marker_left = insert == Some(index);
        let marker_right = insert == Some(tab_count) && index + 1 == tab_count;

        let inner: AnyElement = if editing {
            if let Some(edit) = &self.editing
                && edit.tab == tab
            {
                div()
                    .flex()
                    .items_center()
                    .h_full()
                    .w_full()
                    .min_w_0()
                    .px(metrics.spacing_sm())
                    .rounded(metrics.radius_sm())
                    .border_1()
                    .border_color(ui.focus)
                    .bg(ui.surface_raised)
                    .text_color(ui.text)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(|shell, event: &KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" {
                            shell.cancel_rename(cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        TextField::new(edit.input.clone())
                            .size(ControlSize::Xs)
                            .borderless(),
                    )
                    .into_any_element()
            } else {
                div().child(title.clone()).into_any_element()
            }
        } else {
            let close_this = cx.entity().downgrade();
            TabView::new(format!("tab-{}", tab.get()), title.clone())
                .icon_source(icon.clone())
                .full_width()
                .active(active)
                .focused(focused && active)
                .status(status)
                .on_close(move |_, _, cx| {
                    if let Some(this) = close_this.upgrade() {
                        let _ = this.update(cx, |shell, cx| shell.close_tab_id(tab, cx));
                    }
                })
                .into_any_element()
        };

        div()
            .id(("tab", tab.get()))
            .relative()
            .flex()
            .items_center()
            .h_full()
            .flex_basis(metrics.tab_width())
            .min_w(metrics.tab_min_width())
            .flex_grow_0()
            .flex_shrink_1()
            .overflow_hidden()
            .cursor_pointer()
            .on_click(cx.listener(move |shell, event: &ClickEvent, window, cx| {
                if editing {
                    cx.stop_propagation();
                    return;
                }
                if event.click_count() == 2 {
                    shell.begin_rename(tab, window, cx);
                } else {
                    shell.focus_tab(tab, window, cx);
                }
            }))
            .on_drag(
                TabDrag {
                    tab,
                    title: drag_title,
                    icon,
                    status,
                },
                |drag, _, _, cx| {
                    cx.new(|_| DragGhost::tab(drag.title.clone(), drag.icon.clone(), drag.status))
                },
            )
            .on_drop::<TabDrag>(cx.listener(move |shell, drag: &TabDrag, window, cx| {
                // El indicador de inserción manda: así el drop coincide con la
                // barra que el usuario ve, incluso dentro del mismo chip.
                let insert = shell
                    .tab_insert
                    .filter(|(pane, _)| *pane == pane_id)
                    .map(|(_, index)| index)
                    .unwrap_or(index);
                shell.drop_tab(drag, pane_id, insert, window, cx);
            }))
            .context_menu(move |menu, _, _| {
                let this = menu_this.clone();
                menu.item(PopupMenuItem::new("Renombrar").on_click({
                    let this = this.clone();
                    move |_, window, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                shell.begin_rename(tab, window, cx);
                            });
                        }
                    }
                }))
                .item(PopupMenuItem::new("Duplicar").on_click({
                    let this = this.clone();
                    move |_, window, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                shell.duplicate_tab_with_window(tab, window, cx);
                            });
                        }
                    }
                }))
                .item(PopupMenuItem::new("Mover al panel siguiente").on_click({
                    let this = this.clone();
                    move |_, window, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                shell.focus_tab(tab, window, cx);
                                shell.move_focused_tab_next_pane(window, cx);
                            });
                        }
                    }
                }))
                .separator()
                .item(PopupMenuItem::new("Cerrar pestaña").on_click({
                    let this = this.clone();
                    move |_, _, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| shell.close_tab_id(tab, cx));
                        }
                    }
                }))
            })
            .child(inner)
            .when(marker_left || marker_right, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(metrics.spacing_xxs())
                        .bottom(metrics.spacing_xxs())
                        .w(metrics.tab_insert_width())
                        .rounded(metrics.hairline())
                        .bg(ui.accent)
                        .when(marker_left, |this| this.left_0())
                        .when(marker_right, |this| this.right_0()),
                )
            })
            .into_any_element()
    }

    fn scroll_tabs(&mut self, pane: PaneId, event: &ScrollEvent, cx: &mut Context<'_, Self>) {
        let Some(scroll) = self.pane_scroll.get(&pane) else {
            return;
        };
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => delta.y,
            ScrollDelta::Lines(delta) => px(delta.y * 24.0),
        };
        let offset = scroll.offset();
        let max = scroll.max_offset().x;
        let x = (offset.x - delta).min(px(0.0)).max(-max);
        if x != offset.x {
            scroll.set_offset(point(x, offset.y));
            cx.notify();
        }
    }

    /// Actualiza el destino visible del drag de una pestaña.
    fn update_tab_drag_target(
        &mut self,
        event: &axf_ui::gpui::DragMoveEvent<TabDrag>,
        cx: &mut Context<'_, Self>,
    ) {
        let position = event.event.position;
        let metrics = theme::metrics(cx);
        // Primero se desplaza la strip y después se calcula el índice: si se
        // hiciera al revés, el destino quedaría un paso por detrás del offset.
        let scrolled = self.auto_scroll_tab_drag(position, metrics);
        let edge = metrics.tab_drop_edge_ratio();
        let drop = {
            let bounds = self.body_bounds.borrow();
            bounds
                .iter()
                .find(|(_, pane_bounds)| bounds_contain(**pane_bounds, position))
                .map(|(pane, pane_bounds)| TabDropTarget {
                    pane: *pane,
                    zone: tab_drop_zone(position, *pane_bounds, edge),
                })
        };
        let insert = if drop.is_none() {
            self.tab_insert_at(position, metrics)
        } else {
            None
        };
        if scrolled || drop != self.tab_drop || insert != self.tab_insert {
            self.tab_drop = drop;
            self.tab_insert = insert;
            cx.notify();
        }
    }

    /// Índice de inserción dentro de la strip bajo el puntero.
    fn tab_insert_at(
        &self,
        position: Point<Pixels>,
        metrics: UiMetrics,
    ) -> Option<(PaneId, usize)> {
        let (pane, bounds, offset) = self
            .pane_scroll
            .iter()
            .find(|(_, scroll)| bounds_contain(scroll.bounds(), position))
            .map(|(pane, scroll)| (*pane, scroll.bounds(), scroll.offset()))?;
        let tab_count = self.workspace.pane(pane)?.tabs.len();
        let width = tab_slot_width(
            bounds.size.width,
            tab_count,
            metrics.tab_min_width(),
            metrics.tab_width(),
        );
        // `scroll.bounds()` es el viewport estable; el offset lo desplaza el
        // contenido, no el contenedor.
        let local_x = f32::from(position.x - bounds.left() - offset.x);
        let index = insert_index_at(local_x, width, tab_count);
        Some((pane, index))
    }

    /// Desplaza una sola vez la strip bajo el puntero cuando el drag está en un borde.
    ///
    /// GPUI entrega `on_drag_move` a todos los elementos que lo registran, no
    /// solo al origen del drag. Por eso este listener existe una única vez: si
    /// cada pestaña lo registrara, la velocidad del scroll crecería con el
    /// número de pestañas. Devuelve si el offset cambió.
    fn auto_scroll_tab_drag(&mut self, position: Point<Pixels>, metrics: UiMetrics) -> bool {
        let Some((pane, bounds, offset)) = self
            .pane_scroll
            .iter()
            .find(|(_, scroll)| bounds_contain(scroll.bounds(), position))
            .map(|(pane, scroll)| (*pane, scroll.bounds(), scroll.offset()))
        else {
            return false;
        };
        let Some(scroll) = self.pane_scroll.get(&pane) else {
            return false;
        };
        let x = tab_autoscroll_offset(
            offset.x,
            scroll.max_offset().x,
            position.x,
            bounds.left(),
            bounds.right(),
            metrics.drag_edge(),
            metrics.drag_nudge(),
        );
        if x == offset.x {
            return false;
        }
        scroll.set_offset(point(x, offset.y));
        true
    }

    fn drop_tab(
        &mut self,
        drag: &TabDrag,
        target: PaneId,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // El índice cacheado puede venir de un movimiento anterior; al soltar
        // manda la posición real del puntero sobre la strip.
        let index = self
            .tab_insert_at(window.mouse_position(), theme::metrics(cx))
            .filter(|(pane, _)| *pane == target)
            .map(|(_, index)| index)
            .unwrap_or(index);
        // `move_tab` quita la pestaña antes de insertarla. Si se mueve dentro
        // del mismo panel y hacia la derecha, el índice visible (el hueco de
        // la barra) pierde una posición al quitar.
        let same_pane_from = self
            .workspace
            .pane_of_tab(drag.tab)
            .filter(|source| *source == target)
            .and_then(|_| {
                self.workspace
                    .pane(target)
                    .and_then(|pane| pane.tabs.iter().position(|tab| *tab == drag.tab))
            });
        let index = adjusted_insert_index(same_pane_from, index);
        if let Err(error) = self.workspace.move_tab(drag.tab, target, index) {
            self.tab_drop = None;
            self.tab_insert = None;
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        self.finish_tab_drop(drag.tab, window, cx);
    }

    /// Suelta una pestaña en un destino con zona: centro mueve, borde divide.
    fn drop_tab_target(
        &mut self,
        drag: &TabDrag,
        target: TabDropTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(axis) = target.zone.axis() else {
            self.drop_tab(drag, target.pane, usize::MAX, window, cx);
            return;
        };
        // Un panel nuevo debe verse: el zoom temporal dejaría oculto el split.
        self.zoomed = None;
        match self
            .workspace
            .split_pane_with_tab(target.pane, axis, target.zone.side(), drag.tab)
        {
            Ok(_) => self.finish_tab_drop(drag.tab, window, cx),
            Err(error) => {
                self.tab_drop = None;
                self.tab_insert = None;
                self.set_notice(error.to_string(), true, cx);
            }
        }
    }

    fn finish_tab_drop(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.tab_drop = None;
        self.tab_insert = None;
        self.pending_focus = Some(tab);
        self.reveal_focus();
        self.ensure_terminal(tab, cx);
        if let Some(entry) = self
            .workspace
            .tab(tab)
            .and_then(|tab| self.terminals.get(&tab.terminal_id))
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        }
        self.persist(cx);
        cx.notify();
    }

    fn move_focused_tab_next_pane(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(tab) = self.workspace.focus().tab else {
            return;
        };
        let leaves = self.workspace.layout().leaves();
        if leaves.len() < 2 {
            return;
        }
        let current = self
            .workspace
            .pane_of_tab(tab)
            .and_then(|pane| leaves.iter().position(|id| *id == pane))
            .unwrap_or(0);
        let target = leaves[(current + 1) % leaves.len()];
        if let Err(error) = self.workspace.move_tab(tab, target, usize::MAX) {
            self.set_notice(error.to_string(), true, cx);
            return;
        }
        self.pending_focus = Some(tab);
        self.ensure_terminal(tab, cx);
        if let Some(entry) = self
            .workspace
            .tab(tab)
            .and_then(|tab| self.terminals.get(&tab.terminal_id))
        {
            entry.view.update(cx, |view, cx| view.focus(window, cx));
        }
        self.persist(cx);
        cx.notify();
    }

    fn render_empty_pane(&mut self, pane: PaneId, cx: &mut Context<'_, Self>) -> AnyElement {
        let metrics = theme::metrics(cx);
        EmptyState::new(
            icons::TERMINAL,
            "Panel vacío",
            "Abre una terminal o divide el panel para empezar.",
        )
        .action(
            div()
                .flex()
                .gap(metrics.spacing_sm())
                .child(
                    Button::new(format!("empty-new-{}", pane.get()), "Nueva terminal")
                        .small()
                        .primary()
                        .icon(icons::PLUS)
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.new_tab_in_pane(pane, window, cx);
                        })),
                )
                .child(
                    Button::new(format!("empty-split-{}", pane.get()), "Dividir")
                        .small()
                        .icon(icons::SPLIT_HORIZONTAL)
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.split_pane_action(pane, SplitAxis::Horizontal, window, cx);
                        })),
                ),
        )
        .into_any_element()
    }

    fn new_tab_in_pane(&mut self, pane: PaneId, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.new_tab_in_pane_with_profile(pane, None, window, cx);
    }

    fn new_tab_in_pane_with_profile(
        &mut self,
        pane: PaneId,
        profile: Option<ProfileId>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let project = self
            .workspace
            .current_project()
            .or_else(|| self.workspace.projects().first().map(|project| project.id));
        let Some(project) = project else {
            self.prompt_open_project(cx);
            return;
        };
        let requested =
            profile.unwrap_or_else(|| self.theme_host.settings.terminal.default_profile.clone());
        let Some(title) = self.profile_title(&requested) else {
            self.set_notice(
                format!("El perfil `{requested}` no existe en config.toml."),
                true,
                cx,
            );
            return;
        };
        match self.workspace.new_tab(pane, project, &requested, &title) {
            Ok(new_tab) => {
                self.workspace.focus_pane(pane).ok();
                self.reveal_project(project);
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn split_pane_action(
        &mut self,
        pane: PaneId,
        axis: SplitAxis,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.workspace.tab_count() >= MAX_TABS {
            self.set_notice(
                format!("Límite de {MAX_TABS} terminales alcanzado; cierra una pestaña."),
                true,
                cx,
            );
            return;
        }
        let project = self
            .workspace
            .current_project()
            .or_else(|| self.workspace.projects().first().map(|project| project.id));
        let Some(project) = project else {
            self.prompt_open_project(cx);
            return;
        };
        let new_pane = match self.workspace.split_pane(pane, axis) {
            Ok(pane) => pane,
            Err(error) => {
                self.set_notice(error.to_string(), true, cx);
                return;
            }
        };
        let Some((profile, title)) = self.default_profile() else {
            self.set_notice(
                "No hay un perfil por defecto válido en config.toml.",
                true,
                cx,
            );
            return;
        };
        match self.workspace.new_tab(new_pane, project, &profile, &title) {
            Ok(new_tab) => {
                self.spawn_terminal(new_tab.tab, new_tab.terminal, window, cx);
                self.persist(cx);
                cx.notify();
            }
            Err(error) => self.set_notice(error.to_string(), true, cx),
        }
    }

    fn render_tab_content(&mut self, tab: TabId, cx: &mut Context<'_, Self>) -> AnyElement {
        let Some(model_tab) = self.workspace.tab(tab).cloned() else {
            return div().into_any_element();
        };
        let metrics = theme::metrics(cx);
        let title = self.tab_title(tab, cx);
        if let Some(error) = self.spawn_errors.get(&model_tab.terminal_id).cloned() {
            let profile_missing = self.theme_host.profile(&model_tab.profile_id).is_none();
            return EmptyState::new(icons::ALERT_TRIANGLE, title, error)
                .action(
                    div()
                        .flex()
                        .gap(metrics.spacing_sm())
                        .child(
                            Button::new(format!("spawn-retry-{}", tab.get()), "Reintentar")
                                .small()
                                .primary()
                                .icon(icons::RESTART)
                                .on_click(cx.listener(move |shell, _, window, cx| {
                                    shell.restart_tab_id(tab, window, cx);
                                })),
                        )
                        .when(profile_missing, |this| {
                            this.child(
                                Button::new(
                                    format!("spawn-default-{}", tab.get()),
                                    "Usar perfil predeterminado",
                                )
                                .small()
                                .icon(icons::RESTART)
                                .on_click(cx.listener(
                                    move |shell, _, window, cx| {
                                        shell.use_default_profile(tab, window, cx);
                                    },
                                )),
                            )
                        })
                        .child(
                            Button::new(format!("spawn-close-{}", tab.get()), "Cerrar pestaña")
                                .small()
                                .ghost()
                                .on_click(cx.listener(move |shell, _, _, cx| {
                                    shell.close_tab_id(tab, cx);
                                })),
                        ),
                )
                .into_any_element();
        }
        if let Some(entry) = self.terminals.get(&model_tab.terminal_id) {
            if let Some(error) = entry.view.read(cx).error().map(str::to_owned) {
                return EmptyState::new(icons::ALERT_TRIANGLE, title, error)
                    .action(
                        div().flex().gap(metrics.spacing_sm()).child(
                            Button::new(format!("terminal-retry-{}", tab.get()), "Reintentar")
                                .small()
                                .primary()
                                .icon(icons::RESTART)
                                .on_click(cx.listener(move |shell, _, window, cx| {
                                    shell.restart_tab_id(tab, window, cx);
                                })),
                        ),
                    )
                    .into_any_element();
            }
            return div()
                .size_full()
                .child(entry.view.clone())
                .into_any_element();
        }

        if self.theme_host.profile(&model_tab.profile_id).is_none() {
            return EmptyState::new(
                icons::ALERT_TRIANGLE,
                title,
                format!(
                    "El perfil `{}` ya no existe en config.toml. Elige el perfil predeterminado o cierra la pestaña; no se arranca otro shell por sorpresa.",
                    model_tab.profile_id
                ),
            )
            .action(
                div()
                    .flex()
                    .gap(metrics.spacing_sm())
                    .child(
                        Button::new(
                            format!("missing-profile-default-{}", tab.get()),
                            "Usar perfil predeterminado",
                        )
                        .small()
                        .primary()
                        .icon(icons::RESTART)
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.use_default_profile(tab, window, cx);
                        })),
                    )
                    .child(
                        Button::new(
                            format!("missing-profile-close-{}", tab.get()),
                            "Cerrar pestaña",
                        )
                        .small()
                        .ghost()
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.close_tab_id(tab, cx);
                        })),
                    ),
            )
            .into_any_element();
        }

        if self.pending_terminals.contains(&model_tab.terminal_id) {
            return EmptyState::new(
                icons::RESTART,
                title,
                "Restaurando la terminal y su historial…",
            )
            .into_any_element();
        }

        let missing = self
            .workspace
            .project(model_tab.project_id)
            .is_some_and(|project| !project.root.is_dir());
        if !missing {
            return EmptyState::new(
                icons::RESTART,
                title,
                "Pestaña pendiente de restaurar. Se abrirá al activarla.",
            )
            .action(
                div()
                    .flex()
                    .gap(metrics.spacing_sm())
                    .child(
                        Button::new(format!("restore-now-{}", tab.get()), "Restaurar ahora")
                            .small()
                            .primary()
                            .icon(icons::RESTART)
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.ensure_terminal(tab, cx);
                            })),
                    )
                    .child(
                        Button::new(format!("restore-close-{}", tab.get()), "Cerrar pestaña")
                            .small()
                            .ghost()
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.close_tab_id(tab, cx);
                            })),
                    ),
            )
            .into_any_element();
        }

        EmptyState::new(
            icons::ALERT_TRIANGLE,
            title,
            "La carpeta del proyecto no existe.",
        )
        .action(
            div()
                .flex()
                .gap(metrics.spacing_sm())
                .child(
                    Button::new(
                        format!("restore-restart-{}", tab.get()),
                        "Reiniciar terminal",
                    )
                    .small()
                    .primary()
                    .icon(icons::RESTART)
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.restart_tab_id(tab, window, cx);
                    })),
                )
                .child(
                    Button::new(format!("restore-close-{}", tab.get()), "Cerrar pestaña")
                        .small()
                        .ghost()
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.close_tab_id(tab, cx);
                        })),
                ),
        )
        .into_any_element()
    }

    fn reload_themes_impl(&mut self, cx: &mut Context<'_, Self>) {
        let report = self.theme_host.reload(
            axf_config::WatchEvent {
                config: true,
                themes: true,
            },
            cx,
        );
        let applied = report.applied;
        let has_error = !report.errors.is_empty();
        let mut parts = report.errors;
        parts.extend(report.messages);
        let had = !parts.is_empty();
        if had {
            self.set_notice(parts.join(" "), has_error, cx);
        }
        if applied && !had {
            self.set_notice("Configuración y temas recargados.", false, cx);
        }
    }

    fn open_themes_folder_impl(&mut self, cx: &mut Context<'_, Self>) {
        let Some(dir) = self.theme_host.themes_dir.clone() else {
            self.set_notice("No se pudo resolver el directorio de temas.", true, cx);
            return;
        };
        if let Err(error) = std::fs::create_dir_all(&dir) {
            self.set_notice(
                format!("No se pudo crear {}: {error}", dir.display()),
                true,
                cx,
            );
            return;
        }
        cx.open_with_system(&dir);
    }

    fn import_theme_impl(&mut self, cx: &mut Context<'_, Self>) {
        let receiver = cx.prompt_for_paths(axf_ui::gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Importar tema TOML".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.first().cloned()
            {
                let result = this.update(cx, |shell, cx| {
                    let result = shell.theme_host.import_theme(&path, cx);
                    match result {
                        Ok(destination) => shell.set_notice(
                            format!("Tema importado en {}", destination.display()),
                            false,
                            cx,
                        ),
                        Err(error) => shell.set_notice(error, true, cx),
                    }
                });
                result.ok();
            }
        })
        .detach();
    }

    fn export_theme_impl(&mut self, cx: &mut Context<'_, Self>) {
        let directory = self.theme_host.config_dir.clone();
        let suggested = format!("{}.toml", sanitize_file_name(&self.theme_host.selected));
        let receiver = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = receiver.await {
                let result = this.update(cx, |shell, cx| {
                    match shell.theme_host.export_theme(&path, cx) {
                        Ok(path) => shell.set_notice(
                            format!("Tema exportado en {}", path.display()),
                            false,
                            cx,
                        ),
                        Err(error) => shell.set_notice(error, true, cx),
                    }
                });
                result.ok();
            }
        })
        .detach();
    }

    fn render_toast(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        if self.notices.is_empty() {
            return None;
        }
        let metrics = theme::metrics(cx);
        let toasts = self
            .notices
            .iter()
            .map(|notice| {
                let toast = if notice.error {
                    Toast::error(notice.text.clone())
                } else {
                    Toast::new(notice.text.clone())
                };
                match &notice.detail {
                    Some(detail) => toast.detail(detail.clone()),
                    None => toast,
                }
            })
            .collect::<Vec<_>>();
        Some(
            div()
                .absolute()
                .bottom(metrics.spacing_xl())
                .right(metrics.spacing_xl())
                .flex()
                .flex_col()
                .gap(metrics.spacing_sm())
                .children(toasts)
                .into_any_element(),
        )
    }

    fn render_theme_modal(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let theme = state.theme.meta.clone();
        let metrics = state.metrics;
        let fonts = state.fonts.clone();
        let names = self.theme_host.theme_names();
        let selected = self.theme_host.selected.clone();
        let preview = self.theme_host.preview_variant.is_some();
        let themes_dir = self
            .theme_host
            .themes_dir
            .as_ref()
            .map(|dir| dir.display().to_string())
            .unwrap_or_else(|| "—".into());
        let config_path = self
            .theme_host
            .config_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "—".into());

        let mut menu = Menu::new();
        for (name, variant) in names {
            let this = cx.entity().downgrade();
            let label = name.clone();
            menu = menu.item(
                MenuItem::new(format!("{name} · {}", variant.label()))
                    .icon(if variant.is_dark() {
                        icons::MOON
                    } else {
                        icons::SUN
                    })
                    .checked(name == selected)
                    .on_click(move |_, _, cx| {
                        if let Some(this) = this.upgrade() {
                            let _ = this.update(cx, |shell, cx| {
                                if shell.theme_host.select_theme(&label, cx) {
                                    shell.set_notice(format!("Tema: {label}"), false, cx);
                                }
                                cx.notify();
                            });
                        }
                    }),
            );
        }

        let toggle = Toggle::new("theme-preview-variant", preview)
            .label("Previsualizar variante opuesta")
            .on_click(cx.listener(|shell, checked, _, cx| {
                if *checked {
                    shell.theme_host.toggle_preview_variant(cx);
                } else {
                    shell.theme_host.set_preview_variant(None, cx);
                }
                cx.notify();
            }));

        let info = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_xxs())
            .text_size(metrics.font_size * 0.75)
            .text_color(theme::state(cx).theme.ui.muted)
            .child(format!(
                "UI: {} · {} px · escala {:.2}",
                fonts.ui, state.ui_font_size, state.metrics.scale
            ))
            .child(format!(
                "Terminal: {} · {} px",
                fonts.mono, state.mono_font_size
            ))
            .when(!fonts.warnings.is_empty(), |this| {
                this.child(fonts.warnings.join(" "))
            })
            .when(!fonts.errors.is_empty(), |this| {
                this.child(fonts.errors.join(" "))
            })
            .child(format!("Config: {config_path}"))
            .child(format!("Temas: {themes_dir}"));

        let footer = div()
            .flex()
            .flex_wrap()
            .gap(metrics.spacing_xs())
            .child(
                Button::new("theme-open-folder", "Abrir carpeta")
                    .small()
                    .icon(icons::FOLDER_OPEN)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.open_themes_folder_impl(cx);
                    })),
            )
            .child(
                Button::new("theme-import", "Importar")
                    .small()
                    .icon(icons::FILE_IMPORT)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.import_theme_impl(cx);
                    })),
            )
            .child(
                Button::new("theme-export", "Exportar")
                    .small()
                    .icon(icons::FILE_EXPORT)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.export_theme_impl(cx);
                    })),
            )
            .child(
                Button::new("theme-reload", "Recargar")
                    .small()
                    .icon(icons::RESTART)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.reload_themes_impl(cx);
                    })),
            )
            .child(
                Button::new("theme-close", "Cerrar")
                    .small()
                    .ghost()
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.close_theme_modal(window, cx);
                    })),
            );

        let modal = Modal::new("theme-modal", "Tema y fuentes")
            .description(format!(
                "{} · {} · {}",
                theme.name,
                theme.variant.label(),
                theme.author.as_deref().unwrap_or("Axfterm")
            ))
            .body(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_sm())
                    .child(
                        div()
                            .max_h(metrics.font_size * 18.0)
                            .overflow_y_scrollbar()
                            .child(menu),
                    )
                    .child(toggle)
                    .child(info),
            )
            .footer(footer)
            .on_close(cx.listener(|shell, _, window, cx| {
                shell.close_theme_modal(window, cx);
            }))
            .into_any_element();

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context("ThemeModal")
            .track_focus(&self.theme_modal_focus)
            .on_key_down(cx.listener(|shell, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    shell.close_theme_modal(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(modal)
            .into_any_element()
    }
}
/// Vista que sigue al cursor durante un drag.
enum DragGhost {
    /// Etiqueta simple (divisores).
    Label(SharedString),
    /// Copia visual de la pestaña arrastrada.
    Tab {
        title: SharedString,
        icon: ProfileIconSource,
        status: Option<TabStatus>,
    },
}

impl DragGhost {
    fn new(label: impl Into<SharedString>) -> Self {
        Self::Label(label.into())
    }

    fn tab(title: SharedString, icon: ProfileIconSource, status: Option<TabStatus>) -> Self {
        Self::Tab {
            title,
            icon,
            status,
        }
    }
}

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let metrics = theme::metrics(cx);
        let ui = theme::state(cx).theme.ui;
        match self {
            Self::Label(label) => div()
                .px(metrics.spacing_sm())
                .py(metrics.spacing_xxs())
                .rounded(metrics.radius_sm())
                .bg(ui.surface_raised)
                .border_1()
                .border_color(ui.accent)
                .text_color(ui.text)
                .text_size(metrics.font_size)
                .child(label.clone())
                .into_any_element(),
            Self::Tab {
                title,
                icon,
                status,
            } => div()
                .w(metrics.tab_width())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.accent)
                .bg(ui.surface_raised)
                // Ligeramente translúcida: deja ver el destino debajo sin
                // perder legibilidad.
                .opacity(0.8)
                .overflow_hidden()
                .child(
                    TabView::new("drag-ghost", title.clone())
                        .icon_source(icon.clone())
                        .status(*status)
                        .active(true)
                        .focused(true)
                        .full_width(),
                )
                .into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axf_ui::gpui::size;

    #[test]
    fn split_keys_are_stable_and_unambiguous() {
        assert_eq!(split_key(&[]), "");
        assert_eq!(split_key(&[Branch::First]), "0");
        assert_eq!(split_key(&[Branch::Second]), "1");
        assert_eq!(
            split_key(&[Branch::Second, Branch::First, Branch::Second]),
            "101"
        );
    }

    #[test]
    fn the_title_centers_on_the_window_and_avoids_the_chrome() {
        let typical =
            title_bar_title_geometry(px(1280.0), px(240.0), px(138.0), px(480.0), px(8.0));
        assert_eq!(typical.max_width, px(480.0));
        assert_eq!(typical.padding_left, px(0.0));
        assert_eq!(typical.padding_right, px(102.0));

        let narrow = title_bar_title_geometry(px(600.0), px(200.0), px(138.0), px(480.0), px(8.0));
        assert_eq!(narrow.max_width, px(184.0));
        assert_eq!(narrow.padding_right, px(62.0));

        // La sidebar ocupa más de media ventana: no hay centro real libre, así
        // que se centra en el hueco y se recorta al espacio disponible.
        let wide_sidebar =
            title_bar_title_geometry(px(900.0), px(480.0), px(138.0), px(480.0), px(8.0));
        assert_eq!(wide_sidebar.max_width, px(266.0));
        assert_eq!(wide_sidebar.padding_left, px(0.0));
        assert_eq!(wide_sidebar.padding_right, px(0.0));

        // Caso simétrico inverso, no alcanzable en Windows, pero la geometría
        // no debe depender de que la sidebar sea la franja más ancha.
        let wide_controls =
            title_bar_title_geometry(px(1280.0), px(120.0), px(200.0), px(480.0), px(8.0));
        assert_eq!(wide_controls.padding_left, px(80.0));
        assert_eq!(wide_controls.padding_right, px(0.0));
    }

    #[test]
    fn tab_scroll_offsets_stay_within_the_scrollable_range() {
        let step = px(100.0);
        let max = px(250.0);
        assert_eq!(tab_scroll_offset(px(-200.0), max, step), px(-100.0));
        assert_eq!(tab_scroll_offset(px(-50.0), max, step), px(0.0));
        assert_eq!(tab_scroll_offset(px(-200.0), max, -step), px(-250.0));
        assert_eq!(tab_scroll_offset(px(0.0), max, -step), px(-100.0));
        assert_eq!(tab_scroll_offset(px(-10.0), px(0.0), step), px(0.0));
    }

    #[test]
    fn powershell_uses_its_brand_icon_and_the_rest_falls_back() {
        assert_eq!(terminal_tab_icon(Some("pwsh")), icons::BRAND_POWERSHELL);
        assert_eq!(terminal_tab_icon(Some("pwsh.exe")), icons::BRAND_POWERSHELL);
        assert_eq!(
            terminal_tab_icon(Some(r"C:\Program Files\PowerShell\7\pwsh.exe")),
            icons::BRAND_POWERSHELL
        );
        assert_eq!(
            terminal_tab_icon(Some("powershell.exe")),
            icons::BRAND_POWERSHELL
        );
        assert_eq!(terminal_tab_icon(Some("cmd.exe")), icons::TERMINAL);
        assert_eq!(terminal_tab_icon(Some("/bin/bash")), icons::TERMINAL);
        assert_eq!(terminal_tab_icon(None), icons::TERMINAL);
    }

    #[test]
    fn tab_drop_zones_prefer_the_nearest_edge_within_the_band() {
        let bounds = Bounds::new(point(px(100.0), px(50.0)), size(px(400.0), px(200.0)));
        let ratio = 0.25;
        assert_eq!(
            tab_drop_zone(point(px(110.0), px(150.0)), bounds, ratio),
            TabDropZone::Left
        );
        assert_eq!(
            tab_drop_zone(point(px(490.0), px(150.0)), bounds, ratio),
            TabDropZone::Right
        );
        assert_eq!(
            tab_drop_zone(point(px(300.0), px(60.0)), bounds, ratio),
            TabDropZone::Top
        );
        assert_eq!(
            tab_drop_zone(point(px(300.0), px(240.0)), bounds, ratio),
            TabDropZone::Bottom
        );
        assert_eq!(
            tab_drop_zone(point(px(300.0), px(150.0)), bounds, ratio),
            TabDropZone::Center
        );
        // En una esquina gana el borde más cercano.
        assert_eq!(
            tab_drop_zone(point(px(105.0), px(55.0)), bounds, ratio),
            TabDropZone::Left
        );
        assert!(bounds_contain(bounds, point(px(100.0), px(50.0))));
        assert!(!bounds_contain(bounds, point(px(99.0), px(150.0))));
    }

    #[test]
    fn insert_index_accounts_for_removing_the_dragged_tab() {
        assert_eq!(adjusted_insert_index(Some(0), 2), 1);
        assert_eq!(adjusted_insert_index(Some(2), 0), 0);
        assert_eq!(adjusted_insert_index(Some(1), 1), 1);
        assert_eq!(adjusted_insert_index(None, usize::MAX), usize::MAX);
        assert_eq!(adjusted_insert_index(Some(0), usize::MAX), usize::MAX - 1);
    }

    #[test]
    fn tab_widths_compress_uniformly_between_min_and_max() {
        let min = px(90.0);
        let max = px(160.0);
        // Cabe el ancho objetivo con holgura: no crece de más.
        assert_eq!(tab_slot_width(px(1000.0), 3, min, max), 160.0);
        // Se reparte el ancho disponible hasta el objetivo.
        assert_eq!(tab_slot_width(px(480.0), 3, min, max), 160.0);
        assert_eq!(tab_slot_width(px(300.0), 3, min, max), 100.0);
        // Por debajo del mínimo manda el mínimo y la strip hace scroll.
        assert_eq!(tab_slot_width(px(120.0), 3, min, max), 90.0);
        assert_eq!(tab_slot_width(px(120.0), 0, min, max), 0.0);
    }

    #[test]
    fn insert_index_moves_at_each_tab_center() {
        // Tres pestañas de 200: los medios están en 100, 300 y 500.
        assert_eq!(insert_index_at(0.0, 200.0, 3), 0);
        assert_eq!(insert_index_at(99.0, 200.0, 3), 0);
        assert_eq!(insert_index_at(100.0, 200.0, 3), 1);
        assert_eq!(insert_index_at(299.0, 200.0, 3), 1);
        assert_eq!(insert_index_at(300.0, 200.0, 3), 2);
        assert_eq!(insert_index_at(599.0, 200.0, 3), 3);
        assert_eq!(insert_index_at(900.0, 200.0, 3), 3);
        assert_eq!(insert_index_at(10.0, 200.0, 0), 0);
        assert_eq!(insert_index_at(10.0, 0.0, 3), 0);
    }

    #[test]
    fn tab_autoscroll_steps_once_and_clamps() {
        let left = px(100.0);
        let right = px(700.0);
        let edge = px(20.0);
        let nudge = px(15.0);
        let max = px(500.0);

        // Fuera de la franja no se mueve.
        assert_eq!(
            tab_autoscroll_offset(px(-100.0), max, px(400.0), left, right, edge, nudge),
            px(-100.0)
        );
        // Cada movimiento aplica exactamente un paso, nunca uno por pestaña.
        assert_eq!(
            tab_autoscroll_offset(px(-100.0), max, px(690.0), left, right, edge, nudge),
            px(-115.0)
        );
        assert_eq!(
            tab_autoscroll_offset(px(-100.0), max, px(110.0), left, right, edge, nudge),
            px(-85.0)
        );
        // Se acota a los extremos.
        assert_eq!(
            tab_autoscroll_offset(px(-495.0), max, px(690.0), left, right, edge, nudge),
            px(-500.0)
        );
        assert_eq!(
            tab_autoscroll_offset(px(-5.0), max, px(110.0), left, right, edge, nudge),
            px(0.0)
        );
        // Sin overflow no hay desplazamiento.
        assert_eq!(
            tab_autoscroll_offset(px(0.0), px(0.0), px(690.0), left, right, edge, nudge),
            px(0.0)
        );
    }

    #[test]
    fn shell_path_titles_are_ignored() {
        assert!(title_is_shell_path(
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            Some("pwsh")
        ));
        assert!(title_is_shell_path(
            r"Administrador: C:\Program Files\PowerShell\7\pwsh.exe",
            Some("pwsh")
        ));
        assert!(title_is_shell_path("pwsh", Some("pwsh")));
        assert!(title_is_shell_path("/bin/bash", Some("/bin/bash")));
        assert!(!title_is_shell_path(r"C:\Proyectos\axfterm", Some("pwsh")));
        assert!(!title_is_shell_path("axfterm", Some("pwsh")));
        assert!(!title_is_shell_path("pwsh.exe", None));
    }

    #[test]
    fn a_manual_tab_name_wins_over_osc_and_the_profile() {
        let mut tab = axf_workspace::Tab::new(
            ProjectId::new(1),
            TerminalId::new(2),
            ProfileId::new("pwsh"),
            "PowerShell 7",
        );
        assert_eq!(
            resolve_tab_title(
                &tab,
                Some(r"C:\Program Files\PowerShell\7\pwsh.exe"),
                Some("pwsh"),
                Some("adventure2")
            ),
            "PowerShell 7"
        );

        tab.custom_title = true;
        tab.title = "Mi shell".into();
        assert_eq!(
            resolve_tab_title(&tab, Some("C:\\otro"), Some("pwsh"), Some("adventure2")),
            "Mi shell"
        );

        tab.custom_title = false;
        tab.title = "PowerShell 7".into();
        assert_eq!(
            resolve_tab_title(&tab, Some("nvim ~/proyecto"), Some("pwsh"), None),
            "nvim ~/proyecto"
        );
        assert_eq!(
            resolve_tab_title(
                &tab,
                Some(r"C:\Program Files\PowerShell\7\pwsh.exe"),
                Some("pwsh"),
                None
            ),
            "PowerShell 7"
        );

        tab.auto_title.clear();
        assert_eq!(
            resolve_tab_title(&tab, None, None, Some("adventure2")),
            "PowerShell 7"
        );
        tab.title.clear();
        assert_eq!(
            resolve_tab_title(&tab, None, None, Some("adventure2")),
            "adventure2"
        );
        assert_eq!(resolve_tab_title(&tab, None, None, None), "Terminal");
    }

    fn sidebar_temp_project(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("axfterm-sidebar-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("carpeta temporal");
        dir
    }

    fn sidebar_profile() -> ProfileId {
        ProfileId::new("shell")
    }

    const SIDEBAR_PROFILE_TITLE: &str = "Shell";

    fn sidebar_workspace(name: &str) -> (Workspace, ProjectId, TabId) {
        let mut workspace = Workspace::new();
        let root = sidebar_temp_project(name);
        let opened = workspace
            .open_project(&root, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
            .expect("abre proyecto");
        let tab = opened.tab.expect("pestaña inicial");
        (workspace, opened.project, tab.tab)
    }

    #[test]
    fn sidebar_view_shows_expanded_projects_and_their_tabs() {
        let (workspace, project, tab) = sidebar_workspace("view-all");
        let groups = sidebar_view(&workspace, SidebarSection::All);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].project, project);
        assert_eq!(groups[0].tabs, vec![tab]);
        assert!(groups[0].show_tabs);
        assert_eq!(
            sidebar_rows_from_groups(&groups),
            vec![SidebarRow::Project(project), SidebarRow::Tab(tab)]
        );
    }

    #[test]
    fn sidebar_view_keeps_empty_projects() {
        let mut workspace = Workspace::new();
        let root = sidebar_temp_project("empty-project");
        let (project, _) = workspace
            .register_project(&root)
            .expect("registra proyecto vacío");

        let groups = sidebar_view(&workspace, SidebarSection::All);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].project, project);
        assert!(groups[0].tabs.is_empty());
        assert_eq!(
            sidebar_rows_from_groups(&groups),
            vec![SidebarRow::Project(project)]
        );
        assert!(sidebar_view(&workspace, SidebarSection::Agents).is_empty());
    }

    fn test_agent(
        id: AgentSessionId,
        kind: AgentKind,
        pid: u32,
        state: AgentState,
        seconds: u64,
    ) -> AgentSession {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
        let mut session = AgentSession::new(id, kind, pid, now);
        let mut report = AgentReport::new(state);
        report.message = Some(format!("tarea {id}"));
        session.apply_report(&report, now);
        session
    }

    #[test]
    fn sidebar_agents_filter_keeps_agent_tabs_and_orders_by_priority() {
        let (mut workspace, project, tab) = sidebar_workspace("agents-section");
        let pane = workspace.pane_of_tab(tab).expect("panel");
        let second = workspace
            .new_tab(pane, project, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
            .expect("segunda pestaña")
            .tab;
        let _third_without_agent = workspace
            .new_tab(pane, project, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
            .expect("tercera pestaña sin agente")
            .tab;

        workspace.set_tab_agents(
            tab,
            vec![test_agent(
                1,
                AgentKind::OpenCode,
                10,
                AgentState::Working,
                1,
            )],
        );
        workspace.set_tab_agents(
            second,
            vec![test_agent(
                2,
                AgentKind::Copilot,
                11,
                AgentState::NeedsAttention,
                2,
            )],
        );
        let _ = workspace.set_project_expanded(project, false);

        let groups = sidebar_view(&workspace, SidebarSection::Agents);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].show_tabs, "en Agentes siempre se despliegan");
        assert_eq!(groups[0].tabs, vec![second, tab], "atención primero");

        let rows = sidebar_rows_from_groups(&groups);
        assert_eq!(
            rows,
            vec![
                SidebarRow::Project(project),
                SidebarRow::Tab(second),
                SidebarRow::Tab(tab),
            ]
        );
    }

    #[test]
    fn sidebar_sections_toggle_between_all_and_agents() {
        assert_eq!(SidebarSection::All.toggled(), SidebarSection::Agents);
        assert_eq!(SidebarSection::Agents.toggled(), SidebarSection::All);
        assert_eq!(SidebarSection::All.label(), "Todas");
        assert_eq!(SidebarSection::Agents.label(), "Agentes");
    }

    #[test]
    fn agent_states_map_to_tab_status_and_icons() {
        assert_eq!(
            tab_status_from_state(AgentState::NeedsAttention),
            TabStatus::Attention
        );
        assert_eq!(
            tab_status_from_state(AgentState::Finished),
            TabStatus::Finished
        );
        assert_eq!(tab_status_from_state(AgentState::Error), TabStatus::Error);
        assert_eq!(
            tab_status_from_state(AgentState::Working),
            TabStatus::Working
        );
        assert_eq!(agent_icon(AgentKind::OpenCode), icons::BRAND_OPENCODE);
        assert_eq!(agent_icon(AgentKind::Copilot), icons::BRAND_COPILOT);
    }

    #[test]
    fn collapsed_project_shows_its_most_urgent_agent_state() {
        assert_eq!(most_urgent_agent_state([]), None);
        assert_eq!(
            most_urgent_agent_state([AgentState::Idle, AgentState::Working, AgentState::Error]),
            Some(AgentState::Working)
        );
        assert_eq!(
            most_urgent_agent_state([AgentState::Working, AgentState::NeedsAttention]),
            Some(AgentState::NeedsAttention)
        );
    }

    #[test]
    fn durations_are_compact() {
        assert_eq!(format_duration(Duration::from_secs(9)), "9s");
        assert_eq!(format_duration(Duration::from_secs(61)), "1m 01s");
        assert_eq!(format_duration(Duration::from_secs(3_601)), "1h 00m");
    }

    #[test]
    fn osc_status_maps_to_agent_states_and_attention() {
        let mut status = ProgramStatus {
            state: ProgramState::Blocked,
            kind: Some("auth".into()),
            progress: Some(5),
            app: Some("copilot".into()),
            title: None,
            message: Some("Login".into()),
            id: None,
        };
        let report = agent_report_from_status(&status);
        assert_eq!(report.state, AgentState::NeedsAttention);
        assert_eq!(report.attention, Some(AttentionKind::Auth));
        assert!(!report.clear);

        status.state = ProgramState::Clear;
        let report = agent_report_from_status(&status);
        assert!(report.clear);
        assert_eq!(report.state, AgentState::Idle);
    }

    #[test]
    fn reconcile_adds_matches_and_closes_processes() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let mut next_id = 1;
        let mut sessions = Vec::new();
        let (changed, transitions) = reconcile_agents(
            &mut sessions,
            &[DetectedAgent {
                kind: AgentKind::OpenCode,
                pid: 42,
            }],
            true,
            None,
            true,
            now,
            &mut next_id,
        );
        assert!(changed);
        assert!(transitions.is_empty());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].state, AgentState::Working);

        let (changed, transitions) =
            reconcile_agents(&mut sessions, &[], true, None, false, now, &mut next_id);
        assert!(changed);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].state, AgentState::Finished);
        assert_eq!(transitions.len(), 1);

        // Si el usuario lanza un segundo agente del mismo tipo, reutiliza la
        // única sesión de la terminal con su nuevo PID.
        let (changed, _) = reconcile_agents(
            &mut sessions,
            &[
                DetectedAgent {
                    kind: AgentKind::OpenCode,
                    pid: 99,
                },
                DetectedAgent {
                    kind: AgentKind::OpenCode,
                    pid: 100,
                },
            ],
            true,
            None,
            false,
            now,
            &mut next_id,
        );
        assert!(changed);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].pid, 99);
        assert!(sessions[0].alive);
    }

    #[test]
    fn shell_exit_uses_the_exit_code_for_pending_sessions() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let mut next_id = 5;
        let mut sessions = vec![test_agent(
            1,
            AgentKind::OpenCode,
            42,
            AgentState::Working,
            90,
        )];
        let (_, transitions) = reconcile_agents(
            &mut sessions,
            &[],
            false,
            Some(axf_terminal::ExitInfo {
                code: Some(1),
                success: false,
            }),
            false,
            now,
            &mut next_id,
        );
        assert_eq!(sessions[0].state, AgentState::Error);
        assert_eq!(transitions.len(), 1);
    }

    #[test]
    fn context_path_hides_the_root_and_compacts_parents() {
        let separator = std::path::MAIN_SEPARATOR;
        let root = std::path::PathBuf::from("proyecto");
        let inside = root.join("crates").join("app");
        let outside = std::path::PathBuf::from("otro").join("lugar");

        assert_eq!(
            context_path(&root, &inside),
            Some(format!("crates{separator}app"))
        );
        assert_eq!(context_path(&root, &root), None);
        assert_eq!(
            context_path(&root, &outside),
            Some(format!("…{separator}lugar"))
        );
    }
}
