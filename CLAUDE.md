# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Minimal Electron app that renders git diffs using Monaco's `createDiffEditor`. No build step — Monaco loads from CDN. Takes a repo path as an argument, making it worktree-friendly.

## Commands

```bash
npm install          # Install Electron
npm start            # Launch (uses cwd as repo)
npx electron . /path # Launch against a specific repo/worktree
./launch.sh /path    # Launcher script (used by shell alias / Ghostty keybind)
```

There are no tests, linters, or build steps.

## Architecture

Five files total:

- **main.js** — Main process. Runs git commands via `execSync`, parses `git status --porcelain`, serves original/modified file content over IPC. Repo path comes from `process.argv[2]` (passed through by `launch.sh`, NOT by `npm start` without `--`).
- **preload.js** — contextBridge exposing `window.gitDiff` with four IPC methods: `getChangedFiles`, `getFileDiff`, `getRepoInfo`, `refresh`.
- **index.html** — All UI and renderer logic in one file. Loads Monaco 0.44.0 from cdnjs CDN. Contains CSS, sidebar file list, diff editor setup, keyboard handlers.
- **launch.sh** — Shell wrapper that resolves its own directory and calls `npx electron . "$REPO_PATH"`.
- **package.json** — Single dependency: `electron`.

## Key Patterns

- All git interaction is synchronous (`execSync`) in the main process — acceptable for a local tool, but means large repos can block the UI during refresh.
- Monaco models are created per file selection and must be `.dispose()`d before creating new ones to avoid memory leaks.
- The `git()` helper swallows errors and returns empty string — file content functions rely on this for missing files (new/deleted).
- Language detection is a simple extension-to-Monaco-language map in `detectLanguage()`.
