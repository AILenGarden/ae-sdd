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
| gate_constraint | headings: L1:41 3. Rust 工具链（从源码构建时必须）; L1:139 ⚠️ 注意：没有 .ae-sdd/ 时 gate-intercept 默认放行（见 source/HARNESS.md）; L2:272 10. 禁止; keyword_hits: 6 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L1:10 ae-sdd Install — 安装引导 SKILL; L3:77 3.1 模式 A：构建原生二进制; L3:98 3.2 模式 B：编译并分发到各 host（开发者）; +3 more; keyword_hits: 66 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L1:113 项目级 hook（写到 <项目>/.claude/settings.json）; L1:116 全局级 hook（写到 ~/.claude/settings.json）; L1:138 或 项目级：cat <项目>/.claude/settings.json; +1 more; keyword_hits: 18 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
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
| 1 | 41 | 3. Rust 工具链（从源码构建时必须） |
| 1 | 44 | 4. Git |
| 1 | 47 | 5. Claude Code / Codex 是否已装 |
| 1 | 51 | 6. ae-sdd 是否已装 |
| 1 | 55 | 7. 项目根（如果是给项目接 ae-sdd） |
| 2 | 63 | 2. 选择安装模式（4 选 1） |
| 2 | 75 | 3. 执行 install（按平台分支） |
| 3 | 77 | 3.1 模式 A：构建原生二进制 |
| 3 | 98 | 3.2 模式 B：编译并分发到各 host（开发者） |
| 1 | 102 | 提交后由 .githooks/post-commit 自动执行；也可手工跑： |
| 3 | 109 | 3.3 模式 C：仅写 hooks（已装过 ae-sdd） |
| 1 | 112 | 项目级 hook（写到 <项目>/.claude/settings.json） |
| 1 | 115 | 全局级 hook（写到 ~/.claude/settings.json） |
| 2 | 121 | 4. 验证（必跑，不通过不交付） |
| 1 | 126 | 1. 主入口存在 |
| 1 | 129 | 2. CLI 可执行 |
| 1 | 131 | 预期：{"name": "ae-sdd", "version": "3.1.1", ...} |
| 1 | 133 | 3. hooks 配置 |
| 1 | 135 | 或 项目级：cat <项目>/.claude/settings.json |
| 1 | 137 | 4. （如项目接 ae-sdd）检查项目是否有 .ae-sdd/ 目录 |
| 1 | 139 | ⚠️ 注意：没有 .ae-sdd/ 时 gate-intercept 默认放行（见 source/HARNESS.md） |
| 2 | 146 | 4.5 自动化模式提示（🆕 v3.8.0，可选） |
| 2 | 162 | 5. 常见失败（FAQ，按 OS 分组） |
| 3 | 164 | 5.1 二进制与工具链相关 |
| 3 | 172 | 5.2 Hook 相关 |
| 3 | 181 | 5.3 安装相关 |
| 3 | 189 | 5.4 macOS 特殊 |
| 3 | 194 | 5.5 Windows 特殊 |
| 2 | 202 | 6. 卸载 |
| 1 | 205 | 1. 卸载 SKILL（含备份） |
| 1 | 208 | 2. 手动清理项目级 hooks（如果之前 init-hooks 写过） |
| 1 | 209 | 项目级 .claude/settings.json 的 hooks 字段需手动删除 |
| 1 | 210 | 或重写：ae-sdd init-hooks <项目路径>（只覆盖 hooks，不卸载 SKILL） |
| 1 | 212 | 3. 验证卸载 |
| 2 | 220 | 7. 与其他 SKILL 的边界 |
| 2 | 236 | 8. 触发后输出模板 |
| 2 | 259 | 9. 执行清单（按 TodoWrite 拆活） |
| 2 | 272 | 10. 禁止 |

## Inline References

| ref |
| --- |
| .ae-sdd/ |
| .claude/settings.json |
| /ae-sdd |
| ae-sdd automation enable |
| ae-sdd init-hooks --uninstall |
| ae-sdd init-hooks <项目路径> |
| ae-sdd state read |
| ae-sdd version |
| ae-sdd-install-skill.md |
| ae-sdd-update-skill.md |
| ae-sdd.md |
| cargo build --workspace --release |
| cargo run -p ae-sdd-build --release -- post-commit |
| ~/.ae-sdd/distributors.json |
| source/HARNESS.md §⚠️ 升级注意 |
| ~/.claude/skills/ae-sdd.uninstalled.<时间戳> |
| ~/.claude/skills/ae-sdd/ |
| ~/.codex/skills/ae-sdd/ |
| ~/.hermes/skills/ae-sdd/ |
