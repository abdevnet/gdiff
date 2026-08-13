use crate::highlight::{Engine, Span};
use crate::theme::Theme;
use eframe::egui::{
    text::LayoutJob, Align, Color32, FontId, Pos2, Rect, RichText, ScrollArea, Sense, Stroke,
    TextFormat, Ui, Vec2,
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

pub fn show(ui: &mut Ui, doc: &DiffDoc, theme: &Theme, side_by_side: bool) {
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

    ScrollArea::both()
        .auto_shrink([false, false])
        .id_salt("diff_scroll")
        .show_viewport(ui, |ui, viewport| {
            ui.set_width(ui.available_width().max(avail.x));
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
                let row_rect = Rect::from_min_size(Pos2::new(origin.x, y), Vec2::new(width, ROW_H));
                paint_row(ui, row_rect, &doc.rows[i], theme, side_by_side, gutter_w);
            }
        });
}

fn paint_row(
    ui: &mut Ui,
    rect: Rect,
    row: &DiffRow,
    theme: &Theme,
    side_by_side: bool,
    gutter_w: f32,
) {
    if side_by_side {
        let mid = rect.center().x;
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
    }
}
