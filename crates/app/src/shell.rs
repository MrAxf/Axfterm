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
use crate::font_select::{FontList, FontValue, font_list};
use crate::history::HistoryStore;
use crate::keymap::Keymap;
use crate::palette::{
    self, PaletteHistory, PaletteIcon, PaletteItem, PaletteKind, PaletteMode, PaletteState,
    PaletteTarget,
};
use crate::settings::{
    DetectState, ExternalOutcome, PendingDelete, SettingsSection, SettingsState,
};
use crate::theme_host::{ApplyReport, ThemeHost};
use crate::{
    AgentNextAttention, ClosePane, CloseTab, DuplicateTab, ExportTheme, FocusNextPane,
    FocusPreviousPane, FocusSidebar, ImportTheme, MoveTabNextPane, NewTab, NextTab, NextTheme,
    OpenCommandPalette, OpenProject, OpenSearchPalette, OpenSettings, OpenThemesFolder,
    PALETTE_CONTEXT, PaletteAccept, PaletteCancel, PaletteFilterAgents, PaletteFilterAll,
    PaletteFilterPanes, PaletteFilterProfiles, PaletteFilterProjects, PaletteFilterTabs,
    PaletteNext, PalettePrevious, PaletteSwitchMode, PreviousTab, PreviousTheme, Quit,
    ReloadThemes, RenameTab, RestartTab, SETTINGS_CONTEXT, SHORTCUTS_CONTEXT, SIDEBAR_CONTEXT,
    SettingsCancel, SettingsNextSection, SettingsPreviousSection, SettingsSave, ShowShortcuts,
    SplitHorizontal, SplitVertical, ToggleSidebar, ToggleSidebarAgents, ToggleThemeVariant,
    ToggleZoom, WORKSPACE_CONTEXT, keys,
};
use axf_agents::{
    AgentKind, AgentReport, AgentSession, AgentSessionId, AgentState, AgentTransition,
    AttentionKind, DetectedAgent, ProcessScanner, ProcessSnapshot,
};
use axf_config::{Profile, ProfileId};
use axf_terminal::{
    ProgramState, ProgramStatus, TerminalSignal, TerminalSnapshot, TerminalStyle, TerminalView,
};
use axf_ui::components::{
    Button, ControlSize, EmptyState, Icon, IconButton, IconSize, Kbd, Modal, ProfileIcon,
    ProfileIconSource, Tab as TabView, TabStatus, TextField, Toast, text_tooltip,
};
use axf_ui::gpui::base::FocusTrapElement as _;
use axf_ui::gpui::component::button::{
    Button as KitButton, ButtonVariants as _, Toggle as KitToggle, ToggleGroup,
};
use axf_ui::gpui::component::input::{InputEvent, InputState};
use axf_ui::gpui::component::menu::{
    ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem,
};
use axf_ui::gpui::component::scroll::ScrollableElement as _;
use axf_ui::gpui::component::select::{Select, SelectEvent, SelectState};
use axf_ui::gpui::component::switch::Switch;
use axf_ui::gpui::component::{
    ActiveTheme, Disableable as _, ElementExt as _, Sizable as _, WindowExt as _,
};
use axf_ui::gpui::prelude::FluentBuilder as _;

#[cfg(target_os = "windows")]
use axf_ui::gpui::svg;
use axf_ui::gpui::{
    AnyElement, AppContext, Bounds, ClickEvent, Context, Div, ElementId, Entity, FocusHandle,
    Focusable, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, Keystroke, MouseButton,
    ParentElement, Pixels, Point, Render, ScrollDelta, ScrollEvent, ScrollHandle, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled, Subscription, Window, WindowControlArea,
    div, point, px, relative, transparent_black,
};
use axf_ui::icons;
use axf_ui::metrics::UiMetrics;
use axf_ui::theme;
use axf_workspace::{
    Branch, PaneId, PaneTree, ProjectId, SessionFile, SidebarSection, SplitAxis, TabId, TerminalId,
    Workspace,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

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

/// Pestañas con alguna sesión que requiere atención, en orden de proyectos.
fn attention_tabs(workspace: &Workspace) -> Vec<TabId> {
    workspace
        .projects()
        .iter()
        .flat_map(|project| workspace.project_tabs(project.id))
        .filter(|tab| {
            workspace.tab_agents(*tab).is_some_and(|sessions| {
                sessions
                    .iter()
                    .any(|session| session.state == AgentState::NeedsAttention)
            })
        })
        .collect()
}

/// Siguiente pestaña en atención, ciclando desde la actual.
fn next_attention(attention: &[TabId], current: Option<TabId>) -> Option<TabId> {
    let first = *attention.first()?;
    Some(
        current
            .and_then(|current| attention.iter().position(|tab| *tab == current))
            .map(|index| attention[(index + 1) % attention.len()])
            .unwrap_or(first),
    )
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

/// Editor inline de un perfil dentro del panel de ajustes.
struct ProfileEditor {
    id: ProfileId,
    name: Entity<InputState>,
    command: Entity<InputState>,
    icon: Option<PathBuf>,
    error: Option<String>,
}

/// Campo numérico ajustable del panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StepField {
    UiFontSize,
    UiScale,
    TerminalFontSize,
    LineHeight,
    ScanInterval,
}

/// Vista raíz de la ventana.
pub(crate) struct Shell {
    workspace: Workspace,
    session_path: Option<PathBuf>,
    focus: FocusHandle,
    sidebar_focus: FocusHandle,
    settings_focus: FocusHandle,
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
    palette: Option<PaletteState>,
    palette_subscription: Option<Subscription>,
    palette_history: PaletteHistory,
    ui_font_select: Option<Entity<SelectState<FontList>>>,
    terminal_font_select: Option<Entity<SelectState<FontList>>>,
    font_select_subscriptions: Vec<Subscription>,
    shortcuts_modal: bool,
    shortcuts_focus: FocusHandle,
    keymap: Keymap,
    theme_host: ThemeHost,
    settings: Option<SettingsState>,
    settings_scroll: ScrollHandle,
    settings_profile_editor: Option<ProfileEditor>,
    settings_shortcut_input: Option<Entity<InputState>>,
    settings_shortcut_subscription: Option<Subscription>,
    keystroke_interceptor: Option<Subscription>,
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
        keymap: Keymap,
        keymap_notes: Vec<String>,
    ) -> Self {
        let focus = cx.focus_handle();
        let sidebar_focus = cx.focus_handle();
        let settings_focus = cx.focus_handle();
        let shortcuts_focus = cx.focus_handle();
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
            settings_focus,
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
            palette: None,
            palette_subscription: None,
            palette_history: PaletteHistory::default(),
            ui_font_select: None,
            terminal_font_select: None,
            font_select_subscriptions: Vec::new(),
            shortcuts_modal: false,
            shortcuts_focus,
            keymap,
            theme_host,
            settings: None,
            settings_scroll: ScrollHandle::new(),
            settings_profile_editor: None,
            settings_shortcut_input: None,
            settings_shortcut_subscription: None,
            keystroke_interceptor: None,
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
        shell.keystroke_interceptor = Some(cx.intercept_keystrokes(move |event, _window, cx| {
            let handled = this
                .update(cx, |shell, cx| {
                    shell.settings_capture_keystroke(&event.keystroke, cx)
                })
                .unwrap_or(false);
            if handled {
                cx.stop_propagation();
            }
        }));
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
        warnings.extend(keymap_notes);
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
                    let panel_config = event.config && shell.settings.is_some();
                    let report = if panel_config {
                        let source = shell
                            .theme_host
                            .config_path
                            .as_deref()
                            .and_then(|path| axf_config::read_config_text(path).ok().flatten());
                        shell.settings_external_change(source, cx);
                        if event.themes {
                            shell.theme_host.reload(
                                axf_config::WatchEvent {
                                    config: false,
                                    themes: true,
                                },
                                cx,
                            )
                        } else {
                            ApplyReport::default()
                        }
                    } else {
                        shell.theme_host.reload(event, cx)
                    };
                    if report.applied {
                        cx.notify();
                    }
                    let mut keymap_notes = Vec::new();
                    if report.applied {
                        let keymap = Keymap::resolve(&shell.theme_host.settings.keymap);
                        keymap_notes.extend(keymap.errors.iter().cloned());
                        keymap_notes.extend(keymap.warnings.iter().cloned());
                        keymap_notes.extend(keymap.install(cx));
                        shell.keymap = keymap;
                        cx.notify();
                    }
                    let had = !report.errors.is_empty() || !report.messages.is_empty();
                    let has_error = !report.errors.is_empty();
                    let mut parts = report.errors;
                    parts.extend(report.messages);
                    if !parts.is_empty() {
                        shell.set_notice(parts.join(" "), has_error, cx);
                    }
                    if !keymap_notes.is_empty() {
                        shell.set_notice(keymap_notes.join(" "), true, cx);
                    }
                    if !had && keymap_notes.is_empty() && !panel_config {
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
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_palette(PaletteMode::Commands, window, cx);
    }

    fn open_search_palette(
        &mut self,
        _: &OpenSearchPalette,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_palette(PaletteMode::Search, window, cx);
    }

    /// Abre una de las dos paletas; si la otra estaba abierta, la reemplaza.
    fn open_palette(&mut self, mode: PaletteMode, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self
            .palette
            .as_ref()
            .is_some_and(|palette| palette.mode == mode)
        {
            return;
        }
        self.palette = None;
        self.palette_subscription = None;
        self.shortcuts_modal = false;
        let items = self.palette_items(mode, cx);
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(mode.placeholder()));
        let subscription = cx.subscribe(&input, |shell, input, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let query = input.read(cx).value().to_string();
            let history = shell.palette_history.clone();
            if let Some(palette) = shell.palette.as_mut() {
                palette.refilter(&query, &history);
            }
            cx.notify();
        });
        let palette = PaletteState::new(mode, input.clone(), items, &self.palette_history);
        self.palette = Some(palette);
        self.palette_subscription = Some(subscription);
        let handle = input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    /// Recolecta las entradas de cada paleta. El orden de inserción es el
    /// desempate estable.
    fn palette_items(&mut self, mode: PaletteMode, cx: &mut Context<'_, Self>) -> Vec<PaletteItem> {
        match mode {
            PaletteMode::Commands => self.command_items(),
            PaletteMode::Search => self.search_items(cx),
        }
    }

    /// Acciones del registro, sin las que ya cubren la búsqueda o el modal de tema.
    fn command_items(&self) -> Vec<PaletteItem> {
        crate::actions::palette_actions()
            .map(|action| PaletteItem {
                id: format!("cmd:{}", action.id),
                label: action.label.to_owned(),
                detail: Some(action.category.label().to_owned()),
                search: format!("{} {}", action.id, action.keywords.join(" ")),
                kind: PaletteKind::Command,
                icon: PaletteIcon::Svg(crate::actions::category_icon(action.category)),
                shortcut: self.keymap.chords_for(action.id),
                target: PaletteTarget::Command(action.id),
            })
            .collect()
    }

    /// Navegación: proyectos, paneles, pestañas, agentes y perfiles.
    fn search_items(&mut self, cx: &mut Context<'_, Self>) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        let projects: Vec<(ProjectId, String, std::path::PathBuf)> = self
            .workspace
            .projects()
            .iter()
            .map(|project| (project.id, project.name.clone(), project.root.clone()))
            .collect();
        for (project, name, root) in projects {
            let count = self.workspace.project_tabs(project).len();
            items.push(PaletteItem {
                id: format!("project:{}", project.get()),
                label: name,
                detail: Some(format!(
                    "{count} terminal{} · {}",
                    if count == 1 { "" } else { "es" },
                    root.display()
                )),
                search: root.display().to_string(),
                kind: PaletteKind::Project,
                icon: PaletteIcon::Svg(axf_ui::icons::FOLDER),
                shortcut: None,
                target: PaletteTarget::RevealProject(project),
            });
        }
        for pane in self.workspace.layout().leaves() {
            let count = self
                .workspace
                .pane(pane)
                .map(|pane| pane.tabs.len())
                .unwrap_or(0);
            items.push(PaletteItem {
                id: format!("pane:{}", pane.get()),
                label: format!("Panel {}", pane.get()),
                detail: Some(format!(
                    "{count} pestaña{}",
                    if count == 1 { "" } else { "s" }
                )),
                search: format!("pane panel {}", pane.get()),
                kind: PaletteKind::Pane,
                icon: PaletteIcon::Svg(axf_ui::icons::SPLIT_HORIZONTAL),
                shortcut: None,
                target: PaletteTarget::FocusPane(pane),
            });
        }
        let tabs: Vec<(TabId, ProjectId)> = self
            .workspace
            .projects()
            .iter()
            .flat_map(|project| {
                self.workspace
                    .project_tabs(project.id)
                    .into_iter()
                    .map(move |tab| (tab, project.id))
            })
            .collect();
        for (tab, project_id) in &tabs {
            let title = self.tab_title(*tab, cx);
            let project = self
                .workspace
                .project(*project_id)
                .map(|project| project.name.clone())
                .unwrap_or_default();
            let attention = self.workspace.tab_agents(*tab).is_some_and(|sessions| {
                sessions
                    .iter()
                    .any(|session| session.state == AgentState::NeedsAttention)
            });
            items.push(PaletteItem {
                id: format!("tab:{}", tab.get()),
                label: format!("{}{title}", if attention { "! " } else { "" }),
                detail: Some(project),
                search: format!("terminal tab pestaña {}", tab.get()),
                kind: PaletteKind::Tab,
                icon: PaletteIcon::Profile(self.tab_icon(*tab)),
                shortcut: None,
                target: PaletteTarget::FocusTab(*tab),
            });
        }
        for (tab, project_id) in &tabs {
            let Some(sessions) = self.workspace.tab_agents(*tab) else {
                continue;
            };
            for session in sessions {
                if session.state != AgentState::NeedsAttention {
                    continue;
                }
                let title = self.tab_title(*tab, cx);
                let project = self
                    .workspace
                    .project(*project_id)
                    .map(|project| project.name.clone())
                    .unwrap_or_default();
                items.push(PaletteItem {
                    id: format!("agent:{}:{}", tab.get(), session.id),
                    label: format!("{} requiere atención · {title}", session.kind.as_str()),
                    detail: Some(project),
                    search: format!("agente agent atencion attention {}", session.kind.as_str()),
                    kind: PaletteKind::Agent,
                    icon: PaletteIcon::Svg(agent_icon(session.kind)),
                    shortcut: None,
                    target: PaletteTarget::JumpToAttention(*tab),
                });
            }
        }
        let profiles: Vec<(ProfileId, String)> = self
            .theme_host
            .profiles()
            .iter()
            .map(|profile| (profile.id.clone(), profile.name.clone()))
            .collect();
        for (id, name) in profiles {
            let icon = self.theme_host.profile_icon(&id);
            items.push(PaletteItem {
                id: format!("profile:{}", id.as_str()),
                label: format!("Nueva terminal · {name}"),
                detail: Some(format!("perfil {}", id.as_str())),
                search: format!("perfil profile terminal nueva {}", id.as_str()),
                kind: PaletteKind::Profile,
                icon: PaletteIcon::Profile(icon),
                shortcut: None,
                target: PaletteTarget::NewTabWithProfile(id),
            });
        }
        items
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.palette.take().is_none() {
            return;
        }
        self.palette_subscription = None;
        self.restore_workspace_focus(window, cx);
        cx.notify();
    }

    fn palette_next(&mut self, _: &PaletteNext, _: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(palette) = self.palette.as_mut() {
            palette.next();
            cx.notify();
        }
    }

    fn palette_previous(
        &mut self,
        _: &PalettePrevious,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(palette) = self.palette.as_mut() {
            palette.previous();
            cx.notify();
        }
    }

    fn palette_switch_mode(
        &mut self,
        _: &PaletteSwitchMode,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(next) = self.palette.as_ref().map(|palette| match palette.mode {
            PaletteMode::Commands => PaletteMode::Search,
            PaletteMode::Search => PaletteMode::Commands,
        }) else {
            return;
        };
        self.palette_set_mode(next, window, cx);
    }

    /// Cambia de paleta sin cerrar el overlay y sin perder consulta ni filtro.
    fn palette_set_mode(
        &mut self,
        mode: PaletteMode,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((current, query)) = self
            .palette
            .as_ref()
            .map(|palette| (palette.mode, palette.input.read(cx).value().to_string()))
        else {
            return;
        };
        if current != mode {
            let items = self.palette_items(mode, cx);
            let history = self.palette_history.clone();
            if let Some(palette) = self.palette.as_mut() {
                palette.set_mode(mode, items, &query, &history);
            }
        }
        self.palette_focus_input(window, cx);
        cx.notify();
    }

    fn palette_set_filter(
        &mut self,
        filter: Option<PaletteKind>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !self
            .palette
            .as_ref()
            .is_some_and(|palette| palette.mode == PaletteMode::Search)
        {
            return;
        }
        let Some(query) = self
            .palette
            .as_ref()
            .map(|palette| palette.input.read(cx).value().to_string())
        else {
            return;
        };
        let history = self.palette_history.clone();
        if let Some(palette) = self.palette.as_mut() {
            palette.set_filter(filter, &query, &history);
        }
        self.palette_focus_input(window, cx);
        cx.notify();
    }

    fn palette_filter_all(
        &mut self,
        _: &PaletteFilterAll,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(None, window, cx);
    }

    fn palette_filter_tabs(
        &mut self,
        _: &PaletteFilterTabs,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Tab), window, cx);
    }

    fn palette_filter_panes(
        &mut self,
        _: &PaletteFilterPanes,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Pane), window, cx);
    }

    fn palette_filter_projects(
        &mut self,
        _: &PaletteFilterProjects,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Project), window, cx);
    }

    fn palette_filter_profiles(
        &mut self,
        _: &PaletteFilterProfiles,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Profile), window, cx);
    }

    fn palette_filter_agents(
        &mut self,
        _: &PaletteFilterAgents,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.palette_set_filter(Some(PaletteKind::Agent), window, cx);
    }

    /// Devuelve el foco al input tras pulsar un segmento de modo o de filtro.
    fn palette_focus_input(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(input) = self.palette.as_ref().map(|palette| palette.input.clone()) else {
            return;
        };
        let handle = input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    fn palette_accept(
        &mut self,
        _: &PaletteAccept,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((id, target)) = self.palette.as_ref().and_then(|palette| {
            let row = palette.current()?;
            let item = palette.item_at(row);
            Some((item.id.clone(), item.target.clone()))
        }) else {
            return;
        };
        self.palette_history.record(&id);
        self.close_palette(window, cx);
        self.run_palette_target(target, window, cx);
    }

    fn palette_cancel(
        &mut self,
        _: &PaletteCancel,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
            return;
        }
        if self.shortcuts_modal {
            self.close_shortcuts(window, cx);
        }
    }

    fn palette_click(&mut self, row: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(palette) = self.palette.as_mut() {
            if row >= palette.rows.len() {
                return;
            }
            palette.selected = row;
        }
        self.palette_accept(&PaletteAccept, window, cx);
    }

    fn run_palette_target(
        &mut self,
        target: PaletteTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match target {
            PaletteTarget::Command(id) => {
                if let Some(action) = crate::actions::action_for(id) {
                    // `App::dispatch_action` reentra en la ventana que ya está
                    // actualizando este shell y el despacho se pierde. Window
                    // encola el despacho para después del frame actual.
                    window.dispatch_action(action, cx);
                }
            }
            PaletteTarget::FocusTab(tab) | PaletteTarget::JumpToAttention(tab) => {
                self.focus_tab(tab, window, cx);
            }
            PaletteTarget::FocusPane(pane) => {
                if self.workspace.focus_pane(pane).is_ok() {
                    if let Some(tab) = self.workspace.focus().tab {
                        self.focus_tab(tab, window, cx);
                    } else {
                        window.focus(&self.focus, cx);
                        cx.notify();
                    }
                }
            }
            PaletteTarget::RevealProject(project) => {
                self.workspace.sidebar_mut().visible = true;
                if self.sidebar_section != SidebarSection::All {
                    self.sidebar_section = SidebarSection::All;
                    self.workspace.sidebar_mut().section = SidebarSection::All;
                }
                let _ = self.workspace.set_project_expanded(project, true);
                self.sidebar_cursor = Some(SidebarRow::Project(project));
                self.persist(cx);
                cx.notify();
            }
            PaletteTarget::NewTabWithProfile(profile) => {
                self.new_tab_for(None, Some(profile), window, cx);
            }
        }
    }

    fn toggle_shortcuts(
        &mut self,
        _: &ShowShortcuts,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.shortcuts_modal {
            self.close_shortcuts(window, cx);
        } else {
            self.close_palette(window, cx);
            self.close_settings(window, cx);
            self.shortcuts_modal = true;
            window.focus(&self.shortcuts_focus, cx);
            cx.notify();
        }
    }

    fn close_shortcuts(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.shortcuts_modal {
            self.shortcuts_modal = false;
            self.restore_workspace_focus(window, cx);
            cx.notify();
        }
    }

    fn agent_next_attention(
        &mut self,
        _: &AgentNextAttention,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let attention = attention_tabs(&self.workspace);
        let Some(next) = next_attention(&attention, self.workspace.focus().tab) else {
            self.set_sidebar_section(SidebarSection::Agents, cx);
            self.set_notice("Ningún agente requiere atención.", false, cx);
            return;
        };
        self.focus_tab(next, window, cx);
    }

    // --- Ajustes -------------------------------------------------------
    /// Abre (o cierra) el panel de ajustes con el foco dentro.
    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.settings.is_some() {
            self.close_settings(window, cx);
            return;
        }
        self.close_palette(window, cx);
        self.shortcuts_modal = false;
        let baseline = self
            .theme_host
            .config_path
            .as_deref()
            .and_then(|path| axf_config::read_config_text(path).ok().flatten());
        self.settings = Some(SettingsState::new(
            self.theme_host.settings.clone(),
            baseline,
        ));
        self.settings_scroll = ScrollHandle::new();
        self.settings_profile_editor = None;
        self.ensure_font_selects(window, cx);
        self.ensure_shortcut_input(window, cx);
        window.focus(&self.settings_focus, cx);
        cx.notify();
    }

    fn show_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.open_settings(window, cx);
    }

    fn settings_cancel(
        &mut self,
        _: &SettingsCancel,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(settings) = &self.settings
            && settings.capturing.is_some()
        {
            if let Some(settings) = &mut self.settings {
                settings.cancel_capture();
            }
            cx.notify();
            return;
        }
        if self.settings_profile_editor.is_some() {
            self.settings_profile_editor = None;
            cx.notify();
            return;
        }
        self.close_settings(window, cx);
    }

    fn settings_save(&mut self, _: &SettingsSave, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.save_settings(cx);
    }

    fn settings_next_section(
        &mut self,
        _: &SettingsNextSection,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(settings) = &mut self.settings {
            settings.move_section(1);
            self.settings_scroll = ScrollHandle::new();
            cx.notify();
        }
    }

    fn settings_previous_section(
        &mut self,
        _: &SettingsPreviousSection,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(settings) = &mut self.settings {
            settings.move_section(-1);
            self.settings_scroll = ScrollHandle::new();
            cx.notify();
        }
    }

    /// Cierra el panel descartando el borrador y restaurando la vista activa.
    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.settings.take().is_none() {
            return;
        }
        self.settings_profile_editor = None;
        self.settings_shortcut_input = None;
        self.settings_shortcut_subscription = None;
        self.font_select_subscriptions.clear();
        self.ui_font_select = None;
        self.terminal_font_select = None;
        let messages = self.theme_host.clear_preview(cx);
        if !messages.is_empty() {
            tracing::debug!("preview de ajustes descartada: {}", messages.join(" "));
        }
        self.restore_workspace_focus(window, cx);
        cx.notify();
    }

    /// Aplica el borrador al estado visual sin escribir en disco.
    fn preview_settings(&mut self, cx: &mut Context<'_, Self>) {
        let Some(settings) = &self.settings else {
            return;
        };
        let messages = self.theme_host.preview_settings(&settings.draft, cx);
        if !messages.is_empty()
            && let Some(settings) = &mut self.settings
        {
            settings.set_notice(messages.join(" "), false);
        }
        cx.notify();
    }

    /// Guarda el borrador validado y con la política de conflicto de ADR 0007.
    fn save_settings(&mut self, cx: &mut Context<'_, Self>) {
        if self.settings_profile_editor.is_some() {
            if let Some(settings) = &mut self.settings {
                settings.set_notice(
                    "Aplica o cancela la edición del perfil antes de guardar.".into(),
                    true,
                );
            }
            cx.notify();
            return;
        }
        if self
            .settings
            .as_ref()
            .is_some_and(|settings| settings.capturing.is_some())
        {
            if let Some(settings) = &mut self.settings {
                settings.set_notice(
                    "Termina o cancela la captura del atajo antes de guardar.".into(),
                    true,
                );
            }
            cx.notify();
            return;
        }
        let Some(path) = self.theme_host.config_path.clone() else {
            if let Some(settings) = &mut self.settings {
                settings.set_notice("No se pudo resolver config.toml.".into(), true);
            }
            cx.notify();
            return;
        };
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let errors = settings.validate();
        if !errors.is_empty() {
            if let Some(settings) = &mut self.settings {
                settings.set_errors(errors.clone());
                settings.set_notice(errors[0].clone(), true);
            }
            cx.notify();
            return;
        }
        let current = axf_config::read_config_text(&path).ok().flatten();
        if current != settings.baseline && !settings.overwrite {
            if let Some(settings) = &mut self.settings {
                settings.conflict = Some(crate::settings::Conflict {
                    source: current.unwrap_or_default(),
                    summary:
                        "config.toml cambió fuera del panel. Guardar sobrescribirá ese cambio; \
                              recarga para descartar el borrador."
                            .into(),
                });
            }
            cx.notify();
            return;
        }
        let base = settings.base.clone();
        let draft = settings.draft.clone();
        match axf_config::write_config(&path, &base, &draft) {
            Ok(written) => {
                let messages = self.theme_host.commit_settings(draft.clone(), cx);
                if let Some(settings) = &mut self.settings {
                    settings.commit_saved(written);
                    if !messages.is_empty() {
                        settings.set_notice(messages.join(" "), false);
                    }
                }
                let keymap = Keymap::resolve(&draft.keymap);
                let mut keymap_errors = keymap.errors.clone();
                keymap_errors.extend(keymap.install(cx));
                self.keymap = keymap;
                if !keymap_errors.is_empty() {
                    self.set_notice(keymap_errors.join(" "), true, cx);
                }
                cx.notify();
            }
            Err(error) => {
                if let Some(settings) = &mut self.settings {
                    settings.set_errors(vec![error.to_string()]);
                    settings.set_notice(error.to_string(), true);
                }
                cx.notify();
            }
        }
    }

    /// Resultado de un cambio externo mientras el panel está abierto.
    fn settings_external_change(&mut self, source: Option<String>, cx: &mut Context<'_, Self>) {
        let Some(source) = source else {
            return;
        };
        let parsed = axf_config::parse_str(&source, "config.toml");
        let Some(outcome) = self
            .settings
            .as_mut()
            .map(|settings| settings.external_change(source, parsed))
        else {
            return;
        };
        match outcome {
            ExternalOutcome::Reloaded => {
                let Some(draft) = self
                    .settings
                    .as_ref()
                    .map(|settings| settings.draft.clone())
                else {
                    return;
                };

                // El archivo ya cambió: la config activa pasa a ser la nueva y
                // el panel deja de previsualizar. Cancelar no debe revertirla.
                let messages = self.theme_host.commit_settings(draft.clone(), cx);
                let keymap = Keymap::resolve(&draft.keymap);
                let mut notes = keymap.errors.clone();
                notes.extend(keymap.install(cx));
                self.keymap = keymap;
                if let Some(settings) = &mut self.settings {
                    let mut parts = messages;
                    parts.extend(notes);
                    settings.set_notice(
                        if parts.is_empty() {
                            "config.toml cambió fuera del panel; borrador recargado.".into()
                        } else {
                            parts.join(" ")
                        },
                        false,
                    );
                }
            }
            ExternalOutcome::Conflict => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(
                        "Hay cambios externos sin guardar. Recarga o sigue editando; \
                         guardar pedirá confirmación."
                            .into(),
                        true,
                    );
                }
            }
            ExternalOutcome::Invalid(message) => {
                tracing::debug!("{message}");
            }
            ExternalOutcome::Ignored => {}
        }
        cx.notify();
    }

    /// Recarga el archivo externo descartando el borrador.
    fn reload_settings_from_disk(&mut self, cx: &mut Context<'_, Self>) {
        let Some(path) = self.theme_host.config_path.clone() else {
            return;
        };
        let source = axf_config::read_config_text(&path).ok().flatten();
        let parsed = source
            .as_deref()
            .map(|source| axf_config::parse_str(source, "config.toml"));
        match (source, parsed) {
            (Some(source), Some(Ok(config))) => {
                // El usuario descarta el borrador: la config del disco pasa a
                // ser la activa, no una previsualización.
                let messages = self.theme_host.commit_settings(config.clone(), cx);
                let keymap = Keymap::resolve(&config.keymap);
                let mut notes = keymap.errors.clone();
                notes.extend(keymap.install(cx));
                self.keymap = keymap;
                if let Some(settings) = &mut self.settings {
                    settings.reload_external(config, source);
                    let mut parts = messages;
                    parts.extend(notes);
                    if !parts.is_empty() {
                        settings.set_notice(parts.join(" "), false);
                    }
                }
                cx.notify();
            }
            (_, Some(Err(error))) => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(format!("No se pudo recargar: {error}"), true);
                }
                cx.notify();
            }
            _ => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(
                        "config.toml no existe; se conserva el borrador.".into(),
                        true,
                    );
                }
                cx.notify();
            }
        }
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

                // Si el shell tampoco arrancó, manda su error, no el del historial.
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
        let settings = self
            .settings
            .is_some()
            .then(|| self.render_settings(window, cx));
        let shortcuts = self.shortcuts_modal.then(|| self.render_shortcuts(cx));
        let palette = self
            .palette
            .is_some()
            .then(|| self.render_palette(window, cx));
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
            .on_action(cx.listener(Self::open_search_palette))
            .on_action(cx.listener(Self::toggle_sidebar_agents))
            .on_action(cx.listener(Self::show_settings))
            .on_action(cx.listener(Self::toggle_theme_variant))
            .on_action(cx.listener(Self::next_theme))
            .on_action(cx.listener(Self::previous_theme))
            .on_action(cx.listener(Self::reload_themes))
            .on_action(cx.listener(Self::open_themes_folder))
            .on_action(cx.listener(Self::import_theme))
            .on_action(cx.listener(Self::export_theme))
            .on_action(cx.listener(Self::agent_next_attention))
            .on_action(cx.listener(Self::toggle_shortcuts))
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
            .children(settings)
            .children(shortcuts)
            .children(palette)
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
            .tooltip(format!("Ajustes ({})", keys::format_chord(keys::SETTINGS)))
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_settings(window, cx);
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
                "Buscar pestañas, paneles y proyectos ({})",
                keys::format_chord(keys::SEARCH_PALETTE)
            )))
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_palette(PaletteMode::Search, window, cx);
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
                    .child("Buscar pestañas, paneles y proyectos…"),
            );
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
                                .item(PopupMenuItem::new("Ajustes…").on_click({
                                    let this = this.clone();
                                    move |_, window, cx| {
                                        if let Some(this) = this.upgrade() {
                                            let _ = this.update(cx, |shell, cx| {
                                                shell.open_settings(window, cx);
                                            });
                                        }
                                    }
                                }))
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
                    "El perfil `{}` ya no existe en config.toml. Elige el perfil predeterminado o \
                     cierra la pestaña; no se arranca otro shell por sorpresa.",
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
                        .on_click(cx.listener(
                            move |shell, _, window, cx| {
                                shell.use_default_profile(tab, window, cx);
                            },
                        )),
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

    fn render_palette(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(palette) = self.palette.as_ref() else {
            return div().into_any_element();
        };
        let input = palette.input.clone();
        let rows: Vec<(usize, PaletteItem, Vec<usize>)> = palette
            .rows
            .iter()
            .enumerate()
            .map(|(row, entry)| {
                let item = palette.item_at(entry).clone();
                (row, item, entry.indices.clone())
            })
            .collect();
        let selected = palette.selected;
        let scroll = palette.scroll.clone();
        let mode = palette.mode;
        let filter = palette.filter;
        let empty = rows.is_empty();
        let list_max_h = metrics
            .palette_list_max_height(window.viewport_size().height, mode == PaletteMode::Search);
        let body = if empty {
            let hint = if mode == PaletteMode::Search && filter.is_some() {
                "Pulsa Ctrl+1 para ver todo o prueba con otra palabra."
            } else {
                "Prueba con otra palabra."
            };
            div()
                .flex()
                .items_center()
                .justify_center()
                .min_h((metrics.font_size * 10.0).min(list_max_h))
                .py(metrics.spacing_lg())
                .child(EmptyState::new(icons::SEARCH, palette.empty_text(), hint).compact())
                .into_any_element()
        } else {
            let mut list = div()
                .id("palette-list")
                .flex()
                .flex_col()
                .gap(metrics.spacing_xs())
                .max_h(list_max_h)
                .track_scroll(&scroll)
                .overflow_y_scroll();
            for (row, item, indices) in rows {
                let active = row == selected;
                let color = if active { ui.accent } else { ui.text };
                let icon = match &item.icon {
                    PaletteIcon::Svg(path) => Icon::new(path)
                        .size(IconSize::Md)
                        .color(color)
                        .into_any_element(),
                    PaletteIcon::Profile(source) => ProfileIcon::new(source.clone())
                        .size(IconSize::Md)
                        .color(color)
                        .into_any_element(),
                };
                let spans: Vec<AnyElement> = palette::highlight(&item.label, &indices)
                    .into_iter()
                    .map(|span| {
                        div()
                            .when(span.matched, |this| this.text_color(ui.accent))
                            .child(span.text)
                            .into_any_element()
                    })
                    .collect();
                let shortcut = item
                    .shortcut
                    .clone()
                    .map(|shortcut| Kbd::new(shortcut).into_any_element());
                list = list.child(
                    div()
                        .id(("palette-row", row))
                        .flex()
                        .items_center()
                        .gap(metrics.spacing_sm())
                        .flex_shrink_0()
                        .min_h(metrics.control_height_md())
                        .px(metrics.palette_row_padding())
                        .py(metrics.palette_row_padding())
                        .rounded(metrics.radius_sm())
                        .when(active, |this| this.bg(ui.selection))
                        .when(!active, move |this| {
                            this.hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                        })
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.palette_click(row, window, cx);
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(metrics.palette_icon_lane())
                                .flex_shrink_0()
                                .child(icon),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(metrics.hairline())
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(metrics.spacing_xxs())
                                        .text_size(metrics.palette_label_size())
                                        .line_height(metrics.font_size * 1.05)
                                        .children(spans),
                                )
                                .when_some(item.detail.clone(), |this, detail| {
                                    this.child(
                                        div()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .text_size(metrics.palette_detail_size())
                                            .line_height(metrics.font_size * 0.9)
                                            .text_color(ui.muted)
                                            .child(detail),
                                    )
                                }),
                        )
                        .when(mode == PaletteMode::Search, |this| {
                            this.child(
                                div()
                                    .w(metrics.font_size * 5.5)
                                    .flex_shrink_0()
                                    .text_right()
                                    .text_size(metrics.palette_meta_size())
                                    .text_color(ui.muted)
                                    .child(item.kind.label()),
                            )
                        })
                        .children(shortcut),
                );
            }
            list.vertical_scrollbar(&scroll).into_any_element()
        };
        let mode_control = ToggleGroup::new("palette-mode")
            .small()
            .segmented()
            .single()
            .child(
                KitToggle::new("palette-mode-commands")
                    .label("Comandos")
                    .checked(mode == PaletteMode::Commands)
                    .tooltip("Cambiar a comandos (Ctrl+Tab)"),
            )
            .child(
                KitToggle::new("palette-mode-search")
                    .label("Búsqueda")
                    .checked(mode == PaletteMode::Search)
                    .tooltip("Cambiar a búsqueda (Ctrl+Tab)"),
            )
            .on_click(cx.listener(|shell, checks: &Vec<bool>, window, cx| {
                let mode = if checks.get(1).copied().unwrap_or(false) {
                    PaletteMode::Search
                } else {
                    PaletteMode::Commands
                };
                shell.palette_set_mode(mode, window, cx);
            }));
        let mut filter_control = ToggleGroup::new("palette-filter")
            .small()
            .segmented()
            .single()
            .child(
                KitToggle::new("palette-filter-all")
                    .label("Todo")
                    .checked(filter.is_none())
                    .tooltip("Todo (Ctrl+1)"),
            );
        for (index, kind) in PaletteKind::SEARCH_FILTERS.into_iter().enumerate() {
            filter_control = filter_control.child(
                KitToggle::new(("palette-filter", kind.order()))
                    .label(kind.filter_label())
                    .checked(filter == Some(kind))
                    .tooltip(format!("{} (Ctrl+{})", kind.filter_label(), index + 2)),
            );
        }
        let filter_control =
            filter_control.on_click(cx.listener(|shell, checks: &Vec<bool>, window, cx| {
                let filter = checks
                    .iter()
                    .position(|checked| *checked)
                    .and_then(|index| index.checked_sub(1))
                    .and_then(|index| PaletteKind::SEARCH_FILTERS.get(index).copied());
                shell.palette_set_filter(filter, window, cx);
            }));
        let header = div()
            .p(metrics.spacing_sm())
            .border_b_1()
            .border_color(ui.border)
            .flex()
            .flex_col()
            .gap(metrics.spacing_sm())
            .child(
                div().w_full().min_w_0().child(
                    TextField::new(input)
                        .size(ControlSize::Md)
                        .prefix_icon(icons::SEARCH)
                        .cleanable(true)
                        .aria_label("Buscar en Axfterm"),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .child(mode_control)
                    .when(mode == PaletteMode::Search, |this| {
                        this.child(filter_control)
                    }),
            );
        let panel = div()
            .w_full()
            .max_w(metrics.palette_width())
            .mx_auto()
            .mt(metrics.titlebar_height() + metrics.spacing_xl())
            .rounded(metrics.radius_lg())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface)
            .shadow_lg()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(header)
            .child(div().p(metrics.spacing_sm()).child(body))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .border_t_1()
                    .border_color(ui.border)
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(metrics.spacing_xs())
                            .child(Kbd::new("↑↓"))
                            .child("navegar")
                            .child(Kbd::new("Enter"))
                            .child("ejecutar"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(metrics.spacing_xs())
                            .child(Kbd::new("Ctrl+Tab"))
                            .child("modo")
                            .when(mode == PaletteMode::Search, |this| {
                                this.child(Kbd::new("Ctrl+1–6")).child("filtrar")
                            })
                            .child(Kbd::new("Esc"))
                            .child("cerrar"),
                    ),
            );
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context(PALETTE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::palette_next))
            .on_action(cx.listener(Self::palette_previous))
            .on_action(cx.listener(Self::palette_accept))
            .on_action(cx.listener(Self::palette_cancel))
            .on_action(cx.listener(Self::palette_switch_mode))
            .on_action(cx.listener(Self::palette_filter_all))
            .on_action(cx.listener(Self::palette_filter_tabs))
            .on_action(cx.listener(Self::palette_filter_panes))
            .on_action(cx.listener(Self::palette_filter_projects))
            .on_action(cx.listener(Self::palette_filter_profiles))
            .on_action(cx.listener(Self::palette_filter_agents))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, window, cx| {
                    shell.close_palette(window, cx);
                }),
            )
            .bg(ui.overlay)
            .child(panel)
            .into_any_element()
    }

    fn render_shortcuts(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut body = div().flex().flex_col().gap(metrics.spacing_sm());
        for category in crate::actions::ActionCategory::ALL {
            let actions: Vec<&crate::actions::ActionMeta> = crate::actions::all()
                .filter(|action| action.category == *category && action.enabled)
                .collect();
            if actions.is_empty() {
                continue;
            }
            let mut rows = div().flex().flex_col().gap(metrics.hairline()).child(
                div()
                    .pt(metrics.spacing_xs())
                    .text_size(metrics.font_size * 0.6875)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ui.muted)
                    .child(category.label()),
            );
            for action in actions {
                let chord = self
                    .keymap
                    .chords_for(action.id)
                    .unwrap_or_else(|| "—".to_owned());
                rows = rows.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(metrics.spacing_sm())
                        .py(metrics.spacing_xxs())
                        .child(
                            div()
                                .min_w_0()
                                .text_size(metrics.font_size * 0.75)
                                .child(action.label),
                        )
                        .child(Kbd::new(chord)),
                );
            }
            body = body.child(rows);
        }
        body = body.child(
            div()
                .pt(metrics.spacing_sm())
                .text_size(metrics.font_size * 0.6875)
                .text_color(ui.muted)
                .child(
                    "Comandos (Ctrl+Shift+K) ejecuta acciones; búsqueda (Ctrl+P) salta a \
                     pestañas, paneles, proyectos, perfiles y agentes en atención. Dentro de \
                     la paleta, Ctrl+Tab cambia de modo y Ctrl+1–6 filtra por tipo. Los ajustes \
                     están en Ctrl+Alt+T. Cambia los atajos en la sección [keymap] de config.toml.",
                ),
        );
        let footer = div().flex().justify_end().child(
            Button::new("shortcuts-close", "Cerrar")
                .small()
                .ghost()
                .on_click(cx.listener(|shell, _, window, cx| {
                    shell.close_shortcuts(window, cx);
                })),
        );
        let modal = Modal::new("shortcuts-modal", "Atajos de teclado")
            .description("Generado desde el registro de acciones")
            .body(
                div()
                    .max_h(metrics.font_size * 24.0)
                    .overflow_y_scrollbar()
                    .child(body),
            )
            .footer(footer)
            .on_close(cx.listener(|shell, _, window, cx| {
                shell.close_shortcuts(window, cx);
            }))
            .into_any_element();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context(SHORTCUTS_CONTEXT)
            .track_focus(&self.shortcuts_focus)
            .on_action(cx.listener(Self::palette_cancel))
            .child(modal)
            .into_any_element()
    }

    // --- Panel de ajustes: estado auxiliar ------------------------------
    /// Fila enfocable del panel: un único `FocusHandle` persistente por id.
    fn settings_focusable(
        window: &mut Window,
        cx: &mut axf_ui::gpui::App,
        id: impl Into<ElementId>,
    ) -> Stateful<Div> {
        let key: ElementId = id.into();
        let handle = window
            .use_keyed_state(key.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        div().id(key).min_w_0().track_focus(&handle).tab_stop(true)
    }

    /// Crea una vez los selectores de fuente que viven mientras dura el panel.
    fn ensure_font_selects(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.ui_font_select.is_some() && self.terminal_font_select.is_some() {
            return;
        }
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        self.font_select_subscriptions.clear();
        let fonts = self.theme_host.installed_fonts(cx);
        let (ui_items, ui_selected) = font_list(&fonts, settings.draft.ui.font_family.as_deref());
        let ui_state = cx.new(|cx| SelectState::new(ui_items, ui_selected, window, cx));
        let subscription =
            cx.subscribe(&ui_state, |shell, _, event: &SelectEvent<FontList>, cx| {
                let SelectEvent::Confirm(value) = event;
                if let Some(value) = value {
                    if let Some(settings) = &mut shell.settings {
                        match value {
                            FontValue::System => settings.set_ui_font(None),
                            FontValue::Family(name) => {
                                settings.set_ui_font(Some(name.to_string()));
                            }
                        }
                    }
                    shell.preview_settings(cx);
                }
            });
        self.font_select_subscriptions.push(subscription);
        self.ui_font_select = Some(ui_state);
        let (terminal_items, terminal_selected) =
            font_list(&fonts, settings.draft.terminal.font_family.as_deref());
        let terminal_state =
            cx.new(|cx| SelectState::new(terminal_items, terminal_selected, window, cx));
        let subscription = cx.subscribe(
            &terminal_state,
            |shell, _, event: &SelectEvent<FontList>, cx| {
                let SelectEvent::Confirm(value) = event;
                if let Some(value) = value {
                    if let Some(settings) = &mut shell.settings {
                        match value {
                            FontValue::System => settings.set_terminal_font(None),
                            FontValue::Family(name) => {
                                settings.set_terminal_font(Some(name.to_string()));
                            }
                        }
                    }
                    shell.preview_settings(cx);
                }
            },
        );
        self.font_select_subscriptions.push(subscription);
        self.terminal_font_select = Some(terminal_state);
    }

    /// Campo de búsqueda de atajos.
    fn ensure_shortcut_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.settings_shortcut_input.is_some() {
            return;
        }
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filtrar acciones y atajos…"));
        let subscription = cx.subscribe(&input, |shell, input, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let query = input.read(cx).value().to_string();
            if let Some(settings) = &mut shell.settings {
                settings.shortcut_query = query;
                settings.shortcut_cursor = 0;
            }
            cx.notify();
        });
        self.settings_shortcut_input = Some(input);
        self.settings_shortcut_subscription = Some(subscription);
    }

    /// Abre el editor inline de un perfil ya existente en el borrador.
    fn open_profile_editor(
        &mut self,
        id: ProfileId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(profile) = self
            .settings
            .as_ref()
            .and_then(|settings| settings.profiles().iter().find(|p| p.id == id))
            .cloned()
        else {
            return;
        };
        let name_value = profile.name.clone();
        let command_value = profile.command.clone();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name_value));
        let command = cx.new(|cx| InputState::new(window, cx).default_value(command_value));
        let handle = name.read(cx).focus_handle(cx);
        self.settings_profile_editor = Some(ProfileEditor {
            id,
            name,
            command,
            icon: profile.icon,
            error: None,
        });
        window.focus(&handle, cx);
        cx.notify();
    }

    /// Añade un perfil vacío y abre su editor.
    fn add_profile(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(id) = self
            .settings
            .as_mut()
            .map(|settings| settings.add_profile())
        else {
            return;
        };
        self.open_profile_editor(id, window, cx);
    }

    /// Valida y aplica los campos del editor inline.
    fn apply_profile_editor(&mut self, cx: &mut Context<'_, Self>) {
        let Some(editor) = self.settings_profile_editor.as_ref() else {
            return;
        };
        let name = editor.name.read(cx).value().trim().to_owned();
        let command = editor.command.read(cx).value().trim().to_owned();
        if name.is_empty() {
            if let Some(editor) = &mut self.settings_profile_editor {
                editor.error = Some("El nombre no puede estar vacío.".into());
            }
            cx.notify();
            return;
        }
        if command.is_empty() {
            if let Some(editor) = &mut self.settings_profile_editor {
                editor.error = Some("El comando no puede estar vacío.".into());
            }
            cx.notify();
            return;
        }
        if let Err(error) =
            Profile::new(editor.id.clone(), name.clone(), command.clone()).command_line()
        {
            if let Some(editor) = &mut self.settings_profile_editor {
                editor.error = Some(format!("Comando inválido: {error}"));
            }
            cx.notify();
            return;
        }
        let id = editor.id.clone();
        let icon = editor.icon.clone();
        if let Some(settings) = &mut self.settings
            && let Err(error) = settings.update_profile(&id, name, command, icon)
        {
            settings.set_notice(error, true);
            cx.notify();
            return;
        }
        self.settings_profile_editor = None;
        cx.notify();
    }

    /// Número de pestañas abiertas que usan un perfil.
    fn profile_in_use(&self, id: &ProfileId) -> usize {
        let mut count = 0;
        for project in self.workspace.projects() {
            for tab in self.workspace.project_tabs(project.id) {
                if self
                    .workspace
                    .tab(tab)
                    .is_some_and(|tab| &tab.profile_id == id)
                {
                    count += 1;
                }
            }
        }
        count
    }

    /// Pide borrar el perfil seleccionado, con protección y aviso de uso.
    fn request_delete_profile(&mut self, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let Some(profile) = settings.selected_profile().cloned() else {
            return;
        };
        if settings.profiles().len() <= 1 {
            self.set_notice("Debe quedar al menos un perfil.", true, cx);
            return;
        }
        if settings.draft.terminal.default_profile == profile.id {
            self.set_notice(
                "Elige otro perfil predeterminado antes de borrar este.",
                true,
                cx,
            );
            return;
        }
        let in_use = self.profile_in_use(&profile.id);
        if in_use > 0 {
            if let Some(settings) = &mut self.settings {
                settings.pending_delete = Some(PendingDelete {
                    id: profile.id,
                    name: profile.name,
                    in_use,
                });
            }
            cx.notify();
            return;
        }
        self.delete_profile_now(&profile.id, cx);
    }

    fn delete_profile_now(&mut self, id: &ProfileId, cx: &mut Context<'_, Self>) {
        let result = self
            .settings
            .as_mut()
            .map(|settings| settings.delete_profile(id));
        match result {
            Some(Ok(())) => {
                if self
                    .settings_profile_editor
                    .as_ref()
                    .is_some_and(|editor| &editor.id == id)
                {
                    self.settings_profile_editor = None;
                }
                self.set_notice("Perfil eliminado del borrador.", false, cx);
            }
            Some(Err(error)) => {
                if let Some(settings) = &mut self.settings {
                    settings.pending_delete = None;
                    settings.set_notice(error.clone(), true);
                }
                self.set_notice(error, true, cx);
            }
            None => {}
        }
        cx.notify();
    }

    /// Confirma el borrado de un perfil en uso.
    fn confirm_delete_profile(&mut self, cx: &mut Context<'_, Self>) {
        let Some(pending) = self
            .settings
            .as_mut()
            .and_then(|settings| settings.pending_delete.take())
        else {
            return;
        };
        self.delete_profile_now(&pending.id, cx);
    }

    /// Lanza el descubrimiento local en segundo plano.
    fn start_detect(&mut self, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        if matches!(settings.detect, DetectState::Running) {
            return;
        }
        settings.detect = DetectState::Running;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let discovery = cx
                .background_executor()
                .spawn(async { axf_config::discover() })
                .await;
            let profiles = discovery.profiles_only();
            let result = this.update(cx, |shell, cx| {
                let Some(settings) = &mut shell.settings else {
                    return;
                };
                let mut candidates = profiles;
                candidates.retain(|profile| {
                    !settings.profiles().iter().any(|existing| {
                        existing.id == profile.id
                            || existing
                                .command
                                .trim()
                                .eq_ignore_ascii_case(profile.command.trim())
                    })
                });
                let count = candidates.len();
                settings.detect = DetectState::Ready(candidates);
                settings.set_notice(
                    if count == 0 {
                        "No hay terminales nuevas que añadir.".into()
                    } else {
                        format!(
                            "{count} terminales detectadas. Añádelas al borrador o descártalas."
                        )
                    },
                    false,
                );
                cx.notify();
            });
            result.ok();
        })
        .detach();
    }

    /// Fusiona los perfiles detectados en el borrador.
    fn merge_detected(&mut self, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        let DetectState::Ready(profiles) = std::mem::take(&mut settings.detect) else {
            return;
        };
        let added = settings.merge_detected(profiles);
        settings.detect = DetectState::Idle;
        settings.set_notice(
            if added == 0 {
                "Ninguno de los perfiles detectados era nuevo.".into()
            } else {
                format!("{added} perfiles añadidos al borrador; guarda para aplicarlos.")
            },
            false,
        );
        cx.notify();
    }

    /// Ajusta un valor numérico y refresca la preview visual.
    fn settings_step(&mut self, field: StepField, delta: f32, cx: &mut Context<'_, Self>) {
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        match field {
            StepField::UiFontSize => settings.step_ui_font_size(delta),
            StepField::UiScale => settings.step_ui_scale(delta),
            StepField::TerminalFontSize => settings.step_terminal_font_size(delta),
            StepField::LineHeight => settings.step_line_height(delta),
            StepField::ScanInterval => settings.step_scan_interval(delta),
        }
        if !matches!(field, StepField::ScanInterval) {
            self.preview_settings(cx);
        } else {
            cx.notify();
        }
    }

    /// Captura la tecla pulsada durante un rebind. Devuelve `true` si la consumió.
    fn settings_capture_keystroke(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(action) = self
            .settings
            .as_ref()
            .and_then(|settings| settings.capturing.clone())
        else {
            return false;
        };
        let key = keystroke.key.to_ascii_lowercase();
        if matches!(
            key.as_str(),
            "control"
                | "ctrl"
                | "shift"
                | "alt"
                | "meta"
                | "super"
                | "win"
                | "cmd"
                | "fn"
                | "function"
        ) {
            return false;
        }
        let result = if key == "escape" {
            if let Some(settings) = &mut self.settings {
                settings.cancel_capture();
            }
            Ok(())
        } else if key == "backspace" || key == "delete" {
            self.settings
                .as_mut()
                .map(|settings| settings.unbind_action(&action))
                .unwrap_or(Ok(()))
        } else {
            let chord = Self::chord_from_keystroke(keystroke);
            if chord.is_empty() {
                return false;
            }
            self.settings
                .as_mut()
                .map(|settings| settings.set_binding(&action, &chord))
                .unwrap_or(Ok(()))
        };
        match result {
            Ok(()) => {
                if let Some(settings) = &mut self.settings
                    && settings.capturing.is_none()
                    && settings.notice.is_none()
                {
                    settings.set_notice("Atajo actualizado en el borrador.".into(), false);
                }
            }
            Err(error) => {
                if let Some(settings) = &mut self.settings {
                    settings.set_notice(error, true);
                }
            }
        }
        cx.notify();
        true
    }

    /// Pide archivos o carpetas de fuentes y los añade al borrador.
    fn settings_add_font_sources(&mut self, files: bool, cx: &mut Context<'_, Self>) {
        let receiver = cx.prompt_for_paths(axf_ui::gpui::PathPromptOptions {
            files,
            directories: !files,
            multiple: true,
            prompt: Some(if files {
                "Añadir archivos de fuente".into()
            } else {
                "Añadir carpeta de fuentes".into()
            }),
        });
        let config_dir = self.theme_host.config_dir.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                let result = this.update(cx, |shell, cx| {
                    let mut added = 0;
                    let mut duplicate = false;
                    if let Some(settings) = &mut shell.settings {
                        for path in paths {
                            if settings.add_font_source(&path, &config_dir) {
                                added += 1;
                            } else {
                                duplicate = true;
                            }
                        }
                        if added > 0 {
                            settings.set_notice(
                                format!("{added} orígenes de fuentes añadidos al borrador."),
                                false,
                            );
                        } else if duplicate {
                            settings.set_notice(
                                "Esos orígenes ya estaban en la lista o se alcanzó el límite de 16."
                                    .into(),
                                true,
                            );
                        }
                    }
                    if added > 0 {
                        shell.preview_settings(cx);
                    }
                    cx.notify();
                });
                result.ok();
            }
        })
        .detach();
    }

    /// Abre la ruta indicada con la aplicación del sistema.
    fn settings_open_path(&mut self, path: Option<PathBuf>, cx: &mut Context<'_, Self>) {
        match path {
            Some(path) => cx.open_with_system(&path),
            None => self.set_notice("La ruta no está disponible.", true, cx),
        }
    }

    /// Elige el icono del perfil en edición.
    fn settings_choose_icon(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let _ = window;
        let receiver = cx.prompt_for_paths(axf_ui::gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Elegir icono del perfil".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.first().cloned()
            {
                let result = this.update(cx, |shell, cx| {
                    if let Some(editor) = &mut shell.settings_profile_editor {
                        editor.icon = Some(path);
                        editor.error = None;
                    }
                    cx.notify();
                });
                result.ok();
            }
        })
        .detach();
    }

    // --- Panel de ajustes: render --------------------------------------
    /// Barra lateral de secciones del panel.
    fn render_settings_rail(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let section = self
            .settings
            .as_ref()
            .map(|settings| settings.section)
            .unwrap_or(SettingsSection::Appearance);
        let mut rows = div().flex().flex_col().gap(metrics.spacing_xxs()).min_w_0();
        for candidate in SettingsSection::ALL {
            let candidate = *candidate;
            let selected = candidate == section;
            let row = Self::settings_focusable(
                window,
                cx,
                format!("settings-section-{}", candidate.label()),
            )
            .flex()
            .items_center()
            .gap(metrics.spacing_sm())
            .min_w_0()
            .h(metrics.control_height_md())
            .px(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .when(selected, |this| this.bg(ui.selection))
            .when(!selected, |this| {
                this.hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
            })
            .on_click(cx.listener(move |shell, _, _, cx| {
                if let Some(settings) = &mut shell.settings {
                    settings.set_section(candidate);
                }
                shell.settings_scroll = ScrollHandle::new();
                cx.notify();
            }))
            .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if let Some(settings) = &mut shell.settings {
                        settings.set_section(candidate);
                    }
                    shell.settings_scroll = ScrollHandle::new();
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                Icon::new(candidate.icon())
                    .size(IconSize::Sm)
                    .color(if selected { ui.accent } else { ui.muted }),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(metrics.font_size * 0.8125)
                    .text_color(if selected { ui.accent } else { ui.text })
                    .child(candidate.label()),
            );
            rows = rows.child(row);
        }
        let rail_width = metrics.font_size * 11.5 + metrics.spacing_sm() * 2.0;
        // Misma compensación de DPI que en `render_settings`: las anchuras de
        // los hijos se escalan y el ancho de la tarjeta no.
        div()
            .w(rail_width / window.scale_factor())
            .max_w(rail_width / window.scale_factor())
            .min_w_0()
            .flex_shrink_0()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .p(metrics.spacing_sm())
                    .border_r_1()
                    .border_color(ui.border)
                    .child(rows),
            )
            .into_any_element()
    }

    /// Cabecera con el título de la sección, la ayuda y los avisos.
    fn render_settings_header(
        &mut self,
        cx: &mut Context<'_, Self>,
        section: SettingsSection,
        notice: Option<(String, bool)>,
        conflict: Option<String>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut header = div()
            .flex()
            .flex_col()
            .w_full()
            .gap(metrics.spacing_xxs())
            .px(metrics.spacing_lg())
            .pt(metrics.spacing_lg())
            .pb(metrics.spacing_sm())
            .child(
                div()
                    .text_size(metrics.font_size * 0.95)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(section.label()),
            )
            .child(
                div()
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.muted)
                    .child(section.description()),
            )
            .when_some(notice, |this, (text, error)| {
                this.child(
                    div()
                        .pt(metrics.spacing_xs())
                        .text_size(metrics.font_size * 0.75)
                        .text_color(if error { ui.danger } else { ui.muted })
                        .child(text),
                )
            });
        if let Some(message) = conflict {
            header = header.child(self.render_settings_conflict(message, cx));
        }
        header.into_any_element()
    }

    /// Pie con el estado del borrador y las acciones de descartar/guardar.
    fn render_settings_footer(
        &mut self,
        cx: &mut Context<'_, Self>,
        dirty: bool,
        errors: &[String],
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let status = if let Some(first) = errors.first() {
            div()
                .min_w_0()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.danger)
                .child(if errors.len() > 1 {
                    format!("{first} (+{} errores)", errors.len() - 1)
                } else {
                    first.clone()
                })
        } else if dirty {
            div()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child("Borrador sin guardar")
        } else {
            div()
                .text_size(metrics.font_size * 0.75)
                .text_color(ui.muted)
                .child("Sin cambios")
        };
        div()
            .flex()
            .w_full()
            .flex_wrap()
            .flex_shrink_0()
            .items_center()
            .gap(metrics.spacing_sm())
            .px(metrics.spacing_lg())
            .py(metrics.spacing_sm())
            .border_t_1()
            .border_color(ui.border)
            .child(status)
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .child(
                        KitButton::new("settings-discard")
                            .small()
                            .ghost()
                            .label("Descartar")
                            .on_click(cx.listener(|shell, _, window, cx| {
                                shell.close_settings(window, cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-save")
                            .small()
                            .primary()
                            .label("Guardar")
                            .disabled(!dirty)
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.save_settings(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    /// Contenido de la sección activa.
    fn render_settings_body(
        &mut self,
        section: SettingsSection,
        body_width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        match section {
            SettingsSection::Appearance => self.render_settings_appearance(window, cx),
            SettingsSection::Terminal => self.render_settings_terminal(window, cx),
            SettingsSection::Profiles => self.render_settings_profiles(body_width, window, cx),
            SettingsSection::Fonts => self.render_settings_fonts(cx),
            SettingsSection::Shortcuts => self.render_settings_shortcuts(window, cx),
            SettingsSection::Agents => self.render_settings_agents(window, cx),
            SettingsSection::Advanced => self.render_settings_advanced(cx),
        }
    }

    fn render_settings(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let metrics = theme::state(cx).metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let section = settings.section;
        let dirty = settings.is_dirty();
        let notice = settings.notice.clone();
        let conflict = settings.conflict_message().map(str::to_owned);
        let errors = settings.errors.clone();
        let schema = settings.draft.schema_version;
        let rail = self.render_settings_rail(window, cx);
        let header = self.render_settings_header(cx, section, notice, conflict);
        let footer = self.render_settings_footer(cx, dirty, &errors);
        let height = (window.viewport_size().height - metrics.font_size * 28.0)
            .max(metrics.font_size * 16.0);
        let card_width = metrics.font_size * 62.0;
        let scale = window.scale_factor().max(1.0);
        let rail_width = metrics.font_size * 11.5 + metrics.spacing_sm() * 2.0;
        // `Modal::width` fija la tarjeta en píxeles físicos, mientras que las
        // anchuras de los hijos se escalan por DPI. Dividir por `scale_factor`
        // alinea ambos sistemas y evita que las filas desborden la tarjeta.
        let body_width = (card_width - rail_width - metrics.spacing_lg() * 4.0) / scale;
        let body = self.render_settings_body(section, body_width, window, cx);
        let scroll = self.settings_scroll.clone();
        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(header)
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_x_hidden()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .vertical_scrollbar(&scroll)
                    .px(metrics.spacing_lg())
                    .pb(metrics.spacing_sm())
                    .child(div().w(body_width).max_w(body_width).min_w_0().child(body)),
            )
            .child(footer);
        let card_body = div()
            .flex()
            .w_full()
            .min_w_0()
            .h(height)
            .min_h_0()
            .overflow_hidden()
            .child(rail)
            .child(content);
        let modal = Modal::new("settings-modal", "Ajustes")
            .description(format!(
                "config.toml · esquema v{schema} · {}",
                if dirty {
                    "cambios sin guardar"
                } else {
                    "sin cambios"
                }
            ))
            .width(card_width)
            .align_top()
            .body(card_body)
            .on_close(cx.listener(|shell, _, window, cx| {
                shell.close_settings(window, cx);
            }))
            .into_any_element();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .key_context(SETTINGS_CONTEXT)
            .track_focus(&self.settings_focus)
            .tab_group()
            .on_action(cx.listener(Self::settings_cancel))
            .on_action(cx.listener(Self::settings_save))
            .on_action(cx.listener(Self::settings_next_section))
            .on_action(cx.listener(Self::settings_previous_section))
            .child(modal)
            .focus_trap("settings-trap", &self.settings_focus)
            .into_any_element()
    }

    fn render_settings_conflict(
        &mut self,
        message: String,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        div()
            .mt(metrics.spacing_sm())
            .flex()
            .flex_wrap()
            .items_center()
            .gap(metrics.spacing_sm())
            .p(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(ui.warning)
            .bg(ui.surface_raised)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.text)
                    .child(message),
            )
            .child(
                KitButton::new("settings-conflict-reload")
                    .small()
                    .label("Recargar")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.reload_settings_from_disk(cx);
                    })),
            )
            .child(
                KitButton::new("settings-conflict-keep")
                    .small()
                    .ghost()
                    .label("Seguir editando")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.keep_editing();
                        }
                        cx.notify();
                    })),
            )
            .child(
                KitButton::new("settings-conflict-overwrite")
                    .small()
                    .danger()
                    .label("Sobrescribir y guardar")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.allow_overwrite();
                        }
                        shell.save_settings(cx);
                    })),
            )
            .into_any_element()
    }

    fn render_settings_appearance(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let appearance = settings.draft.appearance;
        let selected_theme = settings.draft.ui.theme.clone();
        let ui_size = settings.draft.ui.font_size;
        let ui_scale = settings.draft.ui.scale;
        let themes = self.theme_host.theme_names();
        let ui_font_select = self.ui_font_select.clone();
        let mut choices = div().flex().flex_col().gap(metrics.spacing_xxs());
        for (label, value, hint) in [
            (
                "Sistema",
                axf_config::Appearance::System,
                "Sigue el tema del sistema.",
            ),
            (
                "Claro",
                axf_config::Appearance::Light,
                "Fuerza la variante clara.",
            ),
            (
                "Oscuro",
                axf_config::Appearance::Dark,
                "Fuerza la variante oscura.",
            ),
        ] {
            let selected = appearance == value;
            let row = Self::settings_focusable(window, cx, format!("settings-appearance-{label}"))
                .flex()
                .items_center()
                .gap(metrics.spacing_sm())
                .px(metrics.spacing_sm())
                .py(metrics.spacing_xs())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    if let Some(settings) = &mut shell.settings {
                        settings.set_appearance(value);
                    }
                    shell.preview_settings(cx);
                }))
                .child(
                    Icon::new(if selected {
                        icons::CHECK
                    } else {
                        icons::CIRCLE
                    })
                    .size(IconSize::Xs)
                    .color(if selected { ui.accent } else { ui.muted }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(metrics.hairline())
                        .child(div().text_size(metrics.font_size * 0.8125).child(label))
                        .child(
                            div()
                                .text_size(metrics.font_size * 0.6875)
                                .text_color(ui.muted)
                                .child(hint),
                        ),
                );
            choices = choices.child(row);
        }
        let mut theme_list = div().flex().flex_col().gap(metrics.hairline());
        for (name, variant) in themes {
            let selected = name == selected_theme;
            let dark = variant.is_dark();
            let label = name.clone();
            let row = Self::settings_focusable(window, cx, format!("settings-theme-{name}"))
                .flex()
                .items_center()
                .gap(metrics.spacing_sm())
                .px(metrics.spacing_sm())
                .py(metrics.spacing_xs())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    if let Some(settings) = &mut shell.settings {
                        settings.set_theme(&label, dark);
                    }
                    shell.preview_settings(cx);
                }))
                .child(
                    Icon::new(if selected {
                        icons::CHECK
                    } else if dark {
                        icons::MOON
                    } else {
                        icons::SUN
                    })
                    .size(IconSize::Xs)
                    .color(if selected { ui.accent } else { ui.muted }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(metrics.font_size * 0.75)
                        .child(name),
                )
                .child(
                    div()
                        .text_size(metrics.font_size * 0.6875)
                        .text_color(ui.muted)
                        .child(variant.label()),
                );
            theme_list = theme_list.child(row);
        }
        let font_field = ui_font_select.map(|select| {
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xxs())
                .child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child("Fuente de la interfaz"),
                )
                .child(Select::new(&select).small().w_full().placeholder("Sistema"))
                .into_any_element()
        });
        let body = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_lg())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Modo de color"),
                    )
                    .child(choices),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xxs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Catálogo de temas"),
                    )
                    .child(
                        div()
                            .id("settings-theme-list")
                            .max_h(metrics.font_size * 12.0)
                            .overflow_y_scroll()
                            .child(theme_list),
                    ),
            )
            .when_some(font_field, |this, field| this.child(field))
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-ui-size",
                "Tamaño base de la UI",
                format!("{:.0} px", ui_size),
                StepField::UiFontSize,
            ))
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-ui-scale",
                "Escala global",
                format!("{:.2}×", ui_scale),
                StepField::UiScale,
            ));
        body.into_any_element()
    }

    fn render_settings_terminal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let font_select = self.terminal_font_select.clone();
        let font_size = format!("{:.0} px", settings.draft.terminal.font_size);
        let line_height = format!("{:.2}×", settings.draft.terminal.line_height);
        let mono = state.fonts.mono.clone();
        let mono_size = state.mono_font_size;
        let font_field = font_select.map(|select| {
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_xxs())
                .child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child("Fuente monoespaciada"),
                )
                .child(Select::new(&select).small().w_full().placeholder("Sistema"))
                .into_any_element()
        });
        let preview = div()
            .flex()
            .flex_col()
            .gap(metrics.hairline())
            .p(metrics.spacing_sm())
            .rounded(metrics.radius_sm())
            .border_1()
            .border_color(ui.border)
            .bg(ui.surface_raised)
            .child(
                div()
                    .font_family(mono.clone())
                    .text_size(mono_size)
                    .text_color(ui.text)
                    .child("C:\\proyectos\\axfterm ❯ axfterm --help"),
            )
            .child(
                div()
                    .font_family(mono)
                    .text_size(mono_size)
                    .text_color(ui.muted)
                    .child("0123456789 · []{}()<> · áéíóú · ñ"),
            );
        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_lg())
            .when_some(font_field, |this, field| this.child(field))
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-terminal-size",
                "Tamaño de celda",
                font_size,
                StepField::TerminalFontSize,
            ))
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-line-height",
                "Altura de línea",
                line_height,
                StepField::LineHeight,
            ))
            .child(preview)
            .into_any_element()
    }

    /// Filas seleccionables de perfiles dentro de los ajustes.
    fn render_profile_rows(
        &mut self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let profiles = self
            .settings
            .as_ref()
            .map(|settings| settings.profiles().to_vec())
            .unwrap_or_default();
        let cursor = self
            .settings
            .as_ref()
            .map(|settings| settings.profile_cursor)
            .unwrap_or(0);
        let default_profile = self
            .settings
            .as_ref()
            .map(|settings| settings.draft.terminal.default_profile.clone());
        let icons: Vec<ProfileIconSource> = profiles
            .iter()
            .map(|profile| self.theme_host.profile_icon(&profile.id))
            .collect();
        let mut list = div()
            .flex()
            .flex_col()
            .w(width)
            .max_w(width)
            .min_w_0()
            .gap(metrics.hairline());
        let profile_count = profiles.len();
        for (index, profile) in profiles.iter().enumerate() {
            let selected = index == cursor;
            let is_default = default_profile.as_ref() == Some(&profile.id);
            let row =
                Self::settings_focusable(window, cx, format!("settings-profile-{}", profile.id))
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .w(width)
                    .max_w(width)
                    .min_w_0()
                    .when(selected, |this| this.bg(ui.selection))
                    .when(!selected, |this| {
                        this.hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                    })
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.profile_cursor = index;
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "up" => {
                                if let Some(settings) = &mut shell.settings {
                                    settings.profile_cursor =
                                        settings.profile_cursor.saturating_sub(1);
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }
                            "down" => {
                                if let Some(settings) = &mut shell.settings {
                                    settings.profile_cursor = (settings.profile_cursor + 1)
                                        .min(profile_count.saturating_sub(1));
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }
                            "enter" | "space" => {
                                if let Some(settings) = &mut shell.settings {
                                    settings.profile_cursor = index;
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }
                            _ => {}
                        }
                    }))
                    .child(
                        ProfileIcon::new(icons[index].clone())
                            .size(IconSize::Sm)
                            .color(ui.text),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.hairline())
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.75)
                                    .child(profile.name.clone()),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(metrics.font_size * 0.6875)
                                    .text_color(ui.muted)
                                    .child(profile.command.clone()),
                            ),
                    )
                    .when(is_default, |this| {
                        this.child(
                            div()
                                .text_size(metrics.font_size * 0.625)
                                .text_color(ui.accent)
                                .child("Predeterminado"),
                        )
                    });
            list = list.child(row);
        }
        list.into_any_element()
    }

    /// Botones para añadir, editar, duplicar, mover, marcar o borrar perfiles.
    fn render_profile_toolbar(&mut self, width: Pixels, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let metrics = state.metrics;
        let selected_id = self
            .settings
            .as_ref()
            .and_then(|settings| settings.profiles().get(settings.profile_cursor))
            .map(|profile| profile.id.clone());
        let running = matches!(
            self.settings.as_ref().map(|settings| &settings.detect),
            Some(DetectState::Running)
        );
        let mut toolbar = div()
            .flex()
            .flex_wrap()
            .w(width)
            .max_w(width)
            .min_w_0()
            .gap(metrics.spacing_xs());
        toolbar = toolbar
            .child(
                KitButton::new("settings-profile-add")
                    .small()
                    .label("Añadir")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.add_profile(window, cx);
                    })),
            )
            .child(
                KitButton::new("settings-profile-detect")
                    .small()
                    .ghost()
                    .label("Detectar terminales")
                    .disabled(running)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.start_detect(cx);
                    })),
            )
            .when_some(selected_id, |this, id| {
                let edit_id = id.clone();
                let duplicate_id = id.clone();
                let up_id = id.clone();
                let down_id = id.clone();
                this.child(
                    KitButton::new("settings-profile-edit")
                        .small()
                        .label("Editar")
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.open_profile_editor(edit_id.clone(), window, cx);
                        })),
                )
                .child(
                    KitButton::new("settings-profile-duplicate")
                        .small()
                        .ghost()
                        .label("Duplicar")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            let result = shell
                                .settings
                                .as_mut()
                                .map(|settings| settings.duplicate_profile(&duplicate_id));
                            if let Some(Err(error)) = result {
                                shell.set_notice(error, true, cx);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-profile-up")
                        .small()
                        .ghost()
                        .label("Subir")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.move_profile(&up_id, -1);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-profile-down")
                        .small()
                        .ghost()
                        .label("Bajar")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.move_profile(&down_id, 1);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-profile-default")
                        .small()
                        .ghost()
                        .label("Predeterminado")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                if let Err(error) = settings.set_default_profile(&id) {
                                    settings.set_notice(error, true);
                                }
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-profile-delete")
                        .small()
                        .danger()
                        .label("Eliminar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.request_delete_profile(cx);
                        })),
                )
            });
        toolbar.into_any_element()
    }

    /// Resultado del descubrimiento local de terminales.
    fn render_profile_detect(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let mut detect = div().flex().flex_col().gap(metrics.spacing_xs());
        if let Some(settings) = self.settings.as_ref() {
            match &settings.detect {
                DetectState::Running => {
                    detect = detect.child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Buscando terminales en PATH…"),
                    );
                }
                DetectState::Ready(found) => {
                    let mut found_list = div().flex().flex_col().gap(metrics.hairline());
                    for profile in found {
                        found_list = found_list.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(metrics.spacing_sm())
                                .text_size(metrics.font_size * 0.75)
                                .child(
                                    Icon::new(icons::TERMINAL)
                                        .size(IconSize::Xs)
                                        .color(ui.muted),
                                )
                                .child(profile.name.clone())
                                .child(
                                    div()
                                        .text_size(metrics.font_size * 0.6875)
                                        .text_color(ui.muted)
                                        .child(profile.command.clone()),
                                ),
                        );
                    }
                    detect = detect.child(found_list).child(
                        div()
                            .flex()
                            .gap(metrics.spacing_xs())
                            .child(
                                KitButton::new("settings-profile-detect-add")
                                    .small()
                                    .primary()
                                    .label(format!("Añadir {}", found.len()))
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        shell.merge_detected(cx);
                                    })),
                            )
                            .child(
                                KitButton::new("settings-profile-detect-discard")
                                    .small()
                                    .ghost()
                                    .label("Descartar")
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        if let Some(settings) = &mut shell.settings {
                                            settings.detect = DetectState::Idle;
                                        }
                                        cx.notify();
                                    })),
                            ),
                    );
                }
                DetectState::Idle => {}
            }
        }
        detect.into_any_element()
    }

    /// Confirmación al borrar un perfil en uso.
    fn render_profile_delete_warning(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let pending = self
            .settings
            .as_ref()
            .and_then(|settings| settings.pending_delete.clone())?;
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        Some(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(metrics.spacing_sm())
                .p(metrics.spacing_sm())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.warning)
                .text_size(metrics.font_size * 0.75)
                .child(format!(
                    "«{}» se usa en {} pestaña(s) abiertas. Al cerrarlas no podrán arrancar.",
                    pending.name, pending.in_use
                ))
                .child(
                    KitButton::new("settings-profile-delete-confirm")
                        .small()
                        .danger()
                        .label("Eliminar perfil")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.confirm_delete_profile(cx);
                        })),
                )
                .child(
                    KitButton::new("settings-profile-delete-cancel")
                        .small()
                        .ghost()
                        .label("Cancelar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.pending_delete = None;
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }

    /// Editor inline del perfil seleccionado.
    fn render_profile_editor_panel(&mut self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let editor = self.settings_profile_editor.as_ref()?;
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let name = editor.name.clone();
        let command = editor.command.clone();
        let icon = editor.icon.clone();
        let error = editor.error.clone();
        let id = editor.id.clone();
        Some(
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing_sm())
                .p(metrics.spacing_sm())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.border)
                .child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("Editar «{id}»")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(metrics.spacing_xxs())
                        .child(
                            div()
                                .text_size(metrics.font_size * 0.6875)
                                .text_color(ui.muted)
                                .child("Nombre visible"),
                        )
                        .child(
                            TextField::new(name)
                                .size(ControlSize::Md)
                                .aria_label("Nombre"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(metrics.spacing_xxs())
                        .child(
                            div()
                                .text_size(metrics.font_size * 0.6875)
                                .text_color(ui.muted)
                                .child("Comando (ejecutable y argumentos)"),
                        )
                        .child(
                            TextField::new(command)
                                .size(ControlSize::Md)
                                .aria_label("Comando"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(metrics.spacing_xs())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(metrics.font_size * 0.6875)
                                .text_color(ui.muted)
                                .child(icon.map(|icon| icon.display().to_string()).unwrap_or_else(
                                    || "Sin icono propio: se usa el del ejecutable.".into(),
                                )),
                        )
                        .child(
                            KitButton::new("settings-profile-icon")
                                .small()
                                .ghost()
                                .label("Elegir icono…")
                                .on_click(cx.listener(|shell, _, window, cx| {
                                    shell.settings_choose_icon(window, cx);
                                })),
                        )
                        .child(
                            KitButton::new("settings-profile-icon-clear")
                                .small()
                                .ghost()
                                .label("Quitar icono")
                                .on_click(cx.listener(|shell, _, _, cx| {
                                    if let Some(editor) = &mut shell.settings_profile_editor {
                                        editor.icon = None;
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .when_some(error, |this, error| {
                    this.child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.danger)
                            .child(error),
                    )
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(metrics.spacing_xs())
                        .child(
                            KitButton::new("settings-profile-cancel")
                                .small()
                                .ghost()
                                .label("Cancelar")
                                .on_click(cx.listener(|shell, _, _, cx| {
                                    shell.settings_profile_editor = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            KitButton::new("settings-profile-apply")
                                .small()
                                .primary()
                                .label("Aplicar")
                                .on_click(cx.listener(|shell, _, _, cx| {
                                    shell.apply_profile_editor(cx);
                                })),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_settings_profiles(
        &mut self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let rows = self.render_profile_rows(width, window, cx);
        let toolbar = self.render_profile_toolbar(width, cx);
        let detect = self.render_profile_detect(cx);
        let delete_warning = self.render_profile_delete_warning(cx);
        let editor = self.render_profile_editor_panel(cx);
        div()
            .flex()
            .flex_col()
            .w(width)
            .max_w(width)
            .min_w_0()
            .gap(metrics.spacing_md())
            .child(toolbar)
            .child(rows)
            .child(detect)
            .children(delete_warning)
            .children(editor)
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        "El id del perfil es estable y no cambia al renombrarlo. Un perfil \
                         borrado en uso deja sus pestañas sin arranque hasta reiniciarlas.",
                    ),
            )
            .into_any_element()
    }

    fn render_settings_fonts(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let sources = settings.draft.fonts.sources.clone();
        let config_dir = self.theme_host.config_dir.clone();

        let mut list = div().flex().flex_col().gap(metrics.hairline());
        if sources.is_empty() {
            list = list.child(
                div()
                    .py(metrics.spacing_sm())
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.muted)
                    .child("No hay orígenes adicionales; se usan las fuentes del sistema."),
            );
        }
        for (index, source) in sources.iter().enumerate() {
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .rounded(metrics.radius_sm())
                    .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                    .child(
                        Icon::new(icons::TYPOGRAPHY)
                            .size(IconSize::Xs)
                            .color(ui.muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(metrics.font_size * 0.75)
                            .child(source.display().to_string()),
                    )
                    .child(
                        KitButton::new(("settings-font-remove", index))
                            .small()
                            .ghost()
                            .label("Quitar")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.remove_font_source(index);
                                }
                                shell.preview_settings(cx);
                            })),
                    ),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(metrics.spacing_xs())
                    .child(
                        KitButton::new("settings-font-file")
                            .small()
                            .label("Añadir archivo…")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.settings_add_font_sources(true, cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-font-folder")
                            .small()
                            .ghost()
                            .label("Añadir carpeta…")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.settings_add_font_sources(false, cx);
                            })),
                    ),
            )
            .child(list)
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(format!(
                        "Acepta .ttf, .otf y .ttc. Las rutas dentro de {} se guardan relativas. \
                         Máximo 16 orígenes.",
                        config_dir.display()
                    )),
            )
            .into_any_element()
    }

    fn render_settings_shortcuts(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let _ = window;
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let actions = settings.shortcut_actions();
        let keymap = Keymap::resolve(&settings.draft.keymap);
        let capturing = settings.capturing.clone();
        let input = self.settings_shortcut_input.clone();

        let capture_label = capturing
            .as_deref()
            .and_then(crate::actions::by_id)
            .map(|action| action.label.to_owned());

        let banner = capturing.as_ref().map(|action| {
            let action = action.clone();
            let unbind_action = action.clone();
            let reset_action = action.clone();
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(metrics.spacing_sm())
                .p(metrics.spacing_sm())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.accent)
                .bg(ui.surface_raised)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(metrics.font_size * 0.75)
                        .child(format!(
                            "Pulsa la combinación para «{}»; Supr la quita y Esc cancela.",
                            capture_label.as_deref().unwrap_or(&action)
                        )),
                )
                .child(
                    KitButton::new("settings-shortcut-unbind")
                        .small()
                        .danger()
                        .label("Quitar atajo")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings
                                && let Err(error) = settings.unbind_action(&unbind_action)
                            {
                                settings.set_notice(error, true);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-shortcut-reset")
                        .small()
                        .ghost()
                        .label("Restaurar")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings
                                && let Err(error) = settings.reset_action(&reset_action)
                            {
                                settings.set_notice(error, true);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    KitButton::new("settings-shortcut-cancel")
                        .small()
                        .ghost()
                        .label("Cancelar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.cancel_capture();
                            }
                            cx.notify();
                        })),
                )
                .into_any_element()
        });

        let mut list = div().flex().flex_col().gap(metrics.hairline());
        for action in actions {
            let chords = keymap.chords_for(action.id);
            let action_id = action.id;
            let row =
                Self::settings_focusable(window, cx, format!("settings-shortcut-{action_id}"))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.begin_capture(action_id);
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.hairline())
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.75)
                                    .child(action.label),
                            )
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.625)
                                    .text_color(ui.muted)
                                    .child(action.id),
                            ),
                    )
                    .child(match chords {
                        Some(chords) => Kbd::new(chords).into_any_element(),
                        None => div()
                            .text_size(metrics.font_size * 0.6875)
                            .text_color(ui.muted)
                            .child("Sin atajo")
                            .into_any_element(),
                    });
            list = list.child(row);
        }

        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .when_some(input, |this, input| {
                this.child(
                    TextField::new(input)
                        .size(ControlSize::Md)
                        .prefix_icon(icons::SEARCH)
                        .cleanable(true)
                        .aria_label("Filtrar atajos"),
                )
            })
            .children(banner)
            .child(
                div()
                    .id("settings-shortcut-list")
                    .max_h(metrics.font_size * 26.0)
                    .overflow_y_scroll()
                    .child(list),
            )
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        "Enter sobre una acción captura la combinación. Los atajos de app \
                         requieren Ctrl, Alt o Win; Ctrl+C/D/Z, Tab, Esc y las flechas quedan \
                         para la terminal.",
                    ),
            )
            .into_any_element()
    }

    fn render_settings_agents(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let enabled = settings.draft.agents.enabled;
        let scan = settings.draft.agents.scan_interval_ms;
        let notify_finished = settings.draft.agents.notify_finished;
        let notify_error = settings.draft.agents.notify_error;
        let notify_attention = settings.draft.agents.notify_attention;
        let profiles = settings.profiles().to_vec();
        let preferences: Vec<(String, String)> = SettingsState::AGENT_KEYS
            .iter()
            .map(|agent| {
                let name = settings
                    .preferred_profile(agent)
                    .and_then(|id| profiles.iter().find(|profile| &profile.id == id))
                    .map(|profile| profile.name.clone())
                    .unwrap_or_else(|| "Perfil por defecto".into());
                ((*agent).to_owned(), name)
            })
            .collect();
        let banner = div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_md())
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap(metrics.spacing_sm())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(metrics.hairline())
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.8125)
                                    .child("Detección de agentes"),
                            )
                            .child(
                                div()
                                    .text_size(metrics.font_size * 0.6875)
                                    .text_color(ui.muted)
                                    .child("OSC 7501 y árbol de procesos; sin red ni telemetría."),
                            ),
                    )
                    .child(
                        Switch::new("settings-agents-enabled")
                            .checked(enabled)
                            .on_click(cx.listener(|shell, checked, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.set_agents_enabled(*checked);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(self.render_settings_stepper(
                window,
                cx,
                "settings-agents-scan",
                "Intervalo de escaneo",
                format!("{scan} ms"),
                StepField::ScanInterval,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing_xs())
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.muted)
                            .child("Avisos"),
                    )
                    .child(
                        Switch::new("settings-agents-notify-finished")
                            .checked(notify_finished)
                            .label("Al terminar")
                            .on_click(cx.listener(|shell, checked, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.set_notify_finished(*checked);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Switch::new("settings-agents-notify-error")
                            .checked(notify_error)
                            .label("Al fallar")
                            .on_click(cx.listener(|shell, checked, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.set_notify_error(*checked);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Switch::new("settings-agents-notify-attention")
                            .checked(notify_attention)
                            .label("Al requerir atención")
                            .on_click(cx.listener(|shell, checked, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.set_notify_attention(*checked);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div().flex().flex_col().gap(metrics.spacing_xs()).child(
                    div()
                        .text_size(metrics.font_size * 0.75)
                        .text_color(ui.muted)
                        .child("Perfil preferido por agente"),
                ),
            );
        let mut banner = banner;
        for (agent, current) in preferences {
            let agent_label = if agent == "opencode" {
                "opencode"
            } else {
                "Copilot"
            };
            let click_agent = agent.clone();
            let row =
                Self::settings_focusable(window, cx, format!("settings-agent-profile-{agent}"))
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap(metrics.spacing_sm())
                    .px(metrics.spacing_sm())
                    .py(metrics.spacing_xs())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .hover(move |this| this.bg(ui.surface_raised.opacity(0.6)))
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(settings) = &mut shell.settings {
                            settings.cycle_preferred_profile(&click_agent, 1);
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, _, cx| {
                        let delta = match event.keystroke.key.as_str() {
                            "left" => -1,
                            "right" => 1,
                            _ => return,
                        };
                        if let Some(settings) = &mut shell.settings {
                            settings.cycle_preferred_profile(&agent, delta);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(div().text_size(metrics.font_size * 0.75).child(agent_label))
                    .child(
                        div()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.accent)
                            .child(current),
                    );
            banner = banner.child(row);
        }
        banner
            .child(
                div()
                    .text_size(metrics.font_size * 0.6875)
                    .text_color(ui.muted)
                    .child(
                        "El perfil preferido se usará al abrir una terminal de agente desde la \
                         app; hasta entonces solo se guarda en config.toml.",
                    ),
            )
            .into_any_element()
    }

    fn render_settings_advanced(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let schema = settings.draft.schema_version;
        let config_path = self
            .theme_host
            .config_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "—".into());
        let themes_path = self
            .theme_host
            .themes_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "—".into());
        let pending_defaults = settings.pending_defaults;
        let config_file = self.theme_host.config_path.clone();
        let themes_dir = self.theme_host.themes_dir.clone();
        let mono = state.fonts.mono.clone();

        let path_line = |label: &'static str, path: String| {
            div()
                .flex()
                .flex_col()
                .gap(metrics.hairline())
                .child(
                    div()
                        .text_size(metrics.font_size * 0.6875)
                        .text_color(ui.muted)
                        .child(label),
                )
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(metrics.font_size * 0.75)
                        .child(path),
                )
        };

        let confirm = pending_defaults.then(|| {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(metrics.spacing_sm())
                .p(metrics.spacing_sm())
                .rounded(metrics.radius_sm())
                .border_1()
                .border_color(ui.warning)
                .text_size(metrics.font_size * 0.75)
                .child(
                    "Restaurar apariencia, terminal, fuentes, atajos y agentes a los valores \
                     por defecto. Los perfiles se conservan.",
                )
                .child(
                    KitButton::new("settings-defaults-confirm")
                        .small()
                        .danger()
                        .label("Restaurar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.restore_defaults();
                            }
                            shell.preview_settings(cx);
                        })),
                )
                .child(
                    KitButton::new("settings-defaults-cancel")
                        .small()
                        .ghost()
                        .label("Cancelar")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(settings) = &mut shell.settings {
                                settings.pending_defaults = false;
                            }
                            cx.notify();
                        })),
                )
                .into_any_element()
        });

        div()
            .flex()
            .flex_col()
            .gap(metrics.spacing_lg())
            .child(path_line("Configuración", config_path))
            .child(path_line("Temas", themes_path))
            .child(
                div()
                    .text_size(metrics.font_size * 0.75)
                    .text_color(ui.muted)
                    .child(format!(
                        "Esquema de configuración v{schema}. La escritura conserva comentarios y \
                         orden, y solo aplica el borrador al pulsar Guardar."
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(metrics.spacing_xs())
                    .child(
                        KitButton::new("settings-open-config")
                            .small()
                            .label("Abrir config.toml")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.settings_open_path(config_file.clone(), cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-open-themes")
                            .small()
                            .ghost()
                            .label("Abrir temas")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.settings_open_path(themes_dir.clone(), cx);
                            })),
                    )
                    .child(
                        KitButton::new("settings-restore-defaults")
                            .small()
                            .danger()
                            .label("Restaurar valores por defecto…")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                if let Some(settings) = &mut shell.settings {
                                    settings.pending_defaults = true;
                                }
                                cx.notify();
                            })),
                    ),
            )
            .children(confirm)
            .into_any_element()
    }

    /// Fila numérica ajustable con teclado (←/→) y botones de ratón.
    fn render_settings_stepper(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        id: &'static str,
        label: &'static str,
        value: String,
        field: StepField,
    ) -> AnyElement {
        let state = theme::state(cx);
        let ui = state.theme.ui;
        let metrics = state.metrics;
        let minus = Self::settings_focusable(window, cx, format!("{id}-minus"))
            .flex()
            .items_center()
            .justify_center()
            .size(metrics.control_height_xs())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .hover(move |this| this.bg(ui.surface_raised))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.settings_step(field, -1.0, cx);
            }))
            .child(
                div()
                    .text_size(metrics.font_size * 1.1)
                    .text_color(ui.muted)
                    .child("−"),
            );
        let plus = Self::settings_focusable(window, cx, format!("{id}-plus"))
            .flex()
            .items_center()
            .justify_center()
            .size(metrics.control_height_xs())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .hover(move |this| this.bg(ui.surface_raised))
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.settings_step(field, 1.0, cx);
            }))
            .child(
                div()
                    .text_size(metrics.font_size * 1.1)
                    .text_color(ui.muted)
                    .child("+"),
            );
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(metrics.spacing_sm())
            .min_h(metrics.control_height_md())
            .child(div().text_size(metrics.font_size * 0.8125).child(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing_xxs())
                    .child(minus)
                    .child(
                        div()
                            .min_w(metrics.font_size * 4.5)
                            .text_center()
                            .text_size(metrics.font_size * 0.75)
                            .text_color(ui.text)
                            .child(value),
                    )
                    .child(plus),
            )
            .into_any_element()
    }

    /// Convierte una pulsación en el chord canónico de `config.toml`.
    fn chord_from_keystroke(keystroke: &Keystroke) -> String {
        let mut parts = Vec::new();
        if keystroke.modifiers.control {
            parts.push("ctrl".to_owned());
        }
        if keystroke.modifiers.alt {
            parts.push("alt".to_owned());
        }
        if keystroke.modifiers.shift {
            parts.push("shift".to_owned());
        }
        if keystroke.modifiers.platform {
            parts.push(if cfg!(target_os = "macos") {
                "cmd".to_owned()
            } else {
                "win".to_owned()
            });
        }
        if keystroke.modifiers.function {
            parts.push("fn".to_owned());
        }
        let key = keystroke.key.to_ascii_lowercase();
        if key.is_empty() {
            return String::new();
        }
        parts.push(key);
        parts.join("-")
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
    fn palette_attention_jump_cycles_only_tabs_that_need_attention() {
        let (mut workspace, project, tab) = sidebar_workspace("attention-jump");
        let pane = workspace.pane_of_tab(tab).expect("panel");
        let second = workspace
            .new_tab(pane, project, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
            .expect("segunda pestaña")
            .tab;
        let _third = workspace
            .new_tab(pane, project, &sidebar_profile(), SIDEBAR_PROFILE_TITLE)
            .expect("tercera pestaña")
            .tab;
        workspace.set_tab_agents(
            tab,
            vec![test_agent(
                1,
                AgentKind::OpenCode,
                10,
                AgentState::NeedsAttention,
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
        let attention = attention_tabs(&workspace);
        assert_eq!(attention, vec![tab, second]);
        assert_eq!(next_attention(&attention, Some(tab)), Some(second));
        assert_eq!(next_attention(&attention, Some(second)), Some(tab));
        assert_eq!(next_attention(&attention, None), Some(tab));
        assert_eq!(next_attention(&[], None), None);
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
