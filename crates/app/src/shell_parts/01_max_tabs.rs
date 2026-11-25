use crate::font_select::{FontList, FontValue, font_actual_label, font_list};
use crate::history::HistoryStore;
use crate::keymap::Keymap;
use crate::palette::{
    self, PaletteHistory, PaletteIcon, PaletteItem, PaletteKind, PaletteMode, PaletteState,
    PaletteTarget,
};
use crate::profile_select::{ProfileList, ProfileValue, profile_list};
use crate::settings::{
    DetectState, ExternalOutcome, PendingDelete, SettingsSection, SettingsState,
};
use crate::theme_host::{ApplyReport, ThemeHost};
use crate::wsl_open::WslOpenState;
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
    Button, ButtonVariant, ControlSize, EmptyState, Icon, IconButton, IconSize, Kbd, Modal,
    ProfileIcon, ProfileIconSource, Tab as TabView, TabStatus, TextField, Toast, text_tooltip,
};
use axf_ui::fonts::{MONO_PREFERRED, UI_PREFERRED};
use axf_ui::gpui::base::FocusTrapElement as _;
use axf_ui::gpui::component::button::{
    Button as KitButton, ButtonVariants as _, DropdownButton, Toggle as KitToggle, ToggleGroup,
};
use axf_ui::gpui::component::input::{InputEvent, InputState};
use axf_ui::gpui::component::menu::{
    ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem,
};
use axf_ui::gpui::component::radio::RadioGroup;
use axf_ui::gpui::component::scroll::ScrollableElement as _;
use axf_ui::gpui::component::select::{Select, SelectEvent, SelectState};
use axf_ui::gpui::component::slider::{Slider, SliderEvent, SliderState};
use axf_ui::gpui::component::switch::Switch;
use axf_ui::gpui::component::{
    ActiveTheme, Disableable as _, ElementExt as _, Sizable as _, WindowExt as _,
};
use axf_ui::gpui::prelude::FluentBuilder as _;
#[cfg(target_os = "windows")]
use axf_ui::gpui::svg;
use axf_ui::gpui::{
    Anchor, AnyElement, App, AppContext, Bounds, ClickEvent, Context, Div, ElementId, Entity,
    FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, Keystroke,
    MouseButton,
    ParentElement, Pixels, Point, Render, Role, ScrollDelta, ScrollEvent, ScrollHandle,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled, Subscription, Window,
    WindowControlArea, div, point, px, relative, transparent_black,
};
use axf_ui::icons;
use axf_ui::metrics::UiMetrics;
use axf_ui::theme;
use axf_workspace::{
    Branch, PaneId, PaneTree, ProjectHost, ProjectId, SessionFile, SidebarSection, SplitAxis, TabId,
    TerminalId, Workspace,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
const MAX_TABS: usize = 32;
const APP_NAME: &str = "Axfterm";
const MAX_NOTICES: usize = 3;
const NOTICE_COOLDOWN: Duration = Duration::from_secs(10);
const OUTPUT_QUIET: Duration = Duration::from_millis(2_500);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarRow {
    Project(ProjectId),
    Tab(TabId),
}
struct SidebarGroup {
    project: ProjectId,
    tabs: Vec<TabId>,
    /// `false` cuando el proyecto está plegado.
    show_tabs: bool,
}
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
fn next_attention(attention: &[TabId], current: Option<TabId>) -> Option<TabId> {
    let first = *attention.first()?;
    Some(
        current
            .and_then(|current| attention.iter().position(|tab| *tab == current))
            .map(|index| attention[(index + 1) % attention.len()])
            .unwrap_or(first),
    )
}
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
#[derive(Clone, Debug, PartialEq, Eq)]
enum ProjectHealth {
    /// Comprobación en curso.
    Checking,
    /// Distro y carpeta disponibles.
    Ready,
    /// Distro o carpeta no disponible, con motivo.
    Missing(String),
}

impl ProjectHealth {
    fn missing(&self) -> Option<&str> {
        match self {
            Self::Missing(reason) => Some(reason),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum PendingConfirm {
    ClosePane(PaneId),
    RemoveProject(ProjectId),
}

/// Destino de arranque de una terminal: host y ruta en su dialecto.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SpawnTarget {
    host: ProjectHost,
    path: PathBuf,
}
struct TerminalEntry {
    view: Entity<TerminalView>,
    _subscription: Subscription,
}
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
#[derive(Clone)]
struct TabDrag {
    tab: TabId,
    title: SharedString,
    icon: ProfileIconSource,
    status: Option<TabStatus>,
}
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TabDropTarget {
    pane: PaneId,
    zone: TabDropZone,
}
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
fn adjusted_insert_index(source: Option<usize>, index: usize) -> usize {
    match source {
        Some(from) if from < index => index.saturating_sub(1),
        _ => index,
    }
}
fn tab_slot_width(strip_width: Pixels, count: usize, min_width: Pixels, max_width: Pixels) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let fit = f32::from(strip_width) / count as f32;
    fit.clamp(f32::from(min_width), f32::from(max_width))
}
fn insert_index_at(local_x: f32, tab_width: f32, count: usize) -> usize {
    if count == 0 || tab_width <= 0.0 {
        return 0;
    }
    let slot = (local_x / tab_width + 0.5).floor();
    (slot.max(0.0) as usize).min(count)
}
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
#[derive(Clone)]
struct SplitDrag {
    path: Vec<Branch>,
    axis: SplitAxis,
    key: String,
}
#[derive(Clone, Debug)]
struct Notice {
    text: String,
    detail: Option<String>,
    error: bool,
    /// Clave de deduplicación; los avisos de agente la usan.
    key: Option<String>,
    seq: u64,
}
type AgentTransitions = Vec<(AgentSessionId, AgentTransition)>;
struct AgentTabUpdate {
    tab: TabId,
    changed: bool,
    transitions: AgentTransitions,
}
#[derive(Default)]
struct AgentUiState {
    /// Identificador de la siguiente sesión detectada.
    seq: u64,
    /// Cooldown de avisos por clave.
    notice_cooldowns: HashMap<String, Instant>,
}
/// Modo del modal de perfil.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ProfileEditorMode {
    /// Perfil que aún no está en el borrador.
    New(ProfileId),
    /// Perfil existente que se actualiza en el sitio.
    Edit(ProfileId),
}

impl ProfileEditorMode {
    fn id(&self) -> &ProfileId {
        match self {
            Self::New(id) | Self::Edit(id) => id,
        }
    }
}

struct ProfileEditor {
    mode: ProfileEditorMode,
    name: Entity<InputState>,
    command: Entity<InputState>,
    opencode_command: Entity<InputState>,
    copilot_command: Entity<InputState>,
    icon: Option<PathBuf>,
    error: Option<String>,
}

/// Sliders numéricos del panel; viven mientras dura el borrador.
struct SettingsSliders {
    ui_font_size: Entity<SliderState>,
    ui_scale: Entity<SliderState>,
    terminal_font_size: Entity<SliderState>,
    line_height: Entity<SliderState>,
    scrollback_lines: Entity<SliderState>,
    _subscriptions: Vec<Subscription>,
}

/// Selectores de perfil preferido por agente.
struct AgentProfileSelects {
    /// Ids y nombres con los que se construyeron las listas.
    profile_signature: Vec<(ProfileId, String)>,
    opencode: Entity<SelectState<ProfileList>>,
    copilot: Entity<SelectState<ProfileList>>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StepField {
    ScanInterval,
}
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
    file_drag_hover: FileDragHoverState,
    sidebar_list_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
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
    wsl_open: Option<WslOpenState>,
    project_health: HashMap<ProjectId, ProjectHealth>,
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
    settings_profile_focus: FocusHandle,
    settings_sliders: Option<SettingsSliders>,
    settings_agent_selects: Option<AgentProfileSelects>,
    settings_shortcut_input: Option<Entity<InputState>>,
    settings_shortcut_subscription: Option<Subscription>,
    keystroke_interceptor: Option<Subscription>,
    style_revision: u64,
    appearance_subscription: Option<Subscription>,
    window_title: String,
    startup: Instant,
    startup_logged: bool,
    save_generation: u64,
    quitting: bool,
    close_hook_registered: bool,
    #[cfg(feature = "studio")]
    pub(crate) studio: Option<crate::studio::StudioState>,
}
