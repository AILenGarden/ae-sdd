# Story 章节分层标准

## 1. 权威边界

| 权威源 | 唯一职责 |
| --- | --- |
| `STORY_TEMPLATE` 资源 | 章节顺序、稳定 section ID、`primary/secondary` 层级和空白输出结构 |
| `STORY_WRITING_GUIDE` 资源 | 各 section ID 的适用条件、必填性、来源、写法、示例与 Review 口径 |
| `story_template_sections` | 元数据校验、章节列表、Story ID 解析和分层查询 |
| Story Generate/Review/Update Skill | 调用 Document Storage 与固定解析函数，编排阶段，不保存章节清单 |

主副层级以模板标记为唯一权威。标准、指南和 Skill 均不得复制固定章节名单或根据标题语义重新分类。

模板还负责提供导航结构：每个输出 H2 使用与 section ID 相同的显式 ASCII 锚点；总章节目录和接口目录只作为导航，不新增可解析的 H2 章节。条件章节在正式 Story 中可省略，目录只覆盖实际输出章节。

## 2. 元数据契约

模板中的每个输出二级章节必须紧邻一个完整标记：

```markdown
<!-- ae-sdd:story-section id=main-flow layer=primary -->
## 主流程
```

生成后的 Story 必须把它投影为 ID-only 标记：

```markdown
<!-- ae-sdd:story-section id=main-flow -->
## 主流程
```

约束：

- `id` 是稳定 kebab-case 主键；标题变化不得改变 ID。
- `layer` 只存在于模板，只允许 `primary` 或 `secondary`。
- 模板每个 H2 恰好一个标记；生成 Story 每个 H2 恰好一个 ID-only 标记。
- 模板每个 H2 前有唯一显式锚点；目录中的本地链接必须全部可达。
- 缺失、孤立、重复、非法或未知标记一律 fail closed。
- 指南使用 `<!-- ae-sdd:story-guide section-id=... -->` 与模板 ID 关联；必须一一覆盖且无孤立条目。

## 3. 资源读取与解析

Story Skill 必须通过 Document Storage 的 `STORY_TEMPLATE` 和 `STORY_WRITING_GUIDE` intent 获取 `path/source/content/sha256`。调用方只消费返回的 `content`，不得按路径再次读取。

固定纯函数：

```python
parse_story_sections(template_text, source_path="")
get_primary_story_sections(template_text, source_path="")
get_secondary_story_sections(template_text, source_path="")
parse_story_document_section_ids(story_text, source_path="")
classify_story_section_ids(template_text, section_ids, source_path="")
resolve_story_document_section_ids(template_text, story_text, ...)
validate_story_guide_coverage(template_text, guide_text, ...)
```

解析函数不得定位路径、读取文件、glob、维护标题表或回退到语义猜测。

## 4. 生成与 Review 顺序

```text
Document Storage 返回模板/指南正文
  -> 校验模板元数据和指南 ID 覆盖
  -> get_primary_story_sections()
  -> 生成带 ID-only 标记的主要章节
  -> Review(scope=primary)
  -> get_secondary_story_sections()
  -> 派生带 ID-only 标记的副章节
  -> Review(scope=full)
```

- `primary` Review 只检查解析器返回的主要章节；尚未派生的副章节不得形成 finding。
- `full` Review 必须建立在 `primary` 已通过的基础上，检查全部适用章节及跨章节一致性。
- 主要章节变化会使依赖它的副章节失效，必须重新派生并执行 `full` Review。
- 仅副章节变化不允许静默改变主要章节，只重新执行 `full` Review。

## 5. Update 与历史兼容

Review/Update 优先读取生成 Story 中的 section ID，并用当前模板重新取得标题和层级。因此模板标题或 layer 调整不要求修改 Story Skill。

历史 Story 完全没有 section ID 时，允许一次兼容迁移：

1. 仅按当前模板标题精确、唯一匹配 ID。
2. 任一标题未知、重复或歧义时阻断，不做模糊/语义匹配。
3. 获得文档更新授权后，为全部 H2 补写 ID-only 标记。
4. 文档只要已出现部分 ID，就不得混用历史迁移；缺 ID 的 H2 直接阻断。

## 6. 完成条件

- 模板元数据合法，指南 ID 覆盖完整。
- Story Skills 不含固定章节标题、数量或模板/指南文件路径。
- 主要章节先通过 `Review(scope=primary)`，副章节随后派生并通过 `Review(scope=full)`。
- 生成 Story 的全部 H2 都有稳定 ID-only 标记。
- 模板层级或标题变化可仅修改模板/指南数据，不修改解析函数和 Story Skills。
