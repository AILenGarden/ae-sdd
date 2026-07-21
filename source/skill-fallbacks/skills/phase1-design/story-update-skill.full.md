# Story Update - Story 文档更新 Skill

## 目标

安全更新既有 Story。Document Storage 返回当前模板、撰写指南和 Story 正文；`story_template_sections` 按稳定 section ID 分类变更；本 Skill 不维护章节清单。

## 资源加载

1. 通过 `resolve_read_resource("STORY_TEMPLATE")` 和 `resolve_read_resource("STORY_WRITING_GUIDE")` 获取 `path/source/content/sha256`。
2. 通过 `ae-sdd doc resolve --intent STORY --story-id {storyId}` 获取正式 Story 资源并消费其正文。
3. 所有正文均来自 Document Storage 响应；禁止 Skill 按路径再次打开。
4. 模板、指南或 Story 解析失败时 fail closed。

## 更新流程

1. 解析当前模板和指南，校验 section ID 覆盖、显式锚点和导航链接；更新后必须通过 `validate_story_document_navigation`。
2. 解析 Story 的 ID-only 标记；新文档按 ID 识别变更范围。
3. 历史无 ID 文档只允许标题精确、唯一迁移；迁移后补齐全部显式锚点和 ID-only 标记。
4. 调用 `classify_story_section_ids(template.content, changed_ids)` 判断主要或副章节。
5. 主要章节变化：重新确认受影响输入，更新主要章节，执行 `Review(scope=primary)`，使依赖副章节失效并重新派生，最后执行 `Review(scope=full)`。
6. 仅副章节变化：禁止改变主要章节，更新后执行 `Review(scope=full)`。
7. 仅模板标题变化：保持 Story section ID 不变；按当前模板标题刷新展示，不修改层级逻辑。

## 变更分类

- 业务目标、范围、流程、契约、状态、错误码、数据、配置、非功能、实现设计、AC、依赖变化，按当前模板 layer 判为主要变更。
- 任务映射、人工任务和未决问题变化，按当前模板 layer 判为副变更。
- 无语义的格式变化只做最小原地更新，但仍须保留 ID-only 标记。

## 输出与禁止事项

- 正文原地更新，使用 `ae-sdd doc save --intent STORY`；不生成旁车报告或 changelog。
- 回写结构化 Review finding 和验证证据，不写过程 Markdown。
- 禁止标题近似匹配或语义猜测、部分 ID 与无 ID 混用、直接读取固定路径或跳过 primary Review。
