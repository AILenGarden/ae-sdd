---
name: postman-tool
description: Postman platform adapter for ae-sdd. When the user provides a test-env URL, ae-sdd builds AC scenarios into a Postman collection, runs the monitor, and records http-external-supplemental evidence. Does not replace the L2 local HTTP main chain.
---

# Postman Tool Skill

## 1. Purpose

The Postman Tool lets ae-sdd validate HTTP behavior of a deployed test-env URL
through the Postman platform, without the Agent inventing request details or
silently faking responses.

AI may draft request definitions from AC scenarios. The tool owns:

- resolving the authenticated user and target workspace
- building AC scenarios into a Postman collection
- running the monitor and collecting run results
- returning structured supplemental evidence

## 2. Profile Location

Postman credentials are owned by the PostmanMCP installed in the Agent; they
never enter `source/`, `dist/`, or reports. Workspace and collection config
lives in:

```text
<project>/.ae-sdd/secrets/postman.local.json
```

The schema is defined in
[`postman-profile.schema.md`](../../standards/toolsets/postman-profile.schema.md).

## 3. Capability

This skill declares how ae-sdd calls the PostmanMCP tools. It does not own a
`ae-sdd` CLI subcommand; Postman is an MCP-backed toolset.

| Step | PostmanMCP tools | Output |
|---|---|---|
| resolve identity/workspace | `getAuthenticatedUser`, `getWorkspaces` | ownerId, teamId, workspaceId |
| build collection | `createCollection`, `createCollectionRequest` | collectionId, requestId[] |
| run + collect | `runMonitor`, `getMonitorRunResults`, `listRunsForExecution` | runId, stats, failures |
| reconcile | `getCollection` (model=full) | request definition audit |
| optional mock | `createMock`, `createMockServerResponse` | mockId, 5xx fault injection |

AC scenarios fed into `createCollectionRequest` must come from the
[`http-scenario-strategy`](../../standards/testing/be-http-scenario-strategy.md)
derivation chain, not a fixed CRUD list.

## 4. Phase Rules

| Phase | Allowed Postman Usage |
|---|---|
| RA | none |
| Design | none |
| CodingPlan | none |
| Coding | none |
| Review (Test) | optional supplemental HTTP verification against a user-provided test-env URL |

## 5. Hard Rules

- No Postman token in repo files or reports.
- No loopback URL; Postman runs from cloud regions and cannot reach local ports.
- Do not replace the L2 local HTTP main chain (`internalMocks=false`).
- If PostmanMCP is unavailable, return `blocked`; do not fall back to curl or
  hand-written HTTP.
- Run results must be archived as real `http-external-supplemental` evidence;
  never hand-edit or invent results.
- Postman evidence does not count toward L2/L4 dual-stage completion.
