/// Espera antes de activar la pestaña bajo un drag de archivos.
const FILE_DRAG_HOVER_DELAY: Duration = Duration::from_millis(500);
/// Objetivo que se activa al mantener archivos arrastrados sobre la UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileDragHover {
    /// Pestaña de un panel.
    Tab(TabId),
    /// Terminal listada en la sidebar.
    SidebarTab(TabId),
    /// Proyecto plegado en la sidebar.
    SidebarProject(ProjectId),
}
/// Cambio de pestaña durante un drag de archivos.
///
/// `armed` permanece fijado tras activar para no repetir el foco en cada
/// movimiento; `seq` descarta los temporizadores que ya no corresponden al
/// objetivo bajo el puntero.
#[derive(Default)]
struct FileDragHoverState {
    armed: Option<FileDragHover>,
    seq: u64,
}
impl FileDragHoverState {
    /// Registra el objetivo bajo el puntero.
    ///
    /// Devuelve la secuencia del temporizador que debe activarlo, o `None` si
    /// ya estaba armado o el puntero salió de él.
    fn observe(&mut self, target: FileDragHover, over: bool) -> Option<u64> {
        if !over {
            if self.armed == Some(target) {
                self.armed = None;
            }
            return None;
        }
        if self.armed == Some(target) {
            return None;
        }
        self.seq = self.seq.wrapping_add(1);
        self.armed = Some(target);
        Some(self.seq)
    }
    /// Comprueba que el temporizador sigue siendo el vigente.
    fn is_current(&self, target: FileDragHover, seq: u64) -> bool {
        self.armed == Some(target) && self.seq == seq
    }
    /// Cancela el objetivo armado e invalida los temporizadores pendientes.
    fn clear(&mut self) {
        if self.armed.take().is_some() {
            self.seq = self.seq.wrapping_add(1);
        }
    }
}
/// Indica si el puntero está dentro del objetivo y de su viewport visible.
///
/// Los bounds de una fila desplazada por scroll siguen existiendo fuera de la
/// lista; el viewport evita activar filas que el usuario no ve.
fn file_drag_over(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    viewport: Option<Bounds<Pixels>>,
) -> bool {
    bounds_contain(bounds, position)
        && viewport.is_none_or(|viewport| bounds_contain(viewport, position))
}
impl Shell {
    /// Registra el paso de un drag de archivos sobre un objetivo.
    ///
    /// GPUI entrega `on_drag_move` a todos los elementos registrados, no solo
    /// al que está bajo el puntero; cada uno comprueba sus bounds y el
    /// viewport que lo recorta antes de llamar aquí.
    fn observe_file_drag(
        &mut self,
        target: FileDragHover,
        over: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(seq) = self.file_drag_hover.observe(target, over) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(FILE_DRAG_HOVER_DELAY)
                .await;
            let _ = this.update_in(cx, |shell, window, cx| {
                if shell.file_drag_hover.is_current(target, seq) && cx.has_active_drag() {
                    shell.activate_file_drag_hover(target, window, cx);
                }
            });
        })
        .detach();
    }

    /// Activa el objetivo sobre el que se mantiene un drag de archivos.
    fn activate_file_drag_hover(
        &mut self,
        target: FileDragHover,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.file_drag_blocked() {
            return;
        }
        match target {
            FileDragHover::Tab(tab) | FileDragHover::SidebarTab(tab) => {
                if self.workspace.focus().tab != Some(tab) {
                    self.focus_tab(tab, window, cx);
                }
            }
            FileDragHover::SidebarProject(project) => {
                if self.sidebar_section != SidebarSection::All
                    || self.workspace.expanded_projects().contains(&project)
                {
                    return;
                }
                let _ = self.workspace.set_project_expanded(project, true);
                self.persist(cx);
                cx.notify();
            }
        }
    }

    /// Hay una superficie modal que no debe cambiar de pestaña por hover.
    fn file_drag_blocked(&self) -> bool {
        self.settings.is_some()
            || self.palette.is_some()
            || self.shortcuts_modal
            || self.wsl_open.is_some()
    }

    /// Bounds del viewport visible de la lista de la sidebar.
    fn sidebar_viewport(&self) -> Option<Bounds<Pixels>> {
        *self.sidebar_list_bounds.borrow()
    }
}
