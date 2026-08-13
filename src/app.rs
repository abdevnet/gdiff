use crate::config;
use crate::diff_view::{self, DiffDoc};
use crate::git::{self, ChangedFile, TreeNode};
use crate::highlight::Engine;
use crate::theme::{Catalog, Theme, ThemeGroup};
use crate::watcher::{self, WatchEvent};
use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Key, Layout, Margin, RichText, ScrollArea, Sense,
    Stroke, TextEdit, Ui, Vec2,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

pub struct GdiffApp {
    repo: PathBuf,
    repo_input: String,
    repo_invalid: bool,
    branch: String,
    files: Vec<ChangedFile>,
    selected: Option<usize>,
    multi: HashSet<usize>,
    side_by_side: bool,
    theme_id: String,
    theme_filter: String,
    catalog: Catalog,
    theme: Theme,
    explorer_open: bool,
    explorer: Option<Vec<TreeNode>>,
    explorer_expanded: HashSet<String>,
    loaded: Option<LoadedDiff>,
    loading_diff: bool,
    pending_discard: Option<Vec<String>>,
    engine: Engine,
    job_tx: Sender<Job>,
    msg_rx: Receiver<Msg>,
    watch_rx: Receiver<WatchEvent>,
    _watch: watcher::Handle,
    refresh_token: u64,
    refresh_in_flight: bool,
    refresh_again: bool,
    editor_command: Option<String>,
    status: Option<String>,
    status_ok: bool,
    commit_message: String,
    commit_in_flight: bool,
    push_in_flight: bool,
    ahead: u32,
    behind: u32,
    has_upstream: bool,
    pending_commit_all: bool,
}

struct LoadedDiff {
    path: String,
    staged: bool,
    language: String,
    original: String,
    modified: String,
    doc: DiffDoc,
}

enum Job {
    Refresh {
        repo: PathBuf,
        token: u64,
    },
    LoadDiff {
        repo: PathBuf,
        path: String,
        staged: bool,
        token: u64,
    },
    LoadTree {
        repo: PathBuf,
    },
    Stage {
        repo: PathBuf,
        paths: Vec<String>,
    },
    Unstage {
        repo: PathBuf,
        paths: Vec<String>,
    },
    Discard {
        repo: PathBuf,
        paths: Vec<String>,
    },
    SetRepo {
        path: PathBuf,
        token: u64,
    },
    OpenEditor {
        repo: PathBuf,
        path: String,
        cmd: Option<String>,
    },
    Commit {
        repo: PathBuf,
        message: String,
        stage_all: bool,
    },
    Push {
        repo: PathBuf,
    },
}

enum Msg {
    Files {
        files: Vec<ChangedFile>,
        info: git::RepoInfo,
        token: u64,
    },
    Diff {
        path: String,
        staged: bool,
        original: String,
        modified: String,
        language: String,
        #[allow(dead_code)]
        token: u64,
    },
    Tree(Vec<TreeNode>),
    RepoFailed {
        error: String,
    },
    Error(String),
    Committed {
        summary: String,
        files: Vec<ChangedFile>,
        info: git::RepoInfo,
    },
    Pushed {
        summary: String,
        info: git::RepoInfo,
    },
}

impl GdiffApp {
    pub fn new(cc: &eframe::CreationContext<'_>, repo: PathBuf) -> Self {
        let cfg = config::load();
        let catalog = Catalog::load();
        let theme_id = cfg
            .theme
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "default".into());
        let theme = catalog.resolve(&theme_id);
        theme.apply(&cc.egui_ctx);

        let (job_tx, job_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        thread::Builder::new()
            .name("gdiff-git".into())
            .spawn(move || worker(job_rx, msg_tx, ctx))
            .expect("spawn git worker");

        let (watch_tx, watch_rx) = mpsc::channel();
        let watch = watcher::Handle::start(repo.clone(), watch_tx, cc.egui_ctx.clone());

        let info = git::get_repo_info(&repo);
        let mut app = Self {
            repo_input: repo.display().to_string(),
            repo,
            repo_invalid: false,
            branch: info.branch,
            files: Vec::new(),
            selected: None,
            multi: HashSet::new(),
            side_by_side: cfg.side_by_side.unwrap_or(true),
            theme_id,
            theme_filter: String::new(),
            catalog,
            theme,
            explorer_open: true,
            explorer: None,
            explorer_expanded: HashSet::new(),
            loaded: None,
            loading_diff: false,
            pending_discard: None,
            engine: Engine::new(),
            job_tx,
            msg_rx,
            watch_rx,
            _watch: watch,
            refresh_token: 1,
            refresh_in_flight: false,
            refresh_again: false,
            editor_command: cfg.editor_command,
            status: None,
            status_ok: false,
            commit_message: String::new(),
            commit_in_flight: false,
            push_in_flight: false,
            ahead: info.ahead,
            behind: info.behind,
            has_upstream: info.has_upstream,
            pending_commit_all: false,
        };
        app.request_refresh();
        let _ = app.job_tx.send(Job::LoadTree {
            repo: app.repo.clone(),
        });
        app
    }

    fn request_refresh(&mut self) {
        if self.refresh_in_flight {
            self.refresh_again = true;
            return;
        }
        self.refresh_in_flight = true;
        self.refresh_token += 1;
        let _ = self.job_tx.send(Job::Refresh {
            repo: self.repo.clone(),
            token: self.refresh_token,
        });
    }

    fn request_diff(&mut self, path: String, staged: bool) {
        self.loading_diff = true;
        self.refresh_token += 1;
        let _ = self.job_tx.send(Job::LoadDiff {
            repo: self.repo.clone(),
            path,
            staged,
            token: self.refresh_token,
        });
    }

    fn select_index(&mut self, index: usize) {
        if index >= self.files.len() {
            return;
        }
        self.selected = Some(index);
        let file = &self.files[index];
        if let Some(loaded) = &self.loaded {
            if loaded.path == file.path && loaded.staged == file.staged {
                return;
            }
        }
        self.request_diff(file.path.clone(), file.staged);
    }

    fn set_theme(&mut self, ctx: &egui::Context, id: String) {
        if id == self.theme_id {
            return;
        }
        self.theme_id = id.clone();
        self.theme = self.catalog.resolve(&id);
        self.theme.apply(ctx);
        config::set_theme(&id);
        if let Some(loaded) = &self.loaded {
            let doc = DiffDoc::build(
                &loaded.original,
                &loaded.modified,
                &loaded.path,
                &self.theme,
                &self.engine,
            );
            if let Some(loaded) = self.loaded.as_mut() {
                loaded.doc = doc;
            }
        }
    }

    fn change_repo(&mut self, path: PathBuf) {
        self.refresh_token += 1;
        let _ = self.job_tx.send(Job::SetRepo {
            path,
            token: self.refresh_token,
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(WatchEvent::Changed) = self.watch_rx.try_recv() {
            self.request_refresh();
        }
        while let Ok(msg) = self.msg_rx.try_recv() {
            self.handle_msg(ctx, msg);
        }
    }

    fn handle_msg(&mut self, ctx: &egui::Context, msg: Msg) {
        match msg {
            Msg::Files { files, info, token } => {
                if token < self.refresh_token && self.refresh_in_flight {
                    // stale, but still mark flight if this was the in-flight refresh
                }
                self.refresh_in_flight = false;
                self.apply_repo_info(ctx, &info);

                let prev_path = self
                    .selected
                    .and_then(|i| self.files.get(i).map(|f| (f.path.clone(), f.staged)));
                let prev_fp = fingerprint(&self.files);
                let new_fp = fingerprint(&files);
                self.files = files;

                if prev_fp == new_fp {
                    if let Some(sel) = self.selected {
                        if let Some(file) = self.files.get(sel) {
                            self.request_diff(file.path.clone(), file.staged);
                        }
                    }
                } else {
                    self.multi.clear();
                    let restored = prev_path.and_then(|(p, staged)| {
                        self.files
                            .iter()
                            .position(|f| f.path == p && f.staged == staged)
                            .or_else(|| self.files.iter().position(|f| f.path == p))
                    });
                    if self.files.is_empty() {
                        self.selected = None;
                        self.loaded = None;
                    } else {
                        self.select_index(restored.unwrap_or(0));
                    }
                }

                if self.refresh_again {
                    self.refresh_again = false;
                    self.request_refresh();
                }
            }
            Msg::Diff {
                path,
                staged,
                original,
                modified,
                language,
                token: _,
            } => {
                self.loading_diff = false;
                let same = self.loaded.as_ref().is_some_and(|l| {
                    l.path == path
                        && l.staged == staged
                        && l.original == original
                        && l.modified == modified
                });
                if same {
                    return;
                }
                let doc = DiffDoc::build(&original, &modified, &path, &self.theme, &self.engine);
                self.loaded = Some(LoadedDiff {
                    path,
                    staged,
                    language,
                    original,
                    modified,
                    doc,
                });
            }
            Msg::Tree(tree) => self.explorer = Some(tree),
            Msg::RepoFailed { error } => {
                self.repo_invalid = true;
                self.set_status(error, false);
            }
            Msg::Error(e) => {
                self.commit_in_flight = false;
                self.push_in_flight = false;
                self.set_status(e, false);
            }
            Msg::Committed {
                summary,
                files,
                info,
            } => {
                self.commit_in_flight = false;
                self.commit_message.clear();
                self.apply_repo_info(ctx, &info);
                self.files = files;
                self.selected = None;
                self.loaded = None;
                self.multi.clear();
                self.set_status(summary, true);
                if self.files.is_empty() {
                    // keep empty state
                } else {
                    self.select_index(0);
                }
            }
            Msg::Pushed { summary, info } => {
                self.push_in_flight = false;
                self.apply_repo_info(ctx, &info);
                self.set_status(summary, true);
            }
        }
    }

    fn apply_repo_info(&mut self, ctx: &egui::Context, info: &git::RepoInfo) {
        if self.repo != info.repo_path {
            self._watch.set_repo(info.repo_path.clone());
            self.explorer = None;
            self.explorer_expanded.clear();
        }
        self.repo = info.repo_path.clone();
        self.repo_input = info.repo_path.display().to_string();
        self.repo_invalid = false;
        self.branch = info.branch.clone();
        self.ahead = info.ahead;
        self.behind = info.behind;
        self.has_upstream = info.has_upstream;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "Git Diff Viewer — {}",
            info.repo_name
        )));
    }

    fn set_status(&mut self, msg: impl Into<String>, ok: bool) {
        self.status = Some(msg.into());
        self.status_ok = ok;
    }

    fn request_commit(&mut self, stage_all: bool) {
        let message = self.commit_message.trim().to_string();
        if message.is_empty() {
            self.set_status("Commit message is empty", false);
            return;
        }
        if self.commit_in_flight {
            return;
        }
        let has_staged = self.files.iter().any(|f| f.staged);
        if !stage_all && !has_staged {
            if self.files.is_empty() {
                self.set_status("Nothing to commit", false);
                return;
            }
            self.pending_commit_all = true;
            return;
        }
        self.commit_in_flight = true;
        let _ = self.job_tx.send(Job::Commit {
            repo: self.repo.clone(),
            message,
            stage_all,
        });
    }

    fn request_push(&mut self) {
        if self.push_in_flight {
            return;
        }
        self.push_in_flight = true;
        let _ = self.job_tx.send(Job::Push {
            repo: self.repo.clone(),
        });
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let editing = ctx.egui_wants_keyboard_input();
        if editing {
            return;
        }
        ctx.input(|i| {
            if i.key_pressed(Key::ArrowDown) && !self.files.is_empty() {
                let next = self
                    .selected
                    .map(|s| (s + 1).min(self.files.len() - 1))
                    .unwrap_or(0);
                self.multi.clear();
                self.select_index(next);
            }
            if i.key_pressed(Key::ArrowUp) && !self.files.is_empty() {
                let next = self.selected.map(|s| s.saturating_sub(1)).unwrap_or(0);
                self.multi.clear();
                self.select_index(next);
            }
            if i.key_pressed(Key::Z) && !i.modifiers.command && !i.modifiers.ctrl {
                if let Some(i) = self.selected {
                    if let Some(file) = self.files.get(i) {
                        let _ = self.job_tx.send(Job::OpenEditor {
                            repo: self.repo.clone(),
                            path: file.path.clone(),
                            cmd: self.editor_command.clone(),
                        });
                    }
                }
            }
            if i.key_pressed(Key::R) && !i.modifiers.command && !i.modifiers.ctrl {
                self.loaded = None;
                self.selected = None;
                self.request_refresh();
            }
        });
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            let dir = if path.is_file() {
                path.parent().unwrap_or(&path).to_path_buf()
            } else {
                path
            };
            self.repo_input = dir.display().to_string();
            self.change_repo(dir);
        }
    }
}

impl eframe::App for GdiffApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.handle_drops(&ctx);
        self.handle_keys(&ctx);
        self.discard_modal(ui);
        self.commit_all_modal(ui);

        let theme = self.theme.clone();

        egui::Panel::top("header")
            .frame(
                Frame::new()
                    .fill(theme.bg_panel)
                    .inner_margin(Margin::symmetric(12, 8))
                    .stroke(Stroke::new(1.0, theme.border)),
            )
            .show(ui, |ui| self.header(ui, &theme));

        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(280.0)
            .min_size(200.0)
            .max_size(600.0)
            .frame(
                Frame::new()
                    .fill(theme.bg_panel)
                    .inner_margin(Margin::symmetric(0, 0))
                    .stroke(Stroke::new(1.0, theme.border)),
            )
            .show(ui, |ui| {
                // Lock width to the panel so children cannot grow it each frame.
                let w = ui.available_width();
                ui.set_min_width(w);
                ui.set_max_width(w);
                self.sidebar(ui, &theme);
            });

        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme.editor_bg).inner_margin(0))
            .show(ui, |ui| self.editor(ui, &theme));
    }
}

impl GdiffApp {
    fn header(&mut self, ui: &mut Ui, theme: &Theme) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;

            let mut input = self.repo_input.clone();
            let te = TextEdit::singleline(&mut input)
                .desired_width(360.0)
                .font(egui::TextStyle::Monospace)
                .text_color(if self.repo_invalid {
                    theme.status_deleted
                } else {
                    theme.text
                });
            let resp = ui.add(te);
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.repo_input = input.clone();
                self.change_repo(PathBuf::from(input.trim()));
            } else if resp.changed() {
                self.repo_input = input;
                self.repo_invalid = false;
            }

            Frame::new()
                .fill(theme.branch_bg)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(8, 2))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(if self.branch.is_empty() {
                            "—"
                        } else {
                            &self.branch
                        })
                        .color(theme.branch_fg)
                        .size(12.0),
                    );
                });

            let n = self.files.len();
            ui.label(
                RichText::new(format!("{n} file{} changed", if n == 1 { "" } else { "s" }))
                    .color(theme.text_muted)
                    .size(12.0),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.theme_picker(ui, theme);
                ui.label(RichText::new("Theme").color(theme.text_muted).size(12.0));
                ui.add_space(8.0);
                self.mode_toggle(ui, theme, false, "Inline");
                self.mode_toggle(ui, theme, true, "Side by Side");
            });
        });
    }

    fn mode_toggle(&mut self, ui: &mut Ui, theme: &Theme, side: bool, label: &str) {
        let active = self.side_by_side == side;
        let fill = if active {
            theme.accent
        } else {
            theme.bg_control
        };
        let text = if active {
            theme.accent_on
        } else {
            theme.text_muted
        };
        let btn = egui::Button::new(RichText::new(label).color(text).size(12.0))
            .fill(fill)
            .stroke(Stroke::new(1.0, theme.accent))
            .corner_radius(CornerRadius::same(4));
        if ui.add(btn).clicked() {
            self.side_by_side = side;
            config::set_diff_mode(side);
        }
    }

    fn theme_picker(&mut self, ui: &mut Ui, theme: &Theme) {
        let current = self.catalog.display_name(&self.theme_id);
        let mut picked: Option<String> = None;
        egui::ComboBox::from_id_salt("theme_picker")
            .selected_text(RichText::new(&current).size(12.0))
            .width(180.0)
            .show_ui(ui, |ui| {
                ui.set_min_width(240.0);
                ui.add(
                    TextEdit::singleline(&mut self.theme_filter)
                        .desired_width(220.0)
                        .hint_text("Filter themes…"),
                );
                let filter = self.theme_filter.to_ascii_lowercase();
                let list = self.catalog.list();
                for group in [
                    ThemeGroup::Builtin,
                    ThemeGroup::Dark,
                    ThemeGroup::Light,
                    ThemeGroup::Contrast,
                ] {
                    let items: Vec<_> = list
                        .iter()
                        .filter(|t| t.group == group)
                        .filter(|t| {
                            filter.is_empty()
                                || t.name.to_ascii_lowercase().contains(&filter)
                                || t.id.to_ascii_lowercase().contains(&filter)
                        })
                        .collect();
                    if items.is_empty() {
                        continue;
                    }
                    let label = match group {
                        ThemeGroup::Builtin => "Built-in",
                        ThemeGroup::Dark => "Dark",
                        ThemeGroup::Light => "Light",
                        ThemeGroup::Contrast => "High Contrast",
                    };
                    ui.label(
                        RichText::new(label)
                            .color(theme.text_muted)
                            .small()
                            .strong(),
                    );
                    for item in items {
                        if ui
                            .selectable_label(item.id == self.theme_id, &item.name)
                            .clicked()
                        {
                            picked = Some(item.id.clone());
                        }
                    }
                }
            });
        if let Some(id) = picked {
            self.set_theme(ui.ctx(), id);
        }
    }

    fn sidebar(&mut self, ui: &mut Ui, theme: &Theme) {
        let w = ui.available_width();
        ui.set_min_width(w);
        ui.set_max_width(w);
        egui::Panel::bottom("shortcut_hint")
            .resizable(false)
            .show_separator_line(true)
            .frame(
                Frame::new()
                    .fill(theme.bg_panel)
                    .inner_margin(Margin::symmetric(12, 8))
                    .stroke(Stroke::new(1.0, theme.border)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    kbd(ui, "Up", theme);
                    kbd(ui, "Down", theme);
                    ui.label(RichText::new("navigate").color(theme.text_muted).size(11.0));
                    ui.add_space(6.0);
                    kbd(ui, "Z", theme);
                    ui.label(RichText::new("editor").color(theme.text_muted).size(11.0));
                    ui.add_space(6.0);
                    kbd(ui, "R", theme);
                    ui.label(RichText::new("refresh").color(theme.text_muted).size(11.0));
                });
            });

        if self.explorer_open {
            egui::Panel::bottom("file_explorer")
                .resizable(true)
                .default_size(320.0)
                .min_size(96.0)
                .frame(
                    Frame::new()
                        .fill(theme.bg_panel)
                        .inner_margin(Margin::symmetric(0, 4)),
                )
                .show(ui, |ui| {
                    self.explorer_header(ui, theme);
                    ScrollArea::vertical()
                        .id_salt("explorer")
                        .auto_shrink([false, false])
                        .show(ui, |ui| match &self.explorer {
                            None => {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Loading…").color(theme.text_muted).size(12.0),
                                );
                            }
                            Some(tree) => {
                                let nodes = tree.clone();
                                self.draw_tree(ui, theme, &nodes);
                            }
                        });
                });
        } else {
            egui::Panel::bottom("file_explorer_closed")
                .resizable(false)
                .show_separator_line(true)
                .frame(
                    Frame::new()
                        .fill(theme.bg_panel)
                        .inner_margin(Margin::symmetric(0, 4)),
                )
                .show(ui, |ui| {
                    self.explorer_header(ui, theme);
                });
        }

        self.changes_section(ui, theme);
    }

    fn changes_section(&mut self, ui: &mut Ui, theme: &Theme) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new("GIT CHANGES")
                    .color(theme.text_muted)
                    .size(11.0)
                    .strong(),
            );
            ui.label(
                RichText::new(self.files.len().to_string())
                    .color(theme.text_muted)
                    .size(10.0),
            );
            if self.ahead > 0 || self.behind > 0 {
                ui.label(
                    RichText::new(format!("↑{} ↓{}", self.ahead, self.behind))
                        .color(theme.text_muted)
                        .size(10.0),
                );
            }
        });

        self.commit_box(ui, theme);

        if self.multi.len() >= 2 {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("{} selected", self.multi.len()))
                        .color(theme.text_muted)
                        .size(12.0),
                );
                if small_btn(ui, "Stage", theme).clicked() {
                    let paths: Vec<String> = self
                        .multi
                        .iter()
                        .filter_map(|i| self.files.get(*i))
                        .filter(|f| !f.staged)
                        .map(|f| f.path.clone())
                        .collect();
                    if !paths.is_empty() {
                        let _ = self.job_tx.send(Job::Stage {
                            repo: self.repo.clone(),
                            paths,
                        });
                    }
                    self.multi.clear();
                }
                if small_btn(ui, "Unstage", theme).clicked() {
                    let paths: Vec<String> = self
                        .multi
                        .iter()
                        .filter_map(|i| self.files.get(*i))
                        .filter(|f| f.staged)
                        .map(|f| f.path.clone())
                        .collect();
                    if !paths.is_empty() {
                        let _ = self.job_tx.send(Job::Unstage {
                            repo: self.repo.clone(),
                            paths,
                        });
                    }
                    self.multi.clear();
                }
                if small_btn(ui, "Clear", theme).clicked() {
                    self.multi.clear();
                }
            });
        }

        let staged: Vec<usize> = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.staged)
            .map(|(i, _)| i)
            .collect();
        let unstaged: Vec<usize> = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.staged)
            .map(|(i, _)| i)
            .collect();

        ScrollArea::vertical()
            .id_salt("changes")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.files.is_empty() {
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Working tree clean ✓")
                                .color(theme.text_muted)
                                .size(13.0),
                        );
                    });
                    return;
                }
                if !staged.is_empty() {
                    self.group_header(ui, theme, "Staged", staged.len(), false);
                    let idxs = staged.clone();
                    for i in idxs {
                        self.file_row(ui, theme, i);
                    }
                }
                if !unstaged.is_empty() {
                    self.group_header(ui, theme, "Unstaged", unstaged.len(), true);
                    let idxs = unstaged.clone();
                    for i in idxs {
                        self.file_row(ui, theme, i);
                    }
                }
            });
    }

    fn group_header(&mut self, ui: &mut Ui, theme: &Theme, title: &str, n: usize, stage: bool) {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("{title} ({n})"))
                    .color(theme.text_muted)
                    .size(10.0)
                    .strong(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                let label = if stage { "+" } else { "−" };
                if ui
                    .add(
                        egui::Button::new(RichText::new(label).color(theme.text_muted))
                            .fill(Color32::TRANSPARENT)
                            .frame(false),
                    )
                    .on_hover_text(if stage { "Stage all" } else { "Unstage all" })
                    .clicked()
                {
                    let paths: Vec<String> = self
                        .files
                        .iter()
                        .filter(|f| f.staged != stage)
                        .map(|f| f.path.clone())
                        .collect();
                    if !paths.is_empty() {
                        if stage {
                            let _ = self.job_tx.send(Job::Stage {
                                repo: self.repo.clone(),
                                paths,
                            });
                        } else {
                            let _ = self.job_tx.send(Job::Unstage {
                                repo: self.repo.clone(),
                                paths,
                            });
                        }
                    }
                }
            });
        });
    }

    fn file_row(&mut self, ui: &mut Ui, theme: &Theme, index: usize) {
        let Some(file) = self.files.get(index).cloned() else {
            return;
        };
        let active = self.selected == Some(index);
        let selected = self.multi.contains(&index);
        let fill = if active {
            theme.bg_selected_strong
        } else if selected {
            theme.bg_selected
        } else {
            Color32::TRANSPARENT
        };

        let mut row_clicked = false;
        Frame::new()
            .fill(fill)
            .inner_margin(Margin::symmetric(12, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(file.status.letter())
                            .color(theme.status_color(file.status))
                            .monospace()
                            .strong()
                            .size(11.0),
                    );
                    let can_discard = !file.staged && file.status != git::FileStatus::Added;
                    let row_w = ui.available_width();
                    ui.allocate_ui_with_layout(
                        Vec2::new(row_w, 18.0),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            ui.set_min_width(row_w);
                            ui.set_max_width(row_w);
                            let action = if file.staged { "−" } else { "+" };
                            if icon_btn(ui, action, theme)
                                .on_hover_text(if file.staged { "Unstage" } else { "Stage" })
                                .clicked()
                            {
                                if file.staged {
                                    let _ = self.job_tx.send(Job::Unstage {
                                        repo: self.repo.clone(),
                                        paths: vec![file.path.clone()],
                                    });
                                } else {
                                    let _ = self.job_tx.send(Job::Stage {
                                        repo: self.repo.clone(),
                                        paths: vec![file.path.clone()],
                                    });
                                }
                            }
                            if can_discard {
                                if icon_btn(ui, "↩", theme)
                                    .on_hover_text("Discard changes")
                                    .clicked()
                                {
                                    self.pending_discard = Some(vec![file.path.clone()]);
                                }
                            }
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                if ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(&file.path).color(theme.text).size(13.0),
                                        )
                                        .truncate()
                                        .sense(Sense::click()),
                                    )
                                    .clicked()
                                {
                                    row_clicked = true;
                                }
                            });
                        },
                    );
                });
            });

        if row_clicked {
            let mods = ui.input(|i| i.modifiers);
            if mods.command || mods.ctrl {
                if !self.multi.remove(&index) {
                    self.multi.insert(index);
                }
                self.select_index(index);
            } else if mods.shift {
                if let Some(cur) = self.selected {
                    let (a, b) = if cur < index {
                        (cur, index)
                    } else {
                        (index, cur)
                    };
                    for i in a..=b {
                        self.multi.insert(i);
                    }
                }
                self.select_index(index);
            } else {
                self.multi.clear();
                self.select_index(index);
            }
        }
    }

    fn explorer_header(&mut self, ui: &mut Ui, theme: &Theme) {
        let chevron = if self.explorer_open { "v" } else { ">" };
        let header = format!("{chevron}  FILE EXPLORER");
        if ui
            .add_sized(
                [ui.available_width(), 22.0],
                egui::Button::new(
                    RichText::new(header)
                        .color(theme.text_muted)
                        .size(11.0)
                        .strong(),
                )
                .fill(Color32::TRANSPARENT)
                .frame(false),
            )
            .clicked()
        {
            self.explorer_open = !self.explorer_open;
            if self.explorer_open && self.explorer.is_none() {
                let _ = self.job_tx.send(Job::LoadTree {
                    repo: self.repo.clone(),
                });
            }
        }
    }

    fn draw_tree(&mut self, ui: &mut Ui, theme: &Theme, nodes: &[TreeNode]) {
        for node in nodes {
            if node.is_dir {
                let open = self.explorer_expanded.contains(&node.path);
                let mark = if open { "v" } else { ">" };
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("{mark}  {}", node.name))
                                .color(theme.text)
                                .size(13.0),
                        )
                        .fill(Color32::TRANSPARENT)
                        .frame(false),
                    )
                    .clicked()
                {
                    if open {
                        self.explorer_expanded.remove(&node.path);
                    } else {
                        self.explorer_expanded.insert(node.path.clone());
                    }
                }
                if self.explorer_expanded.contains(&node.path) {
                    ui.indent(egui::Id::new(&node.path), |ui| {
                        self.draw_tree(ui, theme, &node.children);
                    });
                }
            } else {
                ui.label(RichText::new(&node.name).color(theme.text).size(13.0));
            }
        }
    }

    fn editor(&mut self, ui: &mut Ui, theme: &Theme) {
        if let Some(loaded) = &self.loaded {
            Frame::new()
                .fill(theme.bg_control)
                .inner_margin(Margin::symmetric(16, 6))
                .stroke(Stroke::new(1.0, theme.border))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&loaded.path)
                                .color(theme.text)
                                .strong()
                                .size(13.0),
                        );
                        ui.label(
                            RichText::new(&loaded.language)
                                .color(theme.accent)
                                .size(11.0),
                        );
                    });
                });
            let side = self.side_by_side;
            diff_view::show(ui, &loaded.doc, theme, side);
        } else {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("⎕").color(theme.text_muted).size(48.0));
                    ui.label(
                        RichText::new(if self.loading_diff {
                            "Loading diff…"
                        } else {
                            "Select a file to view diff"
                        })
                        .color(theme.text_muted)
                        .size(14.0),
                    );
                    if let Some(status) = &self.status {
                        ui.add_space(8.0);
                        ui.label(RichText::new(status).color(theme.status_deleted).size(12.0));
                    }
                });
            });
        }
    }

    fn discard_modal(&mut self, ui: &mut Ui) {
        let Some(paths) = self.pending_discard.clone() else {
            return;
        };
        let mut close = false;
        let mut confirm = false;
        egui::Modal::new(egui::Id::new("discard_modal")).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            ui.heading("Discard changes?");
            ui.add_space(6.0);
            let label = if paths.len() == 1 {
                format!("Discard changes to {}?", paths[0])
            } else {
                format!("Discard changes to {} files?", paths.len())
            };
            ui.label(label);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                if ui
                    .add(egui::Button::new("Discard").fill(self.theme.status_deleted))
                    .clicked()
                {
                    confirm = true;
                    close = true;
                }
            });
        });
        if confirm {
            let _ = self.job_tx.send(Job::Discard {
                repo: self.repo.clone(),
                paths,
            });
        }
        if close {
            self.pending_discard = None;
        }
    }

    fn commit_box(&mut self, ui: &mut Ui, theme: &Theme) {
        ui.add_space(4.0);
        ui.add_space(4.0);
        let hint = if cfg!(target_os = "macos") {
            "Message (⌘Enter to commit)"
        } else {
            "Message (Ctrl+Enter to commit)"
        };
        let box_w = (ui.available_width() - 16.0).max(80.0);
        let resp = ui.add_sized(
            [box_w, 56.0],
            TextEdit::multiline(&mut self.commit_message)
                .id_salt("commit_message")
                .hint_text(hint)
                .font(egui::TextStyle::Body)
                .desired_width(box_w),
        );
        if resp.has_focus()
            && ui.input(|i| i.key_pressed(Key::Enter) && (i.modifiers.command || i.modifiers.ctrl))
        {
            self.request_commit(false);
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let commit_label = if self.commit_in_flight {
                "Committing…"
            } else {
                "Commit"
            };
            let commit = ui.add_sized(
                [ui.available_width() * 0.5 - 6.0, 24.0],
                egui::Button::new(
                    RichText::new(commit_label)
                        .color(theme.accent_on)
                        .size(12.0),
                )
                .fill(theme.accent)
                .stroke(Stroke::new(1.0, theme.accent))
                .corner_radius(CornerRadius::same(4)),
            );
            if commit.clicked() && !self.commit_in_flight {
                self.request_commit(false);
            }

            let push_label = if self.push_in_flight {
                "Pushing…".to_string()
            } else if self.ahead > 0 {
                format!("Push ({})", self.ahead)
            } else {
                "Push".to_string()
            };
            let push = ui.add_sized(
                [ui.available_width() - 8.0, 24.0],
                egui::Button::new(RichText::new(push_label).color(theme.text).size(12.0))
                    .fill(theme.bg_control)
                    .stroke(Stroke::new(1.0, theme.accent))
                    .corner_radius(CornerRadius::same(4)),
            );
            if push.clicked() && !self.push_in_flight {
                self.request_push();
            }
        });

        if let Some(status) = &self.status {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let color = if self.status_ok {
                    theme.status_added
                } else {
                    theme.status_deleted
                };
                ui.add(egui::Label::new(RichText::new(status).color(color).size(11.0)).wrap());
            });
        }
        ui.add_space(4.0);
    }

    fn commit_all_modal(&mut self, ui: &mut Ui) {
        if !self.pending_commit_all {
            return;
        }
        let mut close = false;
        let mut confirm = false;
        egui::Modal::new(egui::Id::new("commit_all_modal")).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            ui.heading("No staged changes");
            ui.add_space(6.0);
            ui.label("There are no staged changes to commit. Stage all changes and commit?");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                if ui
                    .add(egui::Button::new("Commit All").fill(self.theme.accent))
                    .clicked()
                {
                    confirm = true;
                    close = true;
                }
            });
        });
        if confirm {
            self.request_commit(true);
        }
        if close {
            self.pending_commit_all = false;
        }
    }
}

fn fingerprint(files: &[ChangedFile]) -> String {
    files
        .iter()
        .map(|f| format!("{}:{}:{}", f.path, f.status.label(), f.staged))
        .collect::<Vec<_>>()
        .join("\n")
}

fn kbd(ui: &mut Ui, s: &str, theme: &Theme) {
    Frame::new()
        .fill(theme.bg_control)
        .stroke(Stroke::new(1.0, theme.accent))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(s).color(theme.text).size(10.0));
        });
}

fn small_btn(ui: &mut Ui, label: &str, theme: &Theme) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(theme.text_muted).size(11.0))
            .fill(theme.bg_panel)
            .stroke(Stroke::new(1.0, theme.accent))
            .corner_radius(CornerRadius::same(3)),
    )
}

fn icon_btn(ui: &mut Ui, label: &str, theme: &Theme) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(theme.text_muted).size(14.0))
            .fill(Color32::TRANSPARENT)
            .frame(false)
            .min_size(Vec2::new(18.0, 16.0)),
    )
}

fn worker(job_rx: Receiver<Job>, msg_tx: Sender<Msg>, ctx: egui::Context) {
    while let Ok(job) = job_rx.recv() {
        let msg = match job {
            Job::Refresh { repo, token } => match git::get_changed_files(&repo) {
                Ok(files) => Msg::Files {
                    files,
                    info: git::get_repo_info(&repo),
                    token,
                },
                Err(e) => Msg::Error(e),
            },
            Job::LoadDiff {
                repo,
                path,
                staged,
                token,
            } => {
                let diff = git::get_file_diff(&repo, &path, staged);
                Msg::Diff {
                    path,
                    staged,
                    original: diff.original,
                    modified: diff.modified,
                    language: diff.language.to_string(),
                    token,
                }
            }
            Job::LoadTree { repo } => Msg::Tree(git::get_file_tree(&repo)),
            Job::Stage { repo, paths } => match git::stage_files(&repo, &paths) {
                Ok(files) => Msg::Files {
                    files,
                    info: git::get_repo_info(&repo),
                    token: u64::MAX,
                },
                Err(e) => Msg::Error(e),
            },
            Job::Unstage { repo, paths } => match git::unstage_files(&repo, &paths) {
                Ok(files) => Msg::Files {
                    files,
                    info: git::get_repo_info(&repo),
                    token: u64::MAX,
                },
                Err(e) => Msg::Error(e),
            },
            Job::Discard { repo, paths } => match git::discard_files(&repo, &paths) {
                Ok(files) => Msg::Files {
                    files,
                    info: git::get_repo_info(&repo),
                    token: u64::MAX,
                },
                Err(e) => Msg::Error(e),
            },
            Job::SetRepo { path, token } => match git::resolve_repo(&path) {
                Ok(repo) => match git::get_changed_files(&repo) {
                    Ok(files) => Msg::Files {
                        files,
                        info: git::get_repo_info(&repo),
                        token,
                    },
                    Err(e) => Msg::RepoFailed { error: e },
                },
                Err(e) => Msg::RepoFailed { error: e },
            },
            Job::OpenEditor { repo, path, cmd } => {
                let abs = repo.join(&path);
                if let Err(e) = git::open_in_editor(&abs, cmd.as_deref()) {
                    Msg::Error(e)
                } else {
                    continue;
                }
            }
            Job::Commit {
                repo,
                message,
                stage_all,
            } => {
                if stage_all {
                    if let Err(e) = git::stage_all(&repo) {
                        Msg::Error(e)
                    } else {
                        match git::commit(&repo, &message) {
                            Ok(summary) => match git::get_changed_files(&repo) {
                                Ok(files) => Msg::Committed {
                                    summary,
                                    files,
                                    info: git::get_repo_info(&repo),
                                },
                                Err(e) => Msg::Error(e),
                            },
                            Err(e) => Msg::Error(e),
                        }
                    }
                } else {
                    match git::commit(&repo, &message) {
                        Ok(summary) => match git::get_changed_files(&repo) {
                            Ok(files) => Msg::Committed {
                                summary,
                                files,
                                info: git::get_repo_info(&repo),
                            },
                            Err(e) => Msg::Error(e),
                        },
                        Err(e) => Msg::Error(e),
                    }
                }
            }
            Job::Push { repo } => match git::push(&repo) {
                Ok(summary) => Msg::Pushed {
                    summary,
                    info: git::get_repo_info(&repo),
                },
                Err(e) => Msg::Error(e),
            },
        };
        let _ = msg_tx.send(msg);
        ctx.request_repaint();
    }
}
