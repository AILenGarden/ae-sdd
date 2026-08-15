# 源 SKILL 瘦身标准

> 适用范围：`source/SKILL.md` 与 `source/skills/**/*.md`。
> 标准版本：`ae-sdd-source-slim/v2`。
> 模板：`source/templates/skill/source-skill-slim-entry-template.md`。

## 1. 目标

源 SKILL 瘦身不是自由删减，也不是把长文改写成短文。它是一个可验证的源码变换：

- 完整原文先复制到 `source/skill-fallbacks/**`。
- 源入口改为短、可审查的 slim entry。
- slim entry 必须包含语义识别结果、fallback 哈希、加载契约、目录索引和引用索引。
- 编译器必须从 fallback 取得 runtime fallback，不能把 slim entry 当成完整语义。

瘦身的目标是降低日常加载成本，同时不丢失任何流程、门禁、约束、模板、命令或设计语义。

## 2. 权威关系

| 层级 | 路径 | 权威内容 |
| --- | --- | --- |
| 源入口 | `source/SKILL.md`、`source/skills/**/*.md` | 路由、语义索引、加载契约 |
| 源 fallback | `source/skill-fallbacks/**` | 瘦身前完整原文，语义锚点 |
| 编译 runtime | `dist/ae-sdd/**` | Agent 运行时入口和 compact slices |
| 设计文档 | `source/docs/ae-sdd-design.md`、`source/docs/ae-sdd-implementation-architecture.md`、`source/docs/skill-runtime-compiler.md` | 能力边界、实现边界、编译契约 |
| 工具事实 | `crates/**`、`bins/ae-sdd-cli` | gate/state/CLI 的执行真相 |

当 slim entry 与 fallback 冲突时，先检查 `source_fallback_sha256`。该哈希基于去 BOM、统一 LF 的 canonical UTF-8 内容；哈希正确时，以 fallback 为完整语义来源；执行 gate/state 时，CLI 输出仍高于任何 SKILL 文字。

## 3. 语义识别清单

瘦身前必须识别并登记以下语义类别。识别结果进入 slim entry 的 `## Semantic Inventory` 表，不允许只凭人工直觉删减。

| 类别 | 要识别的语义 | 设计对齐 |
| --- | --- | --- |
| `identity_trigger` | name、description、入口、触发条件、适用场景 | `ae-sdd-design.md` 路由与主入口设计 |
| `workflow_route` | Step/Phase、状态机、路由分支、执行顺序、重入逻辑 | `ae-sdd-design.md` §2/§16，`update-graph.json` |
| `gate_constraint` | G-* 门禁、MUST/必须/禁止/BLOCK/WARN/ASK_USER | `crates/ae-sdd-gates/src/registry.rs:GateRegistry` |
| `tool_command` | `ae-sdd ...` 命令、脚本、API、工具调用契约 | `ae-sdd-implementation-architecture.md` CLI/模块边界 |
| `state_data` | state/config/manifest/JSON/YAML 字段、phase、reviewConsensus | `crates/ae-sdd-store/src`（StateAuthority）与状态设计 |
| `output_doc_contract` | 产物、模板、文档保存、ChangeLog、报告格式 | document-storage 与 `source/templates/**` |
| `resource_reference` | standards/templates/skills/assets/docs 等引用路径 | `source/standards/**`、`source/templates/**` |
| `design_alignment` | 设计-实现对齐、update-check、UC、架构约束 | 三份设计文档与 `update-check` |
| `fallback_only_detail` | 示例、FAQ、历史背景、长解释、低频说明 | 只保留索引，完整内容留在 fallback |

语义类别的含义是“必须被看见并可追溯”，不是“必须全部展开在 slim entry”。长流程和精确措辞可以留在 fallback，但必须能从 slim entry 判断它们存在并定位。

## 4. SOP

1. 发现源文件：只处理 `source/SKILL.md` 与 `source/skills/**/*.md`。
2. 选择条目：调用方必须显式传入每个目标 `--skill`；禁止为了修复一个 fallback 而遍历并改写整棵 `source/skills/`。
3. 读取语义输入：已有 slim entry 的 refresh 必须读取 `skill-fallbacks/**` 下的 `source_fallback`；fallback 不得自引用、不得已经标记 `source_slimmed: true`，禁止从 slim entry 二次瘦身；非 root 的未瘦身入口不属于此命令的写入范围。首次处理 `source/SKILL.md` 是唯一例外：`--refresh` 只能从固定 `skill-fallbacks/SKILL.full.md` 建立 slim entry，而建立前的 `--validate` 必须失败。输入在哈希、渲染和校验前统一为去 BOM、LF 换行的 canonical UTF-8 内容。
4. 复制 fallback：首次瘦身时先写入 `source/skill-fallbacks/**`，不得覆盖已有 fallback。
5. 语义识别：按 §3 分类识别 frontmatter、heading、关键词、inline references。
6. 模板渲染：按 `source/templates/skill/source-skill-slim-entry-template.md` 输出 slim entry。
7. 机器校验：使用 `ae-sdd-build source-slim --source source --skill <relative-entry> --validate` 校验 fallback 哈希、schema、标准路径、模板路径、必备 section、语义 inventory hash 与 canonical-byte 重渲染一致性。
8. refresh：使用 `ae-sdd-build source-slim --source source --skill <relative-entry> --refresh`；它只写入与渲染结果不同的目标条目。
9. 编译与验证：按 native build/release 作业重建 runtime，并运行 `cargo test -p ae-sdd-build --test source_slim` 和适用的 release 验证。

## 5. 禁止事项

- 禁止手工删除源 SKILL 大段内容后再补一个短说明。
- 禁止没有 fallback 就写 `source_slimmed: true`。
- 禁止把 `source_fallback` 指向 `skill-fallbacks/**` 之外的路径、自身或已 slim 的入口。
- 禁止从已经瘦身的入口再次摘要、改写或“再瘦一遍”。
- 禁止手工修改由 `source-slim` 生成的 section；`--upgrade` 仅是历史兼容别名，新的调用必须使用 `--refresh`。
- 禁止把 slim entry 当成完整语义来源参与 runtime fallback。
- 禁止只更新源 SKILL 而不同步设计文档、模板或本标准。
- 禁止在 slim entry 中隐藏关键 gate、命令、状态字段或文档产物契约。

## 6. 验收定义

一次源 SKILL 瘦身完成，必须同时满足：

- 每个 slim entry 有 `source_slim_schema: ae-sdd-source-slim/v2`。
- 每个 `source_fallback_sha256` 与 `source/skill-fallbacks/**` 的 canonical UTF-8 内容一致。
- 每个 slim entry 与模板重渲染结果 canonical-byte 一致。
- 对已修改 fallback 的每个指定 entry，`source-slim --refresh` 后 `source-slim --validate` 均通过。
- `## Semantic Inventory` 至少能追到身份/触发语义，并按内容覆盖工作流、门禁、工具、状态、文档、资源或设计对齐语义。
- 编译后的 `dist/ae-sdd/runtime/**/fallback/SKILL.full.md` 来自源 fallback，而不是 slim entry。
- 对应 native runtime/release 验证通过。
