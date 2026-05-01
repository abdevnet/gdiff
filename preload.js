const { contextBridge, ipcRenderer } = require("electron");
const fs = require("fs");
const os = require("os");
const path = require("path");

// Inject the saved theme as a data attribute before any page script runs,
// so the FOUC script can read it without touching localStorage (which has
// a cold-start cost in fresh Electron renderers).
try {
  const cfg = JSON.parse(
    fs.readFileSync(path.join(os.homedir(), ".gdiff-viewer.json"), "utf-8"),
  );
  if (cfg.theme) {
    document.documentElement.dataset.themeInit = cfg.theme;
  }
} catch {}

contextBridge.exposeInMainWorld("gitDiff", {
  getChangedFiles: () => ipcRenderer.invoke("get-changed-files"),
  getFileDiff: (filePath, staged) =>
    ipcRenderer.invoke("get-file-diff", filePath, staged),
  getRepoInfo: () => ipcRenderer.invoke("get-repo-info"),
  refresh: () => ipcRenderer.invoke("refresh"),
  stageFiles: (paths) => ipcRenderer.invoke("stage-files", paths),
  unstageFiles: (paths) => ipcRenderer.invoke("unstage-files", paths),
  discardFiles: (paths) => ipcRenderer.invoke("discard-files", paths),
  getFileTree: (subPath) => ipcRenderer.invoke("get-file-tree", subPath),
  openInEditor: (filePath) => ipcRenderer.invoke("open-in-editor", filePath),
  getThemes: () => ipcRenderer.invoke("get-themes"),
  getConfig: () => ipcRenderer.invoke("get-config"),
  setConfig: (partial) => ipcRenderer.invoke("set-config", partial),
  setRepo: (newRepo) => ipcRenderer.invoke("set-repo", newRepo),
  onFilesChanged: (callback) => ipcRenderer.on("files-changed", callback),
});
