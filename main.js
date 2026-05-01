const { app, BrowserWindow, ipcMain } = require("electron");
const path = require("path");
const {
  getChangedFiles, getFileDiff, detectLanguage,
  stageFiles, unstageFiles, discardFiles, getFileTree, getRepoInfo,
  repoPath, getThemes, getConfig, setConfig, openInEditor,
} = require("./server");

let mainWindow;
let currentRepo = repoPath;

function createWindow() {
  const iconPath = path.join(__dirname, "icon.png");
  const opts = {
    width: 1400,
    height: 900,
    title: `Git Diff Viewer — ${path.basename(currentRepo)}`,
    backgroundColor: "#1e0528",
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, "preload.js"),
    },
  };
  try {
    if (require("fs").existsSync(iconPath)) opts.icon = iconPath;
  } catch {}

  mainWindow = new BrowserWindow(opts);

  mainWindow.loadFile("index.html");
}

// IPC Handlers
ipcMain.handle("get-changed-files", () => getChangedFiles(currentRepo));

ipcMain.handle("get-file-diff", (_, filePath, staged) => {
  const { original, modified } = getFileDiff(filePath, staged, currentRepo);
  const language = detectLanguage(filePath);
  return { original, modified, language, filePath };
});

ipcMain.handle("get-repo-info", () => getRepoInfo(currentRepo));
ipcMain.handle("refresh", () => getChangedFiles(currentRepo));
ipcMain.handle("stage-files", (_, filePaths) => stageFiles(filePaths, currentRepo));
ipcMain.handle("unstage-files", (_, filePaths) => unstageFiles(filePaths, currentRepo));
ipcMain.handle("discard-files", (_, filePaths) => discardFiles(filePaths, currentRepo));
ipcMain.handle("get-file-tree", (_, subPath) =>
  getFileTree(
    subPath ? path.join(currentRepo, subPath) : currentRepo,
    currentRepo,
    subPath || "",
  ),
);
ipcMain.handle("get-themes", () => getThemes());
ipcMain.handle("get-config", () => getConfig());
ipcMain.handle("set-config", (_, partial) => setConfig(partial));
ipcMain.handle("open-in-editor", (_, filePath) => {
  openInEditor(path.resolve(currentRepo, filePath));
});

ipcMain.handle("set-repo", async (_, newRepo) => {
  const { execSync } = require("child_process");
  let resolved;
  try {
    resolved = execSync("git rev-parse --show-toplevel", {
      cwd: path.resolve(newRepo), encoding: "utf-8",
    }).trim();
  } catch {
    return { ok: false, error: "not a git repository" };
  }
  currentRepo = resolved;
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.setTitle(`Git Diff Viewer — ${path.basename(currentRepo)}`);
  }
  await restartWatcher();
  return { ok: true, repoPath: currentRepo };
});

// File watcher
let watchDebounce = null;
let fileWatcher = null;
let gitWatcher = null;

async function startWatcher() {
  const { execSync } = require("child_process");
  const chokidar = await import("chokidar");

  const trackedFiles = execSync("git ls-files", {
    cwd: currentRepo, encoding: "utf-8", maxBuffer: 10 * 1024 * 1024,
  }).trim().split("\n").filter(Boolean).map(f => path.join(currentRepo, f));

  function notify() {
    clearTimeout(watchDebounce);
    watchDebounce = setTimeout(() => {
      if (mainWindow && !mainWindow.isDestroyed()) {
        mainWindow.webContents.send("files-changed");
      }
    }, 300);
  }

  fileWatcher = chokidar.watch(trackedFiles, {
    ignoreInitial: true, persistent: true, depth: 0,
  });
  fileWatcher.on("all", notify);
  fileWatcher.on("error", () => {});

  gitWatcher = chokidar.watch([
    path.join(currentRepo, ".git", "index"),
    path.join(currentRepo, ".git", "HEAD"),
  ], { ignoreInitial: true, persistent: true });
  gitWatcher.on("all", notify);
  gitWatcher.on("error", () => {});
}

async function restartWatcher() {
  await Promise.all([fileWatcher?.close(), gitWatcher?.close()]);
  fileWatcher = null;
  gitWatcher = null;
  await startWatcher();
}

app.whenReady().then(() => {
  const iconPath = path.join(__dirname, "icon.png");
  try {
    if (require("fs").existsSync(iconPath) && app.dock) {
      app.dock.setIcon(iconPath);
    }
  } catch {}
  createWindow();
  startWatcher();
});

app.on("window-all-closed", async () => {
  await Promise.all([
    fileWatcher?.close(),
    gitWatcher?.close(),
  ]);
  app.quit();
});
