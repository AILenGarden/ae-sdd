const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("monitorApi", {
  loadPreferences: () => ipcRenderer.invoke("load-preferences"),
  savePreferences: (preferences) => ipcRenderer.invoke("save-preferences", preferences),
  chooseDirectory: (defaultPath) => ipcRenderer.invoke("choose-directory", defaultPath),
  scanWorkspaces: (rootPath) => ipcRenderer.invoke("scan-workspaces", rootPath),
  loadWorkspaceDetail: (rootPath) => ipcRenderer.invoke("load-workspace-detail", rootPath),
  openPath: (targetPath) => ipcRenderer.invoke("open-path", targetPath),
  windowControl: (action) => ipcRenderer.invoke("window-control", action)
});
