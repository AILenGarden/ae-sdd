# Project Context

> This file is filled in by the user and read by the DDD Architecture Coach.
> Edits here do NOT need any manual sync — the skill's personal cursor (`.claude/arch-state.md` `last_touched.at`) is updated automatically when you next invoke a phase command.
> Format: YAML-in-Markdown. The coach parses ```yaml blocks.
> Write your content in whatever language you work in — Chinese or English is fine.

---

## Project Description (required)

3-5 sentences describing the product, target customer, and core value proposition.

Example (in Chinese):

> 「悠約」是多租戶 AI 客服 + 預約排程 SaaS。通路為 Telegram Bot，通知包含 Bot 文字、語音電話、SMS。目標客戶是中小企業主，核心價值是「商家接上 Telegram，顧客用自然語言就能完成預約」。一人團隊、預算敏感、選擇 self-host LLM 控制推論成本。

Your description:

```
<write your project description here>
```

---

## Coach Output Root (required; set during Bootstrap)

Where the DDD Architecture Coach writes its outputs (discovery / decisions / spec for every BC). Default `docs/ddd/`. Common alternatives: `docs/architecture/` (mixed with general architecture docs), `docs/` (no existing docs folder), `packages/<name>/docs/ddd/` (monorepo sub-package). Use a relative path from your project root; trailing slash optional.

```yaml
coach_output_root: docs/ddd/
```

---

## Tech Stack (required; mark unresolved fields as TBD)

```yaml
backend:
  language:              # e.g., C# 13 / Go 1.23 / Node.js 22
  framework:             # e.g., .NET 10 / Gin / NestJS
  architecture_style:    # e.g., Modular Monolith + Hexagonal per BC

frontend:
  framework:             # e.g., React / Vue / SwiftUI
  target_platforms:      # Web / iOS / Android / Desktop

database:
  primary:               # e.g., PostgreSQL 17
  cache:                 # e.g., Redis / Upstash

llm:
  model:                 # e.g., Gemma 4 E4B / gpt-4o-mini / Claude Haiku
  hosting:               # self-host / OpenAI / Anthropic / Google API / other
  reasoning:             # why this choice (cost / latency / privacy / capability)

messaging:               # e.g., MediatR in-process / Kafka / RabbitMQ / none
```

---

## Cloud Provider (required)

```yaml
primary:                 # e.g., AWS / GCP / Azure / Vultr / RunPod / self-host
llm_inference:           # if different from primary, e.g., RunPod
cdn:                     # e.g., Cloudflare / none
```

---

## Team & Constraints (required)

```yaml
team_size:               # 1 / 2-5 / 6-15 / 16+
budget_sensitivity:      # Low / Medium / High
timeline:                # rough deadline or milestones, e.g., MVP 3 months, beta 6 months

special_focus:           # pick 2-3, don't select all
  - AI feasibility
  # - performance
  # - cost
  # - team onboarding speed
  # - compliance
```

---

## bc-developer Model (optional, set by Bootstrap)

Bootstrap will ask which model the `bc-developer` subagent should use. The selected value is recorded here so it survives reprovisioning. If unset, Bootstrap defaults to `sonnet`.

```yaml
bc_developer_model:        # sonnet (default, balanced) | haiku (fast, routine TDD) | opus (deep reasoning, complex domains)
```

---

## Existing Decisions (optional)

If this is an existing project (not greenfield), list decisions already made. If greenfield, leave this blank and the coach will run Phase 1 in full mode.

```yaml
existing_decisions:
  - decision:            # e.g., Modular Monolith
    reason:              # e.g., 1-person team, microservices overhead not justified
  - decision:
    reason:
```

---

## Domain Constraints (optional)

Regulatory, industry, or security requirements. The coach will factor these into Phase 2/3 decisions.

```yaml
domain_constraints:
  # - GDPR / CCPA personal data protection
  # - HIPAA medical compliance
  # - PCI DSS payment compliance
  # - financial transactions, zero-tolerance errors
  # - special-industry content moderation (adult / medical / financial)
  # - strict multi-tenant data isolation
```

---

## Notes (optional)

Any other context, preferences, or landmines the coach should know about.

```
<free-form>
```
