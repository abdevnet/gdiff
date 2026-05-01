const { app, BrowserWindow, ipcMain } = require("electron");
const { execFile } = require("child_process");
const path = require("path");
const {
  getChangedFiles, getFileDiff, detectLanguage,
  stageFiles, unstageFiles, getFileTree, getRepoInfo, repoPath,
  getThemes,
} = require("./server");

let mainWindow;

function createWindow() {
  const iconPath = path.join(__dirname, "icon.png");
  const opts = {
    width: 1400,
    height: 900,
    title: `Git Diff Viewer — ${path.basename(repoPath)}`,
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
ipcMain.handle("get-changed-files", () => getChangedFiles());

ipcMain.handle("get-file-diff", (_, filePath, staged) => {
  const { original, modified } = getFileDiff(filePath, staged);
  const language = detectLanguage(filePath);
  return { original, modified, language, filePath };
});

ipcMain.handle("get-repo-info", () => getRepoInfo());
ipcMain.handle("refresh", () => getChangedFiles());
ipcMain.handle("stage-files", (_, filePaths) => stageFiles(filePaths));
ipcMain.handle("unstage-files", (_, filePaths) => unstageFiles(filePaths));
ipcMain.handle("get-file-tree", (_, subPath) => getFileTree(subPath ? path.join(repoPath, subPath) : undefined, undefined, subPath || ""));
ipcMain.handle("get-themes", () => getThemes());
ipcMain.handle("open-in-editor", (_, filePath) => {
  const abs = path.resolve(repoPath, filePath);
  execFile("open", ["-a", "Zed", abs]);
});

// File watcher — watch tracked files + git index (not entire repo tree)
let watchDebounce = null;
let fileWatcher = null;
let gitWatcher = null;

async function startWatcher() {
  const { execSync } = require("child_process");
  const chokidar = await import("chokidar");

  const trackedFiles = execSync("git ls-files", {
    cwd: repoPath, encoding: "utf-8", maxBuffer: 10 * 1024 * 1024,
  }).trim().split("\n").filter(Boolean).map(f => path.join(repoPath, f));

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
    path.join(repoPath, ".git", "index"),
    path.join(repoPath, ".git", "HEAD"),
  ], { ignoreInitial: true, persistent: true });
  gitWatcher.on("all", notify);
  gitWatcher.on("error", () => {});
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
