const { app, BrowserWindow, ipcMain } = require("electron");
const path = require("path");
const {
  getChangedFiles, getFileDiff, detectLanguage,
  stageFiles, unstageFiles, getRepoInfo, repoPath,
} = require("./server");

let mainWindow;

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    title: `Git Diff Viewer — ${path.basename(repoPath)}`,
    backgroundColor: "#1e0528",
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, "preload.js"),
    },
  });

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

// File watcher
let watchDebounce = null;
async function startWatcher() {
  const chokidar = await import("chokidar");
  const watcher = chokidar.watch(repoPath, {
    ignored: [/node_modules/, /\.git\/(?!index)/, /\.git$/],
    ignoreInitial: true,
    persistent: true,
  });

  watcher.on("all", () => {
    clearTimeout(watchDebounce);
    watchDebounce = setTimeout(() => {
      if (mainWindow && !mainWindow.isDestroyed()) {
        mainWindow.webContents.send("files-changed");
      }
    }, 300);
  });

  app.on("before-quit", () => watcher.close());
}

app.whenReady().then(() => {
  createWindow();
  startWatcher();
});
app.on("window-all-closed", () => app.quit());
