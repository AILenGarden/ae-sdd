const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("monitorApi", {
  loadPreferences: () => ipcRenderer.invoke("load-preferences"),
  savePreferences: (preferences) => ipcRenderer.invoke("save-preferences", preferences),
  chooseDirectory: (defaultPath) => ipcRenderer.invoke("choose-directory", defaultPath),
  scanWorkspaces: (rootPath) => ipcRenderer.invoke("scan-workspaces", rootPath),
  loadWorkspaceDetail: (rootPath) => ipcRenderer.invoke("load-workspace-detail", rootPath),
  watchWorkspaces: (rootPath) => ipcRenderer.invoke("watch-workspaces", rootPath),
  unwatchWorkspaces: () => ipcRenderer.invoke("unwatch-workspaces"),
  onWorkspaceFilesChanged: (callback) => {
    const listener = (_event, payload) => callback(payload);
    ipcRenderer.on("workspace-files-changed", listener);
    return () => ipcRenderer.removeListener("workspace-files-changed", listener);
  },
  openPath: (targetPath) => ipcRenderer.invoke("open-path", targetPath),
  windowControl: (action) => ipcRenderer.invoke("window-control", action)
});
