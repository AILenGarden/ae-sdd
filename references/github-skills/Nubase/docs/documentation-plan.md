# Documentation Plan

This plan is for the first GitHub-ready documentation set. The goal is to make Nubase understandable to three audiences:

- developers building apps with AI Coding tools
- developers comparing it with Supabase
- contributors reading the code for the first time
- early adopters trying to self-host it

## Positioning

Nubase should be described as:

> An open-source backend service born for AI-native apps and AI Coding: Memory, Database, Storage, and Auth in one self-hostable platform, with database-per-project isolation.

Avoid positioning Nubase as "a full Supabase replacement" until Realtime, Edge Functions, backups, and production operational tooling exist. The stronger and more accurate positioning is:

> A Supabase-style developer experience rebuilt around AI Memory, AI Coding workflows, MCP-friendly operations, and multi-project self-hosting.

## Documentation Goals

1. Explain why AI-native apps and AI Coding need a backend service built for them.
2. Show what works today.
3. Compare honestly with Supabase.
4. Give a working local quickstart.
5. Explain the architecture clearly enough for contributors.
6. Define the open-source/commercial boundary.
7. Make missing features explicit.

## Proposed File Structure

```text
README.md
docs/
  README.md
  documentation-plan.md
  product-overview.md
  supabase-comparison.md
  getting-started.md
  architecture.md
  auth.md
  database.md
  storage.md
  memory.md
  studio.md
  mcp.md
  configuration.md
  deployment.md
  security.md
  contributing.md
```

The first release should include the files already added in this pass:

- `README.md`
- `docs/README.md`
- `docs/documentation-plan.md`
- `docs/product-overview.md`
- `docs/supabase-comparison.md`
- `docs/getting-started.md`
- `docs/architecture.md`

The next pass should add the feature-specific reference docs.

## README Strategy

The README should do five jobs quickly:

1. Give a sharp one-line product definition: backend services born for AI.
2. Explain the AI Coding use case: generated apps need a real backend target.
3. State the differences from Supabase: AI Memory, MCP/agent workflows, and multi-project self-hosting.
4. Show the four pillars.
5. Provide a short quickstart.
6. Link to deeper docs.

Do not overload README with every endpoint. Keep it attractive and scannable.

## Product Overview

`docs/product-overview.md` should cover:

- product thesis
- target users
- four pillars
- AI Coding workflow
- what makes Nubase different
- current status
- recommended open-source/commercial split

This document is the best page to share with people who ask "what is this?"

## Supabase Comparison

`docs/supabase-comparison.md` should be specific and careful.

It should compare:

- project model
- self-hosting model
- database API
- auth
- storage
- realtime
- edge functions
- AI memory
- operations
- commercial/managed features

It should cite official Supabase docs:

- self-hosting single-project limitation
- Cloud organization/project model
- product categories

Avoid dunking on Supabase. The point is to explain tradeoffs and clarify where Nubase is intentionally different.

## Getting Started

`docs/getting-started.md` should provide:

- prerequisites
- start Postgres
- set required env vars
- start backend
- start Studio
- create a platform user
- create and provision a project
- use REST API
- write and search memory
- troubleshooting

The commands should prefer local development defaults and make production warnings explicit.

## Architecture

`docs/architecture.md` should explain:

- metadata database
- project databases
- request routing
- `apikey` plus `Authorization` token model
- database-per-project isolation
- service role vs authenticated vs anon
- Memory tables and retrieval flow
- Storage metadata and object backend
- Studio and platform settings

This is the page contributors should read before touching code.

## Feature Reference Docs

Add these after the first documentation pass.

### `docs/auth.md`

Include:

- auth tables
- signup/login/refresh flows
- email confirmation
- OAuth flow
- per-project JWT secrets
- service role semantics
- endpoint reference
- Supabase compatibility notes

### `docs/database.md`

Include:

- project provisioning
- REST API filters and operators
- range pagination
- embedded resources if supported
- RLS model
- SQL editor
- schema cache
- limitations vs PostgREST

### `docs/storage.md`

Include:

- bucket model
- public/private buckets
- object key layout
- signed URLs
- upload limits
- R2/S3/MinIO configuration
- RLS integration

### `docs/memory.md`

Include:

- memory lifecycle
- `infer=true` vs `infer=false`
- fact extraction
- ADD/UPDATE/DELETE decisions
- embeddings
- BM25
- entity boost
- metadata filters
- history
- safety model
- endpoint reference

### `docs/studio.md`

Include:

- projects
- SQL editor
- auth users
- storage explorer
- memory explorer
- settings
- local development ports

### `docs/mcp.md`

Include:

- available tools
- authentication
- SQL execution warning
- safe local usage
- examples for agents

## Security Documentation

Before public launch, add `docs/security.md` and root `SECURITY.md`.

Topics:

- required secrets
- `PGRST_ENCRYPTION_MASTER_KEY`
- metadata service role key
- public internet hardening
- CORS
- service role handling
- SQL execution risks
- MCP risks
- storage credentials
- log redaction
- key rotation

## Launch Checklist

Before making the repository public:

- Add `LICENSE`.
- Add `SECURITY.md`.
- Add `CONTRIBUTING.md`.
- Remove local agent files such as `.claude/settings.local.json`.
- Remove tracked build cache files such as `*.tsbuildinfo`.
- Review service-role key exposure in project/member APIs.
- Decide which platform/team management features stay open-source.
- Add a production deployment warning to README.
- Run dependency/license scanning.
- Add screenshots or GIFs of Studio.

## Tone and Style

Use direct, technical language.

Good:

> Each project gets its own PostgreSQL database. The metadata database only stores routing and platform configuration.

Avoid:

> Revolutionary next-generation AI backend that changes everything.

Be confident, but precise.
