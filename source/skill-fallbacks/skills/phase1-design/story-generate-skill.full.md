# Story Generate - 从 DR 生成 Story Skill

## 目标

从已确认的上游输入生成结构化 Story。模板定义结构，撰写指南定义写法，Document Storage 定位并读取资源，`story_template_sections` 负责章节解析；本 Skill 只负责阶段编排。

## 输出边界

- 正文只写当前生效的 Story，不生成 Proposal、GeneratePlan、WriterReport、ReviewReport、SourceTrace 或 changelog。
- 不在本 Skill 中维护章节标题、主副清单、数量或文件路径。
- 任何缺少权威输入、section ID 或指南覆盖的情况都停止。

## 资源加载

首先调用 Document Storage：

```text
resolve_read_resource("STORY_TEMPLATE")
resolve_read_resource("STORY_WRITING_GUIDE")
resolve_read_resource("DOC_DENSITY_STANDARD")
```

三个结果必须包含 `path/source/content/sha256`。Skill 只消费返回的 `content`，不得自行拼路径、搜索、打开或读取文件。将模板和指南内容交给 `story_template_sections` 固定函数；密度规范在写作前自检（§4 写作前 5 问）与写作后验收（§5）时执行。

## 流程

### 0. 输入准入

1. 按 `story-input-checklist` 加载 PRD、RA、DR、约束、资产和依赖 Story。
2. 通过 Document Storage 获取模板与指南资源并记录 sha256。
3. `validate_story_section_metadata(template.content)` 必须通过。
4. `validate_story_navigation(template.content)` 必须通过。
5. `validate_story_guide_coverage(template.content, guide.content)` 必须返回空问题。
6. 确认 Story 的 Work Item、Story ID、上游绑定和目标范围。

### 1. 方案决策基线

对每个非平凡实现点核对现有能力、复用候选、模块归属、失败边界和验证方式。没有代码或资产证据时写入未决问题，不得凭命名猜测。

### 2. 主要章节草稿

调用 `get_primary_story_sections(template.content)`，按返回顺序处理 section。每个 section：

1. 查找指南中同 ID 条目；
2. 判断适用条件与必填性；
3. 写入有来源的内容；
4. 在 H2 前先输出与 section ID 相同的显式 ASCII 锚点，再输出 `<!-- ae-sdd:story-section id={section.id} -->`。
5. 按模板的分析→设计→实现顺序写入核心章节；不适用的条件章节省略，不保留空壳。

完成后只提交主要章节，触发 `Review(scope=primary)`。副章节缺失不得阻断本阶段。

### 3. 副章节派生

仅当 primary Review 通过后，调用 `get_secondary_story_sections(template.content)`。副章节只能补充主要章节已确定事实的任务映射、人工操作和未决问题，不得新增范围、规则、状态或核心错误语义。仍使用“显式锚点 + ID-only 标记”，完成后触发 `Review(scope=full)`。

### 4. 写入与循环

- 正式 Story 路径通过 `ae-sdd doc resolve --intent STORY` 获取。
- 原地更新同一 Story；不生成旁车报告。
- Review finding 回写 `state.review.status/findings`。
- 主要章节变化时，使依赖副章节失效并回到 primary；副章节变化只重新执行 full。

## 验收

- 模板和指南资源读取成功且 sha256 可追溯。
- 解析器返回的全部章节均按模板顺序输出。
- 生成 Story 的每个 H2 都有 ID-only 标记。
- 生成 Story 的总章节目录无断链；存在接口时接口目录只列实际接口，接口详情块有稳定锚点和分隔线。
- `validate_story_document_navigation(story.content)` 返回空问题。
- primary 先通过，再派生副章节并通过 full。
- 无固定路径、章节标题、数量或语义分层逻辑。

## 禁止事项

- 禁止跳过 Document Storage 直接读取任何模板、指南或标准。
- 禁止复活旧 C-01～C-09 确认清单作为主副定义。
- 禁止标题近似匹配或语义猜测 section ID。
- 禁止 primary 未通过时写副章节。
