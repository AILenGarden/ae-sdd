const path = require("path");
const fs = require("fs/promises");
const { app, BrowserWindow, dialog, ipcMain, shell } = require("electron");
const { loadWorkspaceDetail, scanForWorkspaces } = require("./workspace");

let mainWindow = null;

function preferencesPath() {
  return path.join(app.getPath("userData"), "preferences.json");
}

async function readPreferences() {
  try {
    const text = await fs.readFile(preferencesPath(), "utf8");
    const parsed = JSON.parse(text);
    return {
      rootPath: typeof parsed.rootPath === "string" ? parsed.rootPath : "",
      selectedRoot: typeof parsed.selectedRoot === "string" ? parsed.selectedRoot : "",
      theme: parsed.theme === "dark" ? "dark" : "light"
    };
  } catch {
    return { rootPath: "", selectedRoot: "", theme: "light" };
  }
}

async function writePreferences(nextPreferences) {
  const current = await readPreferences();
  const preferences = {
    ...current,
    ...Object.fromEntries(
      Object.entries(nextPreferences || {}).filter(([, value]) => typeof value === "string")
    )
  };
  await fs.mkdir(path.dirname(preferencesPath()), { recursive: true });
  await fs.writeFile(preferencesPath(), `${JSON.stringify(preferences, null, 2)}\n`, "utf8");
  return preferences;
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1240,
    height: 820,
    minWidth: 980,
    minHeight: 640,
    backgroundColor: "#f5f5f7",
    title: "ae-sdd Monitor",
    frame: false,
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  });

  mainWindow.loadFile(path.join(__dirname, "index.html"));
}

app.whenReady().then(() => {
  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

ipcMain.handle("load-preferences", async () => readPreferences());

ipcMain.handle("save-preferences", async (_event, preferences) => writePreferences(preferences));

ipcMain.handle("choose-directory", async (_event, defaultPath) => {
  const options = {
    properties: ["openDirectory"]
  };
  if (defaultPath && typeof defaultPath === "string") {
    options.defaultPath = defaultPath;
  }
  const result = await dialog.showOpenDialog(mainWindow, {
    ...options
  });
  if (result.canceled || !result.filePaths.length) {
    return null;
  }
  return result.filePaths[0];
});

ipcMain.handle("scan-workspaces", async (_event, rootPath) => {
  if (!rootPath || typeof rootPath !== "string") {
    throw new Error("rootPath is required");
  }
  return scanForWorkspaces(rootPath);
});

ipcMain.handle("load-workspace-detail", async (_event, rootPath) => {
  if (!rootPath || typeof rootPath !== "string") {
    throw new Error("rootPath is required");
  }
  return loadWorkspaceDetail(rootPath);
});

ipcMain.handle("open-path", async (_event, targetPath) => {
  if (!targetPath || typeof targetPath !== "string") {
    return "path is required";
  }
  return shell.openPath(targetPath);
});

ipcMain.handle("window-control", async (_event, action) => {
  const window = BrowserWindow.fromWebContents(_event.sender);
  if (!window) {
    return false;
  }
  if (action === "minimize") {
    window.minimize();
    return true;
  }
  if (action === "toggle-maximize") {
    if (window.isMaximized()) {
      window.unmaximize();
    } else {
      window.maximize();
    }
    return true;
  }
  if (action === "close") {
    window.close();
    return true;
  }
  return false;
});
