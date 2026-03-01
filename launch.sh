#!/bin/bash
# Launch git-diff-viewer on a repo path
# Usage: ./launch.sh [--serve] [/path/to/repo]
#   --serve   Start HTTP server mode (for browser/cmux)
#   Defaults to current directory if no path given

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if [ "$1" = "--serve" ]; then
  shift
  REPO_PATH="$(cd "${1:-.}" && pwd)"
  # Kill any existing server before starting
  lsof -ti :3420 2>/dev/null | xargs kill 2>/dev/null
  sleep 0.3
  cd "$SCRIPT_DIR"
  node server.js "$REPO_PATH"
else
  REPO_PATH="$(cd "${1:-.}" && pwd)"
  cd "$SCRIPT_DIR"
  npx electron . "$REPO_PATH"
fi
