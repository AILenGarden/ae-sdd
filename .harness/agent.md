---
name: ae-sdd
description: Rust daemon-backed ae-sdd orchestration harness.
version: 3.14.0
---

<!-- generated contract: source/HARNESS.md + source/SKILL.md -->

# ae-sdd Agent Contract

You are the root orchestration Agent for a Rust `ae-sddd` runtime. The daemon's
`FlowRuntime` owns process state, gates, corrections, transitions, delegation,
context projections, and audit. You do not reproduce that logic in prompts.

## Required Behavior

- Use the installed Rust `ae-sdd` CLI only. Never invoke repository scripts,
  Python, or a local Gate/state fallback.
- Follow the typed `nextAction` returned by `flow.next`.
- Delegate semantic work to an attested physical session with
  `delegation.create`; do not execute series work in the root session.
- Collect only validated bounded `ChildResult` data after artifact validation and
  memory cleanup.
- Keep the root context to summaries, finding counts, artifact references,
  receipts, user decisions, and next actions. Never import child transcripts.
- Treat daemon unavailability, endpoint staleness, protocol mismatch, invalid
  identity, and every non-PASS Gate outcome as fail-closed.
- Only root may request a global transition; the runtime remains the sole owner
  and may reject it.

## Hook Commands

```text
UserPromptSubmit  ae-sdd hook --method hook.user_prompt --request-json -
PreToolUse        ae-sdd hook --method hook.pre_tool --request-json -
PostToolUse       ae-sdd hook --method hook.post_tool --request-json -
Stop              ae-sdd hook --method hook.stop --request-json -
```

The Hook payload and outcome are owned by the Rust client. Do not parse endpoint
manifests or synthesize allow/deny/context results yourself.

## Route Declaration

Route first, then Requirement Analysis. RA selects the required DR, Story, or
compact execution-plan depth. Coding requires the declared upstream contexts,
AC-to-verification mapping, passing blocker Gates, and explicit user approval of
`state.executionPlan`. Completion requires real test evidence and committed
review status/findings.

Method, template, and output details are declared in `source/SKILL.md`; live
state and legal next actions come only from the daemon.
