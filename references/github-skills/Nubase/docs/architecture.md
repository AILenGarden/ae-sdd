# Architecture

Nubase is an AI-native backend service implemented as a Spring Boot backend plus a Next.js Studio frontend.

It is designed for two operators:

- humans using Studio
- AI Coding agents using REST APIs and MCP database tools

The backend combines:

- platform/project management
- Auth
- PostgREST-compatible REST API
- Storage
- Memory
- MCP database tools

## System Overview

```text
Client / Studio / Agent
        |
        | apikey + optional Authorization: Bearer <jwt>
        v
Spring Boot API
        |
        | resolves project from apikey
        v
Metadata Database  ---->  Project Database A
                    ---->  Project Database B
                    ---->  Project Database C
```

## Metadata Database

The metadata database is the control plane.

It stores:

- project database configs
- encrypted database passwords
- JWT secrets and tokens
- platform users
- platform user/project mappings
- SQL snippets
- SQL execution records
- platform settings

Important tables:

- `database_configs`
- `platform_users`
- `platform_user_projects`
- `platform_settings`
- `sql_execution_records`
- `sql_snippets`

## Project Databases

Each project has its own physical PostgreSQL database.

It contains:

- `public.*` application tables
- `auth.*` users, sessions, refresh tokens, identities
- `storage.*` buckets and object metadata
- `mem.*` memories, history, entities, session messages, config

This model gives each project an independent database boundary.

## Request Routing

Most API requests include:

```http
apikey: <project token>
Authorization: Bearer <user token>
```

The `apikey` is a JWT that includes:

- project reference
- role

The backend:

1. extracts the `apikey`
2. reads project reference and role
3. loads the project config from metadata database
4. validates the token with the project's JWT secret
5. initializes or reuses a Hikari data source for the project database
6. sets request context
7. optionally validates the user Bearer token

## Token Model

Nubase uses two token layers.

### Project token

The project token is sent as `apikey`.

Roles:

- `anon`
- `authenticated`
- `service_role`

This token selects the project and defines the base database role.

### User token

The user token is sent as:

```http
Authorization: Bearer <jwt>
```

It identifies the end user and is used for:

- `auth.uid()` style RLS
- user-scoped memory access
- user-level API behavior

## Database API

The `/rest/v1/*` API is a Java implementation of the PostgREST style.

Main components:

- request parser
- query planner
- query executor
- schema cache
- RLS/JWT context

The query layer supports table operations such as:

- select
- filter
- order
- limit
- offset
- range
- insert
- update
- delete
- upsert

## Auth

Auth is implemented in Java instead of using GoTrue.

It manages:

- users
- identities
- sessions
- refresh tokens
- password hashing
- token issuing
- OAuth flows
- email confirmation
- admin user operations

Each project has its own auth tables and JWT secret.

## Storage

Storage stores metadata in the project database and object bytes in an S3-compatible backend.

Metadata:

- buckets
- objects
- public/private flags
- owner fields
- file metadata

Object backend:

- Cloudflare R2
- AWS S3
- MinIO
- LocalStack
- compatible S3 APIs

## Memory

Memory is a first-class project schema.

Main flow:

1. client sends messages to `/mem/v1/memories`
2. LLM extracts durable facts
3. LLM decides ADD, UPDATE, DELETE, or NONE
4. embedding provider generates vectors
5. facts are stored in `mem.memories`
6. changes are recorded in `mem.memory_history`
7. entities are extracted and linked in `mem.entities`

Search flow:

1. embed query
2. run vector search
3. run Postgres full-text search
4. optionally extract query entities
5. boost linked memories
6. fuse scores
7. return ranked memories

## Studio

Studio is a Next.js app.

It provides:

- platform login
- project list
- project creation
- database provisioning
- SQL editor
- auth user management
- storage management
- memory explorer
- memory settings
- platform settings

Default ports:

- Studio: `3000`
- website/docs app: `3001`
- backend: `9999`

## MCP Tools

The MCP server exposes database tools for agent workflows.

Tools include:

- list tables
- inspect table structure
- export RLS policies
- execute SQL
- initialize database

These tools are powerful and should only be exposed in trusted environments.

## Security Notes

Important production requirements:

- set `PGRST_ENCRYPTION_MASTER_KEY`
- set `METADATA_SERVICE_ROLE_KEY`
- keep service role keys server-side
- restrict public access to admin endpoints
- review SQL execution before enabling in production
- review MCP exposure before enabling in production
- configure CORS strictly
- rotate provider keys and database passwords
- avoid logging secrets or full query results in production

## Known Gaps

The current architecture does not yet include:

- Realtime
- Edge Functions
- managed backups
- PITR
- HA provisioning
- per-project billing
- enterprise SSO
