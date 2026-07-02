# Agent Enablement Layer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Nubase directly usable by Codex, Claude Code, Cursor, IDEA, and other agents as a memory, database, auth, and storage backend with minimal user setup.

**Architecture:** Extend the existing Spring AI MCP server from database-only tools into a Nubase Agent Enablement Layer. Keep REST APIs as the stable backend surface, expose agent-friendly MCP tools/resources/prompts on top, and add Studio/CLI configuration outputs so users can connect agents with one copy/paste action.

**Tech Stack:** Spring Boot, Spring AI MCP server, existing Nubase Memory/Auth/Storage/PostgREST services, JUnit 5, Mockito, MockMvc, Next.js Studio.

---

## Milestone 1: Memory MCP Minimal Loop

Ship the smallest useful agent loop: retrieve context, write memory, search memory. This makes Nubase valuable to coding agents before broader Auth/Storage integration.

### Task 1: Register Multiple MCP Tool Objects

**Files:**
- Modify: `src/main/java/ai/nubase/mcp/tools/McpConfig.java`
- Test: `src/test/java/ai/nubase/mcp/tools/McpConfigTest.java`

**Step 1: Write the failing test**

Create `McpConfigTest` that mocks `DatabaseMcpTools` and a new `MemoryMcpTools`, calls `toolProvider(...)`, and verifies the provider is created without throwing.

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=McpConfigTest test
```

Expected: compile failure because `MemoryMcpTools` does not exist and `McpConfig.toolProvider` accepts only `DatabaseMcpTools`.

**Step 3: Write minimal implementation**

Create placeholder class:

```java
package ai.nubase.mcp.tools;

import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Component;

@Component
@RequiredArgsConstructor
public class MemoryMcpTools {
}
```

Update `McpConfig`:

```java
@Bean
public ToolCallbackProvider toolProvider(DatabaseMcpTools databaseMcpTools,
                                         MemoryMcpTools memoryMcpTools) {
    return MethodToolCallbackProvider.builder()
            .toolObjects(databaseMcpTools, memoryMcpTools)
            .build();
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
mvn -q -Dtest=McpConfigTest test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/mcp/tools/McpConfig.java src/main/java/ai/nubase/mcp/tools/MemoryMcpTools.java src/test/java/ai/nubase/mcp/tools/McpConfigTest.java
git commit -m "feat: register memory mcp tools"
```

### Task 2: Add `memorySearch` MCP Tool

**Files:**
- Modify: `src/main/java/ai/nubase/mcp/tools/MemoryMcpTools.java`
- Test: `src/test/java/ai/nubase/mcp/tools/MemoryMcpToolsTest.java`
- Reference: `src/main/java/ai/nubase/mem/service/MemoryService.java`
- Reference: `src/main/java/ai/nubase/mem/dto/SearchMemoryRequest.java`

**Step 1: Write the failing test**

Test that `memorySearch(userId, agentId, runId, query, topK)` builds a `SearchMemoryRequest`, calls `memoryService.search`, and returns:

```json
{
  "success": true,
  "results": [...],
  "count": 1
}
```

Also test blank `query` returns `success=false`.

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=MemoryMcpToolsTest test
```

Expected: method not found.

**Step 3: Write minimal implementation**

Add constructor dependency:

```java
private final MemoryService memoryService;
```

Add tool:

```java
@Tool(description = "Search Nubase long-term memory for the current user, agent, or run. Parameters: userId UUID string optional, agentId optional, runId optional, query required, topK optional.")
public Map<String, Object> memorySearch(String userId, String agentId, String runId, String query, Integer topK) {
    if (query == null || query.isBlank()) {
        return Map.of("success", false, "error", "query is required");
    }
    SearchMemoryRequest request = new SearchMemoryRequest();
    request.setUserId(parseUuid(userId));
    request.setAgentId(blankToNull(agentId));
    request.setRunId(blankToNull(runId));
    request.setQuery(query);
    request.setTopK(topK);
    List<MemoryResponse> results = memoryService.search(request);
    return Map.of("success", true, "results", results, "count", results.size());
}
```

Keep helpers private and deterministic:

```java
private UUID parseUuid(String value) {
    return value == null || value.isBlank() ? null : UUID.fromString(value);
}

private String blankToNull(String value) {
    return value == null || value.isBlank() ? null : value;
}
```

**Step 4: Run tests**

Run:

```bash
mvn -q -Dtest=MemoryMcpToolsTest test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/mcp/tools/MemoryMcpTools.java src/test/java/ai/nubase/mcp/tools/MemoryMcpToolsTest.java
git commit -m "feat: add memory search mcp tool"
```

### Task 3: Add `memoryWrite` MCP Tool

**Files:**
- Modify: `src/main/java/ai/nubase/mcp/tools/MemoryMcpTools.java`
- Test: `src/test/java/ai/nubase/mcp/tools/MemoryMcpToolsTest.java`
- Reference: `src/main/java/ai/nubase/mem/dto/AddMemoryRequest.java`
- Reference: `src/main/java/ai/nubase/mem/llm/ChatMessage.java`

**Step 1: Write failing tests**

Add tests for:
- `memoryWrite` rejects blank content.
- `memoryWrite` maps a plain content string to one user message.
- `memoryWrite` passes `userId`, `agentId`, `runId`, and `infer` to `MemoryService.add`.

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=MemoryMcpToolsTest test
```

Expected: method not found.

**Step 3: Write minimal implementation**

Add:

```java
@Tool(description = "Write durable Nubase memory. Parameters: userId UUID string optional, agentId optional, runId optional, content required, infer optional defaults true.")
public Map<String, Object> memoryWrite(String userId, String agentId, String runId, String content, Boolean infer) {
    if (content == null || content.isBlank()) {
        return Map.of("success", false, "error", "content is required");
    }
    AddMemoryRequest request = new AddMemoryRequest();
    request.setUserId(parseUuid(userId));
    request.setAgentId(blankToNull(agentId));
    request.setRunId(blankToNull(runId));
    request.setInfer(infer == null || infer);
    request.setMessages(List.of(ChatMessage.builder()
            .role("user")
            .content(content)
            .build()));
    List<MemoryEventResponse> events = memoryService.add(request);
    return Map.of("success", true, "results", events, "count", events.size());
}
```

**Step 4: Run tests**

Run:

```bash
mvn -q -Dtest=MemoryMcpToolsTest test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/mcp/tools/MemoryMcpTools.java src/test/java/ai/nubase/mcp/tools/MemoryMcpToolsTest.java
git commit -m "feat: add memory write mcp tool"
```

### Task 4: Add `memoryContext` MCP Tool

**Files:**
- Modify: `src/main/java/ai/nubase/mcp/tools/MemoryMcpTools.java`
- Test: `src/test/java/ai/nubase/mcp/tools/MemoryMcpToolsTest.java`

**Step 1: Write failing test**

Test that `memoryContext(userId, agentId, runId, task, topK)`:
- calls search with `task` as query
- returns a concise `context` string
- includes structured `memories`
- returns empty context when no hits exist

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=MemoryMcpToolsTest test
```

Expected: method not found.

**Step 3: Write minimal implementation**

Add:

```java
@Tool(description = "Return compact relevant context for an AI agent before it starts a task. Parameters: userId optional, agentId optional, runId optional, task required, topK optional.")
public Map<String, Object> memoryContext(String userId, String agentId, String runId, String task, Integer topK) {
    Map<String, Object> search = memorySearch(userId, agentId, runId, task, topK == null ? 8 : topK);
    if (Boolean.FALSE.equals(search.get("success"))) {
        return search;
    }
    @SuppressWarnings("unchecked")
    List<MemoryResponse> results = (List<MemoryResponse>) search.get("results");
    String context = results.stream()
            .map(MemoryResponse::getMemory)
            .filter(Objects::nonNull)
            .limit(topK == null ? 8 : topK)
            .collect(Collectors.joining("\n- ", "- ", ""));
    return Map.of("success", true, "context", context, "memories", results, "count", results.size());
}
```

**Step 4: Run tests**

Run:

```bash
mvn -q -Dtest=MemoryMcpToolsTest test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/mcp/tools/MemoryMcpTools.java src/test/java/ai/nubase/mcp/tools/MemoryMcpToolsTest.java
git commit -m "feat: add agent memory context tool"
```

## Milestone 2: Agent Discovery Resources and Instructions

Help agents understand Nubase without asking the user for setup details.

### Task 5: Add Agent Instructions Endpoint

**Files:**
- Create: `src/main/java/ai/nubase/agent/controller/AgentMetadataController.java`
- Test: `src/test/java/ai/nubase/agent/controller/AgentMetadataControllerTest.java`
- Modify: `src/main/resources/application.yml`

**Step 1: Write failing controller tests**

Test:
- `GET /agent/v1/instructions` returns text/plain or JSON with instructions.
- response mentions Memory, REST, Auth, Storage, and MCP.
- endpoint does not include secrets.

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=AgentMetadataControllerTest test
```

Expected: controller missing.

**Step 3: Implement endpoint**

Add a controller returning a stable agent instruction document:

```text
Use Nubase as the project backend.
First call memoryContext for relevant user/project context.
Use listTables/getTableStructure before writing SQL.
Prefer REST APIs for app data.
Use service_role only for server-side/admin actions.
Write durable project decisions with memoryWrite.
```

Do not return API keys or database passwords.

**Step 4: Run tests**

Run:

```bash
mvn -q -Dtest=AgentMetadataControllerTest test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/agent/controller/AgentMetadataController.java src/test/java/ai/nubase/agent/controller/AgentMetadataControllerTest.java
git commit -m "feat: expose agent instructions"
```

### Task 6: Add Project Capability Manifest

**Files:**
- Modify: `src/main/java/ai/nubase/agent/controller/AgentMetadataController.java`
- Test: `src/test/java/ai/nubase/agent/controller/AgentMetadataControllerTest.java`

**Step 1: Write failing tests**

Test `GET /agent/v1/capabilities` returns:

```json
{
  "name": "Nubase",
  "services": {
    "memory": {"enabled": true, "basePath": "/mem/v1"},
    "database": {"enabled": true, "basePath": "/rest/v1"},
    "auth": {"enabled": true, "basePath": "/auth/v1"},
    "storage": {"enabled": true, "basePath": "/storage/v1"},
    "mcp": {"enabled": true, "endpoint": "/mcp"}
  }
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=AgentMetadataControllerTest test
```

Expected: endpoint missing.

**Step 3: Implement minimal manifest**

Hard-code stable public paths first. Later tasks can make this dynamic.

**Step 4: Run tests**

Run:

```bash
mvn -q -Dtest=AgentMetadataControllerTest test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/agent/controller/AgentMetadataController.java src/test/java/ai/nubase/agent/controller/AgentMetadataControllerTest.java
git commit -m "feat: expose agent capability manifest"
```

### Task 7: Document MCP Tool Contract

**Files:**
- Create: `docs/mcp.md`
- Modify: `docs/README.md`
- Modify: `README.md`

**Step 1: Draft `docs/mcp.md`**

Include:
- endpoint: `/mcp`
- required headers: `apikey`, optional `Authorization`
- database tools
- memory tools
- risk levels
- examples for Codex/Claude Code/Cursor

**Step 2: Add examples**

Include minimal JSON-style examples:

```json
{
  "tool": "memoryContext",
  "arguments": {
    "agentId": "codex",
    "task": "Implement billing settings page"
  }
}
```

**Step 3: Link docs**

Add `docs/mcp.md` to `docs/README.md` and root `README.md`.

**Step 4: Verify links**

Run:

```bash
rg -n "mcp.md|Memory MCP|memoryContext" README.md docs
```

Expected: links and examples appear.

**Step 5: Commit**

```bash
git add docs/mcp.md docs/README.md README.md
git commit -m "docs: add agent mcp guide"
```

## Milestone 3: Connect Agent Configuration

Reduce user setup to one page and one copy/paste block.

### Task 8: Add Backend Agent Config Preview Endpoint

**Files:**
- Modify: `src/main/java/ai/nubase/agent/controller/AgentMetadataController.java`
- Test: `src/test/java/ai/nubase/agent/controller/AgentMetadataControllerTest.java`

**Step 1: Write failing tests**

Test `GET /agent/v1/connect-config?client=codex` returns:

```json
{
  "client": "codex",
  "mcp": {
    "endpoint": "http://localhost:9999/mcp",
    "headers": {
      "apikey": "<project-api-key>"
    }
  },
  "env": {
    "NUBASE_URL": "http://localhost:9999"
  }
}
```

Do not include secrets unless the caller is service role or platform admin. For the first pass, return placeholders.

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=AgentMetadataControllerTest test
```

Expected: endpoint missing.

**Step 3: Implement placeholder generator**

Support `client=codex`, `claude-code`, `cursor`, `idea`, and default `generic`.

**Step 4: Run tests**

Run:

```bash
mvn -q -Dtest=AgentMetadataControllerTest test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/agent/controller/AgentMetadataController.java src/test/java/ai/nubase/agent/controller/AgentMetadataControllerTest.java
git commit -m "feat: add agent connect config endpoint"
```

### Task 9: Add Studio Connect Agent Page

**Files:**
- Create: `frontend/apps/studio/src/app/project/[ref]/connect-agent/page.tsx`
- Modify: `frontend/apps/studio/src/components/workspace-shell.tsx`
- Reference: `frontend/apps/studio/src/lib/*`

**Step 1: Inspect existing Studio API helpers**

Run:

```bash
rg -n "fetch\\(|admin/projects|keys|project/\\[ref\\]" frontend/apps/studio/src
```

Use the existing fetch/auth pattern.

**Step 2: Implement page**

Page sections:
- client selector: Codex, Claude Code, Cursor, IDEA, Generic
- MCP endpoint
- required headers
- environment variables
- copy button for JSON config
- warning for service role usage

Keep design consistent with existing Studio pages.

**Step 3: Add navigation entry**

Add “Connect Agent” to project navigation.

**Step 4: Run frontend checks**

Run:

```bash
cd frontend && pnpm --filter @nubase/studio lint
```

If no lint script exists, run the closest existing check from `frontend/package.json` or `frontend/apps/studio/package.json`.

**Step 5: Commit**

```bash
git add frontend/apps/studio/src/app/project/[ref]/connect-agent/page.tsx frontend/apps/studio/src/components/workspace-shell.tsx
git commit -m "feat: add studio agent connection page"
```

### Task 10: Add Copy-Paste Config Templates

**Files:**
- Modify: `frontend/apps/studio/src/app/project/[ref]/connect-agent/page.tsx`
- Create: `docs/agent-connect.md`

**Step 1: Add templates**

Include templates for:
- Codex MCP
- Claude Code MCP
- Cursor MCP
- generic `.env`
- AI Gateway model provider config for clients that support OpenAI-compatible or Anthropic-compatible base URLs

Use placeholders when exact client format changes:

```json
{
  "mcpServers": {
    "nubase": {
      "url": "http://localhost:9999/mcp",
      "headers": {
        "apikey": "YOUR_NUBASE_PROJECT_KEY"
      }
    }
  }
}
```

Also include model gateway environment variables separately from MCP:

```bash
NUBASE_URL=http://localhost:9999
NUBASE_API_KEY=YOUR_NUBASE_PROJECT_KEY
OPENAI_BASE_URL=http://localhost:9999/v1
OPENAI_API_KEY=YOUR_NUBASE_AI_GATEWAY_KEY
ANTHROPIC_BASE_URL=http://localhost:9999
ANTHROPIC_AUTH_TOKEN=YOUR_NUBASE_AI_GATEWAY_KEY
```

Clarify in the docs and Studio UI:
- MCP config gives agents tools to operate Nubase Memory, Database, Auth, and Storage.
- AI Gateway config makes the agent's model calls go through Nubase for routing, usage tracking, pricing, and provider abstraction.
- Some clients can use both at once; some only support MCP or only support custom model base URLs.

**Step 2: Document limitations**

State that exact config file locations differ by client and may change. The stable Nubase MCP contract is endpoint + headers. The stable AI Gateway contract is OpenAI-compatible `/v1/*` plus Anthropic-compatible `/v1/messages` endpoints with a Nubase gateway API key.

**Step 3: Run docs grep**

Run:

```bash
rg -n "Connect Agent|mcpServers|YOUR_NUBASE_PROJECT_KEY|OPENAI_BASE_URL|ANTHROPIC_BASE_URL|AI Gateway" docs frontend/apps/studio/src/app/project
```

Expected: templates appear.

**Step 4: Commit**

```bash
git add frontend/apps/studio/src/app/project/[ref]/connect-agent/page.tsx docs/agent-connect.md
git commit -m "docs: add agent connection templates"
```

## Milestone 4: Safety Guardrails

Make powerful agent actions inspectable and less likely to damage data.

### Task 11: Add SQL Risk Classification Service

**Files:**
- Create: `src/main/java/ai/nubase/mcp/safety/SqlRiskClassifier.java`
- Test: `src/test/java/ai/nubase/mcp/safety/SqlRiskClassifierTest.java`
- Modify: `src/main/java/ai/nubase/mcp/tools/DatabaseMcpTools.java`

**Step 1: Write failing tests**

Classify:
- `select * from todos` as `READ`
- `create table todos (...)` as `SCHEMA_WRITE`
- `insert into todos ...` as `DATA_WRITE`
- `drop table todos` as `DANGEROUS`
- `truncate table todos` as `DANGEROUS`
- mixed statements as highest risk

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=SqlRiskClassifierTest test
```

Expected: class missing.

**Step 3: Implement conservative classifier**

Use token-based matching. Do not try to be a full SQL parser in this task.

**Step 4: Add risk to `executeSql` response**

Return:

```json
{
  "success": true,
  "risk": "SCHEMA_WRITE",
  "results": []
}
```

**Step 5: Run tests**

Run:

```bash
mvn -q -Dtest=SqlRiskClassifierTest,DatabaseMcpToolsTest test
```

Expected: PASS. If `DatabaseMcpToolsTest` does not exist yet, create focused unit coverage for `executeSql`.

**Step 6: Commit**

```bash
git add src/main/java/ai/nubase/mcp/safety/SqlRiskClassifier.java src/test/java/ai/nubase/mcp/safety/SqlRiskClassifierTest.java src/main/java/ai/nubase/mcp/tools/DatabaseMcpTools.java
git commit -m "feat: classify mcp sql risk"
```

### Task 12: Add `dryRun` SQL Tool

**Files:**
- Modify: `src/main/java/ai/nubase/mcp/tools/DatabaseMcpTools.java`
- Test: `src/test/java/ai/nubase/mcp/tools/DatabaseMcpToolsTest.java`

**Step 1: Write failing tests**

Test:
- `executeSqlDryRun("select * from todos")` returns risk and normalized statement count.
- does not call `SqlExecutionService.executeSql`.
- returns `executable=false` for dangerous SQL unless explicitly run through `executeSql`.

**Step 2: Run test to verify it fails**

Run:

```bash
mvn -q -Dtest=DatabaseMcpToolsTest test
```

Expected: method missing.

**Step 3: Implement tool**

Add:

```java
@Tool(description = "Preview SQL risk and statement count without executing it. Use before executeSql for schema or data changes.")
public Map<String, Object> executeSqlDryRun(String sqlQuery) {
    SqlRisk risk = sqlRiskClassifier.classify(sqlQuery);
    return Map.of(
            "success", true,
            "risk", risk.name(),
            "statementCount", countStatements(sqlQuery),
            "executable", risk != SqlRisk.DANGEROUS
    );
}
```

**Step 4: Run tests**

Run:

```bash
mvn -q -Dtest=DatabaseMcpToolsTest test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/mcp/tools/DatabaseMcpTools.java src/test/java/ai/nubase/mcp/tools/DatabaseMcpToolsTest.java
git commit -m "feat: add mcp sql dry run"
```

### Task 13: Add Agent Operation Audit Records

**Files:**
- Create migration: `src/main/resources/db/migration/V2__agent_operation_audit.sql`
- Create entity: `src/main/java/ai/nubase/metadata/entity/AgentOperationAudit.java`
- Create repository: `src/main/java/ai/nubase/metadata/repository/AgentOperationAuditRepository.java`
- Create service: `src/main/java/ai/nubase/agent/service/AgentOperationAuditService.java`
- Test: `src/test/java/ai/nubase/agent/service/AgentOperationAuditServiceTest.java`

**Step 1: Write failing service tests**

Test recording:
- agent id
- tool name
- risk
- success/failure
- error message
- duration
- project ref when available

**Step 2: Add migration**

Create table:

```sql
CREATE TABLE IF NOT EXISTS agent_operation_audit (
    id UUID PRIMARY KEY,
    project_ref VARCHAR(128),
    agent_id VARCHAR(128),
    tool_name VARCHAR(128) NOT NULL,
    risk VARCHAR(64),
    success BOOLEAN NOT NULL,
    error TEXT,
    duration_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS agent_operation_audit_project_created_idx
    ON agent_operation_audit (project_ref, created_at DESC);
```

**Step 3: Implement service**

Keep it best-effort: audit failure must not break the MCP tool result.

**Step 4: Wire audit into `DatabaseMcpTools.executeSql` and memory tools**

Record at least:
- `executeSql`
- `executeSqlDryRun`
- `memorySearch`
- `memoryWrite`
- `memoryContext`

**Step 5: Run tests**

Run:

```bash
mvn -q -Dtest=AgentOperationAuditServiceTest,MemoryMcpToolsTest,DatabaseMcpToolsTest test
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src/main/resources/db/migration/V2__agent_operation_audit.sql src/main/java/ai/nubase/metadata/entity/AgentOperationAudit.java src/main/java/ai/nubase/metadata/repository/AgentOperationAuditRepository.java src/main/java/ai/nubase/agent/service/AgentOperationAuditService.java src/test/java/ai/nubase/agent/service/AgentOperationAuditServiceTest.java src/main/java/ai/nubase/mcp/tools
git commit -m "feat: audit agent operations"
```

## Milestone 5: Auth, Storage, Logs, and OpenAPI Expansion

After memory/database works well, expand Nubase into a fuller agent backend.

### Task 14: Add Auth MCP Tools

**Files:**
- Create: `src/main/java/ai/nubase/mcp/tools/AuthMcpTools.java`
- Modify: `src/main/java/ai/nubase/mcp/tools/McpConfig.java`
- Test: `src/test/java/ai/nubase/mcp/tools/AuthMcpToolsTest.java`
- Reference: `src/main/java/ai/nubase/auth/controller/AdminController.java`

**Step 1: Implement read-first tools**

Add:
- `authListUsers(page, pageSize)`
- `authGetUser(userId)`

**Step 2: Add controlled write tools**

Add:
- `authCreateTestUser(email, password)`
- `authDeleteTestUser(userId)`

Only expose write tools with service role context.

**Step 3: Run tests**

Run:

```bash
mvn -q -Dtest=AuthMcpToolsTest,McpConfigTest test
```

Expected: PASS.

**Step 4: Commit**

```bash
git add src/main/java/ai/nubase/mcp/tools/AuthMcpTools.java src/main/java/ai/nubase/mcp/tools/McpConfig.java src/test/java/ai/nubase/mcp/tools/AuthMcpToolsTest.java
git commit -m "feat: add auth mcp tools"
```

### Task 15: Add Storage MCP Tools

**Files:**
- Create: `src/main/java/ai/nubase/mcp/tools/StorageMcpTools.java`
- Modify: `src/main/java/ai/nubase/mcp/tools/McpConfig.java`
- Test: `src/test/java/ai/nubase/mcp/tools/StorageMcpToolsTest.java`
- Reference: `src/main/java/ai/nubase/auth/controller/storage`

**Step 1: Implement bucket/object metadata tools**

Add:
- `storageListBuckets()`
- `storageCreateBucket(bucketId, isPublic)`
- `storageListObjects(bucketId, prefix, limit)`
- `storageCreateSignedUrl(bucketId, path, expiresInSeconds)`

Avoid raw file upload in the first pass unless there is an existing service API that is easy to reuse.

**Step 2: Run tests**

Run:

```bash
mvn -q -Dtest=StorageMcpToolsTest,McpConfigTest test
```

Expected: PASS.

**Step 3: Commit**

```bash
git add src/main/java/ai/nubase/mcp/tools/StorageMcpTools.java src/main/java/ai/nubase/mcp/tools/McpConfig.java src/test/java/ai/nubase/mcp/tools/StorageMcpToolsTest.java
git commit -m "feat: add storage mcp tools"
```

### Task 16: Add Agent Logs Page in Studio

**Files:**
- Create: `src/main/java/ai/nubase/agent/controller/AgentAuditController.java`
- Test: `src/test/java/ai/nubase/agent/controller/AgentAuditControllerTest.java`
- Create: `frontend/apps/studio/src/app/project/[ref]/agent-logs/page.tsx`
- Modify: `frontend/apps/studio/src/components/workspace-shell.tsx`

**Step 1: Backend list endpoint**

Add `GET /agent/v1/audit?projectRef=&page=&pageSize=` returning paginated records.

**Step 2: Frontend table**

Columns:
- time
- agent id
- tool
- risk
- success
- duration
- error

**Step 3: Run backend tests**

Run:

```bash
mvn -q -Dtest=AgentAuditControllerTest test
```

Expected: PASS.

**Step 4: Run frontend checks**

Run:

```bash
cd frontend && pnpm --filter @nubase/studio lint
```

Expected: PASS or existing lint baseline documented.

**Step 5: Commit**

```bash
git add src/main/java/ai/nubase/agent/controller/AgentAuditController.java src/test/java/ai/nubase/agent/controller/AgentAuditControllerTest.java frontend/apps/studio/src/app/project/[ref]/agent-logs/page.tsx frontend/apps/studio/src/components/workspace-shell.tsx
git commit -m "feat: add agent operation logs"
```

### Task 17: Publish OpenAPI Fallback

**Files:**
- Modify: `pom.xml`
- Create or configure: `src/main/java/ai/nubase/common/config/OpenApiConfig.java`
- Modify: `docs/agent-connect.md`

**Step 1: Add Springdoc dependency**

Add `springdoc-openapi-starter-webmvc-ui` if it is not already present.

**Step 2: Configure grouped APIs**

Groups:
- `memory`
- `auth`
- `storage`
- `agent`
- `ai-gateway`

**Step 3: Verify endpoint**

Run backend, then:

```bash
curl -s http://localhost:9999/v3/api-docs | head
```

Expected: OpenAPI JSON.

**Step 4: Document fallback**

Add docs telling non-MCP tools to use OpenAPI + `apikey` headers.

**Step 5: Commit**

```bash
git add pom.xml src/main/java/ai/nubase/common/config/OpenApiConfig.java docs/agent-connect.md
git commit -m "feat: publish openapi fallback for agents"
```

## Milestone 6: End-to-End Validation

### Task 18: Add Local Agent Smoke Test Script

**Files:**
- Create: `script/agent-smoke-test.sh`
- Modify: `docs/mcp.md`

**Step 1: Create script**

Script should check:
- `/agent/v1/capabilities`
- `/agent/v1/instructions`
- `/mem/v1/search` with a sample query
- `/mcp` health or MCP endpoint availability

Use environment variables:

```bash
NUBASE_URL=http://localhost:9999
NUBASE_API_KEY=...
```

**Step 2: Run script**

Run:

```bash
bash script/agent-smoke-test.sh
```

Expected: clear PASS/FAIL output. If no API key is set, script should fail with a helpful message.

**Step 3: Document usage**

Add smoke test instructions to `docs/mcp.md`.

**Step 4: Commit**

```bash
git add script/agent-smoke-test.sh docs/mcp.md
git commit -m "test: add agent smoke test"
```

### Task 19: Manual E2E With One Agent Client

**Files:**
- Modify: `docs/agent-connect.md`

**Step 1: Start services**

Run:

```bash
docker compose -f pg-docker-compose.yml up -d
mvn spring-boot:run
cd frontend && pnpm dev:studio
```

**Step 2: Create project in Studio**

Use Studio to create/provision a project and copy a service role key.

**Step 3: Connect one MCP client**

Use Codex or Claude Code with the generated MCP config.

**Step 4: Verify agent workflow**

Ask the agent to:
- inspect schema
- write a memory
- retrieve memory context
- create a simple table
- call dry run before SQL write

**Step 5: Record result**

Update `docs/agent-connect.md` with the verified client, date, config shape, and known limitations.

**Step 6: Commit**

```bash
git add docs/agent-connect.md
git commit -m "docs: record agent connection smoke test"
```

## Release Criteria

The feature is ready for a first public release when:

- A fresh project can expose Memory MCP tools through `/mcp`.
- An agent can call `memoryContext`, `memoryWrite`, `memorySearch`, `listTables`, `getTableStructure`, `executeSqlDryRun`, and `executeSql`.
- Studio has a Connect Agent page with copyable config.
- SQL tools expose risk classification.
- Agent operations are auditable.
- Docs explain Codex/Claude Code/Cursor/generic MCP setup.
- At least one real MCP client has been manually verified.

## Recommended Execution Order

1. Tasks 1-4: Memory MCP minimal loop.
2. Tasks 5-7: Instructions and docs.
3. Tasks 8-10: Connect Agent UX.
4. Tasks 11-13: Safety and audit.
5. Tasks 14-17: Auth, Storage, logs, OpenAPI.
6. Tasks 18-19: E2E validation.

Do not start Auth/Storage MCP tools until Memory MCP and Connect Agent are usable. The product value depends first on making agents remember and discover project context reliably.
