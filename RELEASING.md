# ae-sdd Native Release Guide

This guide defines the native Rust release boundary. Creating files or a CI
workflow is not release evidence; the commands and actual platform jobs below
must pass.

## Release Contents

| Artifact | Requirement |
| --- | --- |
| `ae-sdd` | thin native CLI/Hook/admin client |
| `ae-sddd` | native per-user daemon |
| `ae-sdd-build` | native build/audit helper |
| methodology package | declarative SKILL, Harness, standards, templates, assets, protocol metadata |
| service descriptors | generated Windows user task, macOS LaunchAgent, Linux systemd user unit |
| manifests | binary/package/service digests, compatibility and supply-chain metadata |

The release must not contain Python source/bytecode, an interpreter command,
`tools/bin/ae-sdd`, script wrappers, a local Gate/state implementation, or a
logical-spawn/compact fallback. `apps/ae-sdd-monitor/**` is a separate product and
is excluded from this release.

## Required Build And Audit

```text
cargo build --workspace --locked --release
cargo test --workspace --locked --release
cargo run -p ae-sdd-build --locked --release -- compatibility-audit \
  --manifest tests/fixtures/compatibility/legacy-surface.v1.json \
  --expected-commands 113 --expected-operations 24 \
  --expected-gates 36 --expected-scanners 7 \
  --exclude apps/ae-sdd-monitor/**
cargo run -p ae-sdd-build --locked --release -- verify-release \
  --artifact-dir target/release --exclude apps/ae-sdd-monitor/**
cargo deny check
cargo audit
```

Compatibility audit must map every surface to a Rust owner, fixture, and real
evidence. `stub-pass` and non-PASS fallthrough counts are zero. Release scan must
find all three native binaries and zero forbidden runtime marker.

## Cross-Platform Matrix

Actual Windows, macOS, and Linux CI jobs each verify:

- native compile/test and locked dependency graph;
- install, start, handshake/status, drain, stop, upgrade, rollback, uninstall;
- current-user-only endpoint manifest and Named Pipe/UDS access;
- journal recovery and endpoint cleanup after restart;
- release-profile 100-Agent/10-workspace pressure and Hook latency histogram;
- service manager behavior using the generated descriptor for that platform.

Single-host simulation cannot close the platform gate. Preserve the CI run URL,
runner image, toolchain/lock digest, command, exit code, and redacted logs as
evidence.

## Service Lifecycle

The generated-source contract is
[`source/skill-fallbacks/runtime/service-lifecycle-contract.md`](source/skill-fallbacks/runtime/service-lifecycle-contract.md).
Upgrade always verifies the candidate, drains the daemon, checkpoints state,
stops the old process, atomically promotes binary/descriptor, starts, and
handshakes. A failed handshake restores the previous complete native release;
it never launches a legacy script.

## Cutover Gate

Migration proceeds `legacy -> shadow -> rust-canary -> rust-sole-writer` per
workspace. Shadow is read-only comparison. Drain disables the previous writer
before enabling the next writer; double-write count must remain zero.

Legacy deletion requires all of the following:

1. T4 confirms daemon/CLI/Hook/FlowRuntime and exact 113-route reachability.
2. T5 confirms tooling/build/install/distribute parity.
3. Compatibility, release, pressure, recovery, and actual platform matrices pass.
4. Every entry in
   `source/skill-fallbacks/runtime/legacy-runtime-cutover.v1.json` has a real
   evidence reference and no blocked/missing/stub status.
5. The execution owner explicitly authorizes that exact recorded path set.

Before those conditions, legacy files remain only as a migration oracle and are
not bundled. After deletion, rollback means installing the previous complete
native release; the Rust client contains no business fallback.

## Publish Evidence

A release is publishable only when evidence identifies source revision, Cargo
lock digest, Rust version/targets, binary/package/service digests, compatibility
manifest digest, platform CI runs, performance percentiles/CPU/RSS/errors,
cutover mode events, rollback result, and review status/findings.
