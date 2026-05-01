# gdiff-viewer



Lightweight git diff viewer using Monaco's diff editor — VS Code's diff view in your browser. No build step, no heavy dependencies.

![gdiff-viewer screenshot](gdiff.png)

## Quick Start

```bash
npx gdiff-viewer                  # diff the current repo
npx gdiff-viewer /path/to/repo    # diff a specific repo or worktree
```

Each invocation binds an ephemeral port and opens a dedicated Chrome window via `--app`. Press `Ctrl-C` in the terminal to stop the server.

## Features

- **Monaco diff editor** with full syntax highlighting
- **Side-by-side** or **inline** diff modes
- **Stage / unstage / discard** directly from the UI
- **File explorer** sidebar with tree view
- **Auto-refresh** — watches tracked files and git index for changes
- **Keyboard navigation**: arrow keys to browse, `Z` open in editor, `R` refresh
- **Editable repo path** in the header — type or drag-drop a folder to switch
- **Theme picker** — Ghostty Purple, GitHub Dark, plus 326 bundled JetBrains color schemes (rainglow)
- **Configurable editor** for the `Z` shortcut (Zed by default, anything else via config)
- **Worktree-friendly** — pass any path, it resolves to the git root
- Works on **macOS, Windows, and Linux**

## Install Globally (optional)

```bash
npm install -g gdiff-viewer
gdiff /path/to/repo          # or: gdiff-viewer /path/to/repo
```

## Configuration

User settings live in `~/.gdiff-viewer.json`. The file is created automatically the first time you change a setting from the UI; you can also edit it by hand.

```json
{
  "theme": "absent",
  "editorCommand": "code {file}"
}
```

| Key | Default | Notes |
|-----|---------|-------|
| `theme` | `default` | Picker selection. Built-ins: `default`, `github-dark`. JetBrains theme ids are the XML filename (e.g. `absent`, `absent-light`, `absent-contrast`). |
| `editorCommand` | platform default below | Command run when you press `Z` on a file. `{file}` is replaced with the absolute path. |

Default `editorCommand`:

| Platform | Command |
|----------|---------|
| macOS | `open -a Zed {file}` |
| Windows | `notepad.exe {file}` |
| Linux | `xdg-open {file}` |

Examples:

```json
{ "editorCommand": "code {file}" }                         // VS Code
{ "editorCommand": "subl {file}" }                          // Sublime Text
{ "editorCommand": "C:\\\\Tools\\\\notepad++.exe {file}" }  // Notepad++ on Windows
```

## Themes

The picker is grouped: **Built-in**, **Dark**, **Light**, **High Contrast**. The 326 JetBrains schemes are extracted from the [rainglow](https://github.com/rainglow/jetbrains) collection at package time (see `THEMES_LICENSE` for attribution) and ship as a 180 KB JSON.

Both the surrounding UI chrome (sidebar, header, picker) and the Monaco editor adopt the selected theme.

## Architecture

No build step. Monaco and the JetBrains theme data ship inside the package, served locally with gzip — no CDN, no network round trips for the editor.

| File | Purpose |
|------|---------|
| `bin/gdiff.js` | CLI entry — starts server on an ephemeral port, opens Chrome `--app` |
| `server.js` | HTTP API — git commands, file serving, SSE file watcher, config persistence, theme bundle, Monaco static serving |
| `main.js` / `preload.js` | Electron entry — same logic as the HTTP server but over IPC |
| `index.html` | UI + Monaco diff editor (loads from `vs/`) |
| `vs/` | Bundled Monaco 0.44.0 (`min/vs` minus the IntelliSense workers we don't need for read-only diffs) |
| `themes.json` | 326 pre-extracted JetBrains color schemes |
| `scripts/build-themes.js` | One-shot builder: `node scripts/build-themes.js [xml-dir]` regenerates `themes.json` |

## Worktree Usage

```bash
npx gdiff-viewer ~/repos/project-feature-xyz
npx gdiff-viewer ~/repos/project-main
```

Or just type the path into the input at the top of the UI / drag a folder onto it.

## Electron App (optional)

If you prefer a standalone desktop window instead of the browser:

```bash
git clone https://github.com/abdevnet/gdiff.git
cd gdiff
npm install
npm start                       # diff the current directory
npx electron . /path/to/repo    # diff a specific repo
```

The included launcher script wraps that:

```bash
./launch.sh /path/to/repo
```

## License

MIT for the app code. Bundled JetBrains color schemes are MIT — see `THEMES_LICENSE`.
