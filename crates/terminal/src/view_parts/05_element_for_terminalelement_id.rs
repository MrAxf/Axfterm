impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout = window.request_layout(
            Style {
                size: Size::full(),
                ..Default::default()
            },
            std::iter::empty(),
            cx,
        );
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let cols = (f32::from(bounds.size.width) / f32::from(self.metrics.cell_width))
            .floor()
            .max(1.0) as usize;
        let rows = (f32::from(bounds.size.height) / f32::from(self.metrics.line_height))
            .floor()
            .max(1.0) as usize;

        self.view
            .update(cx, |view, _| view.observe_bounds(bounds.origin, cols, rows));

        let mut state = self.state.borrow_mut();
        let visible = rows.min(state.rows);
        let cursor = state.cursor;
        let mut lines = Vec::with_capacity(visible);
        for row in 0..visible {
            if let Some(cached) = state.cache[row].as_ref() {
                lines.push((row, cached.clone()));
                continue;
            }
            let RenderState {
                cells,
                combining,
                cache,
                scratch_text,
                scratch_runs,
                ..
            } = &mut *state;
            row_text_runs(
                &cells[row],
                &combining[row],
                &self.metrics,
                self.debug,
                scratch_text,
                scratch_runs,
            );
            let line = window.text_system().shape_line(
                scratch_text.clone().into(),
                self.metrics.font_size,
                scratch_runs,
                None,
            );
            cache[row] = Some(line.clone());
            lines.push((row, line));
        }

        let preedit = if self.preedit.is_empty() {
            None
        } else {
            let cursor = cursor.unwrap_or(CursorSnapshot {
                line: 0,
                column: 0,
                shape: CursorShape::Block,
            });
            let run = TextRun {
                len: self.preedit.len(),
                font: self.metrics.font.clone(),
                color: self.metrics.palette.foreground,
                background_color: None,
                underline: Some(UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(self.metrics.palette.cursor),
                    wavy: false,
                }),
                strikethrough: None,
            };
            let line = window.text_system().shape_line(
                self.preedit.clone().into(),
                self.metrics.font_size,
                &[run],
                None,
            );
            let origin = point(
                bounds.origin.x + self.metrics.cell_width * cursor.column,
                bounds.origin.y + self.metrics.line_height * cursor.line,
            );
            Some((line, origin))
        };

        PrepaintState {
            lines,
            preedit,
            rows,
            cols,
            origin: bounds.origin,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = self.state.borrow();
        let metrics = &self.metrics;
        let origin = prepaint.origin;
        let cell_width = metrics.cell_width;
        let line_height = metrics.line_height;
        let rows = prepaint.rows.min(state.rows);
        let cols = prepaint.cols.min(state.cols);

        window.paint_layer(bounds, |window| {
            // Fondo del grid: las celdas con color por defecto deben usar la
            // paleta del terminal aunque el contenedor tenga otro token.
            window.paint_quad(fill(bounds, metrics.palette.background));

            for row in 0..rows {
                paint_row_background(
                    &state.cells[row],
                    cols,
                    row,
                    origin,
                    cell_width,
                    line_height,
                    metrics,
                    window,
                    self.debug,
                );
            }

            if let Some(selection) = state.selection {
                for row in 0..rows {
                    paint_selection_row(
                        selection,
                        row,
                        cols,
                        origin,
                        cell_width,
                        line_height,
                        metrics,
                        window,
                    );
                }
            }

            for (row, line) in &prepaint.lines {
                if *row >= rows {
                    continue;
                }
                let line_origin = point(origin.x, origin.y + line_height * *row);
                let _ = line.paint(line_origin, line_height, TextAlign::Left, None, window, cx);
            }

            for row in 0..rows {
                for link in &state.links[row] {
                    paint_hyperlink(
                        link,
                        row,
                        cols,
                        origin,
                        cell_width,
                        line_height,
                        metrics,
                        window,
                    );
                }
            }

            if let Some(cursor) = state.cursor
                && cursor.line < rows
                && cursor.column < cols
            {
                paint_cursor(cursor, origin, cell_width, line_height, metrics, window);
            }

            if let Some((line, origin)) = &prepaint.preedit {
                window.paint_quad(fill(
                    Bounds::new(
                        point(origin.x, origin.y),
                        size(line.width().max(cell_width), line_height),
                    ),
                    metrics.palette.background.opacity(0.96),
                ));
                let _ = line.paint(*origin, line_height, TextAlign::Left, None, window, cx);
            }

            paint_scrollbar(
                bounds,
                rows,
                state.display_offset,
                state.history_size,
                line_height,
                metrics,
                window,
            );

            if state.bell_flash {
                window.paint_quad(fill(bounds, metrics.palette.cursor.opacity(0.06)));
            }

            if self.debug.row_lines {
                let color = metrics.palette.cursor.opacity(0.5);
                for row in 0..=rows {
                    let y = origin.y + line_height * row;
                    window.paint_quad(fill(
                        Bounds::new(point(origin.x, y), size(cell_width * cols, px(1.0))),
                        color,
                    ));
                }
            }
        });
    }
}
impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
fn row_text_runs(
    cells: &[SnapshotCell],
    combining: &[(usize, char)],
    metrics: &Metrics,
    debug: DebugRender,
    text: &mut String,
    runs: &mut Vec<TextRun>,
) {
    text.clear();
    runs.clear();
    let mut current: Option<(RunKey, usize)> = None;
    let mut combining_index = 0;

    for (column, cell) in cells.iter().enumerate() {
        if cell.attrs.wide_spacer {
            continue;
        }

        let ch = if cell.attrs.hidden { ' ' } else { cell.ch };
        let (foreground, _) = resolve_cell_colors(cell, &metrics.palette);
        let key = RunKey {
            color: foreground,
            bold: cell.attrs.bold,
            italic: cell.attrs.italic,
            underline: cell.attrs.underline && !debug.no_underline,
            strikethrough: cell.attrs.strikethrough && !debug.no_underline,
        };
        let mut len = ch.len_utf8();
        text.push(ch);

        while combining_index < combining.len() && combining[combining_index].0 < column {
            combining_index += 1;
        }
        while combining_index < combining.len() && combining[combining_index].0 == column {
            let extra = combining[combining_index].1;
            text.push(extra);
            len += extra.len_utf8();
            combining_index += 1;
        }

        match current {
            Some((previous, previous_len)) if previous == key => {
                current = Some((previous, previous_len + len));
            }
            Some((previous, previous_len)) => {
                runs.push(make_run(previous, previous_len, metrics));
                current = Some((key, len));
            }
            None => current = Some((key, len)),
        }
    }

    if let Some((key, len)) = current {
        runs.push(make_run(key, len, metrics));
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct RunKey {
    color: Hsla,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}
fn make_run(key: RunKey, len: usize, metrics: &Metrics) -> TextRun {
    let mut run_font = metrics.font.clone();
    if key.bold {
        run_font = run_font.bold();
    }
    if key.italic {
        run_font = run_font.italic();
    }

    TextRun {
        len,
        font: run_font,
        color: key.color,
        background_color: None,
        underline: key.underline.then_some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(key.color),
            wavy: false,
        }),
        strikethrough: key.strikethrough.then_some(StrikethroughStyle {
            thickness: px(1.0),
            color: Some(key.color),
        }),
    }
}
fn resolve_cell_colors(cell: &SnapshotCell, palette: &TerminalPalette) -> (Hsla, Hsla) {
    let mut foreground = resolve_color(cell.fg, true, palette);
    let background = resolve_color(cell.bg, false, palette);
    let background = if cell.attrs.reverse {
        std::mem::replace(&mut foreground, background)
    } else {
        background
    };

    if cell.attrs.dim {
        foreground = if matches!(cell.fg, CellColor::Default) {
            palette.dim_foreground()
        } else {
            foreground.opacity(0.75)
        };
    }

    (foreground, background)
}
fn resolve_color(color: CellColor, foreground: bool, palette: &TerminalPalette) -> Hsla {
    match color {
        CellColor::Default => {
            if foreground {
                palette.foreground
            } else {
                palette.background
            }
        }
        CellColor::Indexed(index) => palette.ansi(index),
        CellColor::Rgb(rgb) => axf_ui::gpui::rgb(rgb).into(),
    }
}
