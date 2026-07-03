---
name: ae-sdd-plugin-loader
description: |
  ae-sdd 三层 SKILL 注册表加载协议（🆕 v3.5.0）。
  在 ae-sdd 主编排层加载任何 SKILL 之前，按"项目层 > 全局层 > 仓库根层 > 内置 fallback"
  的优先级合成三层注册表，决定实际加载路径。Agent 收到 `/ae-sdd` 后涉及 SKILL 路由时
  必须先加载本 SKILL 确认加载协议。
  同时承担"用户注册 CodingSKILL"引导职责：用户说"注册插件 / 注册 SKILL / 外挂 CodingStyle"
  时加载本 SKILL 引导生成 registry.yaml + 外挂 SKILL。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/ae-sdd-plugin-loader-skill.full.md
source_fallback_sha256: 7ac7a8b6d222d6f9fbaca47f2e44d092d3e5bbb7e8a7f002c2528824f7739ac3
source_original_bytes: 10951
source_original_lines: 272
source_semantic_inventory_sha256: e539cfa8ba8c6c87761c4f95f5e8020632613a00f110955e4371aabb727fc0fc
source_slimmer: slim_source_skills.py@2
---

# ae-sdd Plugin Loader — 三层 SKILL 注册表加载协议 Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/ae-sdd-plugin-loader-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/ae-sdd-plugin-loader-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/ae-sdd-plugin-loader-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/ae-sdd-plugin-loader-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/ae-sdd-plugin-loader-skill.full.md`
- fallback_sha256: `7ac7a8b6d222d6f9fbaca47f2e44d092d3e5bbb7e8a7f002c2528824f7739ac3`
- original_lines: 272
- original_bytes: 10951
- semantic_inventory_sha256: `e539cfa8ba8c6c87761c4f95f5e8020632613a00f110955e4371aabb727fc0fc`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: ae-sdd 三层 SKILL 注册表加载协议（🆕 v3.5.0）。
在 ae-sdd 主编排层加载任何 SKILL 之前，按"项目层 > 全局层 > 仓库根层 > 内置 fallback"
的优先级合成三层注册表，决定实际加载路径。Agent 收到 `/ae-sdd` 后涉及 SKILL 路由时
必须先加载本 SKILL 确认加载协议。
同时承担"用户注册 CodingSKILL"引导职责：用户说"注册插件 / 注册 SKILL / 外挂 CodingStyle"
时加载本 SKILL 引导生成 registry.yaml + 外挂 SKILL。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; keyword_hits: 4 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:110 §3 注册流程引导（母版维护者 / 项目 owner / 个人开发者 用）; L3:116 3.1 Step 1 — 确认注册层 + 使用方身份; L3:128 3.2 Step 2 — 生成注册表; +3 more; keyword_hits: 19 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | keyword_hits: 18 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L1:12 ae-sdd Plugin Loader — 三层 SKILL 注册表加载协议; L2:225 §4 CLI 命令; L3:227 4.1 `ae-sdd plugin list`; +3 more; keyword_hits: 31 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L1:161 registry.yaml; keyword_hits: 23 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | keyword_hits: 13 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 21; refs: /ae-sdd; <ae-sdd-master>/plugins/registry.yaml; <project>/.ae-sdd/plugins/registry.yaml; +18 more; headings: L3:94 2.3 fallback 默认行为; keyword_hits: 66 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L2:24 §1 三层注册表定义; L2:41 §2 加载协议 SOP（Agent 用）; L2:110 §3 注册流程引导（母版维护者 / 项目 owner / 个人开发者 用）; +4 more; keyword_hits: 20 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L2:270 §7 实施历史; keyword_hits: 3 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 12 | ae-sdd Plugin Loader — 三层 SKILL 注册表加载协议 |
| 2 | 24 | §1 三层注册表定义 |
| 2 | 41 | §2 加载协议 SOP（Agent 用） |
| 3 | 43 | 2.1 加载时机 |
| 3 | 67 | 2.2 三层优先级合成算法 |
| 3 | 94 | 2.3 fallback 默认行为 |
| 3 | 98 | 2.4 加载失败处理 |
| 2 | 110 | §3 注册流程引导（母版维护者 / 项目 owner / 个人开发者 用） |
| 3 | 116 | 3.1 Step 1 — 确认注册层 + 使用方身份 |
| 3 | 128 | 3.2 Step 2 — 生成注册表 |
| 1 | 132 | 项目层 |
| 1 | 134 | 全局层 |
| 3 | 152 | 3.3 Step 3 — 写外挂 SKILL |
| 1 | 161 | registry.yaml |
| 1 | 178 | My Coding Style (TDD + DDD) |
| 3 | 183 | 3.4 Step 4 — 验证 |
| 1 | 187 | 校验三层注册表 + 每个 plugin sanity check |
| 1 | 190 | 查看某 SKILL 的加载路径 |
| 3 | 211 | 3.5 Step 5 — 测试 |
| 2 | 225 | §4 CLI 命令 |
| 3 | 227 | 4.1 `ae-sdd plugin list` |
| 3 | 231 | 4.2 `ae-sdd plugin validate` |
| 3 | 235 | 4.3 `ae-sdd plugin trace <target>` |
| 3 | 239 | 4.4 `ae-sdd plugin init --layer {project\|global}` |
| 2 | 247 | §5 与其他 SKILL 的关系 |
| 2 | 259 | §6 已知缺口 / 留待下个 PR |
| 2 | 270 | §7 实施历史 |

## Inline References

| ref |
| --- |
| /ae-sdd |
| <ae-sdd-master>/plugins/registry.yaml |
| <project>/.ae-sdd/plugins/registry.yaml |
| ae-sdd plugin init --layer {project\|global} |
| ae-sdd plugin list |
| ae-sdd plugin trace <target> |
| ae-sdd plugin trace coding-skill.md |
| ae-sdd plugin validate |
| coding-skill.md |
| plugins/registry.yaml |
| source/SKILL.md |
| source/SKILL.md §路由决策算法 |
| source/docs/plans/2026-06-26-plugin-registry-design.md |
| source/skills/ |
| source/skills/orchestration/ae-sdd-update-skill.md |
| source/standards/constraints/plugin-registry-spec.md |
| source/templates/ |
| source/templates/project-assets/plugin-registry-template.yaml |
| tools/bin/ae-sdd |
| tools/lib/plugin_loader.py |
| ~/.ae-sdd/plugins/registry.yaml |
