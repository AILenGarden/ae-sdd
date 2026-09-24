---
name: al-coding
description: Coding design, code review, and registered coding-skill discovery across languages, architectures, and projects.
metadata:
  display_name: al-coding
  version: 0.5.3
  role: coding-design-and-discovery
---

# al-coding

从需求、约束、现状、依赖和质量目标推导代码设计；对实际变更逐项验证适用规范；通过注册表发现语言、架构、项目和全局技能。

## 必读范围

进入对应任务后，先完成以下读取，再依其规则设计或评审：

| 任务 | 必读内容 |
|---|---|
| Coding | 完整读取 [Coding Core](core/coding-core.md) 和 [注册表](framework/registry.yaml) |
| Review | 完整读取 [Review Core](core/review-core.md)、Coding Core §4 的全部 R01–R11 触发问题和注册表 |
| 技能发现或注册 | 完整读取注册表和 [注册契约](framework/plugin-spec.md) |

Coding 从 D1–D7 推导设计；Review 从实际 diff、需求和级联范围验证实现。一个任务同时包含两者时合并必读范围，已完整读取且未变化的内容复用。

## 适用规范

按已确认的项目、技术栈、架构和任务选择 enabled 条目：`scope: project` 必须精确匹配 `project_id`，global 条目按实际适用范围参与。身份未知时标记项目规范覆盖受限。

完整读取每个选中 SKILL 的正文，再按其适用条件读取参考条款及依赖条款。以规范自身的范围、触发条件和例外确定适用性；每条适用规范均纳入设计或评审，不能因篇幅或只看改动行而跳过。

注册表相对路径以本 SKILL.md 所在目录为根。字段、身份、路径或规范冲突需要裁决时，读取注册契约；冲突未解决时，依赖它的设计决定或评审结论保留待定。

## 条件读取

| 条件 | 继续读取 |
|---|---|
| 形成或评审职责、类型、依赖、契约等结构决定 | [设计思想](core/references/design-principles.md) 中对应章节及其依赖 |
| R01–R11 某项触发，或信息不足以排除 | [风险判定](core/references/risk-decisions.md) 中对应行、相关 Common Branches；命中 Core Path Check 场景时读取该节 |
| Review 需要追溯需求、现状、复用或设计推导 | Coding Core §1–§3、§5 中对应内容 |
| 发现可疑实现或无效验证，需要反例核对 | [反模式](core/be-coding-ai-anti-patterns.md) 中对应条目 |

## 读取完整性

每份必读文件、每个选中章节或条款都读到结束；可分段读取，输出截断或分页时续读缺失部分。搜索命中和目录索引只用于定位，不能替代正文。

在任务工作记录中简要保留已读文件/章节及未解决的适用性、缺失和冲突。必需内容不可访问时，说明受影响范围；Review 按证据不足处理，不输出该范围已通过。变更或新证据扩大范围时补读新增适用内容。
