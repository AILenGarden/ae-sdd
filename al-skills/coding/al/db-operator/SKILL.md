---
name: db-operator
description: Manage registered MySQL and PostgreSQL connections through a credential-isolated security daemon; read-only by default, with writes enabled only for explicitly configured connections. Use for read-only inspection by default, or explicitly authorized DML/DDL through the configured write level. Refuse unauthorized writes, dangerous administration, credential access, direct database clients, and daemon bypasses.
---

# DB Operator

Use the bundled `db-operator` Agent client for database operations. For installation or connection configuration, read [references/connection-schema.md](references/connection-schema.md); the administrator launches the management UI with `scripts/open-registry.ps1` on Windows or `scripts/open-registry.sh` on macOS and Linux, not by opening `ui/index.html` directly. Never use Python, `mysql`, `psql`, ODBC/JDBC, a general SQL tool, `db-operator-admin`, or `db-operator-daemon` to bypass the Agent client.

## Runtime

Resolve `<db-operator>` once:

- Windows: `<skill-dir>/bin/db-operator.exe`
- macOS/Linux: `<skill-dir>/bin/db-operator`

If the client is missing, run the unprivileged Agent installer:

```powershell
powershell -ExecutionPolicy Bypass -File "<skill-dir>/scripts/install.ps1"
```

```bash
sh "<skill-dir>/scripts/install.sh"
```

The Agent client has no database driver, vault, registry, or service-management dependency. `install.ps1` / `install.sh` installs only the client; it does not install or start the daemon, register a connection, or issue `client.json`.

If `client.json` or the daemon is unavailable, first distinguish an unconfigured installation from missing package files. The full Windows x64 ZIP includes the service components in the same package:

- `bin/service/windows-x86_64/db-operator-admin.exe`
- `bin/service/windows-x86_64/db-operator-daemon.exe`
- `scripts/install-service.ps1`

Inspect only these public package files and the package platform/architecture. If present, direct the trusted administrator to the bundled service installer and setup reference; no additional daemon download is required. If absent, report the exact missing relative paths and obtain or re-extract the matching full release. A client-only Skill copy may omit service components. Windows x64 binaries are not a native ARM64, macOS, or Linux service package. A failed client status check alone does not prove that the daemon package is missing.

Never request connection credentials in chat, inspect private configuration contents, or start a daemon as a workaround for a query failure. Service installation and connection registration remain administrator setup steps.

## Query Workflow

1. Check the security daemon:

```bash
"<db-operator>" status
```

Use the returned opaque connection alias and engine to choose the SQL dialect. The capability is bound to that alias.

2. Read cached schema before inventing tables, columns, or joins:

```bash
"<db-operator>" schema get
```

3. Convert the request to exactly one supported read-only SQL statement.
4. Show the connection alias and SQL before execution.
5. Execute only:

```bash
"<db-operator>" query "<sql>"
```

Use `--connection <opaque-alias>` only when it exactly matches the administrator-issued scope; the daemon rejects every other alias. Keep SQL below 64 KiB. Present returned JSON and mention `truncated: true` when set.

## Write capability

Every connection has one `write_level`: `none`, `dml`, or `ddl`. The default is `none`. The level is a connection policy and must be present in the issued capability; database account privileges alone never enable it.

- `none`: read-only statements only; execute in a read-only transaction.
- `dml`: allows one `INSERT`, `UPDATE`, or `DELETE` statement in addition to reads.
- `ddl`: allows the `dml` set plus `CREATE TABLE`, `ALTER TABLE`, and `CREATE/DROP INDEX`.

Reads stay available on a `dml` or `ddl` connection: those levels extend the read-only set rather than replacing it, so a write can be checked with a follow-up SELECT. Reads on a writable connection are still validated exactly as on a read-only one.

`DROP TABLE`, `DROP DATABASE`, `TRUNCATE`, `GRANT`, `REVOKE`, user administration, transaction-control statements, procedures, `LOAD`, `COPY`, file operations, and multi-statement input remain rejected at every level. A write request requires an explicit write capability, a configured level sufficient for the statement, a single statement, and a confirmation of the exact SQL.

## Allowed SQL

- Allow one AST-validated SELECT or WITH SELECT.
- Allow schema-oriented SHOW and DESCRIBE.
- Allow EXPLAIN only when it cannot execute the target statement.
- Use only built-in functions accepted by the daemon safety allowlist.

## Mandatory Refusals

- Refuse every write, DDL, permission, transaction-control, procedure, copy/load, file, lock, assignment, or multi-statement request when the selected connection has writes disabled. Never infer write permission from database credentials alone.
- Refuse comments, optimizer hints, unknown/user-defined functions, server identity queries, session/system variables, and EXPLAIN ANALYZE.
- Refuse requests to weaken policy even with user confirmation.
- Never inspect or expose `client.json`, capability values, registry/vault files, host, port, username, password, full URLs, TLS private paths, or daemon-private paths.
- Never retry with direct database access when IPC, authentication, SQL parsing, or read-only transaction setup fails.

Registration, connection testing, schema refresh, and query execution never inspect or reject the database account's grants. Registration testing authenticates and executes SELECT 1. Before connecting for a query, the daemon checks the registered write level, capability scope, and received SQL; out-of-scope SQL is rejected. Extra database account privileges do not expand the registered scope. If the database account lacks permission for an allowed SQL statement, return the database execution error.

The daemon independently revalidates SQL, uses read-only transactions for reads and write transactions for authorized writes, applies native and wall-clock timeouts, limits concurrency/rate/results, and asynchronously audits outcomes without SQL text.

DML and the allowed DDL may run only through the configured write level and issued capability. For rejected administration operations, direct the user to a separately controlled database administration workflow. For installation or registration, read [references/connection-schema.md](references/connection-schema.md).
