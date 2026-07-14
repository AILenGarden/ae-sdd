---
name: sonar-issue-fix
description: Sonar 问题修复 SKILL。接收 SonarQube、SonarCloud、SonarQube for IDE、MCP 或导出报告中的 issue，在上游 TextEdit、保守硬编码规则、推理修复和人工处理之间做唯一分类，执行防陈旧/防越界校验，并用编译、测试和 Sonar 复扫闭环。由 CodeReview 在收尾闸门前每个评审会话恰好调用一次；用户要求修复 Sonar、质量门失败、处理 rule key 或清理静态分析问题时也触发。
---

# Sonar Issue Fix

## 定位与边界

本 SKILL 是 CodeReview 的 Sonar 专项收尾能力，不是 Sonar 分析器，也不是 IntelliJ UI 自动化脚本。优先消费官方 Sonar issue、rule、quality gate 和完整 quick-fix payload；没有可执行 payload 时，才进入本地保守规则或推理/人工路径。

可以复用 IntelliJ/SonarQube for IDE 已公开的补丁协议思想：一个 quick fix 由目标文件上的有序 `TextEdit(range, newText)` 组成，应用前必须校验当前文件快照。IDEA 的灯泡、`quickFix=true` 或“Quick Fix available”提示只是能力提示，不等于可执行 payload，不能替代实际 `TextEdit`。

权威边界：

- 修复交互与预览语义以 [SonarQube for IDE fixing issues](https://docs.sonarsource.com/sonarqube-for-intellij/using/fixing-issues/) 为依据。
- 上游 edit 数据形状参考 LGPL 的 SonarLint Core/IntelliJ 插件公开实现，只复用协议，不复制 UI 或分析逻辑。
- SonarJava 当前受 `Sonar Source-Available License v1.0` 约束；不得复制、移植或让模型改写 SonarJava 分析器/quick-fix 实现。规则配方必须独立撰写，并只引用公开规则语义和可验证行为。
- Sonar MCP/Web API 是输入通道。没有证据表明其提供完整 quick-fix payload 时，不得把 issue/rule 元数据伪装成 `upstream-edit`。

详细规则与许可证约束见 [`sonar-issue-fix-rules.md`](../../standards/review/sonar-issue-fix-rules.md)。

## 输入契约

接受 Sonar MCP、Sonar Web API、scanner 报告、IDE 导出或用户给出的结构化 issue。凭据只从环境变量/env 或已有凭据存储读取；输出、日志、命令、补丁和报告不得包含 token/令牌、密码或完整认证头。

每条 issue 至少归一化为：

| 字段 | 必需 | 说明 |
| --- | --- | --- |
| `issueKey` | 是 | 稳定 issue 标识；同一次输入按它去重 |
| `ruleKey` | 是 | 带语言命名空间，如 `java:S1128` |
| `target` | 是 | 仓库内相对路径 |
| `range` | 是 | Sonar 报告位置；不能直接视为可写范围 |
| `message` | 是 | 问题描述 |
| `severity` / `type` | 否 | 风险分类依据 |
| `analyzerVersion` | 否 | 规则漂移判断；缺失会降低自动化等级 |
| `quickFix` | 否 | 仅提示上游可能有 fix |
| `upstreamEdits` | 否 | 实际文件 edit payload；完整时才可选 `upstream-edit` |

同一 `issueKey` 出现多次只保留信息最完整的一条；字段冲突时标记冲突并转 `manual`，禁止猜测。无 Sonar 输入或服务不可达时返回 `N/A`，但在 CodeReview 中仍算完成了本会话的一次调用。

## 唯一分类

每条去重后的 issue 必须且只能选择一种模式：

| 模式 | 使用条件 | 可否直接修改 |
| --- | --- | --- |
| `upstream-edit` | 输入含完整、可验证的上游文件 edits 与来源 | 通过全部防护后可以 |
| `registry` | `ruleKey` 命中规则注册表，分析器版本和所有前置条件满足 | 通过全部防护后可以 |
| `reasoned` | 需要理解项目语义，但风险可由现有 Story/CodePlan/测试约束控制 | 先形成 CodeReviewUpdatePlan，按已批准计划修改 |
| `manual` | 安全敏感、契约不清、证据不足、冲突或超出授权范围 | 不可以；只给证据和处置建议 |

未知规则不得静默丢弃，默认进入 `reasoned`；若缺少足够代码/规格/测试上下文则进入 `manual`。

以下类型一律 `manual`，不得盲改或自动修改：security/taint、Security Hotspot、认证/授权、密码学/密钥、并发/锁、事务边界、公共 API/序列化契约，以及可能改变持久化数据含义的规则。

## EditPlan 协议

任何写入前先产出并展示 EditPlan：

```json
{
  "issueKey": "AX-example",
  "ruleKey": "java:S1128",
  "mode": "registry",
  "target": "src/main/java/example/App.java",
  "baseSha256": "<64 hex chars>",
  "edits": [
    {
      "range": {
        "startLine": 3,
        "startColumn": 1,
        "endLine": 4,
        "endColumn": 1
      },
      "newText": ""
    }
  ],
  "provenance": {
    "kind": "registry",
    "analyzerVersion": "<observed version>",
    "source": "source/standards/review/sonar-issue-fix-rules.md"
  }
}
```

约束：

1. `target` 解析后的真实路径必须位于仓库根内；路径逃逸、符号链接逃逸、绝对外部路径一律 `skip`。
2. 写入瞬间重新计算 `baseSha256`。哈希不一致表示陈旧/失配，整条计划 `skip`，重新获取 issue 和文件，不做模糊套用。
3. `range` 必须合法、有序、可映射到当前文本；同一文件 edits 先按起点排序，再检查不得重叠。重叠时整批拒绝。
4. 多文件 quick fix 必须先验证所有目标和 edits，再原子应用；当前工具无法保证原子性时，多文件计划全部拒绝并转 `manual`。
5. 一次只应用一个坐标稳定的文件批次。删除/插入导致后续坐标变化后，必须重跑 Sonar 或重新取得 payload，禁止继续套用旧坐标。
6. 应用前输出 diff 预览；diff 超出 issue 目标、引入凭据、修改生成物/依赖缓存或触及未授权文件时拒绝。

## 执行流程

### 1. 建立调用上下文

记录 `reviewSessionId`、`sonarInvocationCount`、仓库根、基线 commit、输入来源和扫描版本。CodeReview 调用时若本会话 `sonarInvocationCount` 已为 1，立即返回已有结果，不得递归再入。

### 2. 归一化与分类

按 `issueKey` 去重，逐条补齐 `ruleKey`、目标、位置、版本和来源。按照 `upstream-edit -> registry -> reasoned -> manual` 的证据优先级选择唯一模式；优先级不是兜底授权，任一安全禁区都直接覆盖为 `manual`。

### 3. 形成并校验 EditPlan

- `upstream-edit`：必须拿到实际 `TextEdit`；只有 quickFix 提示/标志但没有 payload 时降级。
- `registry`：逐项核对注册表的 analyzer 版本、前置条件、负例和 skip 条件。
- `reasoned`：把根因、候选改法、影响面和验证映射写入 CodeReviewUpdatePlan；未批准不得写入。
- `manual`：记录阻断原因、所需负责人/信息和可复现证据。

随后执行路径、哈希、范围、重叠、多文件原子性和 diff 防护。任何防护失败都不得部分应用。

### 4. 应用最小修改

严格按已验证 EditPlan 修改，不顺手格式化无关代码，不扩大规则范围。`registry` 配方只能调用 [`sonar-issue-fix-rules.md`](../../standards/review/sonar-issue-fix-rules.md) 当前生效条目；禁止从 SonarJava 源码临时抄出新配方。

### 5. 验证闭环

按项目实际工具链执行：

1. 语法检查和 compile/编译。
2. 受影响的 focused test，再执行 CodePlan/TestCase 要求的 test/测试层级。
3. 以同一分析器配置重跑 Sonar；确认原 issue 消失或已不存在。
4. 对比新增问题，Blocker/Critical/阻断级回归必须为 0；quality gate 不得恶化。
5. 保存命令、退出码、扫描任务/报告标识、前后 issue 状态和 diff hash。不得保存 secret。

只有上述证据全部通过，状态才可为 `fixed`。无法验证标记 `unverified`，验证失败或出现回归标记 `failed`，不得报告为 fixed；未改动分别使用 `skipped` 或 `manual`。

### 6. 返回结果

每个 issue 输出：`issueKey`、`ruleKey`、模式、状态、原因、目标、前后哈希、验证命令/结果和剩余风险。汇总输出 `fixed/skipped/manual/unverified/failed` 数量，以及 quality gate 前后状态。

## CodeReview 收尾协议

CodeReview 必须在第六步循环已经收敛后、第七步最终闸门前调用本 SKILL，且每个评审会话恰好一次：

- Sonar 可用：处理当前 review scope 的 issue，并返回证据。
- Sonar 不可用或 scope 无 Sonar 配置：返回 `N/A` 与原因；仍计为本会话已经调用一次。
- 如果本 SKILL 改动源码：重新打开受影响的 compile、测试和 CodeReview 维度，清除相关旧证据后复核；同一评审会话不得调用第二次 Sonar。本轮后续又发生代码变化时，记录 residual risk，由新的 CodeReview 会话再调用一次。
- 本 SKILL 返回 `failed` 或 `unverified`：第七步不得给出通过结论；按异常路径处理。

该协议防止“Sonar 修复触发 CodeReview，CodeReview 又递归触发 Sonar”的循环，同时保证修复后的源码不会带着旧测试证据直接收尾。

## 禁止事项

- 禁止仅凭 rule message 大范围字符串替换。
- 禁止把 IDE 灯泡、quickFix 布尔值或 MCP issue 元数据当作 edit payload。
- 禁止忽略 `baseSha256`、范围重叠或路径逃逸后继续写入。
- 禁止拆开应用无法原子验证的多文件修复。
- 禁止自动修复 security/taint/hotspot、认证、密码学、并发、事务或公共 API 问题。
- 禁止复制 SonarJava 分析器实现来扩充硬编码规则。
- 禁止在输出中打印 Sonar token/令牌或认证头。
- 禁止在未复扫 Sonar 时宣称 issue 已 fixed。
