# ae-sdd

> **版本：** v3.14.0

ae-sdd is a Rust daemon-backed engineering workflow runtime for multiple coding
Agents and workspaces. One per-user `ae-sddd` owns state, Gates, transitions,
delegation, context projections, jobs, and audit. Agents and Hooks use the thin
`ae-sdd` CLI; the SKILL package declares methodology, templates, and output
contracts only.

## Runtime Model

```text
many Agent sessions / Hooks / CLI users
                    |
             Rust ae-sdd client
                    |
          Named Pipe or Unix socket
                    |
                 ae-sddd
       +------------+-------------+
       |            |             |
 FlowRuntime   WorkItemActor   FlowSupervisor
       |            |             |
 delegation    Gates/store    context/compact
```

The root Agent session only advances typed actions, creates delegations,
collects bounded child results, and reports progress. RA, DR, Story, Coding,
Test, and Review series run in independent physical Agent sessions. Root never
imports child transcripts or unbounded source/test payloads.

Route precedes Requirement Analysis. RA then selects the required DR, Story, or
compact `state.executionPlan` depth. Coding requires the declared upstream
contexts, AC-to-verification mapping, blocker Gate PASS, and explicit user plan
approval.

## Native Binaries

| Binary | Purpose |
| --- | --- |
| `ae-sddd` | per-user daemon (`serve`) |
| `ae-sdd` | CLI, Hook, admin, and host-adapter client |
| `ae-sdd-build` | compatibility/release audits, package/service generation, benchmarks |

Build from source:

```text
cargo build --workspace --release
cargo test --workspace --release
cargo run -p ae-sdd-build --release -- compatibility-audit --manifest tests/fixtures/compatibility/legacy-surface.v1.json
cargo run -p ae-sdd-build --release -- verify-release --artifact-dir target/release
```

The released runtime contains native binaries and generated methodology/service
assets. It does not contain an interpreter, Python worker, local business
fallback, or script-based Hook.

## Daemon And Hooks

Direct daemon entry for development:

```text
ae-sddd serve --state-dir <protected-dir> --allowed-root <workspace-parent>...
```

Installed lifecycle commands:

```text
ae-sdd runtime start
ae-sdd runtime status
ae-sdd runtime drain
ae-sdd runtime logs
ae-sdd runtime stop
```

Hook commands consume one JSON wrapper from stdin and emit one host-compatible
outcome:

```text
ae-sdd hook --method hook.user_prompt --request-json -
ae-sdd hook --method hook.pre_tool --request-json -
ae-sdd hook --method hook.post_tool --request-json -
ae-sdd hook --method hook.stop --request-json -
```

An engaged Hook fails closed when the daemon is unavailable. It never runs a
local Gate or mutates state outside the daemon.

## Repository Layout

```text
bins/                  ae-sdd and ae-sddd binaries
crates/                domain, protocol, policy, store, Gates, runtime, client,
                       integrations, build, delegation, context, and host crates
migrations/            runtime metadata migrations
source/                declarative SKILL, standards, templates, and assets
tests/fixtures/         compatibility and protocol golden corpus
dist/                   generated release/package outputs
.github/workflows/      actual Windows/macOS/Linux Rust validation
apps/ae-sdd-monitor/    excluded from this runtime Work Item
```

`tools/` and `scripts/` are retained temporarily as a read-only migration oracle.
They are not release inputs or runtime fallbacks. Their exact deletion gate is
recorded in
[`source/skill-fallbacks/runtime/cutover-contract.md`](source/skill-fallbacks/runtime/cutover-contract.md)
and the 160-path machine manifest beside it.

## Contracts

- [`source/SKILL.md`](source/SKILL.md): declarative runtime method/template/output contract
- [`source/HARNESS.md`](source/HARNESS.md): root Agent and Hook contract
- [`source/docs/ae-sdd-design.md`](source/docs/ae-sdd-design.md): current capability design
- [`source/docs/ae-sdd-implementation-architecture.md`](source/docs/ae-sdd-implementation-architecture.md): Rust implementation boundaries
- [`RELEASING.md`](RELEASING.md): native release, service, cutover, and rollback gates

## License

See [LICENSE](LICENSE).
