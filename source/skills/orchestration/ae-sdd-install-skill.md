---
name: ae-sdd-install
description: ae-sdd 安装引导 SKILL。当 Agent 需要安装/重装/升级 ae-sdd 时触发，引导完成：平台检测 → 选择安装模式 → 执行 install → 写 harness hooks → 验证。当用户说"安装 ae-sdd"/"装 ae-sdd 到 <项目>"/"给 <项目> 接 ae-sdd"/"重装 ae-sdd"/"升级 ae-sdd"/"卸载 ae-sdd"时触发。
version: 1.0.0
allowed_tools:
  - "ae-sdd"   # CLI（init-hooks 子命令）
  - "Bash"     # 执行 install.sh / install.ps1
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/orchestration/ae-sdd-install-skill.full.md
source_fallback_sha256: 100b5c1d3373244805584f8e9696b2a260176fc0919cb68ea2a8f05ad192d3b1
source_original_bytes: 11416
source_original_lines: 287
source_semantic_inventory_sha256: d22d7c3f588a8bb49308814ddb357f9cab6a3bbe611d2b895dea40053f33a64e
source_slimmer: slim_source_skills.py@2
---

# ae-sdd Install — 安装引导 SKILL Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/orchestration/ae-sdd-install-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/orchestration/ae-sdd-install-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/orchestration/ae-sdd-install-skill.full.md`, not this slim entry.

## Summary

- source: `skills/orchestration/ae-sdd-install-skill.md`
- fallback: `skill-fallbacks/skills/orchestration/ae-sdd-install-skill.full.md`
- fallback_sha256: `100b5c1d3373244805584f8e9696b2a260176fc0919cb68ea2a8f05ad192d3b1`
- original_lines: 287
- original_bytes: 11416
- semantic_inventory_sha256: `d22d7c3f588a8bb49308814ddb357f9cab6a3bbe611d2b895dea40053f33a64e`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: ae-sdd 安装引导 SKILL。当 Agent 需要安装/重装/升级 ae-sdd 时触发，引导完成：平台检测 → 选择安装模式 → 执行 install → 写 harness hooks → 验证。当用户说"安装 ae-sdd"/"装 ae-sdd 到 <项目>"/"给 <项目> 接 ae-sdd"/"重装 ae-sdd"/"升级 ae-sdd"/"卸载 ae-sdd"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description, version; headings: L2:18 0. 触发场景与产物对照; L1:129 1. 主入口存在; L2:242 8. 触发后输出模板; keyword_hits: 14 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 4 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L1:41 3. Python（必须 3.8+）; L1:142 ⚠️ 注意：没有 .ae-sdd/ 时 gate-intercept 默认放行（见 source/HARNESS.md）; L2:278 10. 禁止; keyword_hits: 6 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L1:10 ae-sdd Install — 安装引导 SKILL; L3:78 3.1 模式 A：远程一行命令（最常见）; L1:107 或：python scripts/build_dist.py && python scripts/install.py; +3 more; keyword_hits: 66 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L1:113 项目级 hook（写到 <项目>/.claude/settings.json）; L1:116 全局级 hook（写到 ~/.claude/settings.json）; L1:138 或 项目级：cat <项目>/.claude/settings.json; +1 more; keyword_hits: 18 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:242 8. 触发后输出模板; keyword_hits: 7 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 21; refs: .ae-sdd/; .claude/settings.json; /ae-sdd; +18 more; headings: L1:142 ⚠️ 注意：没有 .ae-sdd/ 时 gate-intercept 默认放行（见 source/HARNESS.md）; keyword_hits: 35 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 11 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L2:165 5. 常见失败（FAQ，按 OS 分组）; keyword_hits: 1 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 10 | ae-sdd Install — 安装引导 SKILL |
| 2 | 18 | 0. 触发场景与产物对照 |
| 2 | 29 | 1. 平台检测（必跑，硬前置） |
| 1 | 34 | 1. OS |
| 1 | 36 | Windows 用 $env:OS 或 [System.Environment]::OSVersion |
| 1 | 38 | 2. Shell |
| 1 | 41 | 3. Python（必须 3.8+） |
| 1 | 44 | 4. Git |
| 1 | 47 | 5. Claude Code / Codex 是否已装 |
| 1 | 51 | 6. ae-sdd 是否已装 |
| 1 | 55 | 7. 项目根（如果是给项目接 ae-sdd） |
| 2 | 63 | 2. 选择安装模式（4 选 1） |
| 2 | 76 | 3. 执行 install（按平台分支） |
| 3 | 78 | 3.1 模式 A：远程一行命令（最常见） |
| 3 | 102 | 3.2 模式 B：本地 build + install（开发者） |
| 1 | 107 | 或：python scripts/build_dist.py && python scripts/install.py |
| 3 | 110 | 3.3 模式 C：仅写 hooks（已装过 ae-sdd） |
| 1 | 113 | 项目级 hook（写到 <项目>/.claude/settings.json） |
| 1 | 116 | 全局级 hook（写到 ~/.claude/settings.json） |
| 2 | 124 | 4. 验证（必跑，不通过不交付） |
| 1 | 129 | 1. 主入口存在 |
| 1 | 132 | 2. CLI 可执行 |
| 1 | 134 | 预期：{"name": "ae-sdd", "version": "3.1.1", ...} |
| 1 | 136 | 3. hooks 配置 |
| 1 | 138 | 或 项目级：cat <项目>/.claude/settings.json |
| 1 | 140 | 4. （如项目接 ae-sdd）检查项目是否有 .ae-sdd/ 目录 |
| 1 | 142 | ⚠️ 注意：没有 .ae-sdd/ 时 gate-intercept 默认放行（见 source/HARNESS.md） |
| 2 | 149 | 4.5 自动化模式提示（🆕 v3.8.0，可选） |
| 2 | 165 | 5. 常见失败（FAQ，按 OS 分组） |
| 3 | 167 | 5.1 Python 相关 |
| 3 | 176 | 5.2 Hook 相关 |
| 3 | 185 | 5.3 安装相关 |
| 3 | 194 | 5.4 macOS 特殊 |
| 3 | 199 | 5.5 Windows 特殊 |
| 2 | 207 | 6. 卸载 |
| 1 | 210 | 1. 卸载 SKILL（含备份） |
| 1 | 212 | 或：python scripts/install.py --uninstall（仓库根目录） |
| 1 | 214 | 2. 手动清理项目级 hooks（如果之前 init-hooks 写过） |
| 1 | 215 | 项目级 .claude/settings.json 的 hooks 字段需手动删除 |
| 1 | 216 | 或重写：init-hooks <项目路径> --use-python（只覆盖 hooks，不卸载 SKILL） |
| 1 | 218 | 3. 验证卸载 |
| 2 | 226 | 7. 与其他 SKILL 的边界 |
| 2 | 242 | 8. 触发后输出模板 |
| 2 | 265 | 9. 执行清单（按 TodoWrite 拆活） |
| 2 | 278 | 10. 禁止 |

## Inline References

| ref |
| --- |
| .ae-sdd/ |
| .claude/settings.json |
| /ae-sdd |
| /usr/local/bin |
| ae-sdd automation enable |
| ae-sdd state read |
| ae-sdd-install-skill.md |
| ae-sdd-update-skill.md |
| ae-sdd.md |
| bash scripts/build-dist.sh |
| bash scripts/build-dist.sh && bash scripts/install.sh |
| bash scripts/dev-sync.sh |
| curl -fsSL https://raw.githubusercontent.com/AILenGarden/ae-sdd/main/scripts/install.sh \\| bash |
| python -c "import json; json.load(open('.claude/settings.json'))" |
| python ~/.claude/skills/ae-sdd/tools/bin/ae-sdd init-hooks --uninstall |
| python ~/.claude/skills/ae-sdd/tools/bin/ae-sdd init-hooks <项目路径> --use-python |
| source/HARNESS.md §⚠️ 升级注意 |
| ~/.claude/skills/ae-sdd.uninstalled.<时间戳> |
| ~/.claude/skills/ae-sdd/ |
| ~/.codex/skills/ae-sdd/ |
| ~/.hermes/skills/ae-sdd/ |
