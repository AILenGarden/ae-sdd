# Story Review 检查标准

## 1. Review 资源

Review 必须通过 Document Storage 获取 `STORY_TEMPLATE` 与 `STORY_WRITING_GUIDE` 的 `path/source/content/sha256`，并使用 `story_template_sections` 解析正文。不得使用 Skill 内置标题表或直接读取固定路径。

## 2. 准入检查

1. Story 已绑定正式文档路径，Story ID 与元信息一致。
2. 模板元数据合法，指南 section ID 覆盖完整。
3. `validate_story_navigation(template.content)` 与 `validate_story_document_navigation(story.content)` 均通过；Story 中全部 H2 有显式锚点与 ID-only 标记；历史无标记文档满足精确唯一迁移条件。
4. Story ID 在当前模板中全部可识别，无重复或部分标记。
5. 上游、约束、资产、依赖 Story 和来源输入已按 `story-input-checklist.md` 加载。
6. Review scope 明确为 `primary` 或 `full`。

## 3. Review 范围

### `scope=primary`

- 只检查 `get_primary_story_sections(template.content)` 返回的章节。
- 逐章应用指南相同 section ID 的必填性、来源、写法和红线。
- 尚未派生的副章节缺失、留空或没有 ID 不得形成 finding。
- 无主要章节阻断 finding 时才允许进入副章节派生。

### `scope=full`

- 前置条件是 `primary` Review 已通过且副章节已派生。
- 检查模板全部适用 section ID、指南合规和跨章节一致性。
- 副章节不得引入主要章节中不存在的新范围、规则、状态或核心错误语义。
- 若必须改变主要章节，停止 full Review，使相关副章节失效并回到 `primary`。

## 4. 通用检查维度

| 维度 | 检查内容 |
| --- | --- |
| 上游一致性 | 用户目标、范围、流程、字段、规则和状态与 PRD/RA/DR 一致 |
| 流程闭环 | 主流程、异常流程、状态转换和用户可见结果闭环 |
| 契约一致性 | SPI/REST 请求响应、错误码、幂等、超时和调用双方一致 |
| 数据一致性 | 字段链路、DDL、索引、枚举、CRUD 和迁移一致 |
| 非功能 | 权限、安全、一致性、性能、观测、补偿、灰度和回滚有可验证结论 |
| AC 与验证 | 每个适用行为有 AC，每个 AC 映射可证明结果的验证项 |
| 来源追溯 | 关键事实有权威来源；不涉及有依据；未决项有责任人与影响 |
| 模板/指南 | 无占位符残留，section ID 合法，内容满足相应指南条目 |
| 导航完整性 | 总章节目录覆盖实际 H2；接口目录与 SPI/REST 详情一一对应；无重复锚点、断链或未分隔接口块 |

## 5. 历史 Story 迁移

- 完全没有 section ID 时，允许按当前模板标题精确、唯一匹配。
- 任一标题未知或歧义时阻断，禁止近似或语义匹配。
- 获得更新授权后一次性为全部 H2 补写显式锚点和 ID-only 标记。
- 已存在部分 ID 的 Story 不走迁移；缺 ID 的 H2 直接形成阻断 finding。

## 6. Finding 与结论

finding 至少包含：

- `severity`；
- `sectionId` 与层级；
- 问题、证据和违反的指南/契约；
- 修复动作及是否导致副章节失效。

结论只写入 `state.review.status/findings`：

- `passed`：当前 scope 无未关闭阻断 finding；
- `changes_required`：存在可修复 finding；
- `blocked`：缺权威输入、ID 无法迁移或路由/解析失败。

不生成 ReviewReport、Proposal、Compare、SourceTrace 或 changelog。

## 7. 禁止事项

- 禁止因副章节缺失阻断 `primary`。
- 禁止 `primary` 未通过就执行 `full`。
- 禁止在 `full` 中静默修改主要章节。
- 禁止按标题含义猜主副层级。
- 禁止零发现但不给出覆盖证据。
