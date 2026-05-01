const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("gitDiff", {
  getChangedFiles: () => ipcRenderer.invoke("get-changed-files"),
  getFileDiff: (filePath, staged) =>
    ipcRenderer.invoke("get-file-diff", filePath, staged),
  getRepoInfo: () => ipcRenderer.invoke("get-repo-info"),
  refresh: () => ipcRenderer.invoke("refresh"),
  stageFiles: (paths) => ipcRenderer.invoke("stage-files", paths),
  unstageFiles: (paths) => ipcRenderer.invoke("unstage-files", paths),
  getFileTree: (subPath) => ipcRenderer.invoke("get-file-tree", subPath),
  openInEditor: (filePath) => ipcRenderer.invoke("open-in-editor", filePath),
  listThemes: () => ipcRenderer.invoke("list-themes"),
  loadTheme: (id) => ipcRenderer.invoke("load-theme", id),
  onFilesChanged: (callback) => ipcRenderer.on("files-changed", callback),
});
