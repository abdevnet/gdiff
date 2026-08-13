# CLAUDE.md

Native git diff viewer written in Rust (egui). Binary name is `gdiff`. Replaces the old Node/Monaco/Chrome `--app` stack.

Full project context lives in the dev-projects vault:

`~/projects/obsidian/dev-projects/Projects/ghostty-diff.md`

Read that note first for anything beyond a trivial change. The vault note still describes the JS app until it is updated.

## Commands

```bash
cargo test                        # unit tests (porcelain, themes, temp repo)
cargo run -- /path/to/repo        # native window against a repo or worktree
cargo run                         # current directory
cargo build --release --target aarch64-apple-darwin
```

Windows AMD64 builds run in CI (`x86_64-pc-windows-msvc`). Do not expect that target to link from macOS.

Config is still `~/.gdiff-viewer.json` (`theme`, `editorCommand`).
