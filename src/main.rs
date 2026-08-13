mod app;
mod config;
mod diff_view;
mod git;
mod highlight;
mod theme;
mod watcher;

use app::GdiffApp;
use eframe::egui::{IconData, Vec2, ViewportBuilder};
use std::env;
use std::path::PathBuf;
use std::process;

fn main() -> eframe::Result {
    let repo_arg = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let repo = match git::resolve_repo(&repo_arg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    let name = repo
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| repo.display().to_string());

    let icon = load_icon();
    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(Vec2::new(1400.0, 900.0))
            .with_min_inner_size(Vec2::new(800.0, 500.0))
            .with_title(format!("Git Diff Viewer — {name}"))
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "gdiff",
        native_options,
        Box::new(move |cc| Ok(Box::new(GdiffApp::new(cc, repo)))),
    )
}

fn load_icon() -> IconData {
    let bytes = include_bytes!("../favicon.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let img = img.into_rgba8();
            let (width, height) = img.dimensions();
            IconData {
                rgba: img.into_raw(),
                width,
                height,
            }
        }
        Err(_) => IconData::default(),
    }
}
