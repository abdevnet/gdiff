use eframe::egui::Context;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

pub enum WatchEvent {
    Changed,
}

enum Cmd {
    SetRepo(PathBuf),
    Shutdown,
}

pub struct Handle {
    cmd: Sender<Cmd>,
}

impl Handle {
    pub fn start(repo: PathBuf, tx: Sender<WatchEvent>, ctx: Context) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        thread::Builder::new()
            .name("gdiff-watch".into())
            .spawn(move || run(repo, cmd_rx, tx, ctx))
            .expect("spawn watcher");
        Self { cmd: cmd_tx }
    }

    pub fn set_repo(&self, repo: PathBuf) {
        let _ = self.cmd.send(Cmd::SetRepo(repo));
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.cmd.send(Cmd::Shutdown);
    }
}

fn run(mut repo: PathBuf, cmd_rx: mpsc::Receiver<Cmd>, tx: Sender<WatchEvent>, ctx: Context) {
    let (ev_tx, ev_rx) = mpsc::channel();
    let mut watcher = match RecommendedWatcher::new(ev_tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("gdiff: watcher failed to start: {e}");
            return;
        }
    };
    let _ = watch_repo(&mut watcher, &repo);

    let mut dirty = false;
    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(350)) {
            Ok(Cmd::Shutdown) => break,
            Ok(Cmd::SetRepo(new_repo)) => {
                let _ = watcher.unwatch(&repo);
                repo = new_repo;
                let _ = watch_repo(&mut watcher, &repo);
                dirty = false;
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        while let Ok(ev) = ev_rx.try_recv() {
            if let Ok(event) = ev {
                if !is_noise(&event) {
                    dirty = true;
                }
            }
        }

        if dirty {
            dirty = false;
            let _ = tx.send(WatchEvent::Changed);
            ctx.request_repaint();
        }
    }
}

fn watch_repo(watcher: &mut RecommendedWatcher, repo: &Path) -> notify::Result<()> {
    watcher.watch(repo, RecursiveMode::Recursive)?;
    Ok(())
}

fn is_noise(event: &Event) -> bool {
    event.paths.iter().all(|p| {
        let s = p.to_string_lossy();
        contains_seg(&s, ".git/objects")
            || contains_seg(&s, ".git\\objects")
            || contains_seg(&s, ".git/logs")
            || contains_seg(&s, ".git\\logs")
            || contains_seg(&s, "/node_modules/")
            || contains_seg(&s, "\\node_modules\\")
            || contains_seg(&s, "/target/debug")
            || contains_seg(&s, "/target/release")
            || contains_seg(&s, "\\target\\debug")
            || contains_seg(&s, "\\target\\release")
            || s.ends_with(".DS_Store")
    })
}

fn contains_seg(hay: &str, needle: &str) -> bool {
    hay.contains(needle)
}
