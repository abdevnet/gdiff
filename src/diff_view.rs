use crate::highlight::{Engine, Span};
use crate::theme::Theme;
use eframe::egui::{
    scroll_area::ScrollBarVisibility, text::LayoutJob, Align, Color32, CursorIcon, FontId, Id,
    Pos2, Rect, RichText, ScrollArea, Sense, Stroke, TextFormat, Ui, Vec2,
};
use similar::{DiffTag, TextDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Delete,
    Insert,
}

#[derive(Debug, Clone)]
pub struct LineCell {
    pub no: usize,
    pub spans: Vec<Span>,
    pub kind: LineKind,
}

#[derive(Debug, Clone)]
pub enum DiffRow {
    Both { left: LineCell, right: LineCell },
    LeftOnly { left: LineCell },
    RightOnly { right: LineCell },
}

#[derive(Debug, Clone)]
pub struct DiffDoc {
    pub rows: Vec<DiffRow>,
}

impl DiffDoc {
    pub fn build(
        original: &str,
        modified: &str,
        path: &str,
        theme: &Theme,
        engine: &Engine,
    ) -> Self {
        let old_lines = split_lines(original);
        let new_lines = split_lines(modified);
        let old_hi = engine.highlight(path, original, &theme.tokens);
        let new_hi = engine.highlight(path, modified, &theme.tokens);

        let old_spans = pad_spans(old_hi, old_lines.len(), theme.tokens.default.color);
        let new_spans = pad_spans(new_hi, new_lines.len(), theme.tokens.default.color);

        let diff = TextDiff::from_lines(original, modified);
        let mut rows = Vec::new();

        for op in diff.ops() {
            let old_range = op.old_range();
            let new_range = op.new_range();
            match op.tag() {
                DiffTag::Equal => {
                    for (oi, ni) in old_range.zip(new_range) {
                        rows.push(DiffRow::Both {
                            left: cell(oi, &old_spans, LineKind::Context),
                            right: cell(ni, &new_spans, LineKind::Context),
                        });
                    }
                }
                DiffTag::Delete => {
                    for oi in old_range {
                        rows.push(DiffRow::LeftOnly {
                            left: cell(oi, &old_spans, LineKind::Delete),
                        });
                    }
                }
                DiffTag::Insert => {
                    for ni in new_range {
                        rows.push(DiffRow::RightOnly {
                            right: cell(ni, &new_spans, LineKind::Insert),
                        });
                    }
                }
                DiffTag::Replace => {
                    let olds: Vec<usize> = old_range.collect();
                    let news: Vec<usize> = new_range.collect();
                    let n = olds.len().max(news.len());
                    for i in 0..n {
                        match (olds.get(i).copied(), news.get(i).copied()) {
                            (Some(oi), Some(ni)) => rows.push(DiffRow::Both {
                                left: cell(oi, &old_spans, LineKind::Delete),
                                right: cell(ni, &new_spans, LineKind::Insert),
                            }),
                            (Some(oi), None) => rows.push(DiffRow::LeftOnly {
                                left: cell(oi, &old_spans, LineKind::Delete),
                            }),
                            (None, Some(ni)) => rows.push(DiffRow::RightOnly {
                                right: cell(ni, &new_spans, LineKind::Insert),
                            }),
                            (None, None) => {}
                        }
                    }
                }
            }
        }

        Self { rows }
    }

    pub fn hunk_starts(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut in_hunk = false;
        for (i, row) in self.rows.iter().enumerate() {
            let (del, ins) = row_lanes(row);
            let changed = del || ins;
            if changed && !in_hunk {
                out.push(i);
            }
            in_hunk = changed;
        }
        out
    }
}

pub fn jump_to_row(ctx: &eframe::egui::Context, row: usize) {
    let y = row as f32 * ROW_H;
    ctx.data_mut(|d| d.insert_temp(Id::new("diff_scroll_jump"), y));
    ctx.request_repaint();
}

fn cell(idx: usize, spans: &[Vec<Span>], kind: LineKind) -> LineCell {
    LineCell {
        no: idx + 1,
        spans: spans.get(idx).cloned().unwrap_or_default(),
        kind,
    }
}

fn pad_spans(mut hi: Vec<Vec<Span>>, n: usize, fallback: Color32) -> Vec<Vec<Span>> {
    if hi.len() < n {
        hi.resize_with(n, || {
            vec![Span {
                text: String::new(),
                color: fallback,
                italics: false,
                strong: false,
            }]
        });
    }
    hi
}

pub fn split_lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut v: Vec<&str> = s.split('\n').collect();
    if s.ends_with('\n') {
        v.pop();
    }
    v
}

const ROW_H: f32 = 20.0;
const GUTTER_PAD: f32 = 8.0;
const FONT_SIZE: f32 = 13.0;
const RULER_W: f32 = 16.0;

pub fn show(ui: &mut Ui, doc: &DiffDoc, theme: &Theme, side_by_side: bool, split_ratio: &mut f32) {
    if doc.rows.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new("No differences")
                    .color(theme.text_muted)
                    .size(14.0),
            );
        });
        return;
    }

    let max_no = doc
        .rows
        .iter()
        .flat_map(|r| match r {
            DiffRow::Both { left, right } => [left.no, right.no],
            DiffRow::LeftOnly { left } => [left.no, left.no],
            DiffRow::RightOnly { right } => [right.no, right.no],
        })
        .max()
        .unwrap_or(1);
    let gutter_w =
        ((max_no.to_string().len() as f32) * FONT_SIZE * 0.65 + GUTTER_PAD * 2.0).max(36.0);

    let total_h = doc.rows.len() as f32 * ROW_H + 8.0;
    let avail = ui.available_size();
    let jump_id = Id::new("diff_scroll_jump");
    let jump_y = ui.ctx().data_mut(|d| d.remove_temp::<f32>(jump_id));

    ui.spacing_mut().item_spacing.x = 0.0;
    ui.horizontal(|ui| {
        let pane_h = avail.y;
        let pane_w = (ui.available_width() - RULER_W).max(40.0);
        let mut offset_y = 0.0;
        let mut view_h = pane_h;
        let mut content_h = total_h;
        let mut inner_rect = Rect::NOTHING;
        let ratio = *split_ratio;

        ui.allocate_ui(Vec2::new(pane_w, pane_h), |ui| {
            let mut area = ScrollArea::both()
                .auto_shrink([false, false])
                .id_salt("diff_scroll")
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden);
            if let Some(y) = jump_y {
                area = area.vertical_scroll_offset(y);
            }
            let out = area.show_viewport(ui, |ui, viewport| {
                ui.set_width(ui.available_width().max(pane_w));
                ui.set_height(total_h.max(avail.y));

                let start = ((viewport.min.y / ROW_H).floor() as usize).saturating_sub(2);
                let end = (((viewport.max.y / ROW_H).ceil() as usize) + 2).min(doc.rows.len());
                if start >= end {
                    return;
                }

                let origin = ui.max_rect().min;
                let width = ui.max_rect().width();

                for i in start..end {
                    let y = origin.y + i as f32 * ROW_H;
                    let row_rect =
                        Rect::from_min_size(Pos2::new(origin.x, y), Vec2::new(width, ROW_H));
                    let split_x = if side_by_side {
                        Some(origin.x + width * ratio)
                    } else {
                        None
                    };
                    paint_row(ui, row_rect, &doc.rows[i], theme, split_x, gutter_w);
                }
            });
            offset_y = out.state.offset.y;
            view_h = out.inner_rect.height();
            content_h = out.content_size.y.max(total_h);
            inner_rect = out.inner_rect;
        });

        if side_by_side && inner_rect.width() > 80.0 {
            let mid = inner_rect.min.x + inner_rect.width() * *split_ratio;
            let handle = Rect::from_center_size(
                Pos2::new(mid, inner_rect.center().y),
                Vec2::new(8.0, inner_rect.height()),
            );
            let resp = ui.interact(handle, ui.id().with("diff_split"), Sense::click_and_drag());
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                ui.painter().line_segment(
                    [
                        Pos2::new(mid, inner_rect.min.y),
                        Pos2::new(mid, inner_rect.max.y),
                    ],
                    Stroke::new(2.0, theme.accent),
                );
            }
            if resp.double_clicked() {
                *split_ratio = 0.5;
                crate::config::set_split_ratio(0.5);
            } else if resp.dragged() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    *split_ratio =
                        ((pos.x - inner_rect.min.x) / inner_rect.width()).clamp(0.2, 0.8);
                }
            } else if resp.drag_stopped() {
                crate::config::set_split_ratio(*split_ratio);
            }
        }

        let (ruler, resp) =
            ui.allocate_exact_size(Vec2::new(RULER_W, pane_h), Sense::click_and_drag());
        paint_overview(ui, ruler, doc, theme, offset_y, view_h, content_h);

        if resp.clicked() || resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let frac = ((pos.y - ruler.min.y) / ruler.height().max(1.0)).clamp(0.0, 1.0);
                let max_off = (content_h - view_h).max(0.0);
                let target = (frac * content_h - view_h * 0.5).clamp(0.0, max_off);
                ui.ctx().data_mut(|d| d.insert_temp(jump_id, target));
                ui.ctx().request_repaint();
            }
        }
    });
}

fn paint_overview(
    ui: &mut Ui,
    ruler: Rect,
    doc: &DiffDoc,
    theme: &Theme,
    offset_y: f32,
    view_h: f32,
    content_h: f32,
) {
    ui.painter().rect_filled(ruler, 0.0, theme.bg_panel);
    ui.painter().line_segment(
        [ruler.left_top(), ruler.left_bottom()],
        Stroke::new(1.0, theme.border),
    );

    let n = doc.rows.len().max(1) as f32;
    let h = ruler.height().max(1.0);
    let tick_h = (h / n).max(2.0);
    let mid = ruler.center().x;

    for (i, row) in doc.rows.iter().enumerate() {
        let (del, ins) = row_lanes(row);
        if !del && !ins {
            continue;
        }
        let y = ruler.min.y + (i as f32 / n) * h;
        let y1 = (y + tick_h).min(ruler.max.y);
        if del {
            ui.painter().rect_filled(
                Rect::from_min_max(Pos2::new(ruler.min.x + 1.0, y), Pos2::new(mid, y1)),
                0.0,
                theme.status_deleted,
            );
        }
        if ins {
            ui.painter().rect_filled(
                Rect::from_min_max(Pos2::new(mid, y), Pos2::new(ruler.max.x - 1.0, y1)),
                0.0,
                theme.status_added,
            );
        }
    }

    let max_off = (content_h - view_h).max(0.0);
    if max_off > 0.0 && view_h > 0.0 {
        let thumb_h = ((view_h / content_h) * h).clamp(18.0, h);
        let travel = h - thumb_h;
        let thumb_y = ruler.min.y + (offset_y / max_off) * travel;
        let thumb = Rect::from_min_size(
            Pos2::new(ruler.min.x + 1.0, thumb_y),
            Vec2::new((RULER_W - 2.0).max(4.0), thumb_h),
        );
        ui.painter()
            .rect_filled(thumb, 2.0, crate::theme::with_alpha(theme.text_muted, 140));
    }
}

fn row_lanes(row: &DiffRow) -> (bool, bool) {
    match row {
        DiffRow::Both { left, right } => (
            left.kind == LineKind::Delete,
            right.kind == LineKind::Insert,
        ),
        DiffRow::LeftOnly { left } => (left.kind == LineKind::Delete, false),
        DiffRow::RightOnly { right } => (false, right.kind == LineKind::Insert),
    }
}

fn paint_row(
    ui: &mut Ui,
    rect: Rect,
    row: &DiffRow,
    theme: &Theme,
    split_x: Option<f32>,
    gutter_w: f32,
) {
    if let Some(mid) = split_x {
        let left = Rect::from_min_max(rect.min, Pos2::new(mid, rect.max.y));
        let right = Rect::from_min_max(Pos2::new(mid, rect.min.y), rect.max);
        match row {
            DiffRow::Both { left: l, right: r } => {
                paint_cell(ui, left, Some(l), theme, gutter_w);
                paint_cell(ui, right, Some(r), theme, gutter_w);
            }
            DiffRow::LeftOnly { left: l } => {
                paint_cell(ui, left, Some(l), theme, gutter_w);
                paint_cell(ui, right, None, theme, gutter_w);
            }
            DiffRow::RightOnly { right: r } => {
                paint_cell(ui, left, None, theme, gutter_w);
                paint_cell(ui, right, Some(r), theme, gutter_w);
            }
        }
        ui.painter().line_segment(
            [Pos2::new(mid, rect.min.y), Pos2::new(mid, rect.max.y)],
            Stroke::new(1.0, theme.border),
        );
    } else {
        match row {
            DiffRow::Both { right, .. } => paint_cell(ui, rect, Some(right), theme, gutter_w),
            DiffRow::LeftOnly { left } => paint_cell(ui, rect, Some(left), theme, gutter_w),
            DiffRow::RightOnly { right } => paint_cell(ui, rect, Some(right), theme, gutter_w),
        }
    }
}

fn paint_cell(ui: &mut Ui, rect: Rect, cell: Option<&LineCell>, theme: &Theme, gutter_w: f32) {
    let bg = match cell.map(|c| c.kind) {
        Some(LineKind::Delete) => theme.removed_line,
        Some(LineKind::Insert) => theme.inserted_line,
        _ => theme.editor_bg,
    };
    ui.painter().rect_filled(rect, 0.0, bg);

    let gutter = Rect::from_min_max(rect.min, Pos2::new(rect.min.x + gutter_w, rect.max.y));
    ui.painter().rect_filled(gutter, 0.0, theme.bg_panel);
    if let Some(kind) = cell.map(|c| c.kind) {
        let mark = match kind {
            LineKind::Delete => Some(theme.status_deleted),
            LineKind::Insert => Some(theme.status_added),
            LineKind::Context => None,
        };
        if let Some(color) = mark {
            let stripe =
                Rect::from_min_max(Pos2::new(gutter.max.x - 3.0, gutter.min.y), gutter.max);
            ui.painter().rect_filled(stripe, 0.0, color);
        }
    }

    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    job.wrap.break_anywhere = false;

    if let Some(cell) = cell {
        job.append(
            &format!("{:>4} ", cell.no),
            0.0,
            TextFormat {
                font_id: FontId::monospace(FONT_SIZE),
                color: theme.line_number,
                valign: Align::Center,
                ..Default::default()
            },
        );
        let prefix = match cell.kind {
            LineKind::Delete => "-",
            LineKind::Insert => "+",
            LineKind::Context => " ",
        };
        let pcol = match cell.kind {
            LineKind::Delete => theme.status_deleted,
            LineKind::Insert => theme.status_added,
            LineKind::Context => theme.text_muted,
        };
        job.append(
            prefix,
            0.0,
            TextFormat {
                font_id: FontId::monospace(FONT_SIZE),
                color: pcol,
                valign: Align::Center,
                ..Default::default()
            },
        );
        job.append(
            " ",
            0.0,
            TextFormat {
                font_id: FontId::monospace(FONT_SIZE),
                color: theme.text_muted,
                valign: Align::Center,
                ..Default::default()
            },
        );
        if cell.spans.is_empty() {
            job.append(
                " ",
                0.0,
                TextFormat {
                    font_id: FontId::monospace(FONT_SIZE),
                    color: theme.editor_fg,
                    valign: Align::Center,
                    ..Default::default()
                },
            );
        } else {
            for span in &cell.spans {
                let mut fmt = TextFormat {
                    font_id: FontId::monospace(FONT_SIZE),
                    color: span.color,
                    valign: Align::Center,
                    italics: span.italics,
                    ..Default::default()
                };
                if span.strong {
                    // FontId doesn't carry weight for default fonts; color is enough.
                    fmt.color = span.color;
                }
                job.append(&span.text, 0.0, fmt);
            }
        }
    }

    // Place the galley starting in the gutter so line numbers sit there.
    let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
    let text_pos = Pos2::new(rect.min.x + 4.0, rect.min.y + (ROW_H - FONT_SIZE) * 0.35);
    ui.painter().galley(text_pos, galley, theme.editor_fg);

    let _ = ui.allocate_rect(rect, Sense::hover());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_trailing_newline() {
        assert_eq!(split_lines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(split_lines("a\nb"), vec!["a", "b"]);
        assert!(split_lines("").is_empty());
    }

    #[test]
    fn builds_insert_delete() {
        let theme = crate::theme::Catalog::load().resolve("default");
        let engine = Engine::new();
        let doc = DiffDoc::build("hello\n", "hello\nworld\n", "a.txt", &theme, &engine);
        assert!(doc
            .rows
            .iter()
            .any(|r| matches!(r, DiffRow::RightOnly { .. })));
        assert_eq!(doc.hunk_starts().len(), 1);
    }
}
