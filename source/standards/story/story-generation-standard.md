# Story 生成标准

## 0. 输出边界

- Story 正文只承载当前生效的业务与技术设计，不生成 Proposal、GeneratePlan、WriterReport、SourceTrace、ReviewReport 或 changelog。
- 模板仅定义结构；写法由 `STORY_WRITING_GUIDE` 定义；主副层级由 `STORY_TEMPLATE` 元数据定义。
- Story Generate 不保存章节标题或数量，不根据语义自行划分层级。
- Story 生成结果必须包含可点击的总章节目录；接口存在时必须包含只列实际接口的接口目录。

## 1. 输入范围

按 `story-input-checklist.md` 加载上游文档、项目约束、依赖 Story 和标准资源。模板与指南必须通过 Document Storage 获取：

```text
ae-sdd doc resolve --intent STORY_TEMPLATE --json
ae-sdd doc resolve --intent STORY_WRITING_GUIDE --json
```

两个响应必须包含 `path/source/content/sha256`。Skill 和解析函数不得再次按 `path` 读取文件。

## 2. 模板与指南校验

1. `parse_story_sections(template.content)` 校验全部模板 H2 的 `id + layer`。
2. `validate_story_navigation(template.content)` 校验章节显式锚点、目录链接和接口链接可达。
3. `validate_story_guide_coverage(template.content, guide.content)` 校验指南按 section ID 一一覆盖。
4. 任一标记缺失、重复、孤立、非法、未知或断链时停止，不得猜测。
5. 标题只用于展示；section ID 是模板、指南和生成 Story 的关联主键。

## 3. 分阶段生成

### 3.1 主要章节

1. 调用 `get_primary_story_sections(template.content)`，保持模板顺序。
2. 按指南中相同 section ID 的条目加载适用条件、必填性、来源和红线。
3. 每个输出 H2 前先写入与 section ID 相同的显式 ASCII 锚点，再写入 `<!-- ae-sdd:story-section id={section.id} -->`，不得复制 `layer`。
4. 只生成返回列表中的章节，并按分析→设计→实现顺序保持模板顺序，然后执行 `Review(scope=primary)`。
5. 尚未派生的副章节不得被视为缺陷；不适用的条件章节直接省略。

### 3.2 副章节

1. `primary` Review 通过后调用 `get_secondary_story_sections(template.content)`。
2. 副章节只能补充、映射或记录主要章节已经确定的事实，不得新增范围、业务规则、状态或核心错误语义。
3. 按相同“显式锚点 + ID-only 标记”格式输出，补充区与核心区分隔，再执行 `Review(scope=full)`。
4. 生成副章节时若发现必须改变主要章节，停止并返回 `primary` 阶段。

## 4. 内容质量

每个适用章节都必须：

- 回答指南规定的问题并满足必填性；
- 标注权威来源；条件章节不适用时直接省略，必要的判定依据进入依赖风险或未决问题；
- 与范围、流程、接口、数据、状态、错误和 AC 保持一致；
- 删除未使用的占位符、示例行和不适用结构；
- 对真正未决项给出影响、决策人和截止时间。

不得以代码现状替代业务决策。代码核对只能证明现有能力、命名和复用事实。

## 5. 验收标准与验证矩阵

- AC 使用 Given/When/Then，结果必须可独立观察和判定。
- 每个 AC 至少映射一个验证项；每个验证项说明边界、方法和预期证据。
- 接口 AC 必须声明真实协议边界；内部 Mock 不能单独证明集成成功。
- 验证事实只在真实执行后进入 evidence，不把计划写成已完成记录。

## 6. 写入与退出

1. 正式 Story 路径通过 `doc resolve --intent STORY` 获得。
2. 主阶段和副阶段均原地更新同一 Story，不创建旁车报告。
3. 最终 Story 的每个 H2 都必须有显式锚点和 ID-only 标记；`validate_story_document_navigation(story.content)` 必须通过，总章节目录无断链，接口目录只列实际接口。
4. `primary` 和 `full` Review 均通过、无阻断 finding 后退出。

## 7. 禁止事项

- 禁止把章节列表或层级写进本标准或 Story Skill。
- 禁止 Skill 直接读取模板/指南固定路径。
- 禁止生成 Story 时省略 ID-only 标记或携带模板 `layer`。
- 禁止标题近似匹配、语义猜测或部分 ID 与无 ID 混用。
- 禁止在 `primary` 未通过时派生副章节。
