const { contextBridge, ipcRenderer } = require("electron");

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
