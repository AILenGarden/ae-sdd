# Administrator Setup

Use this reference only for trusted installation or connection registration. The Agent must not run these commands or receive their output when it contains private paths.

## Release Boundaries

- Full Windows x64 release (`db-operator-*-windows-x86_64-*.zip`): Skill, client, administrator tool, daemon, both Windows installers, and management UI in one ZIP. No separate daemon download is required.
- Client-only distribution: Skill, `db-operator`, and unprivileged client installer; it may omit the service files. Obtain the matching full release for administrator setup.
- Source tree: Skill source, Rust source, tests, and platform installers. The Windows release does not include Rust sources or Unix installers/binaries.

In a fully extracted Windows x64 release, check `bin/service/windows-x86_64/db-operator-admin.exe`, `bin/service/windows-x86_64/db-operator-daemon.exe`, and `scripts/install-service.ps1`. Missing files mean an incomplete extraction or a different distribution; report the exact missing path. Existing files plus a failed client status check mean installation/configuration still needs diagnosis, not that a separate daemon ZIP is absent. Verify the recipient's OS/architecture before choosing a package.

The binaries need no Python. Prebuilt packages install no language dependencies. Building from source requires Cargo; installers do not silently install a compiler or modify a developer toolchain.

## Service Installation

Windows x64: extract the entire ZIP to its own directory and open elevated PowerShell in that directory. `scripts/install.ps1` only installs the client; run the bundled service installer for first-time service setup. Replace the example account and client path with the Windows account that will run the Agent. With `-Register`, enter connection details locally and use the same connection alias as `-Connection`:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/install-service.ps1 `
  -AgentAccount "MACHINE\agent-user" `
  -Connection reporting-prod `
  -AgentConfigPath "C:\Users\agent-user\AppData\Roaming\db-operator\client.json" `
  -Register
```

After successful installation, run `bin/db-operator.exe status` as the Agent user. If using a non-default client configuration path, set `DB_OPERATOR_CLIENT_CONFIG` in that user's environment to the issued path. Do not send the contents of this file in chat.

Linux or macOS, as root, from a matching distribution/source tree that contains the Unix installer (not from the Windows x64 ZIP):

```bash
sh scripts/install-service.sh \
  --agent-user agent-user \
  --connection reporting-prod \
  --agent-config /home/agent-user/.config/db-operator/client.json \
  --register
```

On macOS, use the Agent user's `~/Library/Application Support/db-operator/client.json` path instead. The installer creates a dedicated service identity, locks the private data directory, configures a protected Named Pipe DACL or group-scoped Unix socket, issues the query capability, and installs Windows Service, systemd, or LaunchDaemon configuration.

## Registration Entry

Set `DB_OPERATOR_HOME` to the daemon-private data directory, then use `db-operator-admin register`. Do not edit `connections.json`, vault files, capability records, or Agent `client.json` manually.

Registration collects:

| Field | Required | Rules |
|---|---:|---|
| connection name | yes | Opaque alias, 1-64 ASCII letters/digits plus `.`, `_`, `-` |
| engine | yes | `mysql` or `postgresql` |
| host or URL | yes | URL must not contain a password |
| port | yes | Default 3306 for MySQL, 5432 for PostgreSQL |
| database | PostgreSQL: yes | MySQL may omit the default database |
| username | yes | Database account used to execute allowed SQL; extra grants do not block registration |
| password | yes | Hidden prompt or `--password-stdin` only |
| TLS mode | yes | `disable`, `prefer`, `require`, `verify-ca`, `verify-full` |
| CA/cert/key paths | no | Store outside Agent-readable locations |
| environment/tags | no | Administrator metadata only |
| limits | no | Connect timeout, statement timeout, max rows |
| write_level | no | `none` (default), `dml`, or `ddl`; limits submitted SQL |

Example unattended metadata registration:

```bash
secret-command | db-operator-admin register \
  --name reporting-prod \
  --engine postgresql \
  --host db.internal.example \
  --port 5432 \
  --database reporting \
  --username report_reader \
  --ssl-mode verify-full \
  --password-stdin \
  --test
```

Never place a password in a URL, argument, environment variable, file, log, or chat. The admin CLI normalizes allowed URL fields, stores metadata in registry schema v3, and stores the password in the AES-256-GCM daemon vault. The vault master key and encrypted credential records are separate private files; decrypted strings are zeroized after use.

## Configuration Management UI

After service setup, start the local registration UI with `scripts/open-registry.ps1` on Windows or `scripts/open-registry.sh` on macOS and Linux; it requires Node.js and administrator privileges (on macOS/Linux the launcher re-enters through sudo, and the server refuses a non-root session). Do not open `ui/index.html` directly: it uses the local HTTP management API. Choose `none`, `dml`, or `ddl` as the registered write level. The page sends registration to the local admin service; it does not write credentials or registry files directly. Opening the UI does not install the query daemon.

On macOS and Linux the admin tool must run as the service identity (`db-operator` on Linux, `_dboperator` on macOS) so the registry, vault, and capability records keep their owner; the server re-enters every admin call that way and only root may do so. Issuance therefore asks for the Agent account: its home directory decides the `client.json` path, and the staged service-owned file is installed to that account with mode 0600.

The UI covers the full agent-authorization lifecycle, not only registration:

- List, create, edit, and delete connections. Creating a connection can auto-issue the first capability and write the Agent `client.json` immediately (skipped with a notice when another connection already holds the active capability; edits never re-issue). On macOS and Linux, auto-issuance also needs the Agent account filled in on the page, because that account decides the `client.json` path. A failed pre-save connection test (unreachable host, wrong credentials, timeout) leaves the registration unsaved and surfaces the driver error text.
- Show the currently active daemon query capability (connection alias and client config path only; capability values are never returned).
- Refresh the cached schema metadata (`schema refresh`).
- Issue an Agent client capability for a chosen connection (`client issue`) with an editable endpoint and client config output path, optionally restarting the OS service so the daemon loads the new capability. Each issue replaces the single daemon capability slot; the previously active connection stops being queryable until it is reissued. The issued capability carries the connection's registered `write_level` (fetched from the registry, not entered on the page), so a `dml`/`ddl` connection keeps its write scope; issuing through the admin CLI takes `--write-level` explicitly and defaults to `none`.

## SQL Authorization

Registration, connection tests, schema refresh, and queries do not inspect the database account's grants. Registration testing only authenticates and executes SELECT 1. The daemon validates each incoming SQL statement against the registered write level and the caller capability before connecting. Extra database privileges never expand this scope. If the account lacks the database permission needed for an allowed statement, execution returns the database error.

A `dml` or `ddl` level adds statements to the read-only set instead of replacing it: reads stay available on a writable connection under exactly the same checks, which is what makes a write verifiable with a follow-up SELECT. Statements outside the level are refused with the statement type or the blocked construct.

After metadata changes, run:

```bash
db-operator-admin schema refresh
db-operator-admin client issue --connection reporting-prod --endpoint <service-endpoint> --output <agent-client-json>
```

Each issued capability is bound to exactly one registered connection alias. Restart the OS service after rotating capability or connection metadata. Registry schema v2/keyring credentials are not migrated automatically; re-register them into schema v3.
