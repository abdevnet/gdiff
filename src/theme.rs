use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};
use serde::Deserialize;
use std::collections::BTreeMap;

const THEMES_JSON: &str = include_str!("../themes.json");

#[derive(Debug, Clone, Copy)]
pub struct TokenStyle {
    pub color: Color32,
    pub italic: bool,
    pub bold: bool,
}

impl TokenStyle {
    pub fn solid(color: Color32) -> Self {
        Self {
            color,
            italic: false,
            bold: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenColors {
    pub keyword: TokenStyle,
    pub string: TokenStyle,
    pub number: TokenStyle,
    pub comment: TokenStyle,
    pub function: TokenStyle,
    pub type_name: TokenStyle,
    pub operator: TokenStyle,
    pub default: TokenStyle,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub is_dark: bool,
    pub bg_app: Color32,
    pub bg_panel: Color32,
    pub bg_panel_hover: Color32,
    pub bg_control: Color32,
    pub bg_selected: Color32,
    pub bg_selected_strong: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_on: Color32,
    pub status_modified: Color32,
    pub status_added: Color32,
    pub status_deleted: Color32,
    pub status_renamed: Color32,
    pub branch_bg: Color32,
    pub branch_fg: Color32,
    pub editor_bg: Color32,
    pub editor_fg: Color32,
    pub inserted_line: Color32,
    pub removed_line: Color32,
    pub line_number: Color32,
    pub tokens: TokenColors,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledTheme {
    name: String,
    editor_bg: String,
    editor_fg: String,
    chrome: BundledChrome,
    tokens: BundledTokens,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct BundledChrome {
    line_numbers: Option<String>,
    gutter: Option<String>,
    caret: Option<String>,
    caret_row: Option<String>,
    selection: Option<String>,
    added: Option<String>,
    deleted: Option<String>,
    modified: Option<String>,
    separator: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct BundledTokens {
    keyword: Option<TokenAttr>,
    string: Option<TokenAttr>,
    number: Option<TokenAttr>,
    comment: Option<TokenAttr>,
    function: Option<TokenAttr>,
    #[serde(rename = "type")]
    type_name: Option<TokenAttr>,
    operator: Option<TokenAttr>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TokenAttr {
    #[serde(rename = "FOREGROUND")]
    foreground: Option<String>,
    #[serde(rename = "FONT_TYPE")]
    font_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ThemeId {
    pub id: String,
    pub name: String,
    pub group: ThemeGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeGroup {
    Builtin,
    Dark,
    Light,
    Contrast,
}

pub struct Catalog {
    bundled: BTreeMap<String, BundledTheme>,
}

impl Catalog {
    pub fn load() -> Self {
        let bundled = serde_json::from_str(THEMES_JSON).unwrap_or_default();
        Self { bundled }
    }

    pub fn list(&self) -> Vec<ThemeId> {
        let mut out = vec![
            ThemeId {
                id: "default".into(),
                name: "Ghostty Purple".into(),
                group: ThemeGroup::Builtin,
            },
            ThemeId {
                id: "github-dark".into(),
                name: "GitHub Dark".into(),
                group: ThemeGroup::Builtin,
            },
        ];
        for (id, data) in &self.bundled {
            let name = data.name.replace(" (rainglow)", "").trim().to_string();
            let group = if id.ends_with("-light") {
                ThemeGroup::Light
            } else if id.ends_with("-contrast") {
                ThemeGroup::Contrast
            } else {
                ThemeGroup::Dark
            };
            out.push(ThemeId {
                id: id.clone(),
                name,
                group,
            });
        }
        out
    }

    pub fn resolve(&self, id: &str) -> Theme {
        match id {
            "github-dark" => github_dark(),
            "default" => ghostty_purple(),
            other => {
                if let Some(data) = self.bundled.get(other) {
                    from_bundled(other, data)
                } else {
                    ghostty_purple()
                }
            }
        }
    }

    pub fn display_name(&self, id: &str) -> String {
        self.list()
            .into_iter()
            .find(|t| t.id == id)
            .map(|t| t.name)
            .unwrap_or_else(|| id.to_string())
    }
}

impl Theme {
    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.is_dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        visuals.dark_mode = self.is_dark;
        visuals.panel_fill = self.bg_app;
        visuals.window_fill = self.bg_panel;
        visuals.window_stroke = Stroke::new(1.0, self.border);
        visuals.extreme_bg_color = self.bg_control;
        visuals.faint_bg_color = self.bg_panel_hover;
        visuals.code_bg_color = self.editor_bg;
        visuals.override_text_color = Some(self.text);
        visuals.selection.bg_fill = with_alpha(self.accent, 80);
        visuals.selection.stroke = Stroke::new(1.0, self.accent);
        visuals.hyperlink_color = self.accent;
        visuals.warn_fg_color = self.status_modified;
        visuals.error_fg_color = self.status_deleted;
        visuals.widgets.noninteractive.bg_fill = self.bg_panel;
        visuals.widgets.noninteractive.weak_bg_fill = self.bg_panel;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_muted);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        visuals.widgets.inactive.bg_fill = self.bg_control;
        visuals.widgets.inactive.weak_bg_fill = self.bg_control;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, self.accent);
        visuals.widgets.hovered.bg_fill = self.bg_panel_hover;
        visuals.widgets.hovered.weak_bg_fill = self.bg_panel_hover;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, self.accent);
        visuals.widgets.active.bg_fill = self.bg_selected;
        visuals.widgets.active.weak_bg_fill = self.bg_selected;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, self.accent_on);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, self.accent);
        visuals.widgets.open.bg_fill = self.bg_selected;
        visuals.widgets.open.weak_bg_fill = self.bg_selected;
        visuals.widgets.open.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, self.accent);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(4);
        visuals.widgets.active.corner_radius = CornerRadius::same(4);

        let egui_theme = if self.is_dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_theme(if self.is_dark {
            egui::ThemePreference::Dark
        } else {
            egui::ThemePreference::Light
        });
        ctx.style_mut_of(egui_theme, |style| {
            style.visuals = visuals;
            style.spacing.item_spacing = egui::vec2(6.0, 4.0);
            style.spacing.button_padding = egui::vec2(8.0, 4.0);
            style
                .text_styles
                .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
            style.text_styles.insert(
                TextStyle::Button,
                FontId::new(12.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Heading,
                FontId::new(16.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            );
        });
    }

    pub fn status_color(&self, status: crate::git::FileStatus) -> Color32 {
        match status {
            crate::git::FileStatus::Modified => self.status_modified,
            crate::git::FileStatus::Added => self.status_added,
            crate::git::FileStatus::Deleted => self.status_deleted,
            crate::git::FileStatus::Renamed => self.status_renamed,
        }
    }
}

fn ghostty_purple() -> Theme {
    let editor_bg = rgb(0x1e, 0x05, 0x28);
    let editor_fg = rgb(0xff, 0xee, 0xff);
    let accent = rgb(0x95, 0x5a, 0xe7);
    Theme {
        id: "default".into(),
        name: "Ghostty Purple".into(),
        is_dark: true,
        bg_app: editor_bg,
        bg_panel: rgb(0x1a, 0x09, 0x2d),
        bg_panel_hover: rgb(0x25, 0x0e, 0x3d),
        bg_control: rgb(0x32, 0x0f, 0x55),
        bg_selected: rgb(0x33, 0x13, 0x54),
        bg_selected_strong: rgb(0x4a, 0x1a, 0x6e),
        border: rgb(0x33, 0x13, 0x54),
        text: editor_fg,
        text_muted: rgb(0xe6, 0xa6, 0xff),
        accent,
        accent_on: Color32::WHITE,
        status_modified: rgb(0x00, 0xd9, 0xe9),
        status_added: rgb(0x05, 0xcb, 0x0d),
        status_deleted: rgb(0xaa, 0x00, 0xa3),
        status_renamed: rgb(0x4d, 0x6f, 0xff),
        branch_bg: accent,
        branch_fg: Color32::WHITE,
        editor_bg,
        editor_fg,
        inserted_line: with_alpha(rgb(0x05, 0xcb, 0x0d), 28),
        removed_line: with_alpha(rgb(0xaa, 0x00, 0xa3), 28),
        line_number: accent,
        tokens: TokenColors {
            keyword: TokenStyle::solid(rgb(0xe6, 0xa6, 0xff)),
            string: TokenStyle::solid(rgb(0x05, 0xcb, 0x0d)),
            number: TokenStyle::solid(rgb(0x00, 0xd9, 0xe9)),
            comment: TokenStyle {
                color: rgb(0x95, 0x5a, 0xe7),
                italic: true,
                bold: false,
            },
            function: TokenStyle::solid(rgb(0xff, 0xee, 0xff)),
            type_name: TokenStyle::solid(rgb(0xe6, 0xa6, 0xff)),
            operator: TokenStyle::solid(rgb(0xe6, 0xa6, 0xff)),
            default: TokenStyle::solid(editor_fg),
        },
    }
}

fn github_dark() -> Theme {
    let editor_bg = rgb(0x0d, 0x11, 0x17);
    let editor_fg = rgb(0xe6, 0xed, 0xf3);
    Theme {
        id: "github-dark".into(),
        name: "GitHub Dark".into(),
        is_dark: true,
        bg_app: editor_bg,
        bg_panel: rgb(0x16, 0x1b, 0x22),
        bg_panel_hover: rgb(0x1c, 0x21, 0x28),
        bg_control: rgb(0x21, 0x26, 0x2d),
        bg_selected: rgb(0x2d, 0x33, 0x3b),
        bg_selected_strong: rgb(0x1f, 0x6f, 0xeb),
        border: rgb(0x30, 0x36, 0x3d),
        text: editor_fg,
        text_muted: rgb(0x7d, 0x85, 0x90),
        accent: rgb(0x2f, 0x81, 0xf7),
        accent_on: Color32::WHITE,
        status_modified: rgb(0xd2, 0x99, 0x22),
        status_added: rgb(0x3f, 0xb9, 0x50),
        status_deleted: rgb(0xf8, 0x51, 0x49),
        status_renamed: rgb(0x2f, 0x81, 0xf7),
        branch_bg: rgb(0x1f, 0x6f, 0xeb),
        branch_fg: Color32::WHITE,
        editor_bg,
        editor_fg,
        inserted_line: with_alpha(rgb(0x3f, 0xb9, 0x50), 32),
        removed_line: with_alpha(rgb(0xf8, 0x51, 0x49), 32),
        line_number: rgb(0x6e, 0x76, 0x81),
        tokens: TokenColors {
            keyword: TokenStyle::solid(rgb(0xff, 0x7b, 0x72)),
            string: TokenStyle::solid(rgb(0xa5, 0xd6, 0xff)),
            number: TokenStyle::solid(rgb(0x79, 0xc0, 0xff)),
            comment: TokenStyle {
                color: rgb(0x8b, 0x94, 0x9e),
                italic: true,
                bold: false,
            },
            function: TokenStyle::solid(rgb(0xd2, 0xa8, 0xff)),
            type_name: TokenStyle::solid(rgb(0xff, 0xa6, 0x57)),
            operator: TokenStyle::solid(editor_fg),
            default: TokenStyle::solid(editor_fg),
        },
    }
}

fn from_bundled(id: &str, data: &BundledTheme) -> Theme {
    let editor_bg = parse_hex(&data.editor_bg).unwrap_or(rgb(0x1e, 0x1e, 0x1e));
    let editor_fg = parse_hex(&data.editor_fg).unwrap_or(rgb(0xd4, 0xd4, 0xd4));
    let is_dark = luminance(editor_bg) < 128.0;
    let ch = &data.chrome;
    let tk = &data.tokens;
    let accent = token_color(&tk.keyword)
        .or_else(|| ch.caret.as_deref().and_then(parse_hex))
        .or_else(|| token_color(&tk.function))
        .unwrap_or(if is_dark {
            rgb(0x56, 0x9c, 0xd6)
        } else {
            rgb(0x00, 0x66, 0xcc)
        });
    let text_muted = ch
        .line_numbers
        .as_deref()
        .and_then(parse_hex)
        .unwrap_or_else(|| shade(editor_fg, if is_dark { -0.4 } else { 0.4 }));
    let border = ch
        .separator
        .as_deref()
        .and_then(parse_hex)
        .unwrap_or_else(|| shade(editor_bg, if is_dark { 0.10 } else { -0.10 }));
    let panel_bg = ch
        .gutter
        .as_deref()
        .and_then(parse_hex)
        .unwrap_or_else(|| shade(editor_bg, if is_dark { -0.04 } else { 0.03 }));
    let panel_hover = shade(panel_bg, if is_dark { 0.05 } else { -0.05 });
    let control_bg = shade(editor_bg, if is_dark { 0.08 } else { -0.08 });
    let selected_bg = ch
        .selection
        .as_deref()
        .and_then(parse_hex)
        .unwrap_or_else(|| shade(editor_bg, if is_dark { 0.14 } else { -0.14 }));
    let selected_strong = shade(selected_bg, if is_dark { 0.10 } else { -0.10 });
    let status_added = ch
        .added
        .as_deref()
        .and_then(parse_hex)
        .unwrap_or(if is_dark {
            rgb(0x3f, 0xb9, 0x50)
        } else {
            rgb(0x1a, 0x7f, 0x37)
        });
    let status_deleted = ch
        .deleted
        .as_deref()
        .and_then(parse_hex)
        .unwrap_or(if is_dark {
            rgb(0xf8, 0x51, 0x49)
        } else {
            rgb(0xcf, 0x22, 0x2e)
        });
    let status_modified = ch
        .modified
        .as_deref()
        .and_then(parse_hex)
        .unwrap_or(if is_dark {
            rgb(0xd2, 0x99, 0x22)
        } else {
            rgb(0x9a, 0x67, 0x00)
        });

    let name = data.name.replace(" (rainglow)", "");
    Theme {
        id: id.to_string(),
        name,
        is_dark,
        bg_app: editor_bg,
        bg_panel: panel_bg,
        bg_panel_hover: panel_hover,
        bg_control: control_bg,
        bg_selected: selected_bg,
        bg_selected_strong: selected_strong,
        border,
        text: editor_fg,
        text_muted,
        accent,
        accent_on: Color32::WHITE,
        status_modified,
        status_added,
        status_deleted,
        status_renamed: status_modified,
        branch_bg: accent,
        branch_fg: Color32::WHITE,
        editor_bg,
        editor_fg,
        inserted_line: with_alpha(status_added, 32),
        removed_line: with_alpha(status_deleted, 32),
        line_number: text_muted,
        tokens: TokenColors {
            keyword: token_style(&tk.keyword, accent),
            string: token_style(&tk.string, status_added),
            number: token_style(&tk.number, editor_fg),
            comment: token_style(&tk.comment, text_muted),
            function: token_style(&tk.function, editor_fg),
            type_name: token_style(&tk.type_name, editor_fg),
            operator: token_style(&tk.operator, editor_fg),
            default: TokenStyle::solid(editor_fg),
        },
    }
}

fn token_color(attr: &Option<TokenAttr>) -> Option<Color32> {
    attr.as_ref()
        .and_then(|a| a.foreground.as_deref())
        .and_then(parse_hex)
}

fn token_style(attr: &Option<TokenAttr>, fallback: Color32) -> TokenStyle {
    let color = token_color(attr).unwrap_or(fallback);
    let ft = attr
        .as_ref()
        .and_then(|a| a.font_type.as_deref())
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(0);
    TokenStyle {
        color,
        italic: ft == 2 || ft == 3,
        bold: ft == 1 || ft == 3,
    }
}

pub fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn parse_hex(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    let h = if h.len() == 3 {
        let b = h.as_bytes();
        format!(
            "{0}{0}{1}{1}{2}{2}",
            b[0] as char, b[1] as char, b[2] as char
        )
    } else {
        h.to_string()
    };
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(&h, 16).ok()?;
    Some(Color32::from_rgb(
        ((n >> 16) & 0xff) as u8,
        ((n >> 8) & 0xff) as u8,
        (n & 0xff) as u8,
    ))
}

fn luminance(c: Color32) -> f32 {
    0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
}

fn shade(c: Color32, amt: f32) -> Color32 {
    let t = if amt < 0.0 { 0.0 } else { 255.0 };
    let a = amt.abs();
    let mix = |n: u8| -> u8 {
        let n = n as f32;
        (n + (t - n) * a).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_bundled_themes() {
        let cat = Catalog::load();
        let list = cat.list();
        assert!(
            list.len() > 300,
            "expected 326 rainglow + 2 builtins, got {}",
            list.len()
        );
        let t = cat.resolve("absent");
        assert_eq!(t.id, "absent");
        assert!(t.is_dark);
    }

    #[test]
    fn unknown_theme_falls_back() {
        let cat = Catalog::load();
        let t = cat.resolve("does-not-exist");
        assert_eq!(t.id, "default");
    }

    #[test]
    fn parse_hex_variants() {
        assert_eq!(parse_hex("#ff00aa"), Some(rgb(0xff, 0x00, 0xaa)));
        assert_eq!(parse_hex("228a96"), Some(rgb(0x22, 0x8a, 0x96)));
    }
}
