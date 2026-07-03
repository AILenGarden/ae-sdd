const state = {
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
  realtimeRefreshing: false,
  reactiveRefreshing: false,
  lastFullRefreshAt: 0,
  workspaceSignature: "",
  detailSignature: "",
  watchedRootPath: "",
  pendingDetailMotion: "",
  loadingMessage: "",
  preferencesLoaded: false
};

const DETAIL_REFRESH_MS = 30000;
const FULL_SCAN_REFRESH_MS = 120000;
const REACTIVE_REFRESH_DELAY_MS = 180;
let refreshTimer = null;
let reactiveRefreshTimer = null;

const statusLabels = {
  active: "活跃",
  idle: "空闲",
  paused: "暂停",
  completed: "完成",
  blocked: "阻断",
  invalid: "异常",
  unknown: "未知"
};

const memoryStatusLabels = {
  active: "Memory 活跃",
  ready: "Memory 就绪",
  empty: "Memory 空",
  missing: "Memory 缺失",
  blocked: "Memory 阻断",
  invalid: "Memory 异常",
  unknown: "Memory 未知"
};

const tabs = [
  ["overview", "总览"],
  ["timeline", "时间线"],
  ["memory", "Memory"],
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

function workItemCaption(item) {
  return [item?.workItemId, item?.workItemName].filter(Boolean).join(" / ");
}

function workItemLabel(item) {
  return item?.workItemKey || item?.id || item?.activeWorkItem || "";
}

function taskLabel(task) {
  return task?.label || task?.workItemName || task?.workItemKey || task?.currentTask || task?.currentStory || task?.id || "";
}

function selectedTask(detail = state.detail) {
  if (!detail || !state.selectedTaskId) {
    return null;
  }
  return (detail.tasks || []).find((task) => task.id === state.selectedTaskId) || null;
}

function tasksForWorkspace(workspace) {
  if (state.detail?.summary?.rootPath === workspace.rootPath) {
    return state.detail.tasks || [];
  }
  return workspace.tasks || [];
}

function isProjectCollapsed(rootPath) {
  return state.collapsedRoots.includes(rootPath);
}

function setProjectCollapsed(rootPath, collapsed) {
  const next = new Set(state.collapsedRoots);
  if (collapsed) {
    next.add(rootPath);
  } else {
    next.delete(rootPath);
  }
  state.collapsedRoots = Array.from(next);
}

function memoryLabel(memory) {
  const status = memory?.status || "unknown";
  return memoryStatusLabels[status] || status;
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
  status.classList.toggle("live", !state.loading && state.autoRefresh && /响应|实时/.test(state.loadingMessage));
}

function signatureOf(value) {
  return JSON.stringify(value || null);
}

function queueDetailMotion(kind = "drill") {
  state.pendingDetailMotion = kind;
}

function consumeDetailMotion() {
  const detail = $("detail");
  if (!detail || !state.pendingDetailMotion) {
    return;
  }
  const className = `motion-${state.pendingDetailMotion}`;
  detail.classList.remove("motion-drill", "motion-tab");
  void detail.offsetWidth;
  detail.classList.add(className);
  state.pendingDetailMotion = "";
  window.setTimeout(() => detail.classList.remove(className), 520);
}

function renderAutoRefreshButton() {
  const button = $("autoRefreshButton");
  if (!button) {
    return;
  }
  button.classList.toggle("active", state.autoRefresh);
  button.textContent = state.autoRefresh ? "响应" : "手动";
  button.title = state.autoRefresh ? "响应式刷新已开启" : "响应式刷新已关闭";
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
      const tasks = tasksForWorkspace(workspace);
      const collapsed = isProjectCollapsed(workspace.rootPath);
      const projectSelected = workspace.rootPath === state.selectedRoot && !state.selectedTaskId ? " selected" : "";
      const projectCurrent = workspace.rootPath === state.selectedRoot ? " current-project" : "";
      return `
        <div class="project-group${projectCurrent}">
          <div class="project-row">
            <button class="collapse-button${collapsed ? " collapsed" : ""}" data-root="${escapeHtml(workspace.rootPath)}" title="${collapsed ? "展开任务" : "收起任务"}" aria-label="${collapsed ? "展开任务" : "收起任务"}">
              ${tasks.length ? "›" : ""}
            </button>
            <button class="workspace-item status-${escapeHtml(workspace.status)}${projectSelected}" data-root="${escapeHtml(workspace.rootPath)}">
              <span class="status-dot"></span>
              <span class="workspace-main">
                <span class="workspace-title">
                  <strong>${escapeHtml(workspace.name)}</strong>
                  <span>${escapeHtml(statusLabels[workspace.status] || workspace.status)}</span>
                </span>
                <span class="muted-line">${escapeHtml(workspace.phase)} · ${escapeHtml(timeAgo(workspace.lastActivityAt))}</span>
                <span class="muted-line">${escapeHtml(memoryLabel(workspace.memory))} · ${escapeHtml(tasks.length)} 任务</span>
                <span class="path-line">${escapeHtml(shortPath(workspace.rootPath))}</span>
              </span>
            </button>
          </div>
          ${tasks.length ? `
            <div class="task-tree-shell${collapsed ? " collapsed" : ""}">
              <div class="task-tree">
              ${tasks
                .map((task) => {
                  const selected = workspace.rootPath === state.selectedRoot && task.id === state.selectedTaskId ? " selected" : "";
                  return `
                    <button class="task-item status-${escapeHtml(task.status || "unknown")}${selected}" data-root="${escapeHtml(workspace.rootPath)}" data-task-id="${escapeHtml(task.id)}">
                      <span class="tree-line"></span>
                      <span class="status-dot"></span>
                      <span class="task-main">
                        <span class="task-title">
                          <strong>${escapeHtml(taskLabel(task))}</strong>
                          <span>${escapeHtml(statusLabels[task.status] || task.status || "未知")}</span>
                        </span>
                        <span class="muted-line">${escapeHtml([task.phase, memoryLabel(task.memory)].filter(Boolean).join(" · "))}</span>
                      </span>
                    </button>`;
                })
                .join("")}
              </div>
            </div>` : ""}
        </div>`;
    })
    .join("");

  for (const item of document.querySelectorAll(".workspace-item")) {
    item.addEventListener("click", () => selectWorkspace(item.dataset.root, { taskId: "" }));
  }
  for (const item of document.querySelectorAll(".collapse-button")) {
    item.addEventListener("click", () => toggleProjectCollapsed(item.dataset.root));
  }
  for (const item of document.querySelectorAll(".task-item")) {
    item.addEventListener("click", () => selectTask(item.dataset.root, item.dataset.taskId));
  }
}

function renderRoot() {
  $("rootPath").textContent = state.rootPath || "未选择目录";
  $("refreshButton").disabled = !state.rootPath || state.loading;
  renderAutoRefreshButton();
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
  const task = selectedTask(detail);
  $("detail").innerHTML = `
    <div class="workspace-head">
      <div>
        <h1>${escapeHtml(summary.name)}</h1>
        <div class="workspace-path">${escapeHtml(task ? `${summary.rootPath} · ${taskLabel(task)}` : summary.rootPath)}</div>
      </div>
      <div class="head-actions">
        <button id="openWorkspaceButton" class="button">打开目录</button>
      </div>
    </div>
    ${renderContextBoard(summary, detail, task)}
    ${renderMetrics(summary, detail, task)}
    <div class="tabs">
      ${tabs
        .map(([key, label]) => `<button class="tab${state.tab === key ? " active" : ""}" data-tab="${key}">${label}</button>`)
        .join("")}
    </div>
    <div id="tabContent">${renderTabContent(state.tab, detail, task)}</div>`;

  $("openWorkspaceButton").addEventListener("click", () => window.monitorApi.openPath(summary.rootPath));
  $("projectContextButton")?.addEventListener("click", () => selectWorkspace(summary.rootPath, { taskId: "" }));
  for (const tab of document.querySelectorAll(".tab")) {
    tab.addEventListener("click", () => {
      state.tab = tab.dataset.tab;
      queueDetailMotion("tab");
      renderDetail();
    });
  }
  consumeDetailMotion();
}

function renderContextBoard(summary, detail, task) {
  const projectMemory = detail.memory || summary.memory || {};
  return `
    <div class="context-board">
      <button class="context-card project-card${task ? "" : " selected"}" id="projectContextButton">
        <span class="context-kicker">当前项目</span>
        <strong>${escapeHtml(summary.name)}</strong>
        <span>${escapeHtml([summary.phase, statusLabels[summary.status] || summary.status].filter(Boolean).join(" · "))}</span>
        <span>${escapeHtml(memoryLabel(projectMemory))} · ${escapeHtml(detail.tasks?.length || 0)} 任务</span>
      </button>
      <div class="context-card task-card${task ? " selected" : ""}">
        <span class="context-kicker">当前任务</span>
        ${task
          ? `
            <strong>${escapeHtml(taskLabel(task))}</strong>
            <span>${escapeHtml([task.phase, statusLabels[task.status] || task.status].filter(Boolean).join(" · "))}</span>
            <span>${escapeHtml(memoryLabel(task.memory))} · ${escapeHtml(task.memory?.memoryEntries || 0)} 条记忆</span>`
          : `
            <strong>未选择任务</strong>
            <span>右侧显示项目级看板</span>
            <span>${escapeHtml(detail.activeWorkItems?.length || 0)} 个活跃任务</span>`}
      </div>
    </div>`;
}

function renderMetrics(summary, detail, task = null) {
  const progress = (task?.progress || summary.progress) || {};
  const fill = Math.max(0, Math.min(100, Number(progress.percent || 0)));
  const memory = task?.memory || detail.memory || summary.memory || {};
  const activeWorkItem = task ? taskLabel(task) : workItemCaption(summary) || workItemLabel(summary) || summary.currentStory || summary.currentTask;
  const status = task?.status || summary.status;
  const phase = task?.phase || summary.phase;
  const recentAt = task ? task.lastActivityAt || task.memory?.lastMemoryAt : summary.lastActivityAt;
  return `
    <div class="summary-grid">
      <div class="metric">
        <div class="metric-label">状态</div>
        <div class="metric-value">${escapeHtml(statusLabels[status] || status)}</div>
      </div>
      <div class="metric">
        <div class="metric-label">阶段</div>
        <div class="metric-value">${escapeHtml(phase)}</div>
        <div class="progress-track"><div class="progress-fill" style="width:${fill}%"></div></div>
      </div>
      <div class="metric">
        <div class="metric-label">Memory</div>
        <div class="metric-value">${escapeHtml(memoryLabel(memory))}</div>
        <div class="muted-line">${escapeHtml(memory.memoryEntries || 0)} 条 · ${escapeHtml(memory.activeScopeCount || 0)} 活跃 scope</div>
      </div>
      <div class="metric">
        <div class="metric-label">最近活动</div>
        <div class="metric-value">${escapeHtml(timeAgo(recentAt))}</div>
        <div class="muted-line">${escapeHtml(valueOrDash(activeWorkItem))}</div>
      </div>
    </div>`;
}

function renderTabContent(tab, detail, task = null) {
  switch (tab) {
    case "timeline":
      return renderTimeline(detail, task);
    case "memory":
      return renderMemory(detail, task);
    case "workitems":
      return renderWorkItems(detail);
    case "performance":
      return renderPerformance(detail);
    case "raw":
      return `<pre class="code">${escapeHtml(JSON.stringify(task?.state || detail.state, null, 2))}</pre>`;
    case "overview":
    default:
      return renderOverview(detail, task);
  }
}

function renderOverview(detail, task = null) {
  const summary = detail.summary;
  const errors = summary.errors || [];
  const taskPanel = task
    ? `
      <div class="panel active-panel">
        <div class="panel-header">
          <strong>任务摘要</strong>
          <span>${escapeHtml(statusLabels[task.status] || task.status || "未知")}</span>
        </div>
        <div class="panel-body">
          <div class="kv-grid">
            ${kv("taskId", task.id)}
            ${kv("workItemId", task.workItemId)}
            ${kv("workItemName", task.workItemName)}
            ${kv("workItemKey", task.workItemKey)}
            ${kv("phase", task.phase)}
            ${kv("currentStory", task.currentStory)}
            ${kv("currentTask", task.currentTask)}
            ${kv("statePath", task.statePath)}
            ${kv("memory", memoryLabel(task.memory))}
            ${kv("lastActivityAt", task.lastActivityAt)}
          </div>
        </div>
      </div>`
    : "";
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
          ${kv("activeStatePath", summary.activeStatePath)}
          ${kv("workItemId", summary.workItemId)}
          ${kv("workItemName", summary.workItemName)}
          ${kv("workItemKey", summary.workItemKey)}
          ${kv("activeAgents", summary.activeAgentCount)}
          ${kv("taskCount", detail.tasks?.length || 0)}
          ${kv("memory", memoryLabel(detail.memory || summary.memory))}
          ${kv("memoryLastAt", detail.memory?.lastMemoryAt || summary.memoryLastAt)}
          ${kv("statePath", summary.statePath)}
          ${kv("configPath", summary.configPath)}
          ${kv("lastActivityAt", summary.lastActivityAt)}
        </div>
        ${errors.length ? `<div class="error" style="margin-top:14px">${errors.map(escapeHtml).join("<br>")}</div>` : ""}
      </div>
    </div>
    ${taskPanel}
    ${!task && detail.activeWorkItems?.length ? renderActiveWorkItems(detail.activeWorkItems, "overview") : ""}`;
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
          .map((item) => {
            const caption = workItemCaption(item);
            return `
              <div class="active-item status-${escapeHtml(item.status || "unknown")}">
                <span class="status-dot"></span>
                <div class="active-main">
                  <div class="active-title">
                    <strong>${escapeHtml(workItemLabel(item) || item.id)}</strong>
                    <span>${escapeHtml(statusLabels[item.status] || item.status || "活跃")}</span>
                  </div>
                  ${caption ? `<div class="muted-line">${escapeHtml(caption)}</div>` : ""}
                  <div class="muted-line">
                    ${escapeHtml([item.phase, item.source, item.skill, item.role, item.agentId].filter(Boolean).join(" · ") || "-")}
                  </div>
                  ${item.summary ? `<div class="path-line">${escapeHtml(item.summary)}</div>` : ""}
                </div>
                <div class="active-time">${escapeHtml(timeAgo(item.lastActivityAt))}</div>
              </div>`;
          })
          .join("")}
      </div>
    </div>`;
}

function renderTimeline(detail, task = null) {
  const source = task || detail.summary;
  const timelineSource = task || detail;
  const items = [];
  for (const item of timelineSource.history || []) {
    items.push({
      title: item.phase || item.event || "history",
      time: item.timestamp || item.ts,
      meta: item.by ? `by ${item.by}` : "history"
    });
  }
  for (const item of timelineSource.events || []) {
    items.push({
      title: item.event || item.node || "event",
      time: item.ts || item.timestamp,
      meta: [item.node, item.skill, item.txnName, item.reason].filter(Boolean).join(" · ")
    });
  }

  items.sort((a, b) => new Date(a.time || 0).getTime() - new Date(b.time || 0).getTime());

  if (!items.length) {
    items.push({
      title: source.phase,
      time: detail.loadedAt,
      meta: "current"
    });
  }

  return `
    <div class="panel">
      <div class="panel-header">
        <strong>阶段轴</strong>
        <span>${escapeHtml((task?.phaseTimeline || detail.phaseTimeline)?.scale || valueOrDash(source.scale))} · ${escapeHtml(source.phase)}</span>
      </div>
      <div class="panel-body">
        <div class="phase-axis">
          ${((task?.phaseTimeline || detail.phaseTimeline)?.nodes || [])
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

function renderMemory(detail, task = null) {
  const memory = task?.memory || detail.memory || {};
  const title = task ? `任务 Memory · ${taskLabel(task)}` : "项目 Memory";
  return `
    <div class="panel">
      <div class="panel-header">
        <strong>${escapeHtml(title)}</strong>
        <span>${escapeHtml(memoryLabel(memory))}</span>
      </div>
      <div class="panel-body">
        <div class="kv-grid">
          ${kv("status", memoryLabel(memory))}
          ${kv("root", memory.root)}
          ${kv("total", memory.total)}
          ${kv("memoryEntries", memory.memoryEntries)}
          ${kv("projectMemory", memory.projectMemoryCount)}
          ${kv("taskMemory", memory.taskMemoryCount)}
          ${kv("activeScopes", memory.activeScopeCount)}
          ${kv("blockedScopes", memory.blockedScopeCount)}
          ${kv("lastMemoryAt", memory.lastMemoryAt)}
        </div>
      </div>
    </div>
    ${renderMemoryScopes(memory)}
    ${renderMemoryRecent(memory)}`;
}

function renderMemoryScopes(memory) {
  const scopes = [...(memory.blockedScopes || []), ...(memory.activeScopes || [])];
  if (!scopes.length) {
    return `<div class="panel active-panel"><div class="empty-row">没有活跃或阻断的 memory scope</div></div>`;
  }
  return `
    <div class="panel active-panel">
      <div class="panel-header">
        <strong>Memory Scope</strong>
        <span>${escapeHtml(scopes.length)} 个</span>
      </div>
      <table class="table">
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
          ${scopes
            .map((scope) => `
              <tr>
                <td>${escapeHtml(scope.id)}</td>
                <td>${escapeHtml(scope.needsWrite ? "需写入" : "活跃")}</td>
                <td>${escapeHtml(valueOrDash(scope.phase))}</td>
                <td>${escapeHtml([scope.story, scope.task].filter(Boolean).join(" / ") || "-")}</td>
                <td>${escapeHtml(timeAgo(scope.lastActivityAt))}</td>
              </tr>`)
            .join("")}
        </tbody>
      </table>
    </div>`;
}

function renderMemoryRecent(memory) {
  const recent = memory.recent || [];
  if (!recent.length) {
    return `<div class="panel active-panel"><div class="empty-row">未发现 memory 记录</div></div>`;
  }
  return `
    <div class="panel active-panel">
      <div class="panel-header">
        <strong>最近 Memory</strong>
        <span>${escapeHtml(recent.length)} 条</span>
      </div>
      <table class="table">
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
          ${recent
            .map((item) => `
              <tr>
                <td>${escapeHtml(timeAgo(item.timestamp))}</td>
                <td>${escapeHtml(item.memoryScope || item.layer || item.type || "-")}</td>
                <td>${escapeHtml(item.kind || item.type || "-")}</td>
                <td>${escapeHtml(item.summary || item.note || item.reason || "-")}</td>
                <td>${escapeHtml((item.evidence || []).join(" / ") || shortPath(item.path || ""))}</td>
              </tr>`)
            .join("")}
        </tbody>
      </table>
    </div>`;
}

async function savePreferences() {
  await window.monitorApi.savePreferences({
    rootPath: state.rootPath,
    selectedRoot: state.selectedRoot,
    selectedTaskId: state.selectedTaskId,
    collapsedRoots: state.collapsedRoots,
    autoRefresh: state.autoRefresh,
    theme: document.body.classList.contains("dark") ? "dark" : "light"
  });
}

async function startReactiveWatch(rootPath = state.rootPath) {
  if (!state.autoRefresh || !rootPath || !window.monitorApi.watchWorkspaces) {
    return;
  }
  if (state.watchedRootPath === rootPath) {
    return;
  }
  try {
    const result = await window.monitorApi.watchWorkspaces(rootPath);
    state.watchedRootPath = result?.rootPath || rootPath;
  } catch {
    state.watchedRootPath = "";
    setStatus("响应式监听不可用，使用兜底刷新");
  }
}

async function stopReactiveWatch() {
  state.watchedRootPath = "";
  if (window.monitorApi.unwatchWorkspaces) {
    await window.monitorApi.unwatchWorkspaces();
  }
}

function scheduleReactiveRefresh(payload = {}) {
  if (!state.autoRefresh || !state.rootPath) {
    return;
  }
  if (payload.rootPath && payload.rootPath !== state.rootPath) {
    return;
  }
  if (reactiveRefreshTimer) {
    clearTimeout(reactiveRefreshTimer);
  }
  reactiveRefreshTimer = setTimeout(() => {
    reactiveRefreshTimer = null;
    refreshReactive();
  }, REACTIVE_REFRESH_DELAY_MS);
}

async function refreshReactive() {
  if (!state.autoRefresh || state.loading || state.reactiveRefreshing || !state.rootPath) {
    return false;
  }
  state.reactiveRefreshing = true;
  try {
    return await scan(state.rootPath, {
      silent: true,
      reactive: true,
      preferredSelectedRoot: state.selectedRoot,
      preferredSelectedTaskId: state.selectedTaskId
    });
  } finally {
    state.reactiveRefreshing = false;
  }
}

function bindReactiveEvents() {
  if (!window.monitorApi.onWorkspaceFilesChanged) {
    return;
  }
  window.monitorApi.onWorkspaceFilesChanged(scheduleReactiveRefresh);
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
            .map((item) => {
              const caption = workItemCaption(item);
              return `
                <tr>
                  <td>
                    <div>${escapeHtml(workItemLabel(item) || item.id)}</div>
                    ${caption ? `<div class="muted-line">${escapeHtml(caption)}</div>` : ""}
                  </td>
                  <td>${escapeHtml(statusLabels[item.status] || item.status)}</td>
                  <td>${escapeHtml(item.phase)}</td>
                  <td>${escapeHtml(`${item.progress.index > 0 ? item.progress.index : "-"} / ${item.progress.total}`)}</td>
                  <td>${escapeHtml(item.activeAgentCount || 0)}</td>
                  <td>${escapeHtml(timeAgo(item.lastActivityAt))}</td>
                </tr>`;
            })
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
    return false;
  }
  const silent = Boolean(options.silent);
  if (!silent) {
    setLoading(true);
    setStatus("扫描中...");
  }
  renderRoot();
  try {
    const result = await window.monitorApi.scanWorkspaces(rootPath);
    const previousSelected = options.preferredSelectedRoot || state.selectedRoot;
    const previousTask = options.preferredSelectedTaskId ?? state.selectedTaskId;
    const nextWorkspaceSignature = signatureOf(result.workspaces);
    const workspaceChanged = nextWorkspaceSignature !== state.workspaceSignature;
    state.rootPath = result.rootPath;
    state.workspaces = result.workspaces;
    state.workspaceSignature = nextWorkspaceSignature;
    const roots = new Set(state.workspaces.map((workspace) => workspace.rootPath));
    state.collapsedRoots = state.collapsedRoots.filter((root) => roots.has(root));
    state.selectedRoot = state.workspaces.some((workspace) => workspace.rootPath === previousSelected)
      ? previousSelected
      : state.workspaces[0]?.rootPath || "";
    state.lastFullRefreshAt = Date.now();
    if (!silent) {
      state.detail = null;
      await savePreferences();
      await startReactiveWatch(state.rootPath);
    }
    renderRoot();
    if (!silent || workspaceChanged) {
      renderSidebar();
    }
    let detailChanged = false;
    if (state.selectedRoot) {
      detailChanged = await selectWorkspace(state.selectedRoot, { taskId: previousTask, silent });
    } else {
      state.selectedTaskId = "";
      state.detail = null;
      state.detailSignature = "";
      renderDetail();
    }
    if (!silent || workspaceChanged || detailChanged) {
      setStatus(`${silent ? "响应式更新" : "扫描完成"} · ${state.workspaces.length} 个工作区`);
    }
    return workspaceChanged || detailChanged;
  } catch (error) {
    if (!silent) {
      $("detail").innerHTML = `<div class="empty-state"><div class="empty-mark error">error</div><h1>扫描失败</h1><p>${escapeHtml(error.message)}</p></div>`;
    }
    setStatus(silent ? "响应式更新失败" : "扫描失败");
    return false;
  } finally {
    if (!silent) {
      setLoading(false);
    }
    renderRoot();
  }
}

async function selectWorkspace(rootPath, options = {}) {
  if (!rootPath) {
    return false;
  }
  const silent = Boolean(options.silent);
  const nextTaskId = options.taskId ?? (rootPath === state.selectedRoot ? state.selectedTaskId : "");
  state.selectedRoot = rootPath;
  state.selectedTaskId = nextTaskId || "";
  if (!silent) {
    queueDetailMotion("drill");
  }
  if (!silent) {
    state.detail = null;
    state.detailSignature = "";
    renderSidebar();
    renderDetail();
  }
  try {
    const nextDetail = await window.monitorApi.loadWorkspaceDetail(rootPath);
    state.detail = nextDetail;
    if (state.selectedTaskId && !(state.detail.tasks || []).some((task) => task.id === state.selectedTaskId)) {
      state.selectedTaskId = "";
    }
    const nextDetailSignature = signatureOf({
      rootPath,
      selectedTaskId: state.selectedTaskId,
      detail: state.detail
    });
    const detailChanged = nextDetailSignature !== state.detailSignature;
    state.detailSignature = nextDetailSignature;
    if (!silent) {
      await savePreferences();
    }
    if (!silent || detailChanged) {
      renderSidebar();
      renderDetail();
    }
    return detailChanged;
  } catch (error) {
    if (!silent) {
      $("detail").innerHTML = `<div class="empty-state"><div class="empty-mark error">error</div><h1>加载失败</h1><p>${escapeHtml(error.message)}</p></div>`;
    }
    return false;
  }
}

async function selectTask(rootPath, taskId) {
  if (!rootPath || !taskId) {
    return;
  }
  setProjectCollapsed(rootPath, false);
  if (rootPath !== state.selectedRoot || !state.detail) {
    await selectWorkspace(rootPath, { taskId });
    return;
  }
  state.selectedTaskId = taskId;
  await savePreferences();
  if (state.detail) {
    state.detailSignature = signatureOf({
      rootPath,
      selectedTaskId: state.selectedTaskId,
      detail: state.detail
    });
  }
  queueDetailMotion("drill");
  renderSidebar();
  renderDetail();
}

async function toggleProjectCollapsed(rootPath) {
  if (!rootPath) {
    return;
  }
  setProjectCollapsed(rootPath, !isProjectCollapsed(rootPath));
  await savePreferences();
  renderSidebar();
}

async function refreshRealtime() {
  if (!state.autoRefresh || state.loading || state.realtimeRefreshing || !state.rootPath) {
    return false;
  }
  state.realtimeRefreshing = true;
  try {
    const shouldFullScan = !state.lastFullRefreshAt || Date.now() - state.lastFullRefreshAt >= FULL_SCAN_REFRESH_MS;
    if (shouldFullScan) {
      return await scan(state.rootPath, {
        silent: true,
        preferredSelectedRoot: state.selectedRoot,
        preferredSelectedTaskId: state.selectedTaskId
      });
    } else if (state.selectedRoot) {
      const changed = await selectWorkspace(state.selectedRoot, { taskId: state.selectedTaskId, silent: true });
      if (changed) {
        setStatus(`兜底刷新 · ${new Date().toLocaleTimeString("zh-CN")}`);
      }
      return changed;
    }
    return false;
  } finally {
    state.realtimeRefreshing = false;
  }
}

function bindPressFeedback() {
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

  document.addEventListener("pointerdown", (event) => {
    const target = event.target.closest(selector);
    if (!target || target.disabled) {
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
  });
}

function startRealtimeRefresh() {
  if (refreshTimer) {
    clearInterval(refreshTimer);
  }
  refreshTimer = setInterval(refreshRealtime, DETAIL_REFRESH_MS);
}

function bindEvents() {
  bindPressFeedback();
  bindReactiveEvents();
  $("chooseButton").addEventListener("click", chooseDirectory);
  $("refreshButton").addEventListener("click", () => scan());
  $("searchInput").addEventListener("input", (event) => {
    state.query = event.target.value;
    renderSidebar();
  });
  $("themeButton").addEventListener("click", () => {
    document.body.classList.add("theme-switching");
    document.body.classList.toggle("dark");
    savePreferences();
    window.setTimeout(() => document.body.classList.remove("theme-switching"), 420);
  });
  $("autoRefreshButton").addEventListener("click", async () => {
    state.autoRefresh = !state.autoRefresh;
    renderRoot();
    savePreferences();
    if (state.autoRefresh) {
      setStatus("响应式刷新已开启");
      await startReactiveWatch(state.rootPath);
      refreshReactive();
    } else {
      setStatus("响应式刷新已关闭");
      await stopReactiveWatch();
    }
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
    state.autoRefresh = preferences.autoRefresh !== false;
    state.selectedTaskId = preferences.selectedTaskId || "";
    state.collapsedRoots = Array.isArray(preferences.collapsedRoots) ? preferences.collapsedRoots : [];
    renderRoot();
    if (preferences.rootPath) {
      state.rootPath = preferences.rootPath;
      state.selectedRoot = preferences.selectedRoot || "";
      renderRoot();
      await scan(preferences.rootPath, {
        preferredSelectedRoot: preferences.selectedRoot || "",
        preferredSelectedTaskId: preferences.selectedTaskId || ""
      });
      await startReactiveWatch(preferences.rootPath);
    }
  } catch (error) {
    setStatus("配置读取失败");
  }
  startRealtimeRefresh();
}

initialize();
