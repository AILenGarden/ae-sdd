# ae-sdd Shadow, Canary, Cutover, And Rollback Contract

## Writer Invariant

For one workspace at one instant, exactly one implementation may own project
mutation. Shadow mode is read/compare only. There is no Rust/Python double-write
mode and no client-side business fallback.

## Workspace Modes

| Mode | Writer | Rust responsibility |
| --- | --- | --- |
| `legacy` | migration oracle runtime | no daemon write; collect inventory only |
| `shadow` | legacy oracle | Rust reads/evaluates fixtures and records bounded mismatches |
| `rust-canary` | Rust daemon | selected drained workspace; legacy writer disabled |
| `rust-sole-writer` | Rust daemon | normal released mode; legacy runtime not installed |

## Transition Preconditions

### `shadow -> rust-canary`

- compatibility inventory contains exactly 113 CLI leaves, 23 operations (18
  migrated from the Python surface plus 5 `native-addition` entries with no
  Python predecessor), 36 Gates, and 7 scanners;
- mapped Rust owner and native/golden evidence exist for every entry used by the
  selected workspace;
- T4 daemon/Hook/FlowRuntime/delegation/context contracts pass;
- T5 CLI/build/install/distribute parity passes;
- the workspace is drained and a final project-state digest is captured;
- the legacy writer is disabled before the Rust writer is enabled.

### `rust-canary -> rust-sole-writer`

- canary soak has no blocker mismatch, double write, lost mutation, stale PASS,
  cross-workspace leakage, false physical child, or false compact ACK;
- restart/journal/reconciliation tests pass;
- Hook cached p95 and pressure budgets pass in release profile;
- actual Windows, macOS, and Linux lifecycle/security jobs pass;
- upgrade and pre-deletion rollback have been exercised.

## Python Deletion Gate

Legacy deletion is authorized only when all of these are true:

1. T4 owner confirms daemon/CLI/Hook/FlowRuntime parity and exact 113-route reachability.
2. T5 owner confirms tooling/build/install/distribute parity and compatibility audit PASS.
3. `legacy-runtime-cutover.v1.json` has no `blocked`, `missing`, `stub`, or
   `non-pass-fallthrough` entry.
4. `ae-sdd-build compatibility-audit` passes the 113/24/36/7 manifest.
5. `ae-sdd-build verify-release` reports three native binaries and zero Python,
   interpreter, legacy CLI, source fallback, or script-wrapper marker.
6. The parent execution owner explicitly authorizes the recorded deletion set.

Until then, legacy files remain a read-only migration oracle and are not bundled
into a release. Keeping the oracle does not authorize a runtime fallback.

## Deletion Set

The exact tracked deletion candidates and their Rust owner/evidence requirement
are in `legacy-runtime-cutover.v1.json`. Deletion is one reviewed change after the
gate above passes; no broad filesystem wildcard is used.

The Monitor (`apps/ae-sdd-monitor/**`) is excluded from this Work Item and never
appears in the deletion set.

## Rollback

Before legacy deletion, rollback drains the canary workspace, disables Rust
writer admission, verifies the recorded schema-compatible legacy reader, and
atomically restores legacy writer mode. It never runs two writers.

After legacy deletion, rollback installs the previous complete signed native
release, restores its generated service descriptor, restarts the daemon, and
reconciles the project-authoritative journal. The Rust client does not retain a
hidden legacy fallback.

## Cutover Evidence

Required evidence includes mode events, before/after state and policy digests,
drain receipt, writer identity, compatibility manifest digest, release scan,
canary duration, mismatch counts, double-write count, lifecycle CI URLs/logs,
rollback result, and authorizing actor/reason.

