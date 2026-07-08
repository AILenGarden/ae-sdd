# 2026-07-07 | ae-sdd v3.9.3 - 输出核心原则第 4 条「禁止文档承载 changelog」

## Summary

ae-sdd 主入口 SKILL.md「🔴 输出核心原则」原 3 条（基于事实 / 禁止猜测 / 禁止杜撰）扩为 4 条，新增「禁止文档承载 changelog」，要求设计/架构/模板/标准类文档只写当前生效内容，历史变更统一走 `source/CHANGELOG/{YYYY-MM-DD}-{主题}.md`，文档内仅做引用。本次同步在用户级 `AGENTS.md`「强制红线」表加第 11 条，与 ae-sdd 母版声明同源、形成引用闭环。痛点是之前在设计/模板/章节正文混写「变更记录 / 版本号 / 更新历史」段落，破坏主题连续性、加剧检索劣化、让 git blame 失真——属于坏习惯而非细节。

## Changes

| Area | Change |
|---|---|
| source/skill-fallbacks/SKILL.full.md L286 | 输出核心原则表新增第 4 行「🆕 禁止文档承载 changelog」|
| source/SKILL.md frontmatter | version 3.9.2 → 3.9.3；description 增加 v3.9.3 变更条目 |
| README.md L5 | 「版本：v3.9.2（🆕 2026-07-06 …）」→「v3.9.2（🆕 2026-07-07 …）」+ 一句话增项 |
| C:\Users\EDY\.zcode\AGENTS.md L8-L21 | 「强制红线（10条，绝对禁止）」表加第 11 条 + 改标题为「强制红线（绝对禁止）」 + 附注引用关系 |

## 触发原因

- 用户对话内明确说"不要在文档内写 changelog，这是什么坏习惯"，定性为「坏习惯」而非「细节规范」——意味着要放在最高原则层而非禁止事项表。
- ae-sdd-update-skill.md §步骤 4.5 强制要求"修改 SKILL 母版必须写 CHANGELOG"——本次新增原则属于母版行为变更，本次是首次以「声明式原则」而非「门禁 / 工具 / 数据结构」形式扩展 §「输出核心原则」。
- ae-sdd 自身体系已经按「变更走 CHANGELOG/，文档做引用」运转，但缺一条明文全局原则，导致偶发回归（如 ae-sdd-monitor-app.md 等设计中偶现「变更记录」段落）。

## 影响范围

- **行为/语义变化：** 设计师/Agent 撰写/修改任何 ae-sdd 母版设计文档、模板、Standard 时，禁止在正文混写历史变更条目；必须改走 `source/CHANGELOG/` 独立文件。本次**纯声明 + 同步版本行 + 写 CHANGELOG**，未触碰任何门禁语义、CLI、子 SKILL 边界。
- **门禁行为：** 无变化（不新增 gate，也不改既有 gate 行为）。
- **CLI：** 无变化。
- **版本号：** 推进 `source/SKILL.md` version 3.9.2 → 3.9.3（additive 增强，按 ae-sdd 修订号位推进）。
- **破坏性变更：** 无。`README.md`「版本」行的「最新变更」括号内短句风格与既有写法一致；`AGENTS.md` 表行新增第 11 条，标题从「10条」改为「N 条」开放（不再限数）。

## 验证方式

- `python tools/bin/ae-sdd update-check` UC-01（README:5 / SKILL.md / paths.py 三处版本号一致）全绿。
- `python tools/bin/ae-sdd update-check` UC-05（ae-sdd SKILL 边界 / changelog 触发）无新增连带。
- `slim entry.source_fallback_sha256` 与 `skill-fallbacks/SKILL.full.md` 当前 sha256 一致（已重算同步）。
- 人工核对：
  - `source/SKILL.md` frontmatter `version: 3.9.3`、`description` 含 v3.9.3 条目。
  - `source/skill-fallbacks/SKILL.full.md` L286 起表第 4 行存在。
  - `README.md:5` 版本号与 SKILL.md 一致（v3.9.3），「最新变更」括号内提及「禁止文档承载 changelog」并指向 CHANGELOG/。
  - `C:\Users\EDY\.zcode\AGENTS.md` 强制红线表含 11 行、含「变更历史必须落到 CHANGELOG/」附注。
- **遗留项**（不在本次范围，下一次「adapter adapter 重生成」时一并修复）：
  - UC-07 `.harness/.adapter.lock` 的 `ae_sdd_version=3.9.1` / `source_input_sha256` 漂移——属 ae-sdd-harness-adapter SKILL 派生资产，须按 `ae-sdd-update-skill.md §项目结构与设计说明 / ⑥ Harness 适配层` 调用 adapter SKILL 重跑（不手工改 lock）。本次接受该项 warn。
- 后续工作（不在本次）：另起 PR 清理 `source/docs/`、`source/templates/`、`source/standards/` 里现存的内联 changelog 段落，并生成清理清单。

## Reviewer

陈聪
