# Story 生成 Agent 任务分配卡

> **适用场景：** 由 Root Session 调度 Story 生成 Agent（story-writer）时填写本 YAML 任务卡，用于分配 DR/PRD 输入、输出路径与执行标准。

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

output_boundary:
  - 只生成当前生效的 Story 内容；不写过程记录、Agent 对话、门禁日志、Review 循环或生成总结
  - 不创建或更新 CHANGELOG、DR、DR_SUPPLEMENT、DR Review 或 DR 草稿
  - DR 仅作为只读输入；缺失时返回 BLOCK，不得改走 DR 生成
  - Plan / SourceTrace / WriterReport 如被系统要求落盘，仅作内部机器产物，不回写 Story

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
