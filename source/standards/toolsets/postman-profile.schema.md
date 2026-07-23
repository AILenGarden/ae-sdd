# ae-sdd Postman Profile Schema

## 1. Local Path

Postman workspace config is local-only:

```text
<project>/.ae-sdd/secrets/postman.local.json
```

This file must not be committed. The target project must ignore
`.ae-sdd/secrets/`.

## 2. JSON Shape

```json
{
  "workspaceId": "ws-abc123",
  "collectionName": "ae-sdd-verify",
  "monitorRegion": "us-east",
  "envVarRefs": {
    "baseUrl": "AE_TESTENV_BASEURL"
  },
  "note": "token managed by PostmanMCP, never stored here"
}
```

## 3. Policy

- Postman credentials live only inside the Agent-installed PostmanMCP; this
  profile stores IDs and env-var references, never tokens.
- Default mode is read-only verification; `runMonitor` is the only execution
  path and targets a non-loopback test-env URL.
- Reports may include `workspaceId`, `collectionName`, `monitorRegion`, and
  run summaries, but must redact any token or secret.

## 4. Required Evidence

For Test (Review phase) supplemental verification:

- monitor run results (runId, stats, failures)
- collection request reconciliation against AC scenarioIds
- missing Postman access marked as `unverified`, not invented
