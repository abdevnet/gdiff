use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
}

impl FileStatus {
    pub fn letter(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Modified => "modified",
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
        }
    }

    fn from_code(code: char) -> Self {
        match code {
            'A' | '?' => Self::Added,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            _ => Self::Modified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub status: FileStatus,
    pub staged: bool,
}

#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub repo_name: String,
    pub branch: String,
    pub repo_path: PathBuf,
    pub ahead: u32,
    pub behind: u32,
    pub has_upstream: bool,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub original: String,
    pub modified: String,
    pub language: String,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
}

/// Parsed porcelain line before untracked-dir expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorcelainEntry {
    pub index: char,
    pub work: char,
    pub path: String,
    pub untracked_dir: bool,
}

const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    ".DS_Store",
    "bin",
    "obj",
    ".vs",
    "target",
];

pub fn resolve_repo(path: &Path) -> Result<PathBuf, String> {
    let abs = path
        .canonicalize()
        .or_else(|_| {
            if path.is_absolute() {
                Ok(path.to_path_buf())
            } else {
                std::env::current_dir().map(|cwd| cwd.join(path))
            }
        })
        .map_err(|e| format!("{}: {e}", path.display()))?;

    let out = git_ok(&abs, &["rev-parse", "--show-toplevel"])?;
    let root = out.trim();
    if root.is_empty() {
        return Err(format!("{} is not inside a git repository", path.display()));
    }
    Ok(PathBuf::from(root))
}

pub fn get_repo_info(repo: &Path) -> RepoInfo {
    let branch = {
        let current = git_soft(repo, &["branch", "--show-current"]);
        if current.is_empty() {
            git_soft(repo, &["rev-parse", "--short", "HEAD"])
        } else {
            current
        }
    };
    let (ahead, behind, has_upstream) = tracking(repo);
    RepoInfo {
        repo_name: repo
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| repo.display().to_string()),
        branch,
        repo_path: repo.to_path_buf(),
        ahead,
        behind,
        has_upstream,
    }
}

fn tracking(repo: &Path) -> (u32, u32, bool) {
    let counts = git_soft(
        repo,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    );
    parse_ahead_behind(&counts)
}

pub fn parse_ahead_behind(raw: &str) -> (u32, u32, bool) {
    let raw = raw.trim();
    if raw.is_empty() {
        return (0, 0, false);
    }
    let mut parts = raw.split_whitespace();
    let behind = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let ahead = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (ahead, behind, true)
}

pub fn commit(repo: &Path, message: &str) -> Result<String, String> {
    let msg = message.trim();
    if msg.is_empty() {
        return Err("Commit message is empty".into());
    }
    git_ok(repo, &["commit", "-m", msg])
}

pub fn push(repo: &Path) -> Result<String, String> {
    let upstream = git_soft(
        repo,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    );
    if upstream.is_empty() {
        git_ok(repo, &["push", "-u", "origin", "HEAD"])?;
        Ok("Pushed and set upstream to origin".into())
    } else {
        git_ok(repo, &["push"])?;
        Ok(format!("Pushed to {upstream}"))
    }
}

pub fn stage_all(repo: &Path) -> Result<Vec<ChangedFile>, String> {
    let _ = git_soft(repo, &["add", "-A"]);
    get_changed_files(repo)
}

pub fn parse_porcelain(raw: &str) -> Vec<PorcelainEntry> {
    let mut entries = Vec::new();
    for line in raw.split('\n') {
        if line.is_empty() {
            continue;
        }
        if line.len() < 3 {
            continue;
        }
        let index = line.chars().next().unwrap_or(' ');
        let work = line.chars().nth(1).unwrap_or(' ');
        let file_path = line[3..].trim();
        let actual = if let Some((_, after)) = file_path.split_once(" -> ") {
            after
        } else {
            file_path
        };
        let untracked_dir = index == '?' && work == '?' && actual.ends_with('/');
        entries.push(PorcelainEntry {
            index,
            work,
            path: actual.to_string(),
            untracked_dir,
        });
    }
    entries
}

pub fn get_changed_files(repo: &Path) -> Result<Vec<ChangedFile>, String> {
    let raw = git_ok(repo, &["status", "--porcelain"])?;
    let mut results = Vec::new();
    for entry in parse_porcelain(&raw) {
        if entry.untracked_dir {
            let dir_full = repo.join(&entry.path);
            match list_files_recursive(&dir_full, &entry.path) {
                Ok(files) if !files.is_empty() => {
                    for f in files {
                        results.push(ChangedFile {
                            path: f,
                            status: FileStatus::Added,
                            staged: false,
                        });
                    }
                }
                _ => {
                    results.push(ChangedFile {
                        path: entry.path,
                        status: FileStatus::Added,
                        staged: false,
                    });
                }
            }
            continue;
        }
        if entry.index != ' ' && entry.index != '?' {
            results.push(ChangedFile {
                path: entry.path.clone(),
                status: FileStatus::from_code(entry.index),
                staged: true,
            });
        }
        if entry.work != ' ' {
            results.push(ChangedFile {
                path: entry.path,
                status: FileStatus::from_code(entry.work),
                staged: false,
            });
        }
    }
    Ok(results)
}

pub fn get_file_diff(repo: &Path, file_path: &str, staged: bool) -> FileDiff {
    let (original, modified) = if staged {
        (
            file_content(repo, file_path, Version::Head),
            file_content(repo, file_path, Version::Staged),
        )
    } else {
        let index = file_content(repo, file_path, Version::Staged);
        let original = if index.is_empty() {
            file_content(repo, file_path, Version::Head)
        } else {
            index
        };
        (original, file_content(repo, file_path, Version::Working))
    };
    FileDiff {
        original,
        modified,
        language: detect_language(file_path).to_string(),
    }
}

pub fn stage_files(repo: &Path, paths: &[String]) -> Result<Vec<ChangedFile>, String> {
    if paths.is_empty() {
        return get_changed_files(repo);
    }
    let mut args = vec!["add", "--"];
    args.extend(paths.iter().map(String::as_str));
    let _ = git_soft_args(repo, &args);
    get_changed_files(repo)
}

pub fn unstage_files(repo: &Path, paths: &[String]) -> Result<Vec<ChangedFile>, String> {
    if paths.is_empty() {
        return get_changed_files(repo);
    }
    let has_commits = !git_soft(repo, &["rev-parse", "HEAD"]).is_empty();
    let mut args: Vec<&str> = if has_commits {
        vec!["reset", "HEAD", "--"]
    } else {
        vec!["rm", "--cached", "--"]
    };
    args.extend(paths.iter().map(String::as_str));
    let _ = git_soft_args(repo, &args);
    get_changed_files(repo)
}

pub fn discard_files(repo: &Path, paths: &[String]) -> Result<Vec<ChangedFile>, String> {
    if paths.is_empty() {
        return get_changed_files(repo);
    }
    let mut args = vec!["checkout", "--"];
    args.extend(paths.iter().map(String::as_str));
    let _ = git_soft_args(repo, &args);
    get_changed_files(repo)
}

pub fn get_file_tree(repo: &Path) -> Vec<TreeNode> {
    read_tree(repo, repo, "")
}

pub fn detect_language(file_path: &str) -> &'static str {
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "cs" => "csharp",
        "py" => "python",
        "rb" => "ruby",
        "go" => "go",
        "rs" => "rust",
        "java" => "java",
        "cpp" | "cc" | "cxx" => "cpp",
        "c" | "h" => "c",
        "css" => "css",
        "scss" => "scss",
        "html" => "html",
        "xml" | "csproj" => "xml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "sql" => "sql",
        "sh" | "bash" => "shell",
        "ps1" | "psm1" => "powershell",
        "toml" => "toml",
        "dockerfile" => "dockerfile",
        _ => "plaintext",
    }
}

pub fn default_editor_command() -> &'static str {
    if cfg!(target_os = "macos") {
        "open -a Zed {file}"
    } else if cfg!(target_os = "windows") {
        "notepad.exe {file}"
    } else {
        "xdg-open {file}"
    }
}

pub fn open_in_editor(abs_path: &Path, editor_command: Option<&str>) -> Result<(), String> {
    let cmd = editor_command
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_editor_command());
    let abs = abs_path.to_string_lossy();
    let mut parts: Vec<String> = cmd
        .split_whitespace()
        .map(|p| {
            if p == "{file}" {
                abs.to_string()
            } else {
                p.to_string()
            }
        })
        .filter(|p| !p.is_empty())
        .collect();
    if !parts.iter().any(|p| p == abs.as_ref()) {
        parts.push(abs.into_owned());
    }
    if parts.is_empty() {
        return Err("empty editor command".into());
    }
    let exe = parts.remove(0);
    eprintln!("gdiff: opening editor → {exe} {}", join_debug(&parts));

    #[cfg(target_os = "windows")]
    {
        spawn_windows(&exe, &parts)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let child = Command::new(&exe)
            .args(&parts)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("editor spawn failed ({exe}): {e}"))?;
        // Detach: don't wait.
        drop(child);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn spawn_windows(exe: &str, parts: &[String]) -> Result<(), String> {
    fn q(s: &str) -> String {
        format!("'{}'", s.replace('\'', "''"))
    }
    let ps = if parts.is_empty() {
        format!("Start-Process -FilePath {}", q(exe))
    } else {
        let args = parts.iter().map(|p| q(p)).collect::<Vec<_>>().join(", ");
        format!("Start-Process -FilePath {} -ArgumentList {args}", q(exe))
    };
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("editor spawn failed ({exe}): {e}"))?;
    Ok(())
}

fn join_debug(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| format!("{p:?}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Copy)]
enum Version {
    Head,
    Staged,
    Working,
}

fn file_content(repo: &Path, file_path: &str, version: Version) -> String {
    match version {
        Version::Head => {
            let spec = format!("HEAD:{file_path}");
            decode_git_bytes(&git_bytes(repo, &["show", &spec]))
        }
        Version::Staged => {
            let spec = format!(":{file_path}");
            decode_git_bytes(&git_bytes(repo, &["show", &spec]))
        }
        Version::Working => {
            let full = repo.join(file_path);
            match fs::read(&full) {
                Ok(bytes) => decode_bytes(&bytes),
                Err(_) => String::new(),
            }
        }
    }
}

fn decode_git_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    decode_bytes(bytes)
}

fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.contains(&0) {
        return "(binary file)".to_string();
    }
    const MAX: usize = 2 * 1024 * 1024;
    let slice = if bytes.len() > MAX {
        &bytes[..MAX]
    } else {
        bytes
    };
    let mut text = String::from_utf8_lossy(slice).into_owned();
    if bytes.len() > MAX {
        text.push_str("\n\n… truncated (file larger than 2 MB)");
    }
    text
}

fn list_files_recursive(dir: &Path, prefix: &str) -> Result<Vec<String>, std::io::Error> {
    let mut results = Vec::new();
    let entries = fs::read_dir(dir)?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let rel = format!("{prefix}{name}");
        let ft = entry.file_type()?;
        if ft.is_dir() {
            results.extend(list_files_recursive(&entry.path(), &format!("{rel}/"))?);
        } else {
            results.push(rel);
        }
    }
    Ok(results)
}

fn read_tree(repo: &Path, dir: &Path, prefix: &str) -> Vec<TreeNode> {
    let mut entries = Vec::new();
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return entries,
    };
    let mut items: Vec<_> = rd.flatten().collect();
    items.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| {
            a.file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&b.file_name().to_string_lossy().to_lowercase())
        })
    });
    for item in items {
        let name = item.file_name();
        let name_str = name.to_string_lossy();
        if SKIP_DIR_NAMES.contains(&name_str.as_ref()) {
            continue;
        }
        let rel = if prefix.is_empty() {
            name_str.to_string()
        } else {
            format!("{prefix}/{name_str}")
        };
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            let children = read_tree(repo, &item.path(), &rel);
            entries.push(TreeNode {
                name: name_str.into_owned(),
                path: rel,
                is_dir: true,
                children,
            });
        } else {
            entries.push(TreeNode {
                name: name_str.into_owned(),
                path: rel,
                is_dir: false,
                children: Vec::new(),
            });
        }
    }
    entries
}

fn git_ok(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let err = err.trim();
        if err.is_empty() {
            return Err(format!("git {} failed", args.join(" ")));
        }
        return Err(err.to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

fn git_soft(repo: &Path, args: &[&str]) -> String {
    git_soft_args(repo, args)
}

fn git_soft_args(repo: &Path, args: &[&str]) -> String {
    match Command::new("git").args(args).current_dir(repo).output() {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        Err(_) => String::new(),
    }
}

fn git_bytes(repo: &Path, args: &[&str]) -> Vec<u8> {
    match Command::new("git").args(args).current_dir(repo).output() {
        Ok(out) if out.status.success() || !out.stdout.is_empty() => out.stdout,
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_unstaged_modified() {
        let entries = parse_porcelain(" M src/main.rs");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index, ' ');
        assert_eq!(entries[0].work, 'M');
        assert_eq!(entries[0].path, "src/main.rs");
        assert!(!entries[0].untracked_dir);
    }

    #[test]
    fn porcelain_staged_and_unstaged() {
        let entries = parse_porcelain("MM foo.txt");
        assert_eq!(entries[0].index, 'M');
        assert_eq!(entries[0].work, 'M');
    }

    #[test]
    fn porcelain_rename() {
        let entries = parse_porcelain("R  old.rs -> new.rs");
        assert_eq!(entries[0].index, 'R');
        assert_eq!(entries[0].path, "new.rs");
        assert_eq!(FileStatus::from_code('R'), FileStatus::Renamed);
    }

    #[test]
    fn porcelain_untracked_dir() {
        let entries = parse_porcelain("?? dest/");
        assert!(entries[0].untracked_dir);
        assert_eq!(entries[0].path, "dest/");
    }

    #[test]
    fn porcelain_untracked_file() {
        let entries = parse_porcelain("?? new.txt");
        assert!(!entries[0].untracked_dir);
        assert_eq!(entries[0].work, '?');
    }

    #[test]
    fn porcelain_two_char_added_deleted() {
        let entries = parse_porcelain("AD gone.txt");
        assert_eq!(entries[0].index, 'A');
        assert_eq!(entries[0].work, 'D');
    }

    #[test]
    fn language_map() {
        assert_eq!(detect_language("a.rs"), "rust");
        assert_eq!(detect_language("Foo.CS"), "csharp");
        assert_eq!(detect_language("x.tsx"), "typescript");
        assert_eq!(detect_language("noext"), "plaintext");
    }

    #[test]
    fn fingerprint_entries_to_files() {
        let raw = "M  staged.rs\n M work.rs\n?? dest/\n";
        let entries = parse_porcelain(raw);
        assert_eq!(entries.len(), 3);
        let mut files = Vec::new();
        for e in entries {
            if e.untracked_dir {
                continue;
            }
            if e.index != ' ' && e.index != '?' {
                files.push((e.path.clone(), FileStatus::from_code(e.index), true));
            }
            if e.work != ' ' {
                files.push((e.path, FileStatus::from_code(e.work), false));
            }
        }
        assert_eq!(
            files,
            vec![
                ("staged.rs".into(), FileStatus::Modified, true),
                ("work.rs".into(), FileStatus::Modified, false),
            ]
        );
    }

    #[test]
    fn resolve_this_repo() {
        let root = resolve_repo(Path::new(".")).expect("this checkout is a git repo");
        assert!(root.join(".git").exists() || root.join(".git").is_file());
    }

    #[test]
    fn temp_repo_untracked_and_modified() {
        let dir = std::env::temp_dir().join(format!(
            "gdiff-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        assert!(Command::new("git")
            .args(["init"])
            .current_dir(&dir)
            .status()
            .unwrap()
            .success());
        let _ = Command::new("git")
            .args(["config", "user.email", "t@example.com"])
            .current_dir(&dir)
            .status();
        let _ = Command::new("git")
            .args(["config", "user.name", "t"])
            .current_dir(&dir)
            .status();
        fs::write(dir.join("hello.txt"), "one\n").unwrap();
        let files = get_changed_files(&dir).unwrap();
        assert!(
            files
                .iter()
                .any(|f| f.path == "hello.txt" && !f.staged && f.status == FileStatus::Added),
            "{files:?}"
        );
        stage_files(&dir, &["hello.txt".into()]).unwrap();
        let _ = Command::new("git")
            .args(["commit", "-m", "init", "--no-gpg-sign"])
            .current_dir(&dir)
            .status();
        fs::write(dir.join("hello.txt"), "two\n").unwrap();
        let files = get_changed_files(&dir).unwrap();
        assert!(
            files
                .iter()
                .any(|f| f.path == "hello.txt" && !f.staged && f.status == FileStatus::Modified),
            "{files:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_commit_message_rejected() {
        let err = commit(Path::new("."), "   ").unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn parse_tracking_counts() {
        assert_eq!(parse_ahead_behind(""), (0, 0, false));
        assert_eq!(parse_ahead_behind("2\t3"), (3, 2, true));
        assert_eq!(parse_ahead_behind("0 0"), (0, 0, true));
    }
}
