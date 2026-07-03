const state = {
  rootPath: "",
  workspaces: [],
  selectedRoot: "",
  detail: null,
  filter: "all",
  query: "",
  tab: "overview",
  loading: false,
  loadingMessage: "",
  preferencesLoaded: false
};

const statusLabels = {
  active: "活跃",
  idle: "空闲",
  paused: "暂停",
  completed: "完成",
  blocked: "阻断",
  invalid: "异常",
  unknown: "未知"
};

const tabs = [
  ["overview", "总览"],
  ["timeline", "时间线"],
  ["workitems", "工作项"],
  ["performance", "性能"],
  ["raw", "原始状态"]
];

function $(id) {
  return document.getElementById(id);
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
    .replaceAll("'", "&#39;");
}

function valueOrDash(value) {
  return value === null || value === undefined || value === "" ? "-" : String(value);
}

function timeAgo(value) {
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

function shortPath(value) {
  const text = valueOrDash(value);
  if (text.length <= 64) {
    return text;
  }
  return `...${text.slice(-61)}`;
}

function setLoading(loading) {
  state.loading = loading;
  $("chooseButton").disabled = loading;
  $("refreshButton").disabled = loading || !state.rootPath;
  $("chooseButton").textContent = loading ? "扫描中" : "选择目录";
  renderScanStatus();
}

function setStatus(message) {
  state.loadingMessage = message || "";
  renderScanStatus();
}

function renderScanStatus() {
  const status = $("scanStatus");
  if (!status) {
    return;
  }
  status.textContent = state.loadingMessage;
  status.classList.toggle("busy", state.loading);
}

function workspaceCounts() {
  const counts = { all: state.workspaces.length };
  for (const workspace of state.workspaces) {
    counts[workspace.status] = (counts[workspace.status] || 0) + 1;
  }
  return counts;
}

function renderFilters() {
  const counts = workspaceCounts();
  const order = ["all", "active", "blocked", "paused", "idle", "completed", "invalid"];
  $("filters").innerHTML = order
    .filter((key) => key === "all" || counts[key])
    .map((key) => {
      const label = key === "all" ? "全部" : statusLabels[key];
      const active = state.filter === key ? " active" : "";
      return `<button class="filter${active}" data-filter="${key}">${label} ${counts[key] || 0}</button>`;
    })
    .join("");

  for (const button of document.querySelectorAll(".filter")) {
    button.addEventListener("click", () => {
      state.filter = button.dataset.filter;
      renderSidebar();
    });
  }
}

function filteredWorkspaces() {
  const query = state.query.trim().toLowerCase();
  return state.workspaces.filter((workspace) => {
    const statusMatches = state.filter === "all" || workspace.status === state.filter;
    if (!statusMatches) {
      return false;
    }
    if (!query) {
      return true;
    }
    return [workspace.name, workspace.projectKey, workspace.phase, workspace.rootPath]
      .filter(Boolean)
      .some((value) => String(value).toLowerCase().includes(query));
  });
}

function renderSidebar() {
  renderFilters();
  const list = filteredWorkspaces();
  if (!list.length) {
    $("workspaceList").innerHTML = `<div class="empty-row">没有匹配的工作区</div>`;
    return;
  }

  $("workspaceList").innerHTML = list
    .map((workspace) => {
      const selected = workspace.rootPath === state.selectedRoot ? " selected" : "";
      return `
        <button class="workspace-item status-${escapeHtml(workspace.status)}${selected}" data-root="${escapeHtml(workspace.rootPath)}">
          <span class="status-dot"></span>
          <span class="workspace-main">
            <span class="workspace-title">
              <strong>${escapeHtml(workspace.name)}</strong>
              <span>${escapeHtml(statusLabels[workspace.status] || workspace.status)}</span>
            </span>
            <span class="muted-line">${escapeHtml(workspace.phase)} · ${escapeHtml(timeAgo(workspace.lastActivityAt))}</span>
            <span class="path-line">${escapeHtml(shortPath(workspace.rootPath))}</span>
          </span>
        </button>`;
    })
    .join("");

  for (const item of document.querySelectorAll(".workspace-item")) {
    item.addEventListener("click", () => selectWorkspace(item.dataset.root));
  }
}

function renderRoot() {
  $("rootPath").textContent = state.rootPath || "未选择目录";
  $("refreshButton").disabled = !state.rootPath || state.loading;
}

function renderDetail() {
  const detail = state.detail;
  if (!detail) {
    $("detail").innerHTML = `
      <div class="empty-state">
        <div class="empty-mark">.ae-sdd</div>
        <h1>${state.rootPath ? "选择一个工作区" : "选择目录开始扫描"}</h1>
        <p>${state.rootPath ? "左侧列表中没有选中项。" : "左侧会列出找到的 ae-sdd 工作区。"}</p>
      </div>`;
    return;
  }

  const summary = detail.summary;
  $("detail").innerHTML = `
    <div class="workspace-head">
      <div>
        <h1>${escapeHtml(summary.name)}</h1>
        <div class="workspace-path">${escapeHtml(summary.rootPath)}</div>
      </div>
      <div class="head-actions">
        <button id="openWorkspaceButton" class="button">打开目录</button>
      </div>
    </div>
    ${renderMetrics(summary, detail)}
    <div class="tabs">
      ${tabs
        .map(([key, label]) => `<button class="tab${state.tab === key ? " active" : ""}" data-tab="${key}">${label}</button>`)
        .join("")}
    </div>
    <div id="tabContent">${renderTabContent(state.tab, detail)}</div>`;

  $("openWorkspaceButton").addEventListener("click", () => window.monitorApi.openPath(summary.rootPath));
  for (const tab of document.querySelectorAll(".tab")) {
    tab.addEventListener("click", () => {
      state.tab = tab.dataset.tab;
      renderDetail();
    });
  }
}

function renderMetrics(summary, detail) {
  const progress = summary.progress || {};
  const fill = Math.max(0, Math.min(100, Number(progress.percent || 0)));
  return `
    <div class="summary-grid">
      <div class="metric">
        <div class="metric-label">状态</div>
        <div class="metric-value">${escapeHtml(statusLabels[summary.status] || summary.status)}</div>
      </div>
      <div class="metric">
        <div class="metric-label">阶段</div>
        <div class="metric-value">${escapeHtml(summary.phase)}</div>
        <div class="progress-track"><div class="progress-fill" style="width:${fill}%"></div></div>
      </div>
      <div class="metric">
        <div class="metric-label">活跃任务</div>
        <div class="metric-value">${escapeHtml(detail.activeWorkItems?.length || summary.activeAgentCount || 0)}</div>
        <div class="muted-line">${escapeHtml(valueOrDash(summary.activeWorkItem || summary.currentStory || summary.currentTask))}</div>
      </div>
      <div class="metric">
        <div class="metric-label">最近活动</div>
        <div class="metric-value">${escapeHtml(timeAgo(summary.lastActivityAt))}</div>
        <div class="muted-line">${escapeHtml(detail.runtimeStats.count)} 条运行记录</div>
      </div>
    </div>`;
}

function renderTabContent(tab, detail) {
  switch (tab) {
    case "timeline":
      return renderTimeline(detail);
    case "workitems":
      return renderWorkItems(detail);
    case "performance":
      return renderPerformance(detail);
    case "raw":
      return `<pre class="code">${escapeHtml(JSON.stringify(detail.state, null, 2))}</pre>`;
    case "overview":
    default:
      return renderOverview(detail);
  }
}

function renderOverview(detail) {
  const summary = detail.summary;
  const errors = summary.errors || [];
  return `
    <div class="panel">
      <div class="panel-header">
        <strong>状态摘要</strong>
        <span class="${errors.length ? "error" : ""}">${errors.length ? `${errors.length} 个异常` : "OK"}</span>
      </div>
      <div class="panel-body">
        <div class="kv-grid">
          ${kv("projectKey", summary.projectKey)}
          ${kv("phase", summary.phase)}
          ${kv("scale", summary.scale)}
          ${kv("entryNode", summary.entryNode)}
          ${kv("currentStory", summary.currentStory)}
          ${kv("currentTask", summary.currentTask)}
          ${kv("activeWorkItem", summary.activeWorkItem)}
          ${kv("activeAgents", summary.activeAgentCount)}
          ${kv("statePath", summary.statePath)}
          ${kv("configPath", summary.configPath)}
          ${kv("lastActivityAt", summary.lastActivityAt)}
        </div>
        ${errors.length ? `<div class="error" style="margin-top:14px">${errors.map(escapeHtml).join("<br>")}</div>` : ""}
      </div>
    </div>
    ${detail.activeWorkItems?.length ? renderActiveWorkItems(detail.activeWorkItems, "overview") : ""}`;
}

function kv(label, value) {
  return `<div class="kv"><span>${escapeHtml(label)}</span><span>${escapeHtml(valueOrDash(value))}</span></div>`;
}

function renderActiveWorkItems(items, context = "workitems") {
  if (!items?.length) {
    return context === "workitems"
      ? `<div class="panel"><div class="empty-row">当前没有活跃任务</div></div>`
      : "";
  }
  return `
    <div class="panel active-panel">
      <div class="panel-header">
        <strong>活跃任务</strong>
        <span>${escapeHtml(items.length)} 个</span>
      </div>
      <div class="active-list">
        ${items
          .map(
            (item) => `
              <div class="active-item status-${escapeHtml(item.status || "unknown")}">
                <span class="status-dot"></span>
                <div class="active-main">
                  <div class="active-title">
                    <strong>${escapeHtml(item.id)}</strong>
                    <span>${escapeHtml(statusLabels[item.status] || item.status || "活跃")}</span>
                  </div>
                  ${item.workItemName || item.workItemId ? `<div class="muted-line">${escapeHtml([item.workItemId, item.workItemName].filter(Boolean).join(" · "))}</div>` : ""}
                  <div class="muted-line">
                    ${escapeHtml([item.phase, item.source, item.skill, item.role, item.agentId].filter(Boolean).join(" · ") || "-")}
                  </div>
                  ${item.summary ? `<div class="path-line">${escapeHtml(item.summary)}</div>` : ""}
                </div>
                <div class="active-time">${escapeHtml(timeAgo(item.lastActivityAt))}</div>
              </div>`
          )
          .join("")}
      </div>
    </div>`;
}

function renderTimeline(detail) {
  const items = [];
  for (const item of detail.history || []) {
    items.push({
      title: item.phase || item.event || "history",
      time: item.timestamp || item.ts,
      meta: item.by ? `by ${item.by}` : "history"
    });
  }
  for (const item of detail.events || []) {
    items.push({
      title: item.event || item.node || "event",
      time: item.ts || item.timestamp,
      meta: [item.node, item.skill, item.txnName, item.reason].filter(Boolean).join(" · ")
    });
  }

  items.sort((a, b) => new Date(a.time || 0).getTime() - new Date(b.time || 0).getTime());

  if (!items.length) {
    items.push({
      title: detail.summary.phase,
      time: detail.loadedAt,
      meta: "current"
    });
  }

  return `
    <div class="panel">
      <div class="panel-header">
        <strong>阶段轴</strong>
        <span>${escapeHtml(detail.phaseTimeline?.scale || valueOrDash(detail.summary.scale))} · ${escapeHtml(detail.summary.phase)}</span>
      </div>
      <div class="panel-body">
        <div class="phase-axis">
          ${(detail.phaseTimeline?.nodes || [])
            .map(
              (node) => `
                <div class="phase-node ${escapeHtml(node.status)}">
                  <div class="phase-index">${escapeHtml(node.index)}</div>
                  <div class="phase-copy">
                    <div class="phase-title">${escapeHtml(node.label)} <span>${escapeHtml(node.phase)}</span></div>
                    <div class="phase-desc">${escapeHtml(node.description)}</div>
                  </div>
                </div>`
            )
            .join("")}
        </div>
      </div>
    </div>
    <div class="panel event-panel">
      <div class="panel-header">
        <strong>事件流</strong>
        <span>${escapeHtml(items.length)} 条</span>
      </div>
      <div class="panel-body">
        <div class="event-timeline">
          ${items
            .map((item, index) => {
              const current = index === items.length - 1 ? " current" : "";
              return `
                <div class="event-item${current}">
                  <span class="event-point"></span>
                  <div>
                    <div class="event-title">${escapeHtml(item.title)}</div>
                    <div class="event-meta">${escapeHtml(valueOrDash(item.time))}${item.meta ? ` · ${escapeHtml(item.meta)}` : ""}</div>
                  </div>
                </div>`;
            })
            .join("")}
        </div>
      </div>
    </div>`;
}

function renderWorkItems(detail) {
  const active = renderActiveWorkItems(detail.activeWorkItems || [], "workitems");
  if (!detail.workItems.length) {
    return `${active}<div class="panel"><div class="empty-row">未发现工作项 state</div></div>`;
  }
  return `
    ${active}
    <div class="panel">
      <div class="panel-header">
        <strong>全部工作项</strong>
        <span>${escapeHtml(detail.workItems.length)} 个</span>
      </div>
      <table class="table">
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
          ${detail.workItems
            .map(
              (item) => `
                <tr>
                  <td>
                    <div>${escapeHtml(item.id)}</div>
                    ${item.workItemName || item.workItemId ? `<div class="muted-line">${escapeHtml([item.workItemId, item.workItemName].filter(Boolean).join(" · "))}</div>` : ""}
                  </td>
                  <td>${escapeHtml(statusLabels[item.status] || item.status)}</td>
                  <td>${escapeHtml(item.phase)}</td>
                  <td>${escapeHtml(`${item.progress.index > 0 ? item.progress.index : "-"} / ${item.progress.total}`)}</td>
                  <td>${escapeHtml(item.activeAgentCount || 0)}</td>
                  <td>${escapeHtml(timeAgo(item.lastActivityAt))}</td>
                </tr>`
            )
            .join("")}
        </tbody>
      </table>
    </div>`;
}

function renderPerformance(detail) {
  const stats = detail.runtimeStats;
  if (!stats.count) {
    return `<div class="panel"><div class="empty-row">未发现 runtime-stats</div></div>`;
  }
  return `
    <div class="panel">
      <div class="panel-header">
        <strong>运行统计</strong>
        <span>${escapeHtml(stats.count)} 条 · 失败 ${escapeHtml(stats.failures)}</span>
      </div>
      <table class="table">
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
          ${stats.commands
            .map(
              (command) => `
                <tr>
                  <td>${escapeHtml(command.command)}</td>
                  <td>${escapeHtml(command.count)}</td>
                  <td>${escapeHtml(command.failures)}</td>
                  <td>${escapeHtml(command.avgMs)} ms</td>
                  <td>${escapeHtml(command.maxMs)} ms</td>
                  <td>${escapeHtml(timeAgo(command.lastStartedAt))}</td>
                </tr>`
            )
            .join("")}
        </tbody>
      </table>
    </div>`;
}

async function chooseDirectory() {
  setStatus("打开目录选择器...");
  setLoading(true);
  try {
    const directory = await window.monitorApi.chooseDirectory(state.rootPath);
    if (directory) {
      await scan(directory, { preferredSelectedRoot: "" });
    } else {
      setStatus("已取消");
    }
  } catch (error) {
    $("detail").innerHTML = `<div class="empty-state"><div class="empty-mark error">error</div><h1>选择失败</h1><p>${escapeHtml(error.message)}</p></div>`;
    setStatus("选择失败");
  } finally {
    setLoading(false);
  }
}

async function scan(rootPath = state.rootPath, options = {}) {
  if (!rootPath) {
    return;
  }
  setLoading(true);
  setStatus("扫描中...");
  renderRoot();
  try {
    const result = await window.monitorApi.scanWorkspaces(rootPath);
    const previousSelected = options.preferredSelectedRoot || state.selectedRoot;
    state.rootPath = result.rootPath;
    state.workspaces = result.workspaces;
    state.selectedRoot = state.workspaces.some((workspace) => workspace.rootPath === previousSelected)
      ? previousSelected
      : state.workspaces[0]?.rootPath || "";
    state.detail = null;
    await window.monitorApi.savePreferences({
      rootPath: state.rootPath,
      selectedRoot: state.selectedRoot,
      theme: document.body.classList.contains("dark") ? "dark" : "light"
    });
    renderRoot();
    renderSidebar();
    if (state.selectedRoot) {
      await selectWorkspace(state.selectedRoot);
    } else {
      renderDetail();
    }
    setStatus(`扫描完成 · ${state.workspaces.length} 个工作区`);
  } catch (error) {
    $("detail").innerHTML = `<div class="empty-state"><div class="empty-mark error">error</div><h1>扫描失败</h1><p>${escapeHtml(error.message)}</p></div>`;
    setStatus("扫描失败");
  } finally {
    setLoading(false);
    renderRoot();
  }
}

async function selectWorkspace(rootPath) {
  if (!rootPath) {
    return;
  }
  state.selectedRoot = rootPath;
  state.detail = null;
  renderSidebar();
  renderDetail();
  try {
    state.detail = await window.monitorApi.loadWorkspaceDetail(rootPath);
    await window.monitorApi.savePreferences({
      rootPath: state.rootPath,
      selectedRoot: rootPath,
      theme: document.body.classList.contains("dark") ? "dark" : "light"
    });
    renderSidebar();
    renderDetail();
  } catch (error) {
    $("detail").innerHTML = `<div class="empty-state"><div class="empty-mark error">error</div><h1>加载失败</h1><p>${escapeHtml(error.message)}</p></div>`;
  }
}

function bindEvents() {
  $("chooseButton").addEventListener("click", chooseDirectory);
  $("refreshButton").addEventListener("click", () => scan());
  $("searchInput").addEventListener("input", (event) => {
    state.query = event.target.value;
    renderSidebar();
  });
  $("themeButton").addEventListener("click", () => {
    document.body.classList.toggle("dark");
    window.monitorApi.savePreferences({
      rootPath: state.rootPath,
      selectedRoot: state.selectedRoot,
      theme: document.body.classList.contains("dark") ? "dark" : "light"
    });
  });
  $("closeWindowButton").addEventListener("click", () => window.monitorApi.windowControl("close"));
  $("minimizeWindowButton").addEventListener("click", () => window.monitorApi.windowControl("minimize"));
  $("maximizeWindowButton").addEventListener("click", () => window.monitorApi.windowControl("toggle-maximize"));
}

async function initialize() {
  bindEvents();
  renderRoot();
  renderSidebar();
  renderDetail();
  try {
    const preferences = await window.monitorApi.loadPreferences();
    state.preferencesLoaded = true;
    if (preferences.theme === "dark") {
      document.body.classList.add("dark");
    }
    if (preferences.rootPath) {
      state.rootPath = preferences.rootPath;
      state.selectedRoot = preferences.selectedRoot || "";
      renderRoot();
      await scan(preferences.rootPath, { preferredSelectedRoot: preferences.selectedRoot || "" });
    }
  } catch (error) {
    setStatus("配置读取失败");
  }
}

initialize();
