export type Status = "active" | "idle" | "paused" | "completed" | "blocked" | "invalid" | "unknown" | string;

export type TabKey = "overview" | "timeline" | "memory" | "workitems" | "performance" | "raw";

export interface MemoryScope {
  id?: string;
  phase?: string;
  story?: string;
  task?: string;
  needsWrite?: boolean;
  lastActivityAt?: string | null;
}

export interface MemoryRecord {
  timestamp?: string;
  memoryScope?: string;
  layer?: string;
  type?: string;
  kind?: string;
  summary?: string;
  note?: string;
  reason?: string;
  evidence?: string[];
  path?: string;
}

export interface MemorySummary {
  status?: Status;
  root?: string;
  total?: number;
  memoryEntries?: number;
  projectMemoryCount?: number;
  taskMemoryCount?: number;
  activeScopeCount?: number;
  blockedScopeCount?: number;
  lastMemoryAt?: string | null;
  activeScopes?: MemoryScope[];
  blockedScopes?: MemoryScope[];
  recent?: MemoryRecord[];
}

export interface ProgressSummary {
  percent?: number;
  index?: number;
  total?: number;
}

export interface PhaseNode {
  index?: number | string;
  phase?: string;
  label?: string;
  description?: string;
  status?: string;
}

export interface PhaseTimeline {
  scale?: string;
  nodes?: PhaseNode[];
}

export interface TimelineEvent {
  phase?: string;
  event?: string;
  node?: string;
  skill?: string;
  txnName?: string;
  reason?: string;
  by?: string;
  timestamp?: string;
  ts?: string;
}

export interface WorkItem {
  id?: string;
  label?: string;
  workItemId?: string | null;
  workItemName?: string | null;
  workItemKey?: string | null;
  currentStory?: string | null;
  currentTask?: string | null;
  activeWorkItem?: string | null;
  activeStatePath?: string | null;
  statePath?: string | null;
  source?: string | null;
  skill?: string | null;
  role?: string | null;
  agentId?: string | null;
  summary?: string | null;
  phase?: string;
  scale?: string;
  status?: Status;
  progress?: ProgressSummary;
  memory?: MemorySummary;
  lastActivityAt?: string | null;
  activeAgentCount?: number;
  history?: TimelineEvent[];
  events?: TimelineEvent[];
  phaseTimeline?: PhaseTimeline;
  state?: unknown;
}

export interface WorkspaceSummary extends WorkItem {
  rootPath: string;
  name: string;
  projectKey?: string;
  entryNode?: string | null;
  memoryStatus?: Status;
  configPath?: string | null;
  errors?: string[];
  tasks?: WorkItem[];
}

export interface RuntimeCommandStats {
  command?: string;
  count?: number;
  failures?: number;
  avgMs?: number;
  maxMs?: number;
  lastStartedAt?: string | null;
}

export interface RuntimeStats {
  count?: number;
  failures?: number;
  commands?: RuntimeCommandStats[];
}

export interface WorkspaceDetail {
  summary: WorkspaceSummary;
  tasks?: WorkItem[];
  workItems?: WorkItem[];
  activeWorkItems?: WorkItem[];
  memory?: MemorySummary;
  runtimeStats?: RuntimeStats;
  phaseTimeline?: PhaseTimeline;
  history?: TimelineEvent[];
  events?: TimelineEvent[];
  state?: unknown;
  loadedAt?: string;
}

export interface ScanResult {
  rootPath: string;
  workspaces: WorkspaceSummary[];
}

export interface Preferences {
  rootPath?: string;
  selectedRoot?: string;
  selectedTaskId?: string;
  collapsedRoots?: string[];
  autoRefresh?: boolean;
  theme?: "light" | "dark";
}

export interface WorkspaceWatchPayload {
  rootPath?: string;
  eventType?: string;
  path?: string;
  at?: string;
}

export interface MonitorApi {
  loadPreferences: () => Promise<Preferences>;
  savePreferences: (preferences: Preferences) => Promise<Preferences>;
  chooseDirectory: (defaultPath?: string) => Promise<string | null>;
  scanWorkspaces: (rootPath: string) => Promise<ScanResult>;
  loadWorkspaceDetail: (rootPath: string) => Promise<WorkspaceDetail>;
  watchWorkspaces?: (rootPath: string) => Promise<{ rootPath?: string; recursive?: boolean }>;
  unwatchWorkspaces?: () => Promise<boolean>;
  onWorkspaceFilesChanged?: (callback: (payload: WorkspaceWatchPayload) => void) => () => void;
  openPath: (targetPath: string) => Promise<string>;
  windowControl: (action: "close" | "minimize" | "toggle-maximize") => Promise<boolean>;
}

declare global {
  interface Window {
    monitorApi: MonitorApi;
  }
}
