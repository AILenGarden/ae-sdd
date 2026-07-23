# ae-sdd Agent Harness

This Harness connects an Agent host to the Rust ae-sdd daemon. It does not embed
the workflow executor. `FlowRuntime` is the sole process owner.

## Runtime Boundary

- Invoke the installed Rust `ae-sdd` binary only.
- Never run repository scripts or evaluate Gate/state logic in the Agent process.
- Never read or mutate daemon endpoint credentials directly; the Rust client
  performs manifest, handshake, boot, policy, and capability validation.
- If the daemon is unavailable, an engaged session stops. There is no local
  business fallback.

## Hook Mapping

| Host event | Rust client command |
| --- | --- |
| SessionStart (when the host supports it) | `ae-sdd runtime ensure --quiet` |
| UserPromptSubmit | `ae-sdd hook --method hook.user_prompt --request-json -` |
| PreToolUse | `ae-sdd hook --method hook.pre_tool --request-json -` |
| PostToolUse | `ae-sdd hook --method hook.post_tool --request-json -` |
| Stop | `ae-sdd hook --method hook.stop --request-json -` |

`SessionStart` is an eager prewarm, not a correctness dependency. Every
daemon-bound CLI command and each trusted, session-bound Hook still performs its
own call-first/recover-once path: if the endpoint is missing or the local
transport is unavailable, the Rust client ensures the daemon is ready and then
replays the original request once. Hosts without `SessionStart`, and sessions
in which the daemon exits after startup, therefore use the same recovery path.
An unbound host event is prewarmed when possible but remains fail-closed because
there is no authenticated session/capability to send to the daemon.

A per-user daemon that serves several workspaces must receive their trusted
parent roots through `AE_SDD_ALLOWED_ROOTS` before its first bootstrap (a normal
OS path list; `;` on Windows). If it is unset, bootstrap admits the current
workspace only and later clients cannot silently widen that security boundary.

`-` means one JSON request on stdin. The wrapper is:

```json
{
  "params": {},
  "engaged": true,
  "offlineCapability": "optional-ed25519-capability",
  "nowUnixMs": 0
}
```

The Rust client writes one host-compatible Hook outcome to stdout. On an engaged
transport failure it still emits the host's deny/block JSON contract within the
Hook deadline; it does not run a local Gate.

Hosts may additionally map native lifecycle events when supported:

| Host capability | Runtime contract |
| --- | --- |
| SessionStart | prewarm with `runtime ensure --quiet`; the first engaged turn opens the typed session |
| SubagentStart | `delegation.accept`, then `session.open` |
| SubagentStop | `delegation.report`, then `session.close` |
| PreCompact | `compact.request` |
| PostCompact | `context.project`; native ACK is a separate `host.action_ack` |

Unsupported events remain explicitly unsupported. The adapter must not invent a
physical claim or compact ACK.

On Windows the daemon listens on a per-user Named Pipe described by the protected
endpoint manifest. It does not bind a TCP/HTTP port, so no daemon port setting or
firewall rule is required. A current-user Scheduled Task may run `ae-sdd runtime
ensure` at sign-in as an optional latency prewarm; Hooks and CLI recovery remain
the authoritative startup mechanism.

## Agent Roles

| Role | Responsibility | Context allowed |
| --- | --- | --- |
| root | advance typed actions, delegate, collect, ask user, report | flow projection plus bounded child results |
| series | own one RA/DR/Story/Coding/Test/Review series | series projection and scoped sources |
| task | implement one scoped task | task projection and allowed paths |
| reviewer | independently review scoped artifacts | review projection, no worker scratch memory |

The lineage is `root -> series -> task|reviewer`; maximum depth is 2.

## Root Main Session

The main session must stay small:

1. Read the Hook-provided context or request `flow.next`.
2. Execute only the returned orchestration action.
3. Use `delegation.create` for semantic series work.
4. Wait for physical host attestation, not a logical prompt-only child.
5. Use `delegation.collect` and retain only the validated bounded result.
6. Report progress/result to the user and request the next runtime action.

The main session never imports a child transcript, raw source bundle, unbounded
test output, or child memory. It may retain summaries (8 KiB maximum), finding
counts, artifact path/hash/kind references, receipts, and next actions. Only root
may request a global transition, and `FlowRuntime` still decides whether it is
legal.

## Process Contract

Route precedes Requirement Analysis. RA then selects DR, Story, or compact
`executionPlan` depth. Coding begins only after required upstream documents,
verification mapping, blocker Gates, and explicit user plan approval are
committed. Testing records real evidence. Review records status/findings only.

At any disagreement, daemon state and typed errors override this Harness. Method,
template, and output details are declared in the package-root `SKILL.md`.
