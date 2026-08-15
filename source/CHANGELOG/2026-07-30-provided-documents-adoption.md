# 2026-07-30 | 用户提供文档采纳（providedDocuments adoption）与文档关联树投影

## Summary

用户在使用 ae-sdd 前常已自备 PRD/DR/Story 文档，但现状 `workitem.create` 总会铸造新文档路径并走完整生成流程，导致用户文档被重复创建，且权威状态中没有任何 PRD→DR→Story 关联树。本次新增采纳（adoption/register）路径：`workitem.create` 接受可选 `providedDocuments`，daemon 只登记既有文档（不读内容、不写入、不复制用户文件、不创建铸造默认文件），在权威状态构建关联树并直写最深已提供文档的生成后 phase，流程直进 review；`flow.snapshot` 与 context projection 新增派生字段 `documentTree`，状态表关联树由 daemon 投影权威提供。

## Design ledger impact

| Design ID | Impact |
|---|---|
| `D-003` | updated: Work Item state 新增采纳关联树落点（`prdState.docPath`、`drStates`/`storyStates` 嵌套、`parentPrdId`/`parentDrId` 跨 item 父子），create 时直写初始 phase（不经 TransitionPolicy） |
| `D-009` | updated: Document Storage 新增"用户提供文档采纳"语义——与 `document.save` 相对，只登记不写入；已采纳 intent 不适用 §1.3 铸造路径模板 |

## Changes

| Area | Change |
|---|---|
| Rust intake/phase/投影（`crates/**`） | `workitem.create` payload 新增可选 `providedDocuments`（≤64，schema 校验 project-relative/防穿越/文件须存在/parentDocId 链接）；create 时写 `documentPaths[intent]`、`routeDocuments[intent]=true`、关联树与初始 phase；`flow.snapshot`/context projection 派生 `documentTree`（不落库） |
| `source/SKILL.md` | 新增 `## Provided Documents Adoption` 节：C1 payload 契约、只登记语义、关联树与初始 phase、`documentTree` 派生字段；声明状态表关联树由 daemon 投影权威提供，Agent 不得本地扫描文件自拼 |
| `source/HARNESS.md` | intake 段补 `providedDocuments` 采纳路径（bootstrap `workitem.create` 分支）；Process Contract 段补采纳路径与 `documentTree` 投影消费方式 |
| `document-storage-skill.full.md` | 新增 §4.12『用户提供文档的采纳（adoption/register）』（与 `document.save` 区别、C1–C5 全量语义、路径安全约束）；§1.3 路径模板节补"已采纳 intent 不适用铸造模板" |
| `document-storage-skill.md`（slim entry） | Semantic Inventory 补 §4.12 行；`source_fallback_sha256` 更新为 `3b33f498b14d2d95f2c63cccfff1d2fb4dfd6559a7edeff87371fff2dc78fa3a` |
| `source/CHANGELOG/` | 新增本条目 |

## 触发原因

- 用户反馈：已指定文档被 ae-sdd 重复创建（铸造新路径 + 走生成流程），且状态中无 PRD→DR→Story 关联树可查。
- 技术债补齐：`routeDocuments[intent]=true` 跳过机制、`parentPrdId`/`parentDrId` 字段、`drStates` 容器长期存在但无生产者；本次给出台约化的唯一生产入口（create 采纳）。

## 影响范围

- 涉及运行时逻辑：Rust intake（`workitem.create` schema/校验）、phase 直写、投影派生；契约文档同步为权威 prose。
- 不改变已有门禁、子 SKILL 职责边界、既有文档存放路径；无 `providedDocuments` 时 intake 行为完全不变（向后兼容）。
- 破坏性变更：无。`providedDocuments` 为可选字段，旧 payload 语义不变。
- 采纳=只登记：绝不写入用户 path，绝不为已采纳 intent 创建铸造默认文件（C4 硬约束）。

## 验证方式

- Rust 侧：`workitem.create` providedDocuments schema 测试（非法 intent / 重复 docId / `..`/绝对路径 / 文件不存在 / parentDocId 指向未提供文档 → schema 错误）；create 后 `flow.snapshot`/`context.get` 的 `documentTree` 形状 golden 测试；`routeDocuments` 跳过生成 handoff 测试。
- 契约一致性：prose 字段名/JSON 形状与冻结契约 C1–C5 逐字核对；`document-storage-skill.md`（slim entry）`source_fallback_sha256` 与 full 文件 `sha256sum` 一致。
- 人工核对：无 `providedDocuments` 的 create 行为与变更前一致（`documentPaths` 铸造默认值不变）。

## Reviewer

陈聪
