---
name: reviewing-java-code
description: >
  Use when reviewing a pull request, diff, or commit of Java/Spring code —
  before merge approval, when asked "does this look good?", or when checking
  someone else's (or your own earlier) changes. Also use when tempted to
  approve after skimming, when a large diff invites "looks fine overall",
  when review feedback is drifting into style opinions, or when a change
  touches transactions, async code, remote calls, or migrations and needs
  more than a surface read.
---

# Reviewing Java Code

Core principle: a review is a verification, not a vibe — "looks good to me" is only true if you can say what you looked at.

```
THE IRON LAW

NO APPROVAL WITHOUT READING EVERY CHANGED LINE
AND RUNNING THE CHECKS.
A skimmed approval is a forged signature.
```

You review the diff against the stated intent and these criteria — nothing else. If you cannot run the checks (no build access), say so explicitly in the verdict; never imply checks passed when they didn't run.

## The process — all seven passes, in order

### Pass 1: Understand intent

Read the ticket / PR description first, then the diff. Two questions:
- Does the diff do what the description claims?
- Does it do MORE than described? Flag scope creep — unrelated refactors, drive-by changes, new dependencies nobody asked for. Each is review surface the title hides.

**GATE: if you can't state the intent in one sentence, ask before reviewing further.**

### Pass 2: Correctness

Line by line — every changed line, including tests, configs, and migrations:
- Edge cases: null/`Optional` handling, empty collections, boundary values, off-by-one in loops and pagination
- Error paths: swallowed exceptions, `catch (Exception e) {}`, error mapping that loses the cause, missing rollback on failure
- Concurrency: shared mutable state in singleton beans (instance fields on `@Service`/`@Component`), non-thread-safe types (`SimpleDateFormat`, `HashMap`) across threads
- `@Async`/`@Scheduled` pitfalls: self-invocation that bypasses the proxy, lost exceptions (no `AsyncUncaughtExceptionHandler`), overlapping scheduled runs on slow tasks, `@Transactional` not propagating to async threads

### Pass 3: Data

- N+1 queries: a loop over entities touching lazy associations; new `findAll()` on unbounded tables
- New query predicates without a matching index
- Transaction boundaries: `@Transactional` on the right layer? Self-invocation bypassing it? Remote calls held inside a transaction?
- Migration safety: locking on large tables, non-null column without default, dropped/renamed columns under rolling deploy (old pods still reading)

Deep dives → `jpa-database-patterns`.

### Pass 4: Security

- Injection: string-concatenated queries (SQL/JPQL), unvalidated input reaching commands or templates
- Authorization at the boundary: new endpoint — who can call it? `@PreAuthorize`/security config updated? IDs from the path used without ownership check (IDOR)?
- Secrets in code, config files, or test fixtures
- Unsafe deserialization of external input
- Log leakage: PII, tokens, or full payloads in log statements — including new `toString()` on entities

### Pass 5: Resilience

Every NEW remote call (HTTP client, Kafka producer, DB to a new host):
- Timeout configured? (No timeout = default infinite on several clients)
- Retries: is the operation idempotent? Retrying a non-idempotent POST is a bug, not resilience
- Failure handling: what does the caller do when this dependency is down?

Deep dives → `resilience-performance`.

### Pass 6: Tests

- Do tests assert *behavior* (outcomes, state) or just verify mocks were called?
- Negative cases present for new behavior — not only the happy path?
- **The mutation question: if the production code were wrong, would these tests fail?** A test that passes against broken code is decoration.
- Weakened or deleted assertions hiding in the diff? A removed test is a blocker until justified.
- Run the suite where possible: `mvn verify` (Gradle: `./gradlew check`). Report actual output, not assumed output.

### Pass 7: Verdict with evidence

Order findings by severity, each with `file:line` + why it's a problem + a concrete fix:

| Severity | Meaning |
|---|---|
| **Blocker** | Wrong behavior, data loss, security hole, broken migration — cannot merge |
| **Major** | Will cause problems soon (missing timeout, untested error path, N+1 on a hot path) |
| **Minor** | Real but low-stakes (misleading name, dead branch, missing negative test on a cold path) |
| **Nit** | Take or leave; never blocks |

End with explicit verdict: approve / approve-with-comments / request changes — and what evidence supports it (lines read, checks run, output seen).

**Flag only genuine correctness and requirement gaps. Do NOT invent style nits to seem thorough** — five fabricated nits bury the one blocker. An empty findings list after a complete review is a valid, valuable result: say "read all N changed files, ran `mvn verify` (green), no findings."

**Circuit-breaker: 3+ blockers, or a diff too large to actually read line by line → stop the pass-by-pass review.** Return it: ask for the PR to be split or the design re-examined (`designing-systems`). Reviewing an unreviewable PR produces a worthless approval.

## Rationalization table

| Excuse | Reality |
|---|---|
| "The diff is huge, I'll review the important files" | The bug is in the file you skipped — that's why it was skipped. Read everything or return it as too large. |
| "CI is green, so the code works" | CI proves existing tests pass — not that the new tests test anything, nor that untested paths work. |
| "The author is senior, this is surely fine" | Review the code, not the byline. Seniors ship the same race conditions, faster. |
| "It's just a small config/migration change" | Config and migration changes are the top producers of production incidents. Small diff ≠ small blast radius. |
| "I'll approve now and they can fix the comments later" | "Later" merges. An approval with unresolved blockers is an approval of the blockers. |
| "I should find *something* to prove I read it" | Inventing nits is noise that buries signal. "No findings" after a real review is a finding. |
| "Tests exist, so the test pass is done" | Existence isn't quality. Would they fail if the code were wrong? That's the only question. |
| "I don't have time to run the checks" | Then the verdict must say checks weren't run. An approval that implies verification that didn't happen is false. |

## Red flags — stop if you catch yourself writing

- "LGTM" or "looks good overall" before reading every file in the diff
- "I skimmed the test changes"
- "This is probably fine"
- "Approving — minor comments can be addressed later" when one of them is actually a blocker
- A verdict with zero `file:line` references
- A findings list that is all style and naming on a diff that touches transactions or money
- "I didn't run the tests, but they should pass"

Any of these means: go back to the pass you shortcut.

## Verification checklist

Before delivering the verdict:

- [ ] Intent stated in one sentence; scope creep flagged or absent
- [ ] Every changed line read — code, tests, configs, migrations
- [ ] All seven passes done, in order — none skipped as "not applicable" without saying why
- [ ] Checks run (`mvn verify` / `./gradlew check`) with output reported, or explicitly marked not-run
- [ ] Every finding has severity + `file:line` + why + concrete fix
- [ ] No invented nits; no real blocker downgraded to keep things friendly
- [ ] Explicit verdict given, with the evidence behind it

## Related skills

- `jpa-database-patterns` — deep dive for Pass 3 findings (N+1, indexing, transactions)
- `resilience-performance` — deep dive for Pass 5 findings (timeouts, retries, circuit breakers)
- `tdd-java` — the standard Pass 6 holds tests against
- `designing-systems` — where an unreviewable or structurally wrong PR gets sent
- `spring-boot-standards` / `oop-design` — the baseline for what correct code looks like
- `dependency-management` — when the diff adds or bumps dependencies
