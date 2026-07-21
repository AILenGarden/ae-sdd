# Story Review - Story 缺陷挖掘 Skill

## 目标

基于当前权威 Story 模板和撰写指南审查 Story。Document Storage 返回资源正文与 sha256；`story_template_sections` 按 section ID 提供章节和层级；本 Skill 只编排 Review。

## 资源与准入

1. 调用 `resolve_read_resource("STORY_TEMPLATE")` 和 `resolve_read_resource("STORY_WRITING_GUIDE")`。
2. 只消费两个响应中的 `content`，不得自行读取返回的 path。
3. 通过 `parse_story_sections`、`validate_story_navigation`、`validate_story_document_navigation`、`validate_story_guide_coverage` 和 `resolve_story_document_section_ids` 校验模板、指南和 Story。
4. 资源或解析失败时状态为 `blocked`，不得猜测或默认 full。

## Review scope

### `scope=primary`

- 只审 `get_primary_story_sections(template.content)` 返回的章节。
- 逐章按指南同 ID 的必填性、来源、写法和红线检查。
- 副章节尚未派生不构成 finding。
- 通过后才允许派生副章节。

### `scope=full`

- 必须已有 primary 通过结论和副章节派生结果。
- 检查全部适用 section、跨章节一致性、AC/验证矩阵和非功能闭环。
- 副章节不得引入主要章节中不存在的新范围、规则、状态或核心错误语义。
- 若需改变主要章节，停止 full，使受影响副章节失效并回到 primary。

## ID 与历史文档

- 新 Story 以 ID-only 标记为主键，标题只用于展示。
- 历史无 ID Story 只允许标题精确且唯一迁移；未知或歧义标题阻断。
- 已出现部分 ID 时，缺 ID 的 H2 直接阻断，不混用迁移模式。

## 检查维度

- 上游目标、范围、流程、状态、契约、数据和错误码一致。
- 每个适用章节满足撰写指南并有来源或不适用依据。
- 总章节目录覆盖实际 H2；接口目录与 SPI/REST 详情一一对应；无重复锚点、断链或上下黏连的接口块。
- AC 可观察、可独立判定，并有验证矩阵和真实证据边界。
- 不存在占位符残留、隐含业务决定、无责任人的未决项或未声明偏离。

## 输出

只将结构化 findings 写入 `state.review.status/findings`。finding 必须包含严重度、section ID、问题、证据、修复动作和副章节失效影响。不生成 ReviewReport、Proposal、SourceTrace 或 changelog。

## 禁止事项

- 禁止按标题含义或语义猜测主副层级。
- 禁止因副章节缺失阻断 primary。
- 禁止 primary 未通过就执行 full。
- 禁止直接读取模板、指南或标准固定路径。
