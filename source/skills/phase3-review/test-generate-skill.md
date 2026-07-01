---
name: test-generate
description: Test 系列 Step 2 generateSkill。Coding 完成后运行编译、启动与 L1/L2/L3/L4 测试，生成带原始证据链的测试报告。
---

# Test Generate — 测试运行与报告 SKILL

## 与监管器 4 步的关系

本文件只负责 **Test 系列 Step 2：generateSkill**。

| 系列步骤 | 执行方 | 本文件职责 |
|---|---|---|
| Step 1 compact + 调用声明 | 主流程监管器 | 无 |
| Step 2 generateSkill | `test-generate-skill.md` | 运行验证命令并生成 `TEST_REPORT` |
| Step 3 reviewSkill + Loop | `test-review-skill.md` + 主流程监管器 | 按缺陷报告重跑或补证据 |
| Step 4 人工审核 | 主流程监管器 | 提供测试摘要材料 |

禁止在 CodingSkill 内替代本系列；CodingSkill 只交付代码，测试运行与测试报告归本文件。

## 输入

| 输入 | 用途 |
|---|---|
| Story / TestCase / Task / CodingPlan | 确定 AC、用例、真实 HTTP/DB 要求 |
| 变更文件清单 | 选择最小但充分的测试范围 |
| 项目资产 §6.7 / §11.8 | 读取测试框架、环境、惯用写法 |
| `source/templates/testcase/be-testcase-report-template.md` | 测试报告格式 |
| `source/standards/constraints/testing.md` | 测试红线 |

## 流程

### 1. 制定执行矩阵

从 TestCase 文档提取应跑用例，形成矩阵：

| 层级 | 必跑对象 | 证据 |
|---|---|---|
| L1 | 单元测试 / 纯逻辑测试 | TestCase ID + 测试类方法 |
| L2 | 真实 HTTP 接口测试 | `RANDOM_PORT + TestRestTemplate`，MockMvc 降级须写原因 |
| L3 | 真实 DB / SQL / 事务测试 | INSERT/UPDATE 后 SELECT、回滚证据 |
| L4 | 端到端 / 跨 Story 路径 | 全链路步骤与外部依赖 Mock 边界 |

### 2. 执行验证命令

命令必须可复现，并写入测试报告：

| 目标 | 命令要求 |
|---|---|
| 编译 | 父工程根执行 `mvn compile` 或项目等价命令 |
| 服务启动 | 需验证真实 HTTP 时启动应用，记录端口与启动日志 |
| 测试 | 禁止 `-DskipTests`、`maven.test.skip=true`、`testFailureIgnore=true` |
| 扫描 | 运行 `scripts/test_authenticity_scan.py` 或 `ae-sdd gates check --only G-09` |

所有 stdout/stderr、Surefire/Failsafe XML、扫描报告必须归档到 `.auto-engineering/{STORY-ID}/evidence/` 或测试报告可引用路径。

### 3. 生成测试报告

使用 `save_doc(intent="TEST_REPORT", storyId, version={v:N,r:M})` 写报告。报告必须包含：

- 实际命令、工作目录、退出码、Profile。
- 原始日志、XML、扫描报告路径。
- XML 对账：tests / failures / errors / skipped 与报告统计一致。
- TestCase ID ↔ 测试方法 ↔ AC ID 映射。
- L1/L2/L3/L4 结果。
- 失败用例根因分类：代码缺陷 / 测试数据 / 环境问题 / 设计缺陷。
- 修改测试记录：修改原因、证据、用户确认记录。

### 4. 初步判定

| 判定 | 行为 |
|---|---|
| 全部命令成功 + XML 对账一致 + G-09 BLOCKER=0 | 交给 `test-review-skill.md` 独立复核 |
| 有失败 | 报告结论写“不通过”，不得隐藏失败 |
| 证据缺失 / 统计不一致 | 报告作废，补证据后重跑 |

## 禁止事项

| 禁止 | 正确做法 |
|---|---|
| 口头声称“测试通过” | 以命令、日志、XML、扫描报告为证 |
| 跳测或忽略失败 | 去掉跳测参数，失败必须暴露 |
| 人工估算测试数量 | 从 Surefire/Failsafe XML 解析 |
| 修测试代替修代码 | 先判根因；修测试须记录原因并获用户确认 |
| Mock 核心 HTTP/DB 路径 | L2 真实 HTTP，L3 真实 DB；只能解释性降级 |

## 执行清单

| # | 动作 | 产出 | 门禁 |
|---|---|---|---|
| 1 | 读取输入 | 测试执行矩阵 | TestCase/AC/方法映射齐 |
| 2 | 运行编译/启动/测试 | 原始证据 | 无跳测、无忽略失败 |
| 3 | 运行真实性扫描 | 扫描报告 | BLOCKER=0 或报告不通过 |
| 4 | 生成测试报告 | `TEST_REPORT` | XML 与报告对账一致 |
| 5 | 交接复核 | 复核输入清单 | 可由 `test-review-skill.md` 独立验证 |
