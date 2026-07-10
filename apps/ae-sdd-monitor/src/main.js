const path = require("path");
const nodeFs = require("fs");
const fs = require("fs/promises");
const { app, BrowserWindow, dialog, ipcMain, shell } = require("electron");
const { loadWorkspaceDetail, scanForWorkspaces } = require("./workspace");

let mainWindow = null;
let workspaceWatcher = null;
let watchedRootPath = "";
let watchDebounceTimer = null;

const WATCH_DEBOUNCE_MS = 350;

function rendererIndexPath() {
  return path.join(__dirname, "..", "dist", "renderer", "index.html");
}

function isInterestingWatchPath(filename) {
  if (!filename) {
    return true;
  }
  const normalized = String(filename).replaceAll("\\", "/");
  return (
    normalized === ".ae-sdd" ||
    normalized === ".auto-engineering" ||
    normalized.startsWith(".ae-sdd/") ||
    normalized.startsWith(".auto-engineering/") ||
    normalized.endsWith("/.ae-sdd") ||
    normalized.endsWith("/.auto-engineering") ||
    normalized.includes("/.ae-sdd/") ||
    normalized.includes("/.auto-engineering/")
  );
}

function closeWorkspaceWatcher() {
  if (watchDebounceTimer) {
    clearTimeout(watchDebounceTimer);
    watchDebounceTimer = null;
  }
  if (workspaceWatcher) {
    workspaceWatcher.close();
    workspaceWatcher = null;
  }
  watchedRootPath = "";
}

function scheduleWorkspaceChange(eventType, filename) {
  if (!isInterestingWatchPath(filename)) {
    return;
  }
  if (watchDebounceTimer) {
    clearTimeout(watchDebounceTimer);
  }
  watchDebounceTimer = setTimeout(() => {
    watchDebounceTimer = null;
    if (!mainWindow || mainWindow.isDestroyed()) {
      return;
    }
    mainWindow.webContents.send("workspace-files-changed", {
      rootPath: watchedRootPath,
      eventType: eventType || "",
      path: filename ? String(filename) : "",
      at: new Date().toISOString()
    });
  }, WATCH_DEBOUNCE_MS);
}

function startWorkspaceWatcher(rootPath) {
  const absoluteRoot = path.resolve(rootPath);
  if (workspaceWatcher && watchedRootPath === absoluteRoot) {
    return { rootPath: watchedRootPath, recursive: true };
  }

  closeWorkspaceWatcher();
  watchedRootPath = absoluteRoot;

  // Linux 原生不支持 fs.watch 的 recursive: true（仅 macOS/Windows 支持），
  // 且该失败通过异步 'error' 事件抛出（不进同步 try/catch），会导致 watcher
  // 静默关闭且无回退。Linux 直接走非递归监听，避免监听彻底失效（B5 修复）。
  const supportsRecursive = process.platform === "win32" || process.platform === "darwin";
  try {
    if (supportsRecursive) {
      workspaceWatcher = nodeFs.watch(absoluteRoot, { recursive: true }, scheduleWorkspaceChange);
    } else {
      throw new Error("recursive watch unsupported on this platform");
    }
    workspaceWatcher.on("error", closeWorkspaceWatcher);
    return { rootPath: watchedRootPath, recursive: supportsRecursive };
  } catch {
    workspaceWatcher = nodeFs.watch(absoluteRoot, scheduleWorkspaceChange);
    workspaceWatcher.on("error", closeWorkspaceWatcher);
    return { rootPath: watchedRootPath, recursive: false };
  }
}

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
      selectedTaskId: typeof parsed.selectedTaskId === "string" ? parsed.selectedTaskId : "",
      collapsedRoots: Array.isArray(parsed.collapsedRoots) ? parsed.collapsedRoots.filter((item) => typeof item === "string") : [],
      autoRefresh: typeof parsed.autoRefresh === "boolean" ? parsed.autoRefresh : true,
      theme: parsed.theme === "dark" ? "dark" : "light"
    };
  } catch {
    return { rootPath: "", selectedRoot: "", selectedTaskId: "", collapsedRoots: [], autoRefresh: true, theme: "light" };
  }
}

async function writePreferences(nextPreferences) {
  const current = await readPreferences();
  const preferences = {
    ...current,
    ...Object.fromEntries(
      Object.entries(nextPreferences || {}).filter(([, value]) => (
        typeof value === "string" ||
        typeof value === "boolean" ||
        (Array.isArray(value) && value.every((item) => typeof item === "string"))
      ))
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

  const rendererIndex = rendererIndexPath();
  if (!nodeFs.existsSync(rendererIndex)) {
    throw new Error(`Renderer build not found: ${rendererIndex}. Run npm run build:renderer first.`);
  }
  // loadFile 返回 Promise，加载失败（如 renderer html 损坏）时捕获并记录，
  // 避免进程级 unhandled rejection（C11 修复）。
  mainWindow.loadFile(rendererIndex).catch((err) => {
    console.error(`Failed to load renderer: ${rendererIndex}`, err);
  });
  mainWindow.on("closed", () => {
    mainWindow = null;
    closeWorkspaceWatcher();
  });
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
  closeWorkspaceWatcher();
  if (process.platform !== "darwin") {
    app.quit();
  }
});

ipcMain.handle("load-preferences", async () => readPreferences());

ipcMain.handle("save-preferences", async (_event, preferences) => writePreferences(preferences));

ipcMain.handle("watch-workspaces", async (_event, rootPath) => {
  if (!rootPath || typeof rootPath !== "string") {
    throw new Error("rootPath is required");
  }
  const absoluteRoot = path.resolve(rootPath);
  const stat = await fs.stat(absoluteRoot);
  if (!stat.isDirectory()) {
    throw new Error("rootPath must be a directory");
  }
  return startWorkspaceWatcher(absoluteRoot);
});

ipcMain.handle("unwatch-workspaces", async () => {
  closeWorkspaceWatcher();
  return true;
});

ipcMain.handle("choose-directory", async (_event, defaultPath) => {
  const options = {
    properties: ["openDirectory"]
  };
  if (defaultPath && typeof defaultPath === "string") {
    options.defaultPath = defaultPath;
  }
  // mainWindow 可能在窗口关闭瞬间为 null，传 null 会抛 TypeError；
  // 省略父窗口参数则用无模态对话框（C12 修复）。
  const result = await dialog.showOpenDialog(mainWindow || undefined, {
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
