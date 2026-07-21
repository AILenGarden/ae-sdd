---
name: test-generate
description: Test 系列 Step 2 generateSkill。Coding 完成后运行编译、启动与 L1/L2/L3/L4 测试，把真实命令和 artifact 写入 evidence manifest。
---

# Test Generate — 测试运行与 Evidence SKILL

> **v3.12 evidence-only：** 测试执行不再生成 TestReport Markdown。每条真实命令写入 evidence manifest（command、exitCode、summary、artifact）；最终回复只展示失败项和摘要。既有报告仅供只读兼容。

## 与监管器 4 步的关系

本文件只负责 **Test 系列 Step 2：generateSkill**。

| 系列步骤 | 执行方 | 本文件职责 |
|---|---|---|
| Step 1 compact + 调用声明 | 主流程监管器 | 无 |
| Step 2 generateSkill | `test-generate-skill.md` | 运行验证命令并写 immutable evidence |
| Step 3 reviewSkill + Loop | `test-review-skill.md` + 主流程监管器 | 按缺陷报告重跑或补证据 |
| Step 4 人工审核 | 主流程监管器 | 提供测试摘要材料 |

禁止在 CodingSkill 内替代本系列；CodingSkill 只交付代码，测试运行与 evidence 归本文件。

## 输入

| 输入 | 用途 |
|---|---|
| Story / TestCase / Task / CodingPlan | 确定 AC、用例、真实 HTTP/DB 要求 |
| 变更文件清单 | 选择最小但充分的测试范围 |
| 项目资产 §6.7 / §11.8 | 读取测试框架、环境、惯用写法 |
| `source/standards/constraints/testing.md` | 测试红线 |

## 流程

### 0. 先执行场景合同

对新 `scenarioPolicyVersion=1` 的 HTTP AC，先验证 CapabilityModel/ScenarioManifest 和 `G-HTTP-1`。未通过时不得启动 HTTP、数据库或项目测试命令；不得用 Mock 或状态码断言替代缺失场景。

### 1. 制定执行矩阵

先运行 `ae-sdd verify plan --story {STORY-ID} --changed <paths>`，再从 TestCase 文档提取应跑用例。仅 Markdown 变化不得安排 Maven；生产代码变化先跑 focused/module 验证，稳定 implementation fingerprint 后只跑一次最终全量回归。

| 层级 | 必跑对象 | 证据 |
|---|---|---|
| L1 | 单元测试 / 纯逻辑测试 | TestCase ID + 测试类方法 |
| L2 | 本地真实 HTTP 接口测试 | `RANDOM_PORT + HTTP client`，内部主链真实，记录 `http-local` |
| L3 | 真实 DB / SQL / 事务测试 | INSERT/UPDATE 后 SELECT、回滚证据 |
| L4 | 测试环境 HTTP / 跨 Story 路径 | 同一 buildId 的非 loopback endpoint，记录 `http-test-env` |

### 2. 执行验证命令

每条命令执行前先用 `ae-sdd evidence lookup` 按 implementation fingerprint、command hash、toolchain fingerprint 和 artifact SHA-256 查成功证据。完整命中且未超过 freshness window 时复用；失败、篡改、过期或任一 fingerprint 不同必须重跑。命令必须可复现并写入 evidence：

| 目标 | 命令要求 |
|---|---|
| 编译 | 父工程根执行 `mvn compile` 或项目等价命令 |
| 本地 HTTP | 启动真实端口，确认内部 Service/Repository/Mapper/DB 未被 mock，记录 baseUrl/buildId/AC/artifact |
| 测试环境 HTTP | 部署同一 buildId 后请求非 loopback endpoint，记录同组 AC 与 artifact |
| 测试 | 禁止 `-DskipTests`、`maven.test.skip=true`、`testFailureIgnore=true` |
| 扫描 | 运行 `scripts/test_authenticity_scan.py` 或 `ae-sdd gates check --only G-09` |

所有 stdout/stderr、Surefire/Failsafe XML、扫描报告必须归档到 `.auto-engineering/{WORKITEM-ID}/evidence/`，并由单一 `manifest.json` 登记。documentation/review fingerprint 变化不得使只绑定 implementation fingerprint 的 Maven 证据失效。

### 3. 记录 Evidence

使用 `ae-sdd evidence record` 写入真实命令和 artifact；完成后 `ae-sdd evidence finalize --story {STORY-ID}`。接口 AC 必须各有 active `http-local` 与 `http-test-env`，summary 包含：

- `stage=local|test-env`、`baseUrl`、`buildId`、`acIds`、`internalMocks=false`、`result=PASS`。
- local URL 必须是 loopback；test-env URL 必须非 loopback；URL 不含 userinfo/query。
- local evidence 先于 test-env，两个阶段 buildId 相同。
- 外部 sandbox/stub 故障注入写 `http-external-supplemental`，不计入双阶段完成。

### 4. 初步判定

| 判定 | 行为 |
|---|---|
| 全部命令成功 + XML 对账一致 + HTTP 双阶段 evidence + G-09 BLOCKER=0 | 交给 `test-review-skill.md` 独立复核 |
| 有失败 | evidence 记录真实 exitCode/失败 artifact，不得隐藏失败 |
| 缺 test-env、环境不可达或证据不一致 | Work Item 保持 BLOCKED，修复环境或重跑 |

## 禁止事项

| 禁止 | 正确做法 |
|---|---|
| 口头声称“测试通过” | 以命令、日志、XML、扫描报告为证 |
| 跳测或忽略失败 | 去掉跳测参数，失败必须暴露 |
| 人工估算测试数量 | 从 Surefire/Failsafe XML 解析 |
| 修测试代替修代码 | 先判根因；修测试须记录原因并获用户确认 |
| MockMvc 或内部 MockBean/SpyBean 替代接口主链 | 本地真实端口完整内部链 + 同 buildId 测试环境 HTTP；不可降级通过 |
| 代码未变却重复跑 Maven | 先查 evidence manifest；命中可复用证据时禁止重跑 |

## 执行清单

| # | 动作 | 产出 | 门禁 |
|---|---|---|---|
| 1 | 生成 VerificationPlan + 读取输入 | 测试执行矩阵 | 变更分类与 TestCase/AC/方法映射齐 |
| 2 | 运行编译/启动/测试 | 原始证据 | 无跳测、无忽略失败 |
| 3 | 运行真实性扫描 | 扫描报告 | BLOCKER=0 或报告不通过 |
| 4 | 写并 finalize evidence manifest | immutable snapshots | HTTP AC 双阶段、同 buildId、internalMocks=false |
| 5 | 交接复核 | 复核输入清单 | `test-review-skill.md` 按 fingerprint/hash 独立验证 |
