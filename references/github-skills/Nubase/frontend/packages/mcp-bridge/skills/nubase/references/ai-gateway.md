# Nubase AI Gateway Reference

Use this reference when configuring Nubase AI Gateway, model routing, OpenAI-compatible base URLs, Anthropic-compatible base URLs, gateway keys, usage logs, pricing, provider abstraction, or agent model configuration.

Model-call routing (the `/v1` surface below) is separate from MCP tools, but the gateway control plane is exposed as MCP tools.

## Control-Plane Tools

Manage the project's gateway keys and usage without leaving the agent. These require the project key to carry `service_role`.

Read (always available):

- `gateway_list_keys()` — list this project's `nbk_` self-routing keys (no plaintext/secret returned)
- `gateway_usage({ startDate?, endDate? })` — tokens, requests, and cost overview for a date range

Write (gated by `NUBASE_ALLOW_ADMIN_WRITE=true`; otherwise returns `{ success: false, error }` without calling the backend):

- `gateway_issue_key({ name?, description?, expiresAt? })` — the full key is returned exactly once; treat it as a secret
- `gateway_revoke_key({ id })`

When `gateway_issue_key` returns a full key, never write it to Memory, generated code, or docs.

Use AI Gateway for:

- model routing
- provider abstraction
- usage tracking
- pricing
- gateway keys
- OpenAI-compatible and Anthropic-compatible clients

## OpenAI-Compatible Clients

```bash
OPENAI_BASE_URL=<NUBASE_URL>/v1
OPENAI_API_KEY=<gateway key>
```

## Anthropic-Compatible Clients

```bash
ANTHROPIC_BASE_URL=<NUBASE_URL>
ANTHROPIC_AUTH_TOKEN=<gateway key>
```

## Agent Configuration

Configure both surfaces when a client should use Nubase tools and route model calls through Nubase:

- MCP bridge: backend operations and Memory
- AI Gateway env vars: model calls

Do not confuse project keys with AI Gateway keys.

## Safety

Gateway keys should be treated as secrets. Do not write them to Memory, generated frontend code, or documentation examples with real values.
