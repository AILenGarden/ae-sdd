# ae-sdd User Service Lifecycle Contract

## Scope

One `ae-sddd` runs per OS user and serves every allowed workspace for that user.
The installed Rust `ae-sdd` client is the only lifecycle and business client.
Service descriptors are generated from this contract during build/install; they
are not hand-maintained in a user's service directory.

## Daemon Command

Every service manager ultimately launches:

```text
ae-sddd serve --state-dir <protected-user-state-dir> --allowed-root <root>...
```

The service environment supplies explicit allowed roots. No descriptor embeds an
endpoint token, capability private key, user prompt, workspace state, or other
secret. The daemon creates those values at boot and atomically publishes the
protected endpoint manifest.

## Generated Descriptors

| OS | Generated destination | Native manager | Required identity/security |
| --- | --- | --- | --- |
| Windows | per-user Task Scheduler definition for `ae-sdd-daemon` | `schtasks.exe` / Task Scheduler API | current user SID, interactive logon token, no stored password, state/manifest DACL restricted to that SID |
| macOS | `~/Library/LaunchAgents/com.ae-sdd.daemon.plist` | `launchctl bootstrap gui/<uid>` | current login UID, descriptor owned by user, runtime directory `0700`, manifest/socket `0600` |
| Linux | `~/.config/systemd/user/ae-sdd.service` | `systemctl --user` | user manager, `UMask=0077`, runtime directory `0700`, manifest/socket `0600` |

All descriptors set restart-on-failure with bounded backoff, preserve the same
protected state directory across restart, and send ordinary logs only to the
per-user daemon log or native journal. They never log endpoint tokens or payloads.

## Lifecycle

| Operation | Required sequence | Success evidence |
| --- | --- | --- |
| install | verify native release -> stage binaries -> render descriptor -> protect files -> register manager -> start -> handshake/status | descriptor digest, binary digest, boot ID, current-user ACL assertion |
| start | ask native manager to start -> wait for a new protected manifest -> handshake with expected boot/policy | running status and matching boot/policy/build |
| status | `ae-sdd runtime status` | lifecycle, boot ID, build, policy digest, active sessions/jobs; secrets redacted |
| drain | `ae-sdd runtime drain` -> stop admissions -> bound in-flight work -> checkpoint | drain receipt and zero unbounded in-flight mutations |
| stop | drain unless already drained -> `ae-sdd runtime stop` -> native manager stop -> remove stale endpoint manifest | stopped status and absent endpoint |
| logs | `ae-sdd runtime logs` | bounded/redacted records; no endpoint token, capability, prompt, or claim token |
| upgrade | verify candidate -> drain -> checkpoint -> stop -> atomic binary/descriptor promote -> start -> handshake/schema check | before/after build digests, boot ID change, compatibility evidence |
| uninstall | drain/stop -> unregister descriptor -> remove installed binaries/endpoint -> retain durable state unless explicit purge | unregister receipt, endpoint absent, retained-state declaration |

Lifecycle commands are idempotent. Repeating install/start/stop/uninstall returns a
stable receipt or current state, not a second daemon or destructive surprise.

## Platform Requirements

### Windows

- The scheduled task is per-user and runs only as that user; it does not request
  SYSTEM or administrator identity.
- The Named Pipe and endpoint manifest permit only the current user SID.
- Upgrade uses a staged binary outside the live path and an atomic promote after
  drain. A locked binary is replaced only through the native deferred/restart
  path recorded in the receipt.

### macOS

- The LaunchAgent label is `com.ae-sdd.daemon`; installation targets the current
  `gui/<uid>` domain, never a system LaunchDaemon.
- The generated plist contains `ProgramArguments` as an array, not a shell
  command. `KeepAlive` is failure-oriented and shutdown remains drain-aware.

### Linux

- The generated unit is a user unit. `ExecStart` is an absolute Rust binary plus
  argument vector; no shell or script wrapper is used.
- Install/upgrade performs `systemctl --user daemon-reload`, then enable/start or
  restart only after verification and drain.

## Fail-Closed Rules

- A descriptor must not call a repository script or interpreter.
- A missing/invalid candidate binary prevents install or upgrade.
- ACL/permission verification failure prevents endpoint publication and service
  readiness.
- A failed post-upgrade handshake triggers rollback to the previous complete
  native release; it never activates a Python fallback.
- Creating descriptor files is not cross-platform evidence. Windows, macOS, and
  Linux actual CI lifecycle jobs must pass before release cutover.

## Lifecycle Evidence Shape

```json
{
  "schemaVersion": "ae-sdd-service-evidence/v1",
  "platform": "windows|macos|linux",
  "operation": "install|start|status|drain|stop|upgrade|uninstall",
  "binaryDigest": "sha256",
  "descriptorDigest": "sha256",
  "bootIdBefore": "optional-uuid",
  "bootIdAfter": "optional-uuid",
  "aclAssertions": [],
  "receipts": [],
  "status": "PASS|FAIL"
}
```

