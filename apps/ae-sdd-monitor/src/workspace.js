const fs = require("fs/promises");
const path = require("path");

const SKIP_DIRS = new Set([
  ".git",
  ".hg",
  ".svn",
  ".ae-sdd",
  ".auto-engineering",
  "node_modules",
  "dist",
  "release",
  "target",
  "build",
  "out",
  ".venv",
  "venv",
  ".pytest_cache",
  ".idea",
  ".vscode",
  "references"
]);

const PHASE_FLOWS = {
  large: [
    "initialized",
    "ra-generated",
    "dr-generated",
    "story-generated",
    "story-reviewed",
    "testcase-generated",
    "testcase-reviewed",
    "task-generated",
    "task-reviewed",
    "coding-process",
    "coding",
    "test-running",
    "code-reviewed",
    "completed"
  ],
  medium: [
    "initialized",
    "dr-generated",
    "story-generated",
    "story-reviewed",
    "testcase-generated",
    "testcase-reviewed",
    "task-generated",
    "task-reviewed",
    "coding-process",
    "coding",
    "test-running",
    "code-reviewed",
    "completed"
  ],
  small: [
    "initialized",
    "story-generated",
    "story-reviewed",
    "testcase-generated",
    "testcase-reviewed",
    "task-generated",
    "task-reviewed",
    "coding-process",
    "coding",
    "test-running",
    "code-reviewed",
    "completed"
  ],
  micro: [
    "initialized",
    "task-generated",
    "task-reviewed",
    "coding-process",
    "coding",
    "test-running",
    "code-reviewed",
    "completed"
  ]
};

const PHASE_META = {
  initialized: ["初始化", "工作区已建立，等待进入对应规模的第一步。"],
  "ra-generated": ["RA", "需求分析已生成，大任务主链中的需求澄清节点。"],
  "dr-generated": ["DR", "设计需求已生成，中/大任务从这里进入设计分解。"],
  "story-generated": ["Story", "用户故事已生成，进入 Story 复核前的设计展开节点。"],
  "story-reviewed": ["Story Review", "Story 已复核，下一步进入测试用例或任务拆解。"],
  "testcase-generated": ["TestCase", "测试用例已生成，等待测试用例复核。"],
  "testcase-reviewed": ["TestCase Review", "测试用例已复核，进入任务拆分。"],
  "task-generated": ["Task", "任务已生成，微任务通常从这里开始。"],
  "task-reviewed": ["Task Review", "任务已复核，进入编码计划。"],
  "coding-process": ["Coding Process", "编码过程规划节点，准备进入实际编码。"],
  coding: ["Coding", "编码执行中或已进入编码阶段。"],
  "test-running": ["Test", "测试生成/执行/复核阶段。"],
  "code-reviewed": ["Code Review", "代码复核/编码报告节点。"],
  completed: ["完成", "流程已结束。"],
  paused: ["暂停", "流程被暂停，需恢复后继续。"]
};

const SCALE_ALIASES = new Map([
  ["大", "large"],
  ["large", "large"],
  ["big", "large"],
  ["中", "medium"],
  ["medium", "medium"],
  ["小", "small"],
  ["small", "small"],
  ["微", "micro"],
  ["micro", "micro"]
]);

const SCALE_LABELS = {
  large: "大",
  medium: "中",
  small: "小",
  micro: "微"
};

async function pathExists(target) {
  try {
    await fs.access(target);
    return true;
  } catch {
    return false;
  }
}

async function safeStat(target) {
  try {
    return await fs.stat(target);
  } catch {
    return null;
  }
}

async function readJsonFile(file) {
  try {
    const text = await fs.readFile(file, "utf8");
    return { ok: true, value: JSON.parse(text), error: null };
  } catch (error) {
    return { ok: false, value: null, error: `${path.basename(file)}: ${error.message}` };
  }
}

function stripYamlComment(value) {
  let quoted = false;
  let quote = "";
  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if ((char === "'" || char === "\"") && (index === 0 || value[index - 1] !== "\\")) {
      if (!quoted) {
        quoted = true;
        quote = char;
      } else if (quote === char) {
        quoted = false;
      }
    }
    if (char === "#" && !quoted) {
      return value.slice(0, index).trim();
    }
  }
  return value.trim();
}

function coerceYamlValue(raw) {
  const value = stripYamlComment(raw);
  if (value === "") {
    return "";
  }
  if (value === "true") {
    return true;
  }
  if (value === "false") {
    return false;
  }
  if (value === "null" || value === "~") {
    return null;
  }
  if ((value.startsWith("\"") && value.endsWith("\"")) || (value.startsWith("'") && value.endsWith("'"))) {
    return value.slice(1, -1);
  }
  if (/^-?\d+(\.\d+)?$/.test(value)) {
    return Number(value);
  }
  return value;
}

function parseYamlLite(text) {
  const root = {};
  const stack = [{ indent: -1, value: root }];
  const lines = text.split(/\r?\n/);

  for (const line of lines) {
    if (!line.trim() || line.trimStart().startsWith("#")) {
      continue;
    }
    const match = line.match(/^(\s*)([^:#]+):(?:\s*(.*))?$/);
    if (!match) {
      continue;
    }
    const indent = match[1].length;
    const key = match[2].trim();
    const rawValue = match[3] ?? "";

    while (stack.length > 1 && stack[stack.length - 1].indent >= indent) {
      stack.pop();
    }
    const parent = stack[stack.length - 1].value;
    if (rawValue.trim() === "") {
      parent[key] = {};
      stack.push({ indent, value: parent[key] });
    } else {
      parent[key] = coerceYamlValue(rawValue);
    }
  }

  return root;
}

async function readConfig(aeDir) {
  const jsonPath = path.join(aeDir, "config.json");
  if (await pathExists(jsonPath)) {
    const result = await readJsonFile(jsonPath);
    return { value: result.value || {}, error: result.error, path: jsonPath };
  }

  for (const name of ["config.yaml", "config.yml"]) {
    const file = path.join(aeDir, name);
    if (!(await pathExists(file))) {
      continue;
    }
    try {
      const text = await fs.readFile(file, "utf8");
      return { value: parseYamlLite(text), error: null, path: file };
    } catch (error) {
      return { value: {}, error: `${name}: ${error.message}`, path: file };
    }
  }

  return { value: {}, error: null, path: null };
}

function normalizeDate(value) {
  if (!value || typeof value !== "string") {
    return null;
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return null;
  }
  return date.toISOString();
}

function newestDate(values) {
  const times = values
    .filter(Boolean)
    .map((value) => new Date(value).getTime())
    .filter((value) => !Number.isNaN(value));
  if (!times.length) {
    return null;
  }
  return new Date(Math.max(...times)).toISOString();
}

function collectStateTimestamps(state) {
  const values = [];
  for (const item of state?.history || []) {
    values.push(normalizeDate(item.timestamp || item.ts || item.finishedAt || item.startedAt));
  }
  for (const item of state?.events || []) {
    values.push(normalizeDate(item.ts || item.timestamp || item.finishedAt || item.startedAt));
  }
  return values.filter(Boolean);
}

function flowForScale(scale) {
  const key = SCALE_ALIASES.get(String(scale || "").trim()) || "large";
  return PHASE_FLOWS[key] || PHASE_FLOWS.large;
}

function scaleKeyForState(state) {
  return SCALE_ALIASES.get(String(state?.scale || "").trim()) || "large";
}

function phaseProgress(state) {
  const phase = state?.phase || "initialized";
  const flow = flowForScale(state?.scale);
  const index = flow.indexOf(phase);
  if (index < 0) {
    return { current: phase, index: -1, total: flow.length, percent: 0 };
  }
  return {
    current: phase,
    index: index + 1,
    total: flow.length,
    percent: Math.round(((index + 1) / flow.length) * 100)
  };
}

function phaseTimeline(state) {
  const phase = state?.phase || "initialized";
  const scaleKey = scaleKeyForState(state);
  const flow = PHASE_FLOWS[scaleKey] || PHASE_FLOWS.large;
  const pausedFromPhase = state?.pausedFromPhase || null;
  const effectivePhase = phase === "paused" && pausedFromPhase ? pausedFromPhase : phase;
  const currentIndex = flow.indexOf(effectivePhase);
  const nodes = flow.map((node, index) => {
    const meta = PHASE_META[node] || [node, ""];
    let status = "pending";
    if (currentIndex >= 0 && index < currentIndex) {
      status = "done";
    } else if (currentIndex >= 0 && index === currentIndex) {
      status = phase === "paused" ? "paused" : "current";
    }
    return {
      phase: node,
      label: meta[0],
      description: meta[1],
      index: index + 1,
      total: flow.length,
      status
    };
  });

  if (phase !== "paused" && currentIndex < 0) {
    const meta = PHASE_META[phase] || [phase, "当前 state.phase 不在该规模阶段链中。"];
    nodes.push({
      phase,
      label: meta[0],
      description: meta[1],
      index: nodes.length + 1,
      total: nodes.length + 1,
      status: "current"
    });
  }

  return {
    scale: SCALE_LABELS[scaleKey] || scaleKey,
    current: phase,
    pausedFromPhase,
    currentIndex: currentIndex >= 0 ? currentIndex + 1 : -1,
    total: nodes.length,
    nodes
  };
}

async function readRuntimeEvents(aeDir, limit = 120) {
  const directory = path.join(aeDir, "runtime-stats");
  if (!(await pathExists(directory))) {
    return [];
  }

  let files = [];
  try {
    files = await fs.readdir(directory, { withFileTypes: true });
  } catch {
    return [];
  }

  const jsonlFiles = files
    .filter((entry) => entry.isFile() && entry.name.endsWith(".jsonl"))
    .map((entry) => path.join(directory, entry.name))
    .sort()
    .reverse();

  const events = [];
  for (const file of jsonlFiles) {
    let text = "";
    try {
      text = await fs.readFile(file, "utf8");
    } catch {
      continue;
    }
    const lines = text.split(/\r?\n/).filter(Boolean).reverse();
    for (const line of lines) {
      try {
        const event = JSON.parse(line);
        event.__file = file;
        events.push(event);
      } catch {
        // Ignore partial JSONL writes.
      }
      if (events.length >= limit) {
        return events;
      }
    }
  }
  return events;
}

async function listFilesRecursive(directory, predicate) {
  if (!(await pathExists(directory))) {
    return [];
  }
  const files = [];

  async function walk(current) {
    let entries = [];
    try {
      entries = await fs.readdir(current, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const target = path.join(current, entry.name);
      if (entry.isDirectory()) {
        await walk(target);
      } else if (entry.isFile() && (!predicate || predicate(target))) {
        files.push(target);
      }
    }
  }

  await walk(directory);
  return files;
}

async function readJsonlRecords(file) {
  let text = "";
  try {
    text = await fs.readFile(file, "utf8");
  } catch (error) {
    return [{ type: "corrupt", path: file, error: error.message }];
  }

  const records = [];
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) {
      continue;
    }
    try {
      records.push({ ...JSON.parse(line), path: file });
    } catch {
      records.push({ type: "corrupt", raw: line, path: file });
    }
  }
  return records;
}

function summarizeRuntimeStats(events) {
  const count = events.length;
  const failures = events.filter((event) => Number(event.exitCode || 0) !== 0).length;
  const durations = events
    .map((event) => Number(event.durationMs))
    .filter((value) => Number.isFinite(value));
  const totalMs = durations.reduce((sum, value) => sum + value, 0);
  const maxMs = durations.length ? Math.max(...durations) : 0;
  const lastEventAt = newestDate(events.map((event) => normalizeDate(event.finishedAt || event.startedAt)));

  const byCommand = new Map();
  for (const event of events) {
    const command = event.command || "unknown";
    const item = byCommand.get(command) || {
      command,
      count: 0,
      failures: 0,
      totalMs: 0,
      maxMs: 0,
      lastStartedAt: null
    };
    item.count += 1;
    item.failures += Number(event.exitCode || 0) !== 0 ? 1 : 0;
    item.totalMs += Number(event.durationMs || 0);
    item.maxMs = Math.max(item.maxMs, Number(event.durationMs || 0));
    item.lastStartedAt = newestDate([item.lastStartedAt, normalizeDate(event.startedAt)]);
    byCommand.set(command, item);
  }

  const commands = Array.from(byCommand.values())
    .map((item) => ({
      ...item,
      avgMs: item.count ? Math.round((item.totalMs / item.count) * 1000) / 1000 : 0,
      totalMs: Math.round(item.totalMs * 1000) / 1000,
      maxMs: Math.round(item.maxMs * 1000) / 1000
    }))
    .sort((a, b) => b.totalMs - a.totalMs);

  return {
    count,
    failures,
    avgMs: count && durations.length ? Math.round((totalMs / durations.length) * 1000) / 1000 : 0,
    maxMs: Math.round(maxMs * 1000) / 1000,
    lastEventAt,
    commands,
    recent: events.slice(0, 30)
  };
}

function layerToScope(layer) {
  const scopes = {
    L0: "scratch",
    L1: "task",
    L2: "project",
    L3: "pattern",
    L4: "archive"
  };
  return scopes[layer] || "event";
}

function stageStatus(stage) {
  const enterAt = normalizeDate(stage.last_enter_at);
  const exitAt = normalizeDate(stage.last_exit_at);
  const writeAt = normalizeDate(stage.last_write_at);
  const active = Boolean(enterAt && (!exitAt || new Date(enterAt).getTime() > new Date(exitAt).getTime()));
  const needsWrite = Boolean(active && (!writeAt || new Date(writeAt).getTime() < new Date(enterAt).getTime()));
  return {
    active,
    needsWrite,
    lastActivityAt: newestDate([enterAt, exitAt, writeAt])
  };
}

function memoryStatusFrom({ exists, corruptCount = 0, activeScopes = [], blockedScopes = [], total = 0 }) {
  if (!exists) {
    return "missing";
  }
  if (corruptCount > 0) {
    return "invalid";
  }
  if (blockedScopes.length > 0) {
    return "blocked";
  }
  if (activeScopes.length > 0) {
    return "active";
  }
  if (total > 0) {
    return "ready";
  }
  return "empty";
}

function taskMatchValues(task) {
  return new Set(
    [
      task?.id,
      task?.workItemId,
      task?.workItemName,
      task?.workItemKey,
      task?.currentStory,
      task?.currentTask
    ]
      .filter(Boolean)
      .map((value) => String(value))
  );
}

function memoryRecordMatchesTask(record, task) {
  const values = taskMatchValues(task);
  const story = record.story ? String(record.story) : "";
  const taskId = record.task ? String(record.task) : "";
  if (story && values.has(story)) {
    return true;
  }
  if (taskId && values.has(taskId)) {
    return true;
  }
  return false;
}

function summarizeMemorySlice(memorySummary, records, stages) {
  const lastMemoryAt = newestDate([
    ...records.map((record) => normalizeDate(record.timestamp)),
    ...stages.map((stage) => stage.lastActivityAt)
  ]);
  const activeScopes = stages.filter((stage) => stage.active);
  const blockedScopes = stages.filter((stage) => stage.needsWrite);
  const corruptCount = records.filter((record) => record.type === "corrupt").length;
  return {
    status: memoryStatusFrom({
      exists: memorySummary.exists,
      corruptCount,
      activeScopes,
      blockedScopes,
      total: records.length
    }),
    total: records.length,
    memoryEntries: records.filter((record) => record.type === "memory").length,
    activeScopeCount: activeScopes.length,
    blockedScopeCount: blockedScopes.length,
    lastMemoryAt,
    recent: records
      .slice()
      .sort((a, b) => new Date(b.timestamp || 0).getTime() - new Date(a.timestamp || 0).getTime())
      .slice(0, 12),
    activeScopes: activeScopes.slice(0, 12),
    blockedScopes: blockedScopes.slice(0, 12)
  };
}

async function readMemorySummary(aeDir) {
  const memoryRoot = path.join(aeDir, "memory");
  const exists = await pathExists(memoryRoot);
  const jsonlFiles = await listFilesRecursive(memoryRoot, (file) => file.endsWith(".jsonl"));
  const stageFiles = await listFilesRecursive(path.join(memoryRoot, ".stage"), (file) => file.endsWith(".json"));
  const records = [];
  const stages = [];

  for (const file of jsonlFiles) {
    records.push(...(await readJsonlRecords(file)));
  }

  for (const file of stageFiles) {
    const result = await readJsonFile(file);
    const value = result.value || {};
    const status = stageStatus(value);
    stages.push({
      id: path.basename(file, ".json"),
      path: file,
      phase: value.phase || null,
      story: value.story || null,
      task: value.task || null,
      lastEnterAt: normalizeDate(value.last_enter_at),
      lastWriteAt: normalizeDate(value.last_write_at),
      lastExitAt: normalizeDate(value.last_exit_at),
      active: status.active,
      needsWrite: status.needsWrite,
      lastActivityAt: status.lastActivityAt,
      error: result.error
    });
  }

  records.sort((a, b) => new Date(a.timestamp || 0).getTime() - new Date(b.timestamp || 0).getTime());

  const byLayer = {};
  const byScope = {};
  const byPhase = {};
  for (const record of records) {
    const layer = record.layer || (record.type === "memory" ? "unknown" : "event");
    const scope = record.memoryScope || layerToScope(layer);
    const phase = record.phase || "unknown";
    byLayer[layer] = (byLayer[layer] || 0) + 1;
    byScope[scope] = (byScope[scope] || 0) + 1;
    byPhase[phase] = (byPhase[phase] || 0) + 1;
  }

  const activeScopes = stages.filter((stage) => stage.active);
  const blockedScopes = stages.filter((stage) => stage.needsWrite);
  const corruptCount = records.filter((record) => record.type === "corrupt").length + stages.filter((stage) => stage.error).length;
  const lastMemoryAt = newestDate([
    ...records.map((record) => normalizeDate(record.timestamp)),
    ...stages.map((stage) => stage.lastActivityAt)
  ]);

  return {
    exists,
    root: memoryRoot,
    status: memoryStatusFrom({ exists, corruptCount, activeScopes, blockedScopes, total: records.length }),
    total: records.length,
    memoryEntries: records.filter((record) => record.type === "memory").length,
    eventEntries: records.filter((record) => record.type !== "memory").length,
    projectMemoryCount: records.filter((record) => record.layer === "L2" || record.memoryScope === "project").length,
    taskMemoryCount: records.filter((record) => record.layer === "L1" || record.memoryScope === "task").length,
    scratchEventCount: records.filter((record) => record.layer === "L0" || record.memoryScope === "scratch").length,
    activeScopeCount: activeScopes.length,
    blockedScopeCount: blockedScopes.length,
    corruptCount,
    byLayer,
    byScope,
    byPhase,
    lastMemoryAt,
    recent: records.slice().reverse().slice(0, 20),
    activeScopes: activeScopes.slice(0, 20),
    blockedScopes: blockedScopes.slice(0, 20),
    stages,
    records
  };
}

function summarizeTaskMemory(task, memorySummary) {
  if (!memorySummary?.exists) {
    return summarizeMemorySlice(memorySummary || { exists: false }, [], []);
  }
  const records = memorySummary.records.filter((record) => {
    const isTaskMemory = record.layer === "L1" || record.memoryScope === "task" || record.layer === "L0";
    return isTaskMemory && memoryRecordMatchesTask(record, task);
  });
  const stages = memorySummary.stages.filter((stage) => memoryRecordMatchesTask(stage, task));
  return summarizeMemorySlice(memorySummary, records, stages);
}

function deriveStatus({ state, hasState, errors, lastActivityAt, runtimeSummary }) {
  if (errors.length || !hasState) {
    return "invalid";
  }
  const phase = state?.phase;
  if (phase === "paused") {
    return "paused";
  }
  if (phase === "completed") {
    return "completed";
  }
  if (runtimeSummary?.failures > 0 && runtimeSummary?.recent?.[0]?.exitCode !== 0) {
    return "blocked";
  }
  if (Array.isArray(state?.activeAgents) && state.activeAgents.length > 0) {
    return "active";
  }
  if (!lastActivityAt) {
    return "idle";
  }
  const ageMs = Date.now() - new Date(lastActivityAt).getTime();
  if (ageMs >= 0 && ageMs <= 24 * 60 * 60 * 1000) {
    return "active";
  }
  return "idle";
}

function workspaceId(rootPath) {
  return Buffer.from(rootPath).toString("base64url");
}

function nameFromWorkItemKeySegment(value) {
  const text = String(value || "").trim();
  return text ? text.replace(/[-_]+/g, " ") : null;
}

function parseWorkItemKey(value) {
  const key = String(value || "").trim();
  if (!key) {
    return { workItemId: null, workItemName: null, workItemKey: null };
  }

  const splitIndex = key.indexOf("--");
  if (splitIndex > 0) {
    return {
      workItemId: key.slice(0, splitIndex).trim() || null,
      workItemName: nameFromWorkItemKeySegment(key.slice(splitIndex + 2)),
      workItemKey: key
    };
  }

  return { workItemId: key, workItemName: null, workItemKey: key };
}

function resolveWorkItemIdentity(state = {}, fallbackKey = null) {
  const parsed = parseWorkItemKey(state.workItemKey || fallbackKey);
  return {
    workItemId: state.workItemId || parsed.workItemId || null,
    workItemName: state.workItemName || parsed.workItemName || null,
    workItemKey: state.workItemKey || parsed.workItemKey || null
  };
}

function compactActiveEntry(entry) {
  const id = entry.id || entry.workItemKey || entry.workItem || entry.txnName || entry.taskId || entry.storyId || entry.agentId;
  if (!id) {
    return null;
  }
  const identity = resolveWorkItemIdentity(entry, entry.workItem || entry.workItemKey || null);
  return {
    id: String(id),
    workItemId: identity.workItemId,
    workItemName: identity.workItemName || entry.name || null,
    workItemKey: identity.workItemKey,
    activeStatePath: entry.activeStatePath || null,
    statePath: entry.statePath || null,
    source: entry.source || "state",
    status: entry.status || "active",
    phase: entry.phase || null,
    agentId: entry.agentId || null,
    role: entry.role || entry.agentRole || null,
    skill: entry.skill || null,
    lastActivityAt: entry.lastActivityAt || entry.startedAt || entry.updatedAt || entry.acquiredAt || null,
    summary: entry.summary || entry.description || entry.reason || null
  };
}

function addActiveEntry(activeMap, entry) {
  const compact = compactActiveEntry(entry);
  if (!compact) {
    return;
  }
  const existing = activeMap.get(compact.id);
  if (!existing) {
    activeMap.set(compact.id, compact);
    return;
  }
  activeMap.set(compact.id, {
    ...existing,
    ...Object.fromEntries(Object.entries(compact).filter(([, value]) => value !== null && value !== undefined && value !== "")),
    source: Array.from(new Set([existing.source, compact.source].filter(Boolean).join("+").split("+"))).join("+")
  });
}

function deriveActiveWorkItems(state, workItems = []) {
  const activeMap = new Map();

  for (const key of ["activeWorkItem", "currentWorkItem", "currentStory", "currentTask"]) {
    if (state?.[key]) {
      const isWorkItemField = key === "activeWorkItem" || key === "currentWorkItem";
      addActiveEntry(activeMap, {
        id: state.workItemKey || state[key],
        workItemId: state.workItemId,
        workItemName: state.workItemName,
        workItemKey: state.workItemKey || (isWorkItemField ? state[key] : null),
        activeStatePath: state.activeStatePath,
        source: key,
        status: state.phase === "paused" ? "paused" : state.phase === "completed" ? "completed" : "active",
        phase: state.phase,
        lastActivityAt: newestDate(collectStateTimestamps(state))
      });
    }
  }

  for (const agent of Array.isArray(state?.activeAgents) ? state.activeAgents : []) {
    addActiveEntry(activeMap, {
      ...agent,
      id: agent.workItemKey || agent.txnName || agent.workItemId || agent.taskId || agent.storyId || agent.agentId,
      source: "activeAgents",
      status: agent.status || "active"
    });
  }

  for (const item of workItems) {
    if (!["completed", "invalid"].includes(item.status)) {
      addActiveEntry(activeMap, {
        id: item.id,
        workItemId: item.workItemId,
        workItemName: item.workItemName,
        workItemKey: item.workItemKey,
        source: "workItems",
        status: item.status,
        phase: item.phase,
        lastActivityAt: item.lastActivityAt
      });
    }
  }

  return Array.from(activeMap.values()).sort((a, b) => {
    const statusOrder = { active: 0, blocked: 1, paused: 2, idle: 3, unknown: 4 };
    const byStatus = (statusOrder[a.status] ?? 99) - (statusOrder[b.status] ?? 99);
    if (byStatus !== 0) {
      return byStatus;
    }
    return String(a.id).localeCompare(String(b.id), "zh-Hans-CN");
  });
}

function compactMemorySummary(memorySummary) {
  if (!memorySummary) {
    return {
      exists: false,
      status: "missing",
      total: 0,
      memoryEntries: 0,
      eventEntries: 0,
      projectMemoryCount: 0,
      taskMemoryCount: 0,
      scratchEventCount: 0,
      activeScopeCount: 0,
      blockedScopeCount: 0,
      corruptCount: 0,
      byLayer: {},
      byScope: {},
      byPhase: {},
      lastMemoryAt: null,
      recent: [],
      activeScopes: [],
      blockedScopes: []
    };
  }
  const { records, stages, ...compact } = memorySummary;
  return compact;
}

function taskDisplayName(task) {
  return task.workItemName || task.workItemKey || task.currentTask || task.currentStory || task.id;
}

function deriveTaskList(workItems = [], activeWorkItems = [], memorySummary = null) {
  const tasks = new Map();

  function addTask(source, item) {
    const id = item.id || item.workItemKey || item.currentTask || item.currentStory || item.agentId;
    if (!id) {
      return;
    }
    const existing = tasks.get(String(id)) || {};
    const task = {
      ...existing,
      ...Object.fromEntries(Object.entries(item).filter(([, value]) => value !== null && value !== undefined && value !== "")),
      id: String(id),
      source: Array.from(new Set([existing.source, source].filter(Boolean).join("+").split("+"))).join("+")
    };
    task.label = taskDisplayName(task);
    tasks.set(task.id, task);
  }

  for (const item of workItems) {
    addTask("workItems", item);
  }
  for (const item of activeWorkItems) {
    addTask("active", item);
  }

  const result = Array.from(tasks.values()).map((task) => ({
    ...task,
    memory: summarizeTaskMemory(task, memorySummary)
  }));

  result.sort((a, b) => {
    const statusOrder = { active: 0, blocked: 1, paused: 2, idle: 3, completed: 4, invalid: 5, unknown: 6 };
    const byStatus = (statusOrder[a.status] ?? 99) - (statusOrder[b.status] ?? 99);
    if (byStatus !== 0) {
      return byStatus;
    }
    return String(a.label || a.id).localeCompare(String(b.label || b.id), "zh-Hans-CN");
  });
  return result;
}

async function summarizeWorkspace(rootPath) {
  const absoluteRoot = path.resolve(rootPath);
  const aeDir = path.join(absoluteRoot, ".ae-sdd");
  const statePath = path.join(aeDir, "state.json");
  const errors = [];
  const hasAeDir = await pathExists(aeDir);
  const hasState = await pathExists(statePath);

  const config = await readConfig(aeDir);
  if (config.error) {
    errors.push(config.error);
  }

  let state = null;
  if (hasState) {
    const stateResult = await readJsonFile(statePath);
    state = stateResult.value;
    if (stateResult.error) {
      errors.push(stateResult.error);
    }
  }

  const stateStat = await safeStat(statePath);
  const runtimeEvents = await readRuntimeEvents(aeDir, 80);
  const runtimeSummary = summarizeRuntimeStats(runtimeEvents);
  const memorySummary = await readMemorySummary(aeDir);
  const workItems = await readWorkItems(absoluteRoot);
  const activeWorkItems = deriveActiveWorkItems(state || {}, workItems);
  const tasks = deriveTaskList(workItems, activeWorkItems, memorySummary);
  const timestamps = [
    ...collectStateTimestamps(state || {}),
    normalizeDate(runtimeSummary.lastEventAt),
    normalizeDate(memorySummary.lastMemoryAt),
    stateStat ? normalizeDate(stateStat.mtime.toISOString()) : null
  ];
  const lastActivityAt = newestDate(timestamps);
  const projectKey = config.value.projectKey || state?.projectKey || path.basename(absoluteRoot);
  const derivedStatus = deriveStatus({ state, hasState, errors, lastActivityAt, runtimeSummary });
  const status = memorySummary.status === "blocked" && !["invalid", "completed"].includes(derivedStatus)
    ? "blocked"
    : derivedStatus;
  const progress = phaseProgress(state || {});
  const rootWorkItem = resolveWorkItemIdentity(
    state || {},
    state?.activeWorkItem || state?.currentWorkItem || state?.workItemKey || null
  );

  return {
    id: workspaceId(absoluteRoot),
    rootPath: absoluteRoot,
    name: projectKey || path.basename(absoluteRoot),
    projectKey,
    phase: state?.phase || "unknown",
    scale: state?.scale || null,
    entryNode: state?.entryNode || null,
    currentStory: state?.currentStory || null,
    currentTask: state?.currentTask || null,
    activeWorkItem: state?.activeWorkItem || state?.currentWorkItem || rootWorkItem.workItemKey || null,
    activeStatePath: state?.activeStatePath || null,
    workItemId: rootWorkItem.workItemId,
    workItemName: rootWorkItem.workItemName,
    workItemKey: rootWorkItem.workItemKey,
    activeAgentCount: Array.isArray(state?.activeAgents) ? state.activeAgents.length : 0,
    taskCount: tasks.length,
    status,
    lastActivityAt,
    hasAeDir,
    hasState,
    hasConfig: Boolean(config.path),
    statePath: hasState ? statePath : null,
    configPath: config.path,
    runtimeEventCount: runtimeSummary.count,
    runtimeFailureCount: runtimeSummary.failures,
    memoryStatus: memorySummary.status,
    memoryTotal: memorySummary.total,
    memoryProjectCount: memorySummary.projectMemoryCount,
    memoryTaskCount: memorySummary.taskMemoryCount,
    memoryActiveScopeCount: memorySummary.activeScopeCount,
    memoryBlockedScopeCount: memorySummary.blockedScopeCount,
    memoryLastAt: memorySummary.lastMemoryAt,
    memory: compactMemorySummary(memorySummary),
    tasks,
    progress,
    phaseTimeline: phaseTimeline(state || {}),
    errors
  };
}

async function scanForWorkspaces(rootPath, options = {}) {
  const maxDepth = Number.isInteger(options.maxDepth) ? options.maxDepth : 8;
  const maxWorkspaces = Number.isInteger(options.maxWorkspaces) ? options.maxWorkspaces : 500;
  const root = path.resolve(rootPath);
  const found = [];
  const visited = new Set();

  async function walk(directory, depth) {
    if (found.length >= maxWorkspaces || depth < 0) {
      return;
    }
    const absolute = path.resolve(directory);
    if (visited.has(absolute)) {
      return;
    }
    visited.add(absolute);

    const aeDir = path.join(absolute, ".ae-sdd");
    if (await pathExists(aeDir)) {
      found.push(await summarizeWorkspace(absolute));
      return;
    }

    let entries = [];
    try {
      entries = await fs.readdir(absolute, { withFileTypes: true });
    } catch {
      return;
    }

    for (const entry of entries) {
      if (!entry.isDirectory() || entry.isSymbolicLink() || SKIP_DIRS.has(entry.name)) {
        continue;
      }
      await walk(path.join(absolute, entry.name), depth - 1);
      if (found.length >= maxWorkspaces) {
        break;
      }
    }
  }

  await walk(root, maxDepth);

  found.sort((a, b) => {
    const statusOrder = { active: 0, blocked: 1, paused: 2, idle: 3, completed: 4, invalid: 5, unknown: 6 };
    const byStatus = (statusOrder[a.status] ?? 99) - (statusOrder[b.status] ?? 99);
    if (byStatus !== 0) {
      return byStatus;
    }
    return String(a.name).localeCompare(String(b.name), "zh-Hans-CN");
  });

  return {
    rootPath: root,
    scannedAt: new Date().toISOString(),
    workspaces: found
  };
}

async function readWorkItems(rootPath) {
  const directory = path.join(rootPath, ".auto-engineering");
  if (!(await pathExists(directory))) {
    return [];
  }

  let entries = [];
  try {
    entries = await fs.readdir(directory, { withFileTypes: true });
  } catch {
    return [];
  }

  const items = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const statePath = path.join(directory, entry.name, "state.json");
    if (!(await pathExists(statePath))) {
      continue;
    }
    const result = await readJsonFile(statePath);
    const state = result.value || {};
    const stat = await safeStat(statePath);
    const lastActivityAt = newestDate([
      ...collectStateTimestamps(state),
      stat ? normalizeDate(stat.mtime.toISOString()) : null
    ]);
    const progress = phaseProgress(state);
    const identity = resolveWorkItemIdentity(state, entry.name);
    items.push({
      id: identity.workItemKey || entry.name,
      workItemId: identity.workItemId,
      workItemName: identity.workItemName,
      workItemKey: identity.workItemKey || entry.name,
      statePath,
      phase: state.phase || "unknown",
      scale: state.scale || null,
      currentStory: state.currentStory || null,
      currentTask: state.currentTask || null,
      activeAgentCount: Array.isArray(state.activeAgents) ? state.activeAgents.length : 0,
      history: Array.isArray(state.history) ? state.history : [],
      events: Array.isArray(state.events) ? state.events : [],
      state,
      status: deriveStatus({
        state,
        hasState: result.ok,
        errors: result.error ? [result.error] : [],
        lastActivityAt,
        runtimeSummary: null
      }),
      lastActivityAt,
      progress,
      phaseTimeline: phaseTimeline(state),
      error: result.error
    });
  }

  items.sort((a, b) => String(a.id).localeCompare(String(b.id), "zh-Hans-CN"));
  return items;
}

async function loadWorkspaceDetail(rootPath) {
  const absoluteRoot = path.resolve(rootPath);
  const aeDir = path.join(absoluteRoot, ".ae-sdd");
  const summary = await summarizeWorkspace(absoluteRoot);
  const stateResult = summary.hasState ? await readJsonFile(summary.statePath) : { ok: false, value: null, error: "state.json missing" };
  const state = stateResult.value || {};
  const runtimeEvents = await readRuntimeEvents(aeDir, 180);
  const runtimeStats = summarizeRuntimeStats(runtimeEvents);
  const memorySummary = await readMemorySummary(aeDir);
  const workItems = await readWorkItems(absoluteRoot);
  const activeWorkItems = deriveActiveWorkItems(state, workItems);
  const tasks = deriveTaskList(workItems, activeWorkItems, memorySummary);
  const history = Array.isArray(state.history) ? state.history : [];
  const events = Array.isArray(state.events) ? state.events : [];

  return {
    summary,
    state,
    history,
    events,
    workItems,
    activeWorkItems,
    tasks,
    memory: compactMemorySummary(memorySummary),
    phaseTimeline: phaseTimeline(state),
    runtimeStats,
    loadedAt: new Date().toISOString()
  };
}

module.exports = {
  PHASE_FLOWS,
  parseWorkItemKey,
  parseYamlLite,
  summarizeWorkspace,
  scanForWorkspaces,
  loadWorkspaceDetail,
  summarizeRuntimeStats,
  phaseProgress,
  phaseTimeline,
  deriveActiveWorkItems
};
