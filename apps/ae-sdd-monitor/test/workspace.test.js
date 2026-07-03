const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("fs/promises");
const os = require("os");
const path = require("path");

const { deriveActiveWorkItems, loadWorkspaceDetail, parseYamlLite, phaseTimeline, scanForWorkspaces } = require("../src/workspace");

async function writeJson(file, value) {
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

test("parseYamlLite reads shallow and nested config values", () => {
  const result = parseYamlLite(`
projectKey: demo
automation:
  enabled: false
  reviewerTier: 3
`);
  assert.equal(result.projectKey, "demo");
  assert.equal(result.automation.enabled, false);
  assert.equal(result.automation.reviewerTier, 3);
});

test("scanForWorkspaces discovers ae-sdd workspace summaries", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "ae-sdd-monitor-"));
  const workspace = path.join(root, "demo-project");
  await fs.mkdir(path.join(workspace, ".ae-sdd"), { recursive: true });
  await fs.writeFile(path.join(workspace, ".ae-sdd", "config.yaml"), "projectKey: demo\n", "utf8");
  await writeJson(path.join(workspace, ".ae-sdd", "state.json"), {
    version: "1",
    projectKey: "demo",
    phase: "coding-process",
    scale: "小",
    currentStory: "STORY-001",
    history: [{ phase: "initialized", timestamp: "2026-07-03T01:00:00Z" }]
  });
  await writeJson(path.join(workspace, ".ae-sdd", "runtime-stats", "2026-07-03.jsonl"), {});
  await fs.writeFile(
    path.join(workspace, ".ae-sdd", "runtime-stats", "2026-07-03.jsonl"),
    `${JSON.stringify({
      command: "state read",
      exitCode: 0,
      durationMs: 1.5,
      startedAt: "2026-07-03T02:00:00Z",
      finishedAt: "2026-07-03T02:00:01Z"
    })}\n`,
    "utf8"
  );

  const result = await scanForWorkspaces(root);
  assert.equal(result.workspaces.length, 1);
  assert.equal(result.workspaces[0].projectKey, "demo");
  assert.equal(result.workspaces[0].phase, "coding-process");
  assert.equal(result.workspaces[0].status, "active");
  assert.equal(result.workspaces[0].runtimeEventCount, 1);
});

test("loadWorkspaceDetail includes work item state and runtime stats", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "ae-sdd-monitor-detail-"));
  const workspace = path.join(root, "workspace");
  await writeJson(path.join(workspace, ".ae-sdd", "state.json"), {
    version: "1",
    projectKey: "workspace",
    phase: "completed",
    history: [{ phase: "completed", timestamp: "2026-07-03T03:00:00Z" }]
  });
  await writeJson(path.join(workspace, ".auto-engineering", "BUG-1", "state.json"), {
    version: "1",
    workItemId: "BUG-1",
    workItemName: "Login timeout fix",
    workItemKey: "BUG-1--Login-timeout-fix",
    phase: "completed",
    scale: "微"
  });
  await fs.mkdir(path.join(workspace, ".ae-sdd", "runtime-stats"), { recursive: true });
  await fs.writeFile(
    path.join(workspace, ".ae-sdd", "runtime-stats", "2026-07-03.jsonl"),
    `${JSON.stringify({
      command: "gates check",
      exitCode: 1,
      durationMs: 2,
      startedAt: "2026-07-03T03:00:00Z",
      finishedAt: "2026-07-03T03:00:02Z"
    })}\n`,
    "utf8"
  );

  const detail = await loadWorkspaceDetail(workspace);
  assert.equal(detail.summary.projectKey, "workspace");
  assert.equal(detail.summary.status, "completed");
  assert.equal(detail.workItems.length, 1);
  assert.equal(detail.workItems[0].id, "BUG-1--Login-timeout-fix");
  assert.equal(detail.workItems[0].workItemId, "BUG-1");
  assert.equal(detail.workItems[0].workItemName, "Login timeout fix");
  assert.equal(detail.runtimeStats.failures, 1);
});

test("phaseTimeline describes current node in the scale-specific flow", () => {
  const timeline = phaseTimeline({
    phase: "code-reviewed",
    scale: "微"
  });
  assert.equal(timeline.scale, "微");
  assert.equal(timeline.current, "code-reviewed");
  assert.equal(timeline.nodes.length, 8);
  assert.equal(timeline.nodes[6].phase, "code-reviewed");
  assert.equal(timeline.nodes[6].status, "current");
  assert.equal(timeline.nodes[7].status, "pending");
});

test("loadWorkspaceDetail summarizes multiple active work items", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "ae-sdd-monitor-active-"));
  const workspace = path.join(root, "workspace");
  await writeJson(path.join(workspace, ".ae-sdd", "state.json"), {
    version: "1",
    projectKey: "workspace",
    phase: "coding",
    currentTask: "TASK-root",
    activeAgents: [
      {
        agentId: "agent-1",
        txnName: "TASK-agent",
        skill: "coding-skill",
        startedAt: "2026-07-03T04:00:00Z"
      }
    ]
  });
  await writeJson(path.join(workspace, ".auto-engineering", "BUG-1", "state.json"), {
    version: "1",
    workItemId: "BUG-1",
    workItemName: "Login timeout fix",
    workItemKey: "BUG-1--Login-timeout-fix",
    phase: "coding-process",
    scale: "微"
  });

  const detail = await loadWorkspaceDetail(workspace);
  const ids = detail.activeWorkItems.map((item) => item.id);
  assert.deepEqual(ids.sort(), ["BUG-1--Login-timeout-fix", "TASK-agent", "TASK-root"].sort());
  const bug = detail.activeWorkItems.find((item) => item.workItemId === "BUG-1");
  assert.equal(bug.workItemName, "Login timeout fix");
  assert.equal(detail.summary.activeAgentCount, 1);
});

test("deriveActiveWorkItems merges duplicate state and agent sources", () => {
  const items = deriveActiveWorkItems(
    {
      phase: "coding",
      currentTask: "TASK-1",
      activeAgents: [{ agentId: "agent-1", taskId: "TASK-1", skill: "coding-skill" }]
    },
    []
  );
  assert.equal(items.length, 1);
  assert.equal(items[0].id, "TASK-1");
  assert.match(items[0].source, /currentTask/);
  assert.match(items[0].source, /activeAgents/);
});
