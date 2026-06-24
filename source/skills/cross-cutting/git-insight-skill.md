---
name: git-insight
description: Read-only Git insight toolset for ae-sdd. Produces structured status, diff, log, blame, and impact evidence for CodingPlan, CodingReport, CodeReview, and postmortem.
---

# Git Insight Skill

## 1. Purpose

Git Insight turns repository history into structured ae-sdd evidence.

It answers:

- What changed?
- Which modules are affected?
- Which risky surfaces are touched?
- What is the relevant file history?
- Did Coding include unrelated changes?

## 2. Commands

```bash
ae-sdd git status
ae-sdd git diff --base <ref> --head <ref>
ae-sdd git log --path <file> --limit 20
ae-sdd git blame --file <file>
ae-sdd git impact --file <changed-file>
```

## 3. Read-Only Boundary

This Skill must not mutate Git state. It never runs `add`, `commit`, `tag`,
`branch`, `checkout`, `reset`, `clean`, or equivalent destructive shell
compositions.

## 4. Required Usage

| Node | Required Usage |
|---|---|
| CodingPlan | run impact/history when touching legacy or shared files |
| CodingReport | cite changed file evidence |
| CodeReview | use diff and impact output before conclusions |
| Postmortem | use log/blame when tracing repeated defects |

## 5. Risk Hints

`ae-sdd git impact` must produce risk hints for:

- DB/SQL changes
- API surface changes
- security/auth changes
- test changes

Risk hints are not final conclusions. They are prompts for mandatory review
evidence.
