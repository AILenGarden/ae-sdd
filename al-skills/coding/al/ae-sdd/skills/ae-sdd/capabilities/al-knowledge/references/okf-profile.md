# ae-sdd knowledge profile 1

## 标准基线与范围

采用 [OKF SPEC v0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md)。2026-09-22 读取的原始 UTF-8 文件 SHA-256：`26aa5da029278939f914e578107242d9607d4f2dc5fe153272b82f9ed1030101`。此摘要固定参考文本，不运行时联网更新。基础格式与本文件的 `al` 工程约定分别校验，本文不是 Google 标准。

概念为 UTF-8 Markdown + YAML frontmatter；`type` 是基础必填字段。包根 `index.md` 可仅含 `okf_version` frontmatter，子目录 index 不带 frontmatter；`log.md` 如存在使用 ISO 日期二级标题及列表。两者是保留文件，不能放概念字段。未知类型、字段、缺索引及普通断链不导致基础格式拒绝。

## 概念通用字段

本 profile 的概念要求 `type`、`title`、`description` 非空；`al.profile: ae-sdd-knowledge/1`、`al.id` 非空且包内唯一。未知字段保留，未知 profile 报能力限制，不解释成当前版。普通外部 OKF 不必有 al。

`status` 使用 draft/stable/deprecated，缺省沿用 OKF stable，但生产者对未核实草稿显式写 draft。`generated` 为 `{by, at}`；`verified` 为事件列表或单一 `{by, at}`；时间使用带时区 ISO 8601。verified 仅记录整篇真实核验，局部事件放 checked。`stale_after` 为绝对时刻；不会覆盖源码变化引起的过期。

`sources` 是列表，每项 `{id, resource, title?}`；ae-sdd 引用的 source 必须有局部唯一 id。正文脚注 `[^id]` 指向它。resource 可以是 URL 或概念路径；scope 描述可被基础 OKF 消费，但不是可重验的工程证据。本 profile 的确认事实应指向 bundle 内 Reference 或可定位来源概念；外部 URL 通过 Reference 记录核查信息。

## 项目与来源

根 `project.md`：`type: Project Context`，al 增加 `project_id`、`scan_scope`（字符串数组）、`excluded_scope`（数组）、`coverage: partial | complete-in-scope`、`repositories`（别名 → 用途说明或规范 URL）。这是知识范围，不是全局任务注册表。工程库必须有 project.md，合法外部 OKF 不要求。

本地 `Reference` 可带 `al.source`：

```yaml
source:
  repository: main
  path: src/orders.py
  sha256: "<所读原始文件字节的SHA-256>"
  captured_at: "2026-09-22T10:00:00+08:00"
  revision: "<已知的commit；dirty时同时说明工作树>"
  symbol: "Orders.cancel(order_id)"
```

repository 必须已登记，path 是仓库内相对路径。运行来源检查时用户/Agent 用 `--repo main=<实际路径>` 显式绑定；工具不从文档执行命令、解析本机绝对地址或联网。无映射/来源不可读报告 unchecked/unavailable，不能当成未变化。非本地 Reference 在正文记录原 URL、核查内容与限制，不填写虚假文件摘要。

## 断言与核验

`al.claims` 列表：`id`、`anchor`（正文显式 `<a id="..."></a>`）、`evidence_state`、`source_ids`（sources 中的 id）。没有证据时允许 unknown，但正文须解释原因。状态枚举 confirmed/inferred/planned/unknown/stale。

confirmed 项必须带 `checked: {by, at, body_sha256, source_sha256}`。body_sha256 对 frontmatter 后的正文做 CRLF → LF 规范化后计算 UTF-8 摘要；source_sha256 是 source id → 被引用概念的原始字节摘要。工具 `inspect` 给出候选摘要，仅辅助记录，不将状态改为 confirmed。必须先人工/Agent 真实回源检查，再填写 checked。

任何摘要变化只输出 stale 提醒，不写回。来源概念内部还有本地 source 时，额外对实际源文件核查。声明与引用若有无法自动检查的外部证据，报告限制，不推导通过。

## 工程关系

`al.relations` 列表：`id`（本概念局部唯一）、`type`、`target`（概念文件路径，可含目标锚点）、`target_id`（目标 al.id）、`evidence_state`、`source_ids`、confirmed 时的 checked。关系由起点唯一维护；反向边工具派生。

| 类型 | 方向与附加规则 |
| --- | --- |
| contains | 父 Specification → 子 Specification；confirmed 单父无环 |
| implements | API Endpoint → Specification；指向规则锚点；coverage 为 full/partial/missing/unknown/not-applicable；双侧证据回源检查 |
| planned-for | Specification → API Endpoint；必须 planned，不算实现 |
| verifies | 验证规格 → 被验证 Specification/入口；仅表示目标 |
| depends-on / references / relates-to | 起点 → 被依赖/引用/相关项，无层级含义 |
| supersedes | 新概念 → 被替代概念；不能抹去历史 |

`condition`、`reason` 为可选说明。confirmed 目标必须存在且 ID/锚点匹配；其他状态可保留缺失目标并报告缺口。未知关系类型保留并提示不参与类型专用计算。实现覆盖不是测试结果。测试执行放 `al.test_runs`：`result: passed | failed | not-run`、`source_ids`；已执行结果还需 `at`、`by`、`command`，来源必须是实际执行记录，工具不运行或认证它。

## 调用图（按需）

接口可带 `al.entry`，值为包内唯一方法节点 ID。`al.nodes` 每项 `{id, title, kind, symbol?, stop_reason?}`，kind 为 method/boundary；method 需模块限定全签名 symbol，boundary 需 stop_reason。节点只在一个概念定义，其他入口直接引用其 ID。

`al.calls` 每项 `{id, from, to, type, condition?, reason?, evidence_state, source_ids, checked?}`。from 必须是当前概念拥有的节点，to 可指其他概念的节点；边只由起点拥有者维护。类型 calls/dispatches-to/publishes/consumed-by/remote-call，confirmed 需 checked。boundary 可以继续有已核实的消费边；保留停止原因说明当前边界性质。

关系、断言、调用在各自列表内 ID 唯一。各入口图由已登记节点与边有限展开，环和共享节点显示引用；缺失、候选、条件与边界显式，兄弟顺序不暗示实际执行顺序。未登记 entry 或来源无实现时报告缺口。

## 工具输出不变量

基础 OKF errors、工程 profile errors、warnings 分开；语法失败不意味着其他可读概念无效。只读命令绝不修改事实或状态。渲染拒绝结构 errors；stale/unknown 信息仍可以可见地生成，不能伪装 confirmed。

自动校验覆盖结构、引用、声明的摘要和有限关系约束；不证明业务正确、来源权威、测试执行真实性、全项目调用图完整性，也不实现 OKF 计算 attestation。
