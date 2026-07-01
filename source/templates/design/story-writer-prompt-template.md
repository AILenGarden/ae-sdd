# Story 生成 Agent 任务分配卡

```yaml
agent_role: story-writer
story_id: STORY-XXX-BE
task_id: storygen-{STORY-ID}
priority: P0

input:
  - DR 路径: {DR 文件路径}
  - PRD 路径: {PRD 文件路径}
  - 产品原型: {原型文件/链接}
  - 项目资产: {projectKey}.assets.md
  - Story 模板: templates/design/story-template.md

output:
  deliverable: documentStorage.resolve_path(intent="STORY", storyId={STORY-ID})
  report: documentStorage.resolve_path(intent="STORY_WRITER_REPORT", storyId={STORY-ID})

standards:
  - `story-generate-skill.md`
  - `story-generation-standard.md`
  - `story-frontend-contract-standard.md`
  - `be-testcase-strategy.md`
  - 7 阶段全部覆盖
  - 8 道闸全部通过
  - 每个关键结论必须附证据
  - 阻断型问题必须返回 0 结果，不得带病输出

context:
  - Story 焦点: 业务背景 / 主流程 / AC / 接口 / 数据 / Task
  - ①bis: 前端契约六维度
  - 项目分层来源: {projectKey}.assets.md §3
  - 与下游衔接: 写完后触发 Story Review

deadline: {最长执行时间}

report_back:
  channel: mavis communication
  target: {root session id}
  format: {STORY-ID}-Story-WriterReport.md
```
