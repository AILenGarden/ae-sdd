import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  MemoryScope,
  MemorySummary,
  MonitorApi,
  Preferences,
  RuntimeCommandStats,
  TabKey,
  TimelineEvent,
  WorkItem,
  WorkspaceDetail,
  WorkspaceSummary,
  WorkspaceWatchPayload
} from "./types";

const DETAIL_REFRESH_MS = 30000;
const FULL_SCAN_REFRESH_MS = 120000;
const REACTIVE_REFRESH_DELAY_MS = 180;

const STATUS_LABELS: Record<string, string> = {
  active: "活跃",
  idle: "空闲",
  paused: "暂停",
  completed: "完成",
  blocked: "阻断",
  invalid: "异常",
  unknown: "未知"
};

const MEMORY_STATUS_LABELS: Record<string, string> = {
  active: "Memory 活跃",
  ready: "Memory 就绪",
  empty: "Memory 空",
  missing: "Memory 缺失",
  blocked: "Memory 阻断",
  invalid: "Memory 异常",
  unknown: "Memory 未知"
};

const TABS: Array<[TabKey, string]> = [
  ["overview", "总览"],
  ["timeline", "时间线"],
  ["memory", "Memory"],
  ["workitems", "工作项"],
  ["performance", "性能"],
  ["raw", "原始状态"]
];

interface Model {
  rootPath: string;
  workspaces: WorkspaceSummary[];
  selectedRoot: string;
  selectedTaskId: string;
  collapsedRoots: string[];
  detail: WorkspaceDetail | null;
  filter: string;
  query: string;
  tab: TabKey;
  loading: boolean;
  autoRefresh: boolean;
  lastFullRefreshAt: number;
  workspaceSignature: string;
  detailSignature: string;
  watchedRootPath: string;
  statusMessage: string;
  detailMotion: "" | "tab";
  preferencesLoaded: boolean;
}

const initialModel: Model = {
  rootPath: "",
  workspaces: [],
  selectedRoot: "",
  selectedTaskId: "",
  collapsedRoots: [],
  detail: null,
  filter: "all",
  query: "",
  tab: "overview",
  loading: false,
  autoRefresh: true,
  lastFullRefreshAt: 0,
  workspaceSignature: "",
  detailSignature: "",
  watchedRootPath: "",
  statusMessage: "",
  detailMotion: "",
  preferencesLoaded: false
};

type Patch = Partial<Model> | ((previous: Model) => Model);

function signatureOf(value: unknown): string {
  return JSON.stringify(value ?? null);
}

function valueOrDash(value: unknown): string {
  return value === null || value === undefined || value === "" ? "-" : String(value);
}

function classToken(value: unknown, fallback = "unknown"): string {
  return String(value || fallback).replace(/[^a-zA-Z0-9_-]/g, "-") || fallback;
}

function statusLabel(status: unknown): string {
  const key = String(status || "unknown");
  return STATUS_LABELS[key] || key;
}

function memoryLabel(memory?: { status?: unknown } | null): string {
  const key = String(memory?.status || "unknown");
  return MEMORY_STATUS_LABELS[key] || key;
}

function workItemCaption(item?: WorkItem | WorkspaceSummary | null): string {
  return [item?.workItemId, item?.workItemName].filter(Boolean).join(" / ");
}

function workItemLabel(item?: WorkItem | WorkspaceSummary | null): string {
  return item?.workItemKey || item?.id || item?.activeWorkItem || "";
}

function taskLabel(task?: WorkItem | null): string {
  return task?.label || task?.workItemName || task?.workItemKey || task?.currentTask || task?.currentStory || task?.id || "";
}

function timeAgo(value?: string | null): string {
  if (!value) {
    return "-";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "-";
  }
  const seconds = Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000));
  if (seconds < 60) {
    return "刚刚";
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes} 分钟前`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours} 小时前`;
  }
  const days = Math.floor(hours / 24);
  if (days < 30) {
    return `${days} 天前`;
  }
  return date.toLocaleString("zh-CN");
}

function shortPath(value?: string | null): string {
  const text = valueOrDash(value);
  return text.length <= 64 ? text : `...${text.slice(-61)}`;
}

function projectCollapsed(collapsedRoots: string[], rootPath: string): boolean {
  return collapsedRoots.includes(rootPath);
}

function withProjectCollapsed(collapsedRoots: string[], rootPath: string, collapsed: boolean): string[] {
  const next = new Set(collapsedRoots);
  if (collapsed) {
    next.add(rootPath);
  } else {
    next.delete(rootPath);
  }
  return Array.from(next);
}

function selectedTask(detail: WorkspaceDetail | null, selectedTaskId: string): WorkItem | null {
  if (!detail || !selectedTaskId) {
    return null;
  }
  return (detail.tasks || []).find((task) => task.id === selectedTaskId) || null;
}

function workspaceCounts(workspaces: WorkspaceSummary[]): Record<string, number> {
  const counts: Record<string, number> = { all: workspaces.length };
  for (const workspace of workspaces) {
    const key = String(workspace.status || "unknown");
    counts[key] = (counts[key] || 0) + 1;
  }
  return counts;
}

function filterWorkspaces(workspaces: WorkspaceSummary[], filter: string, query: string): WorkspaceSummary[] {
  const normalizedQuery = query.trim().toLowerCase();
  return workspaces.filter((workspace) => {
    const statusMatches = filter === "all" || workspace.status === filter;
    if (!statusMatches) {
      return false;
    }
    if (!normalizedQuery) {
      return true;
    }
    return [
      workspace.name,
      workspace.projectKey,
      workspace.phase,
      workspace.rootPath,
      workspace.activeWorkItem,
      workspace.workItemId,
      workspace.workItemName,
      workspace.workItemKey,
      workspace.memoryStatus,
      ...(workspace.tasks || []).flatMap((task) => [
        task.id,
        task.label,
        task.workItemId,
        task.workItemName,
        task.workItemKey,
        task.currentStory,
        task.currentTask,
        task.memory?.status
      ])
    ]
      .filter(Boolean)
      .some((value) => String(value).toLowerCase().includes(normalizedQuery));
  });
}

function orderedEvents(detail: WorkspaceDetail, task: WorkItem | null): Array<{ title: string; time?: string; meta?: string }> {
  const source = task || detail;
  const items: Array<{ title: string; time?: string; meta?: string }> = [];
  for (const item of source.history || []) {
    items.push({
      title: item.phase || item.event || "history",
      time: item.timestamp || item.ts,
      meta: item.by ? `by ${item.by}` : "history"
    });
  }
  for (const item of source.events || []) {
    items.push({
      title: item.event || item.node || "event",
      time: item.ts || item.timestamp,
      meta: [item.node, item.skill, item.txnName, item.reason].filter(Boolean).join(" · ")
    });
  }
  items.sort((a, b) => new Date(a.time || 0).getTime() - new Date(b.time || 0).getTime());
  if (!items.length) {
    items.push({
      title: (task || detail.summary).phase || "current",
      time: detail.loadedAt,
      meta: "current"
    });
  }
  return items;
}

function bindPressFeedback(): () => void {
  const selector = [
    ".button",
    ".icon-button",
    ".filter",
    ".workspace-item",
    ".task-item",
    ".tab",
    ".context-card",
    ".collapse-button",
    ".traffic-button"
  ].join(",");

  const onPointerDown = (event: PointerEvent) => {
    const target = (event.target as Element | null)?.closest<HTMLElement>(selector);
    if (!target || target.hasAttribute("disabled")) {
      return;
    }

    target.classList.remove("pressing");
    void target.offsetWidth;
    target.classList.add("pressing");
    window.setTimeout(() => target.classList.remove("pressing"), 240);

    const rect = target.getBoundingClientRect();
    const size = Math.max(rect.width, rect.height) * 1.45;
    const ripple = document.createElement("span");
    ripple.className = "tap-highlight";
    ripple.style.width = `${size}px`;
    ripple.style.height = `${size}px`;
    ripple.style.left = `${event.clientX - rect.left}px`;
    ripple.style.top = `${event.clientY - rect.top}px`;
    target.appendChild(ripple);
    ripple.addEventListener("animationend", () => ripple.remove(), { once: true });
  };

  document.addEventListener("pointerdown", onPointerDown);
  return () => document.removeEventListener("pointerdown", onPointerDown);
}

export default function App() {
  const [model, setModel] = useState<Model>(initialModel);
  const modelRef = useRef(model);
  const initializedRef = useRef(false);
  const reactiveRefreshTimer = useRef<number | null>(null);
  const realtimeRefreshing = useRef(false);
  const reactiveRefreshing = useRef(false);
  const refreshRealtimeRef = useRef<() => Promise<boolean>>(async () => false);
  const scheduleReactiveRefreshRef = useRef<(payload?: WorkspaceWatchPayload) => void>(() => undefined);

  const patchModel = useCallback((patch: Patch) => {
    const previous = modelRef.current;
    const next = typeof patch === "function" ? patch(previous) : { ...previous, ...patch };
    modelRef.current = next;
    setModel(next);
  }, []);

  const setStatus = useCallback((statusMessage: string) => {
    patchModel({ statusMessage });
  }, [patchModel]);

  const setLoading = useCallback((loading: boolean) => {
    patchModel({ loading });
  }, [patchModel]);

  const savePreferences = useCallback(async (overrides: Partial<Model> = {}) => {
    const snapshot = { ...modelRef.current, ...overrides };
    await window.monitorApi.savePreferences({
      rootPath: snapshot.rootPath,
      selectedRoot: snapshot.selectedRoot,
      selectedTaskId: snapshot.selectedTaskId,
      collapsedRoots: snapshot.collapsedRoots,
      autoRefresh: snapshot.autoRefresh,
      theme: document.body.classList.contains("dark") ? "dark" : "light"
    });
  }, []);

  const stopReactiveWatch = useCallback(async () => {
    patchModel({ watchedRootPath: "" });
    if (window.monitorApi.unwatchWorkspaces) {
      await window.monitorApi.unwatchWorkspaces();
    }
  }, [patchModel]);

  const startReactiveWatch = useCallback(async (rootPath = modelRef.current.rootPath) => {
    const snapshot = modelRef.current;
    if (!snapshot.autoRefresh || !rootPath || !window.monitorApi.watchWorkspaces) {
      return;
    }
    if (snapshot.watchedRootPath === rootPath) {
      return;
    }
    try {
      const result = await window.monitorApi.watchWorkspaces(rootPath);
      patchModel({ watchedRootPath: result?.rootPath || rootPath });
    } catch {
      patchModel({ watchedRootPath: "" });
      setStatus("响应式监听不可用，使用兜底刷新");
    }
  }, [patchModel, setStatus]);

  const queueDetailMotion = useCallback((kind: "tab") => {
    patchModel({ detailMotion: kind });
    window.setTimeout(() => {
      if (modelRef.current.detailMotion === kind) {
        patchModel({ detailMotion: "" });
      }
    }, 520);
  }, [patchModel]);

  const selectWorkspace = useCallback(async (rootPath: string, options: { taskId?: string; silent?: boolean; reload?: boolean } = {}) => {
    if (!rootPath) {
      return false;
    }
    const snapshot = modelRef.current;
    const silent = Boolean(options.silent);
    const reload = Boolean(options.reload);
    const nextTaskId = options.taskId ?? (rootPath === snapshot.selectedRoot ? snapshot.selectedTaskId : "");
    const sameLoadedWorkspace = snapshot.detail?.summary?.rootPath === rootPath;

    if (sameLoadedWorkspace && !reload) {
      const nextSignature = signatureOf({
        rootPath,
        selectedTaskId: nextTaskId || "",
        detail: snapshot.detail
      });
      patchModel({
        selectedRoot: rootPath,
        selectedTaskId: nextTaskId || "",
        detailSignature: nextSignature
      });
      if (!silent) {
        await savePreferences({ selectedRoot: rootPath, selectedTaskId: nextTaskId || "" });
      }
      return true;
    }

    patchModel({
      selectedRoot: rootPath,
      selectedTaskId: nextTaskId || ""
    });
    if (!silent) {
      setStatus("加载项目...");
    }

    try {
      const nextDetail = await window.monitorApi.loadWorkspaceDetail(rootPath);
      const validTaskId = nextTaskId && (nextDetail.tasks || []).some((task) => task.id === nextTaskId) ? nextTaskId : "";
      const nextDetailSignature = signatureOf({
        rootPath,
        selectedTaskId: validTaskId,
        detail: nextDetail
      });
      const detailChanged = nextDetailSignature !== modelRef.current.detailSignature;
      patchModel({
        detail: nextDetail,
        selectedRoot: rootPath,
        selectedTaskId: validTaskId,
        detailSignature: nextDetailSignature
      });
      if (!silent) {
        await savePreferences({ selectedRoot: rootPath, selectedTaskId: validTaskId });
      }
      return detailChanged;
    } catch (error) {
      if (!silent) {
        setStatus(`加载失败：${error instanceof Error ? error.message : String(error)}`);
      }
      return false;
    }
  }, [patchModel, savePreferences, setStatus]);

  const scan = useCallback(async (rootPath = modelRef.current.rootPath, options: {
    silent?: boolean;
    preferredSelectedRoot?: string;
    preferredSelectedTaskId?: string;
  } = {}) => {
    if (!rootPath) {
      return false;
    }
    const silent = Boolean(options.silent);
    if (!silent) {
      setLoading(true);
      setStatus("扫描中...");
    }

    try {
      const snapshot = modelRef.current;
      const result = await window.monitorApi.scanWorkspaces(rootPath);
      const previousSelected = options.preferredSelectedRoot || snapshot.selectedRoot;
      const previousTask = options.preferredSelectedTaskId ?? snapshot.selectedTaskId;
      const nextWorkspaceSignature = signatureOf(result.workspaces);
      const workspaceChanged = nextWorkspaceSignature !== snapshot.workspaceSignature;
      const roots = new Set(result.workspaces.map((workspace) => workspace.rootPath));
      const selectedRoot = result.workspaces.some((workspace) => workspace.rootPath === previousSelected)
        ? previousSelected
        : result.workspaces[0]?.rootPath || "";
      const collapsedRoots = snapshot.collapsedRoots.filter((root) => roots.has(root));

      patchModel({
        rootPath: result.rootPath,
        workspaces: result.workspaces,
        workspaceSignature: nextWorkspaceSignature,
        collapsedRoots,
        selectedRoot,
        lastFullRefreshAt: Date.now()
      });

      if (!silent) {
        await savePreferences({
          rootPath: result.rootPath,
          selectedRoot,
          selectedTaskId: previousTask,
          collapsedRoots
        });
        await startReactiveWatch(result.rootPath);
      }

      let detailChanged = false;
      if (selectedRoot) {
        detailChanged = await selectWorkspace(selectedRoot, { taskId: previousTask, silent, reload: true });
      } else {
        patchModel({
          selectedTaskId: "",
          detail: null,
          detailSignature: ""
        });
      }

      if (!silent || workspaceChanged || detailChanged) {
        setStatus(`${silent ? "响应式更新" : "扫描完成"} · ${result.workspaces.length} 个工作区`);
      }
      return workspaceChanged || detailChanged;
    } catch (error) {
      setStatus(silent ? "响应式更新失败" : `扫描失败：${error instanceof Error ? error.message : String(error)}`);
      return false;
    } finally {
      if (!silent) {
        setLoading(false);
      }
    }
  }, [patchModel, savePreferences, selectWorkspace, setLoading, setStatus, startReactiveWatch]);

  const selectTask = useCallback(async (rootPath: string, taskId: string) => {
    if (!rootPath || !taskId) {
      return;
    }
    const snapshot = modelRef.current;
    const collapsedRoots = withProjectCollapsed(snapshot.collapsedRoots, rootPath, false);
    patchModel({ collapsedRoots });
    if (rootPath !== snapshot.selectedRoot || !snapshot.detail) {
      await selectWorkspace(rootPath, { taskId, reload: true });
      await savePreferences({ collapsedRoots, selectedRoot: rootPath, selectedTaskId: taskId });
      return;
    }
    patchModel({
      selectedTaskId: taskId,
      detailSignature: signatureOf({
        rootPath,
        selectedTaskId: taskId,
        detail: snapshot.detail
      })
    });
    await savePreferences({ collapsedRoots, selectedRoot: rootPath, selectedTaskId: taskId });
  }, [patchModel, savePreferences, selectWorkspace]);

  const toggleProjectCollapsed = useCallback(async (rootPath: string) => {
    if (!rootPath) {
      return;
    }
    const snapshot = modelRef.current;
    const collapsedRoots = withProjectCollapsed(snapshot.collapsedRoots, rootPath, !projectCollapsed(snapshot.collapsedRoots, rootPath));
    patchModel({ collapsedRoots });
    await savePreferences({ collapsedRoots });
  }, [patchModel, savePreferences]);

  const refreshReactive = useCallback(async () => {
    const snapshot = modelRef.current;
    if (!snapshot.autoRefresh || snapshot.loading || reactiveRefreshing.current || !snapshot.rootPath) {
      return false;
    }
    reactiveRefreshing.current = true;
    try {
      return await scan(snapshot.rootPath, {
        silent: true,
        preferredSelectedRoot: snapshot.selectedRoot,
        preferredSelectedTaskId: snapshot.selectedTaskId
      });
    } finally {
      reactiveRefreshing.current = false;
    }
  }, [scan]);

  const scheduleReactiveRefresh = useCallback((payload: WorkspaceWatchPayload = {}) => {
    const snapshot = modelRef.current;
    if (!snapshot.autoRefresh || !snapshot.rootPath) {
      return;
    }
    if (payload.rootPath && payload.rootPath !== snapshot.rootPath) {
      return;
    }
    if (reactiveRefreshTimer.current) {
      window.clearTimeout(reactiveRefreshTimer.current);
    }
    reactiveRefreshTimer.current = window.setTimeout(() => {
      reactiveRefreshTimer.current = null;
      void refreshReactive();
    }, REACTIVE_REFRESH_DELAY_MS);
  }, [refreshReactive]);

  const refreshRealtime = useCallback(async () => {
    const snapshot = modelRef.current;
    if (!snapshot.autoRefresh || snapshot.loading || realtimeRefreshing.current || !snapshot.rootPath) {
      return false;
    }
    realtimeRefreshing.current = true;
    try {
      const shouldFullScan = !snapshot.lastFullRefreshAt || Date.now() - snapshot.lastFullRefreshAt >= FULL_SCAN_REFRESH_MS;
      if (shouldFullScan) {
        return await scan(snapshot.rootPath, {
          silent: true,
          preferredSelectedRoot: snapshot.selectedRoot,
          preferredSelectedTaskId: snapshot.selectedTaskId
        });
      }
      if (snapshot.selectedRoot) {
        const changed = await selectWorkspace(snapshot.selectedRoot, {
          taskId: snapshot.selectedTaskId,
          silent: true,
          reload: true
        });
        if (changed) {
          setStatus(`兜底刷新 · ${new Date().toLocaleTimeString("zh-CN")}`);
        }
        return changed;
      }
      return false;
    } finally {
      realtimeRefreshing.current = false;
    }
  }, [scan, selectWorkspace, setStatus]);

  const chooseDirectory = useCallback(async () => {
    setStatus("打开目录选择器...");
    setLoading(true);
    try {
      const directory = await window.monitorApi.chooseDirectory(modelRef.current.rootPath);
      if (directory) {
        await scan(directory, { preferredSelectedRoot: "" });
      } else {
        setStatus("已取消");
      }
    } catch (error) {
      setStatus(`选择失败：${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setLoading(false);
    }
  }, [scan, setLoading, setStatus]);

  const toggleTheme = useCallback(async () => {
    document.body.classList.add("theme-switching");
    document.body.classList.toggle("dark");
    await savePreferences();
    window.setTimeout(() => document.body.classList.remove("theme-switching"), 420);
  }, [savePreferences]);

  const toggleAutoRefresh = useCallback(async () => {
    const nextAutoRefresh = !modelRef.current.autoRefresh;
    patchModel({ autoRefresh: nextAutoRefresh });
    await savePreferences({ autoRefresh: nextAutoRefresh });
    if (nextAutoRefresh) {
      setStatus("响应式刷新已开启");
      await startReactiveWatch(modelRef.current.rootPath);
      await refreshReactive();
    } else {
      setStatus("响应式刷新已关闭");
      await stopReactiveWatch();
    }
  }, [patchModel, refreshReactive, savePreferences, setStatus, startReactiveWatch, stopReactiveWatch]);

  const setFilter = useCallback((filter: string) => {
    patchModel({ filter });
  }, [patchModel]);

  const setQuery = useCallback((query: string) => {
    patchModel({ query });
  }, [patchModel]);

  const setTab = useCallback((tab: TabKey) => {
    patchModel({ tab });
    queueDetailMotion("tab");
  }, [patchModel, queueDetailMotion]);

  const showProjectContext = useCallback(() => {
    const detail = modelRef.current.detail;
    if (detail) {
      void selectWorkspace(detail.summary.rootPath, { taskId: "" });
    }
  }, [selectWorkspace]);

  useEffect(() => {
    modelRef.current = model;
  }, [model]);

  useEffect(() => {
    refreshRealtimeRef.current = refreshRealtime;
  }, [refreshRealtime]);

  useEffect(() => {
    scheduleReactiveRefreshRef.current = scheduleReactiveRefresh;
  }, [scheduleReactiveRefresh]);

  useEffect(() => bindPressFeedback(), []);

  useEffect(() => {
    const cleanup = window.monitorApi.onWorkspaceFilesChanged?.((payload) => {
      scheduleReactiveRefreshRef.current(payload);
    });
    return cleanup || undefined;
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      void refreshRealtimeRef.current();
    }, DETAIL_REFRESH_MS);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (initializedRef.current) {
      return;
    }
    initializedRef.current = true;
    const initialize = async () => {
      try {
        const preferences = await window.monitorApi.loadPreferences();
        if (preferences.theme === "dark") {
          document.body.classList.add("dark");
        }
        patchModel({
          preferencesLoaded: true,
          autoRefresh: preferences.autoRefresh !== false,
          selectedTaskId: preferences.selectedTaskId || "",
          collapsedRoots: Array.isArray(preferences.collapsedRoots) ? preferences.collapsedRoots : []
        });
        if (preferences.rootPath) {
          patchModel({
            rootPath: preferences.rootPath,
            selectedRoot: preferences.selectedRoot || ""
          });
          await scan(preferences.rootPath, {
            preferredSelectedRoot: preferences.selectedRoot || "",
            preferredSelectedTaskId: preferences.selectedTaskId || ""
          });
          await startReactiveWatch(preferences.rootPath);
        }
      } catch {
        setStatus("配置读取失败");
      }
    };
    void initialize();
  }, [patchModel, scan, setStatus, startReactiveWatch]);

  const counts = useMemo(() => workspaceCounts(model.workspaces), [model.workspaces]);
  const visibleWorkspaces = useMemo(
    () => filterWorkspaces(model.workspaces, model.filter, model.query),
    [model.filter, model.query, model.workspaces]
  );
  const currentTask = selectedTask(model.detail, model.selectedTaskId);

  return (
    <div className="app">
      <Toolbar
        rootPath={model.rootPath}
        loading={model.loading}
        statusMessage={model.statusMessage}
        autoRefresh={model.autoRefresh}
        onChooseDirectory={chooseDirectory}
        onRefresh={() => void scan()}
        onToggleTheme={toggleTheme}
        onToggleAutoRefresh={toggleAutoRefresh}
      />
      <main className="layout">
        <Sidebar
          workspaces={visibleWorkspaces}
          counts={counts}
          filter={model.filter}
          query={model.query}
          selectedRoot={model.selectedRoot}
          selectedTaskId={model.selectedTaskId}
          collapsedRoots={model.collapsedRoots}
          detail={model.detail}
          onFilter={setFilter}
          onQuery={setQuery}
          onSelectWorkspace={(rootPath) => void selectWorkspace(rootPath, { taskId: "" })}
          onSelectTask={(rootPath, taskId) => void selectTask(rootPath, taskId)}
          onToggleProject={toggleProjectCollapsed}
        />
        <DetailPane
          rootPath={model.rootPath}
          detail={model.detail}
          selectedTask={currentTask}
          tab={model.tab}
          motion={model.detailMotion}
          onTab={setTab}
          onOpenWorkspace={(rootPath) => void window.monitorApi.openPath(rootPath)}
          onProjectContext={showProjectContext}
        />
      </main>
    </div>
  );
}

function Toolbar({
  rootPath,
  loading,
  statusMessage,
  autoRefresh,
  onChooseDirectory,
  onRefresh,
  onToggleTheme,
  onToggleAutoRefresh
}: {
  rootPath: string;
  loading: boolean;
  statusMessage: string;
  autoRefresh: boolean;
  onChooseDirectory: () => void;
  onRefresh: () => void;
  onToggleTheme: () => void;
  onToggleAutoRefresh: () => void;
}) {
  const scanStatusClass = [
    "scan-status",
    loading ? "busy" : "",
    !loading && autoRefresh && /响应|实时/.test(statusMessage) ? "live" : ""
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <header className="toolbar">
      <div className="traffic" aria-label="窗口控制">
        <button type="button" className="traffic-button close" title="关闭" aria-label="关闭" onClick={() => void window.monitorApi.windowControl("close")} />
        <button type="button" className="traffic-button minimize" title="最小化" aria-label="最小化" onClick={() => void window.monitorApi.windowControl("minimize")} />
        <button type="button" className="traffic-button maximize" title="最大化" aria-label="最大化" onClick={() => void window.monitorApi.windowControl("toggle-maximize")} />
      </div>
      <div className="brand">
        <strong>ae-sdd Monitor</strong>
        <span>{rootPath || "未选择目录"}</span>
      </div>
      <div className="toolbar-actions">
        <span className={scanStatusClass}>{statusMessage}</span>
        <button type="button" className="icon-button" title="切换明暗" onClick={onToggleTheme}>Aa</button>
        <button
          type="button"
          className={`button toggle${autoRefresh ? " active" : ""}`}
          title={autoRefresh ? "响应式刷新已开启" : "响应式刷新已关闭"}
          onClick={onToggleAutoRefresh}
        >
          {autoRefresh ? "响应" : "手动"}
        </button>
        <button type="button" className="button primary" disabled={loading} onClick={onChooseDirectory}>
          {loading ? "扫描中" : "选择目录"}
        </button>
        <button type="button" className="button" disabled={loading || !rootPath} onClick={onRefresh}>刷新</button>
      </div>
    </header>
  );
}

function Sidebar({
  workspaces,
  counts,
  filter,
  query,
  selectedRoot,
  selectedTaskId,
  collapsedRoots,
  detail,
  onFilter,
  onQuery,
  onSelectWorkspace,
  onSelectTask,
  onToggleProject
}: {
  workspaces: WorkspaceSummary[];
  counts: Record<string, number>;
  filter: string;
  query: string;
  selectedRoot: string;
  selectedTaskId: string;
  collapsedRoots: string[];
  detail: WorkspaceDetail | null;
  onFilter: (filter: string) => void;
  onQuery: (query: string) => void;
  onSelectWorkspace: (rootPath: string) => void;
  onSelectTask: (rootPath: string, taskId: string) => void;
  onToggleProject: (rootPath: string) => void;
}) {
  const filterItems = ["all", "active", "blocked", "paused", "idle", "completed", "invalid"]
    .filter((key) => key === "all" || counts[key])
    .map((key) => ({
      key,
      label: key === "all" ? "全部" : statusLabel(key),
      count: counts[key] || 0
    }));

  return (
    <aside className="sidebar">
      <div className="search-row">
        <input type="search" placeholder="搜索" autoComplete="off" value={query} onChange={(event) => onQuery(event.currentTarget.value)} />
      </div>
      <div className="filters">
        {filterItems.map((item) => (
          <button
            key={item.key}
            type="button"
            className={`filter${filter === item.key ? " active" : ""}`}
            onClick={() => onFilter(item.key)}
          >
            {item.label} {item.count}
          </button>
        ))}
      </div>
      <div className="workspace-list">
        {workspaces.length ? (
          workspaces.map((workspace) => {
            const tasks = detail?.summary.rootPath === workspace.rootPath ? detail.tasks || [] : workspace.tasks || [];
            return (
              <ProjectGroup
                key={workspace.rootPath}
                workspace={workspace}
                tasks={tasks}
                selectedRoot={selectedRoot}
                selectedTaskId={selectedTaskId}
                collapsed={projectCollapsed(collapsedRoots, workspace.rootPath)}
                onSelectWorkspace={onSelectWorkspace}
                onSelectTask={onSelectTask}
                onToggleProject={onToggleProject}
              />
            );
          })
        ) : (
          <div className="empty-row">没有匹配的工作区</div>
        )}
      </div>
    </aside>
  );
}

const ProjectGroup = memo(function ProjectGroup({
  workspace,
  tasks,
  selectedRoot,
  selectedTaskId,
  collapsed,
  onSelectWorkspace,
  onSelectTask,
  onToggleProject
}: {
  workspace: WorkspaceSummary;
  tasks: WorkItem[];
  selectedRoot: string;
  selectedTaskId: string;
  collapsed: boolean;
  onSelectWorkspace: (rootPath: string) => void;
  onSelectTask: (rootPath: string, taskId: string) => void;
  onToggleProject: (rootPath: string) => void;
}) {
  const projectSelected = workspace.rootPath === selectedRoot && !selectedTaskId;
  const projectCurrent = workspace.rootPath === selectedRoot;

  return (
    <div className={`project-group${projectCurrent ? " current-project" : ""}`} data-root={workspace.rootPath}>
      <div className="project-row">
        <button
          type="button"
          className={`collapse-button${collapsed ? " collapsed" : ""}${!tasks.length ? " empty" : ""}`}
          disabled={!tasks.length}
          title={collapsed ? "展开任务" : "收起任务"}
          aria-label={collapsed ? "展开任务" : "收起任务"}
          onClick={() => onToggleProject(workspace.rootPath)}
        >
          {tasks.length ? "›" : ""}
        </button>
        <button
          type="button"
          className={`workspace-item status-${classToken(workspace.status)}${projectSelected ? " selected" : ""}`}
          onClick={() => onSelectWorkspace(workspace.rootPath)}
        >
          <span className="status-dot" />
          <span className="workspace-main">
            <span className="workspace-title">
              <strong>{workspace.name}</strong>
              <span>{statusLabel(workspace.status)}</span>
            </span>
            <span className="muted-line">{workspace.phase} · {timeAgo(workspace.lastActivityAt)}</span>
            <span className="muted-line">{memoryLabel(workspace.memory)} · {tasks.length} 任务</span>
            <span className="path-line">{shortPath(workspace.rootPath)}</span>
          </span>
        </button>
      </div>
      <div className={`task-tree-shell${collapsed || !tasks.length ? " collapsed" : ""}`} hidden={!tasks.length}>
        <div className="task-tree">
          {tasks.map((task) => (
            <TaskItem
              key={task.id || taskLabel(task)}
              rootPath={workspace.rootPath}
              task={task}
              selected={workspace.rootPath === selectedRoot && task.id === selectedTaskId}
              onSelectTask={onSelectTask}
            />
          ))}
        </div>
      </div>
    </div>
  );
});

const TaskItem = memo(function TaskItem({
  rootPath,
  task,
  selected,
  onSelectTask
}: {
  rootPath: string;
  task: WorkItem;
  selected: boolean;
  onSelectTask: (rootPath: string, taskId: string) => void;
}) {
  return (
    <button
      type="button"
      className={`task-item status-${classToken(task.status)}${selected ? " selected" : ""}`}
      onClick={() => task.id && onSelectTask(rootPath, task.id)}
    >
      <span className="tree-line" />
      <span className="status-dot" />
      <span className="task-main">
        <span className="task-title">
          <strong>{taskLabel(task)}</strong>
          <span>{statusLabel(task.status)}</span>
        </span>
        <span className="muted-line">{[task.phase, memoryLabel(task.memory)].filter(Boolean).join(" · ")}</span>
      </span>
    </button>
  );
});

function DetailPane({
  rootPath,
  detail,
  selectedTask,
  tab,
  motion,
  onTab,
  onOpenWorkspace,
  onProjectContext
}: {
  rootPath: string;
  detail: WorkspaceDetail | null;
  selectedTask: WorkItem | null;
  tab: TabKey;
  motion: "" | "tab";
  onTab: (tab: TabKey) => void;
  onOpenWorkspace: (rootPath: string) => void;
  onProjectContext: () => void;
}) {
  if (!detail) {
    return (
      <section className="detail">
        <div className="empty-state">
          <div className="empty-mark">.ae-sdd</div>
          <h1>{rootPath ? "选择一个工作区" : "选择目录开始扫描"}</h1>
          <p>{rootPath ? "左侧列表中没有选中项。" : "左侧会列出找到的 ae-sdd 工作区。"}</p>
        </div>
      </section>
    );
  }

  const summary = detail.summary;
  return (
    <section className={`detail${motion ? ` motion-${motion}` : ""}`}>
      <div className="workspace-head">
        <div>
          <h1>{summary.name}</h1>
          <div className="workspace-path">{selectedTask ? `${summary.rootPath} · ${taskLabel(selectedTask)}` : summary.rootPath}</div>
        </div>
        <div className="head-actions">
          <button type="button" className="button" onClick={() => onOpenWorkspace(summary.rootPath)}>打开目录</button>
        </div>
      </div>
      <ContextBoard summary={summary} detail={detail} task={selectedTask} onProjectContext={onProjectContext} />
      <MetricsGrid summary={summary} detail={detail} task={selectedTask} />
      <div className="tabs">
        {TABS.map(([key, label]) => (
          <button key={key} type="button" className={`tab${tab === key ? " active" : ""}`} onClick={() => onTab(key)}>
            {label}
          </button>
        ))}
      </div>
      <div id="tabContent">
        <TabContent tab={tab} detail={detail} task={selectedTask} />
      </div>
    </section>
  );
}

function ContextBoard({
  summary,
  detail,
  task,
  onProjectContext
}: {
  summary: WorkspaceSummary;
  detail: WorkspaceDetail;
  task: WorkItem | null;
  onProjectContext: () => void;
}) {
  const projectMemory = detail.memory || summary.memory;
  return (
    <div className="context-board">
      <button type="button" className={`context-card project-card${task ? "" : " selected"}`} onClick={onProjectContext}>
        <span className="context-kicker">当前项目</span>
        <strong>{summary.name}</strong>
        <span>{[summary.phase, statusLabel(summary.status)].filter(Boolean).join(" · ")}</span>
        <span>{memoryLabel(projectMemory)} · {detail.tasks?.length || 0} 任务</span>
      </button>
      <div className={`context-card task-card${task ? " selected" : ""}`}>
        <span className="context-kicker">当前任务</span>
        {task ? (
          <>
            <strong>{taskLabel(task)}</strong>
            <span>{[task.phase, statusLabel(task.status)].filter(Boolean).join(" · ")}</span>
            <span>{memoryLabel(task.memory)} · {task.memory?.memoryEntries || 0} 条记忆</span>
          </>
        ) : (
          <>
            <strong>未选择任务</strong>
            <span>右侧显示项目级看板</span>
            <span>{detail.activeWorkItems?.length || 0} 个活跃任务</span>
          </>
        )}
      </div>
    </div>
  );
}

function MetricsGrid({ summary, detail, task }: { summary: WorkspaceSummary; detail: WorkspaceDetail; task: WorkItem | null }) {
  const progress = task?.progress || summary.progress || {};
  const fill = Math.max(0, Math.min(100, Number(progress.percent || 0)));
  const memory = task?.memory || detail.memory || summary.memory;
  const activeWorkItem = task ? taskLabel(task) : workItemCaption(summary) || workItemLabel(summary) || summary.currentStory || summary.currentTask;
  const status = task?.status || summary.status;
  const phase = task?.phase || summary.phase;
  const recentAt = task ? task.lastActivityAt || task.memory?.lastMemoryAt : summary.lastActivityAt;

  return (
    <div className="summary-grid">
      <div className="metric">
        <div className="metric-label">状态</div>
        <div className="metric-value">{statusLabel(status)}</div>
      </div>
      <div className="metric">
        <div className="metric-label">阶段</div>
        <div className="metric-value">{phase}</div>
        <div className="progress-track"><div className="progress-fill" style={{ width: `${fill}%` }} /></div>
      </div>
      <div className="metric">
        <div className="metric-label">Memory</div>
        <div className="metric-value">{memoryLabel(memory)}</div>
        <div className="muted-line">{memory?.memoryEntries || 0} 条 · {memory?.activeScopeCount || 0} 活跃 scope</div>
      </div>
      <div className="metric">
        <div className="metric-label">最近活动</div>
        <div className="metric-value">{timeAgo(recentAt)}</div>
        <div className="muted-line">{valueOrDash(activeWorkItem)}</div>
      </div>
    </div>
  );
}

function TabContent({ tab, detail, task }: { tab: TabKey; detail: WorkspaceDetail; task: WorkItem | null }) {
  switch (tab) {
    case "timeline":
      return <TimelineTab detail={detail} task={task} />;
    case "memory":
      return <MemoryTab detail={detail} task={task} />;
    case "workitems":
      return <WorkItemsTab detail={detail} />;
    case "performance":
      return <PerformanceTab detail={detail} />;
    case "raw":
      return <pre className="code">{JSON.stringify(task?.state || detail.state, null, 2)}</pre>;
    case "overview":
    default:
      return <OverviewTab detail={detail} task={task} />;
  }
}

function OverviewTab({ detail, task }: { detail: WorkspaceDetail; task: WorkItem | null }) {
  const summary = detail.summary;
  const errors = summary.errors || [];
  return (
    <>
      <Panel
        title="状态摘要"
        suffix={<span className={errors.length ? "error" : ""}>{errors.length ? `${errors.length} 个异常` : "OK"}</span>}
      >
        <KvGrid
          items={[
            ["projectKey", summary.projectKey],
            ["phase", summary.phase],
            ["scale", summary.scale],
            ["entryNode", summary.entryNode],
            ["currentStory", summary.currentStory],
            ["currentTask", summary.currentTask],
            ["activeWorkItem", summary.activeWorkItem],
            ["activeStatePath", summary.activeStatePath],
            ["workItemId", summary.workItemId],
            ["workItemName", summary.workItemName],
            ["workItemKey", summary.workItemKey],
            ["activeAgents", summary.activeAgentCount],
            ["taskCount", detail.tasks?.length || 0],
            ["memory", memoryLabel(detail.memory || summary.memory)],
            ["memoryLastAt", detail.memory?.lastMemoryAt || summary.memory?.lastMemoryAt],
            ["statePath", summary.statePath],
            ["configPath", summary.configPath],
            ["lastActivityAt", summary.lastActivityAt]
          ]}
        />
        {errors.length ? <div className="error" style={{ marginTop: 14 }}>{errors.join("\n")}</div> : null}
      </Panel>
      {task ? (
        <Panel title="任务摘要" suffix={<span>{statusLabel(task.status)}</span>} className="active-panel">
          <KvGrid
            items={[
              ["taskId", task.id],
              ["workItemId", task.workItemId],
              ["workItemName", task.workItemName],
              ["workItemKey", task.workItemKey],
              ["phase", task.phase],
              ["currentStory", task.currentStory],
              ["currentTask", task.currentTask],
              ["statePath", task.statePath],
              ["memory", memoryLabel(task.memory)],
              ["lastActivityAt", task.lastActivityAt]
            ]}
          />
        </Panel>
      ) : null}
      {!task && detail.activeWorkItems?.length ? <ActiveWorkItems items={detail.activeWorkItems} context="overview" /> : null}
    </>
  );
}

function TimelineTab({ detail, task }: { detail: WorkspaceDetail; task: WorkItem | null }) {
  const source = task || detail.summary;
  const events = orderedEvents(detail, task);
  const timeline = task?.phaseTimeline || detail.phaseTimeline;

  return (
    <>
      <Panel title="阶段轴" suffix={<span>{timeline?.scale || valueOrDash(source.scale)} · {source.phase}</span>}>
        <div className="phase-axis">
          {(timeline?.nodes || []).map((node) => (
            <div key={`${node.index}-${node.phase}`} className={`phase-node ${classToken(node.status)}`}>
              <div className="phase-index">{valueOrDash(node.index)}</div>
              <div className="phase-copy">
                <div className="phase-title">{node.label} <span>{node.phase}</span></div>
                <div className="phase-desc">{node.description}</div>
              </div>
            </div>
          ))}
        </div>
      </Panel>
      <Panel title="事件流" suffix={<span>{events.length} 条</span>} className="event-panel">
        <div className="event-timeline">
          {events.map((item, index) => (
            <div key={`${item.time || index}-${item.title}`} className={`event-item${index === events.length - 1 ? " current" : ""}`}>
              <span className="event-point" />
              <div>
                <div className="event-title">{item.title}</div>
                <div className="event-meta">{valueOrDash(item.time)}{item.meta ? ` · ${item.meta}` : ""}</div>
              </div>
            </div>
          ))}
        </div>
      </Panel>
    </>
  );
}

function MemoryTab({ detail, task }: { detail: WorkspaceDetail; task: WorkItem | null }) {
  const memory = task?.memory || detail.memory || {};
  const title = task ? `任务 Memory · ${taskLabel(task)}` : "项目 Memory";
  return (
    <>
      <Panel title={title} suffix={<span>{memoryLabel(memory)}</span>}>
        <KvGrid
          items={[
            ["status", memoryLabel(memory)],
            ["root", memory.root],
            ["total", memory.total],
            ["memoryEntries", memory.memoryEntries],
            ["projectMemory", memory.projectMemoryCount],
            ["taskMemory", memory.taskMemoryCount],
            ["activeScopes", memory.activeScopeCount],
            ["blockedScopes", memory.blockedScopeCount],
            ["lastMemoryAt", memory.lastMemoryAt]
          ]}
        />
      </Panel>
      <MemoryScopes memory={memory} />
      <MemoryRecent memory={memory} />
    </>
  );
}

function MemoryScopes({ memory }: { memory: { blockedScopes?: MemoryScope[]; activeScopes?: MemoryScope[] } }) {
  const scopes = [...(memory.blockedScopes || []), ...(memory.activeScopes || [])];
  if (!scopes.length) {
    return <Panel className="active-panel"><div className="empty-row">没有活跃或阻断的 memory scope</div></Panel>;
  }
  return (
    <Panel title="Memory Scope" suffix={<span>{scopes.length} 个</span>} className="active-panel" padded={false}>
      <table className="table">
        <thead>
          <tr>
            <th>scope</th>
            <th>状态</th>
            <th>phase</th>
            <th>story/task</th>
            <th>最近活动</th>
          </tr>
        </thead>
        <tbody>
          {scopes.map((scope, index) => (
            <tr key={scope.id || index}>
              <td>{scope.id}</td>
              <td>{scope.needsWrite ? "需写入" : "活跃"}</td>
              <td>{valueOrDash(scope.phase)}</td>
              <td>{[scope.story, scope.task].filter(Boolean).join(" / ") || "-"}</td>
              <td>{timeAgo(scope.lastActivityAt)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Panel>
  );
}

function MemoryRecent({ memory }: { memory: MemorySummary }) {
  const recent = memory.recent || [];
  if (!recent.length) {
    return <Panel className="active-panel"><div className="empty-row">未发现 memory 记录</div></Panel>;
  }
  return (
    <Panel title="最近 Memory" suffix={<span>{recent.length} 条</span>} className="active-panel" padded={false}>
      <table className="table">
        <thead>
          <tr>
            <th>时间</th>
            <th>层级</th>
            <th>类型</th>
            <th>摘要</th>
            <th>证据</th>
          </tr>
        </thead>
        <tbody>
          {recent.map((item, index) => (
            <tr key={`${String(item.timestamp || "")}-${index}`}>
              <td>{timeAgo(item.timestamp as string | undefined)}</td>
              <td>{valueOrDash(item.memoryScope || item.layer || item.type)}</td>
              <td>{valueOrDash(item.kind || item.type)}</td>
              <td>{valueOrDash(item.summary || item.note || item.reason)}</td>
              <td>{Array.isArray(item.evidence) ? item.evidence.join(" / ") : shortPath(item.path as string | undefined)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Panel>
  );
}

function WorkItemsTab({ detail }: { detail: WorkspaceDetail }) {
  const workItems = detail.workItems || [];
  return (
    <>
      <ActiveWorkItems items={detail.activeWorkItems || []} context="workitems" />
      {workItems.length ? (
        <Panel title="全部工作项" suffix={<span>{workItems.length} 个</span>} padded={false}>
          <table className="table">
            <thead>
              <tr>
                <th>工作项</th>
                <th>状态</th>
                <th>阶段</th>
                <th>进度</th>
                <th>Agent</th>
                <th>最近活动</th>
              </tr>
            </thead>
            <tbody>
              {workItems.map((item, index) => {
                const caption = workItemCaption(item);
                return (
                  <tr key={item.id || index}>
                    <td>
                      <div>{workItemLabel(item) || item.id}</div>
                      {caption ? <div className="muted-line">{caption}</div> : null}
                    </td>
                    <td>{statusLabel(item.status)}</td>
                    <td>{item.phase}</td>
                    <td>{`${item.progress?.index && item.progress.index > 0 ? item.progress.index : "-"} / ${item.progress?.total || "-"}`}</td>
                    <td>{item.activeAgentCount || 0}</td>
                    <td>{timeAgo(item.lastActivityAt)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </Panel>
      ) : (
        <Panel><div className="empty-row">未发现工作项 state</div></Panel>
      )}
    </>
  );
}

function PerformanceTab({ detail }: { detail: WorkspaceDetail }) {
  const stats = detail.runtimeStats || {};
  if (!stats.count) {
    return <Panel><div className="empty-row">未发现 runtime-stats</div></Panel>;
  }
  return (
    <Panel title="运行统计" suffix={<span>{stats.count} 条 · 失败 {stats.failures || 0}</span>} padded={false}>
      <table className="table">
        <thead>
          <tr>
            <th>命令</th>
            <th>次数</th>
            <th>失败</th>
            <th>平均耗时</th>
            <th>最大耗时</th>
            <th>最近运行</th>
          </tr>
        </thead>
        <tbody>
          {(stats.commands || []).map((command: RuntimeCommandStats, index) => (
            <tr key={command.command || index}>
              <td>{command.command}</td>
              <td>{command.count}</td>
              <td>{command.failures}</td>
              <td>{command.avgMs} ms</td>
              <td>{command.maxMs} ms</td>
              <td>{timeAgo(command.lastStartedAt)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Panel>
  );
}

function ActiveWorkItems({ items, context }: { items: WorkItem[]; context: "overview" | "workitems" }) {
  if (!items.length) {
    return context === "workitems" ? <Panel><div className="empty-row">当前没有活跃任务</div></Panel> : null;
  }
  return (
    <Panel title="活跃任务" suffix={<span>{items.length} 个</span>} className="active-panel" padded={false}>
      <div className="active-list">
        {items.map((item, index) => {
          const caption = workItemCaption(item);
          return (
            <div key={item.id || index} className={`active-item status-${classToken(item.status)}`}>
              <span className="status-dot" />
              <div className="active-main">
                <div className="active-title">
                  <strong>{workItemLabel(item) || item.id}</strong>
                  <span>{statusLabel(item.status || "active")}</span>
                </div>
                {caption ? <div className="muted-line">{caption}</div> : null}
                <div className="muted-line">{[item.phase, item.source, item.skill, item.role, item.agentId].filter(Boolean).join(" · ") || "-"}</div>
                {item.summary ? <div className="path-line">{item.summary}</div> : null}
              </div>
              <div className="active-time">{timeAgo(item.lastActivityAt)}</div>
            </div>
          );
        })}
      </div>
    </Panel>
  );
}

function Panel({
  title,
  suffix,
  className = "",
  padded = true,
  children
}: {
  title?: string;
  suffix?: React.ReactNode;
  className?: string;
  padded?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className={["panel", className].filter(Boolean).join(" ")}>
      {title ? (
        <div className="panel-header">
          <strong>{title}</strong>
          {suffix || null}
        </div>
      ) : null}
      {padded ? <div className="panel-body">{children}</div> : children}
    </div>
  );
}

function KvGrid({ items }: { items: Array<[string, unknown]> }) {
  return (
    <div className="kv-grid">
      {items.map(([label, value]) => (
        <div className="kv" key={label}>
          <span>{label}</span>
          <span>{valueOrDash(value)}</span>
        </div>
      ))}
    </div>
  );
}
