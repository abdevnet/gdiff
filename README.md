# Git Diff Viewer

Lightweight Electron app using Monaco's diff editor to view git changes — essentially VS Code's diff view in a standalone window.

## Setup

```bash
cd git-diff-viewer
npm install
```

## Usage

```bash
# View diffs for current directory's repo
npm start

# View diffs for a specific repo/worktree
npx electron . /path/to/your/repo

# Or use the launcher script
chmod +x launch.sh
./launch.sh /path/to/your/repo
```

### From Ghostty

Add a keybinding in your Ghostty config to launch it directly:

```
# ~/.config/ghostty/config
keybind = super+shift+d=new_window:command=/path/to/git-diff-viewer/launch.sh
```

Or create a shell alias:

```bash
alias gdiff='~/git-diff-viewer/launch.sh'
```

Then from any repo:

```bash
gdiff .
gdiff ~/repos/my-worktree
```

## Features

- **Monaco diff editor** with full syntax highlighting (C#, SQL, PowerShell, TypeScript, etc.)
- **Side-by-side** or **inline** diff modes
- **Staged vs unstaged** grouping
- **Keyboard navigation**: ↑/↓ to browse files, R to refresh
- Auto-detects language from file extension
- Dark theme matching VS Code

## Architecture

Four files, zero build step:

| File | Purpose |
|------|---------|
| `main.js` | Electron main process — runs git commands, serves file content |
| `preload.js` | IPC bridge — exposes `gitDiff` API to renderer |
| `index.html` | UI + Monaco diff editor (loaded from CDN) |
| `package.json` | Just Electron as a dev dependency |

## Worktree-Friendly

Since it takes a path argument, it works great with git worktrees:

```bash
# Check diffs on a feature branch worktree
gdiff ~/repos/project-feature-xyz

# Check main worktree
gdiff ~/repos/project-main
```

## Future Ideas

- File watching with `chokidar` for auto-refresh
- Commit history browser (pick any two commits to diff)
- Integration with Claude Code for AI-powered diff summaries
- Tray icon / global hotkey to open from anywhere
