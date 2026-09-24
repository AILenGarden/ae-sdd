# 类型选择与正文重点

通用骨架见 [概念模板](../templates/concept.md)，可工作的示例在 [fixture](../tests/fixtures/project/.al-knowledge/project.md)。fixture 是虚构领域，不当作业务事实复制。

| type | 正文重点 / al 扩展 |
| --- | --- |
| Project Context | 项目身份、技术栈、模块职责、构建入口和范围；根 project.md 的必填字段见 profile |
| Business Concept | 术语、状态、规则、适用范围、例外；重要断言用 claims + 显式锚点 |
| Architecture Decision | 背景、约束、已确认选择、理由、被否决方案及来源；提案不得写成决定 |
| Implementation Pattern | 适用条件、已有实现定位、例外；观察到的惯例不自动成为强制规范 |
| Specification | `al.spec_id` 保留源 ID，`al.spec_type` 保留项目类型；规则定位、预期、关系和缺口 |
| API Endpoint | `al.interface` 为 `{service, protocol, signature}`；完整路由/契约、版本、生效条件、边界；可选 entry/nodes/calls |
| Reference | 证据摘要、能支持和不能支持的内容；本地文件用 al.source，外部内容记录 URL 与真实核查信息 |

其他类型可按需求使用，未知类型仍可读。只创建有用概念与目录。日志不是任务流水账，只有真实知识变动需要独立说明时才使用 log.md。
