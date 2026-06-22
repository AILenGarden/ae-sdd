---
name: coding-report
description: Coding 报告产出 SKILL — Phase 2 ⑤ Coding 完成后的报告产出环节。统一规范 Coding 报告的章节结构、变更文件清单、编译/测试结果、已知问题、异常路径触发条件。与 `be-coding-report-template.md` 配套使用。🆕 2026-06-06 新建，填补 AE 体系 Phase 2 ⑤ SKILL 缺口。
---

# Coding Report — Coding 报告产出 Skill

> **核心定位：** Coding 完成（编译通过 + 测试通过 + 服务可启动）后，**生成 Coding 报告**，作为 Phase 3 ⑦ Code Review 的输入。报告质量直接决定 Code Review 质量。
>
> **与现有 SKILL 的分工：**
> - `coding-skill.md` = ⑤ Coding 怎么写（生成代码 / 异常追溯）
> - **`coding-report-skill.md`（本文件）** = ⑤ 完成后怎么出报告（章节/门禁/与 CodeReview 衔接）
> - [`templates/coding/be-coding-report-template.md`](../../templates/coding/be-coding-report-template.md) = Coding 报告空白模板
> - `code-review-skill.md` = 评审 Coding 报告

---

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 生成的 Coding 报告在写入磁盘前**必须先调用 [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)** 的 API，**不再手写路径**：
> 1. **路径**（§0.6.1 `resolve_path()`）：通过 `intent=CODING_REPORT` 自动定位到 `ae-sdd-doc/iterations/{YYYY-MM-DD}/Coding/{STORY-ID}/`
> 2. **命名 + 版本号**（§0.6.7 `save_doc()`）：**事件类文档带 `v{N}-r{M}`**（v=Story 版本，r=Coding 轮次）
> 3. **重入判定**（§0.6.11 `get_latest_version()`）：Coding 重入时**新增报告**（r 递增），**不修改历史**
> 4. **ChangeLog**（§5）：`save_doc()` 自动追加
> 5. **.gitignore**（§0.6.13 `check_and_update_gitignore()`）：首次写入时自动维护

| 输出文档 | API 调用 | 命名规则 | 重入时动作 |
|---------|---------|---------|----------|
| Coding 报告 | `save_doc(intent="CODING_REPORT", storyId, version={v:N,r:M})` | 带 `v{N}-r{M}` | **新增**（r 递增）|
| Test 报告（由测试 SKILL 产出）| `save_doc(intent="TEST_REPORT", storyId, version={v:N,r:M})` | 带 `v{N}-r{M}` | **新增**（r 递增）|
| ⑦bis 追溯矩阵 | `save_doc(intent="TRACE_MATRIX", storyId, version={v:N,r:M})` | 带 `v{N}-r{M}` | **新增**（r 递增）|

> 🔴 **关键：** Coding 报告**不修改历史**，每次重入都新增一份（r 递增），保留完整审计轨迹。≥ r4 的旧版本可归档到 `archive/{date}/`。

---

## 目标

Coding 完成后生成**完整、客观、可评审**的 Coding 报告，目标是：

- 让 Code Review 阶段能基于报告快速定位代码（按调用层级 / 工程分类）
- 让测试真实性核查有据可查（8 类禁止 + 5 类必真实）
- 让产出物对账闸能验证所有产物真实存在
- 已知问题透明（不藏不报，AI 自报问题）

---

## Coding Report 总则

### 标尺 1：客观性（🔴 禁止主观描述）

| ❌ 禁用 | ✅ 替换为 |
|--------|----------|
| "代码质量很好" | "mvn compile BUILD SUCCESS / 单测 25/25 通过 / 行数 X" |
| "性能不错" | "EXPLAIN 显示全表扫描/索引扫描,具体见 §3.2" |
| "测试完整" | "L1: 8/8 / L2: 5/5 / L3: 3/3 / L4: N/A / 覆盖率 X%" |
| "无问题" | "🔴 阻断: 0 / 🟠 严重: 0 / 🟡 一般: 2 / 🟢 建议: 5" |

### 标尺 2：完整性（🔴 必填章节不能空）

必填 9 章节：元信息 / 本轮变更 / 编译结果 / 测试结果 / 已知问题 / 产出物对账 / 闸门自检 / 下一步建议 / 异常路径触发

### 标尺 3：可追溯性（🔴 每个声明附证据）

- 文件路径 → 文件:行号
- 测试方法 → TestClass.testXxx
- 错误信息 → 原始 stack trace

---

## 整体流程

```
触发（Phase 2 ⑤ Coding 节点完成 / 用户说"出 Coding 报告"）
    ↓
第一步：读取输入（Coding 报告模板 + Story/CodePlan/项目资产/实际代码/测试报告）
    ↓
第二步：填充 9 章节内容
    ├─ §一 元信息
    ├─ §二 本轮变更文件清单（按项目 §3 层级）
    ├─ §三 编译 + 服务启动结果
    ├─ §四 测试结果（L1/L2/L3/L4 分层）
    ├─ §五 已知问题（自报）
    ├─ §六 产出物对账
    ├─ §七 闸门自检
    ├─ §八 下一步建议
    └─ §九 异常路径触发（如有）
    ↓
第三步：合理性自检（4 维度）
    ↓
第四步：生成 Coding 报告
    ↓
触发下游 Code Review SKILL
```

---

## 触发条件

| 触发场景 | 触发方式 |
|---------|---------|
| Phase 2 ⑤ Coding 节点 | Coding SKILL §完成判定后自动触发 |
| 用户手动 | "出 Coding 报告" / "Coding 完成" / "代码完成" |

---

## 第一步：读取输入

| # | 文件 | 必读 | 用途 |
|---|------|------|------|
| 1 | Coding 报告模板 | `templates/coding/be-coding-report-template.md` | 9 章节结构 |
| 2 | Story 文档 | `documentStorage.resolve_path(intent="STORY", storyId)` | AC/接口契约/数据模型 |
| 3 | 统一版 CodePlan | `{STORY-ID}-CodingPlan.md` | 类骨架/SQL/测试对应 |
| 4 | 项目资产 | `{projectKey}.assets.md §3 §4 §5 §6` | 分层/命名/约束 |
| 5 | 实际代码 | `git diff {base}..{head}` | 变更文件清单 |
| 6 | 测试报告 | `{STORY-ID}-Report-v{N}-r{M}.md` | 测试结果（4 层）|

---

## 第二步：填充 9 章节内容

### §一 元信息

| 字段 | 值 |
|------|---|
| Story ID / 标题 | {STORY-ID} / {标题} |
| Coding 轮次 | r{M}（第几轮 Coding）|
| Story 版本 | v{N} |
| 涉及工程 | {工程1, 工程2}（与项目资产 §2 对齐）|
| 报告时间 | {YYYY-MM-DD HH:mm} |
| 报告人 | AI（Claude Code） |

### §二 本轮变更文件清单（🔴 按项目 §3 层级自上而下）

> **🔴 必读：** 变更文件按项目资产 §3 实际分层（不是变更类型）。从 BFF 入口到测试层自上而下填。

| # | 文件路径 | 变更类型 | 所属工程 | 关键改动 | 涉及行号 |
|---|---------|---------|---------|---------|---------|
| | | 新增/修改/删除 | | | |

> 🔴 **不可省略：** 每行必填"涉及行号"。

### §三 编译 + 服务启动结果

**编译结果：**
- 命令：`mvn clean install -DskipTests`
- 结果：BUILD SUCCESS / BUILD FAILURE
- 编译耗时：X 秒
- 编译警告数：N（≤ 0 才达标）

**服务启动结果（如有 BFF/Service）：**
- 启动命令：`java -jar {bff-module}/target/*.jar`
- 启动耗时：X 秒
- 注册到 Nacos：✅/❌
- 健康检查：`curl /actuator/health` 返回 `{"status":"UP"}` ✅/❌

### §四 测试结果

**分层结果：**

| 层级 | 用例数 | 通过 | 失败 | 通过率 | 覆盖率 |
|------|--------|------|------|--------|--------|
| L1（Service 业务逻辑）| {N} | {M} | {K} | {%} | {%} |
| L2（Controller HTTP 真实）| {N} | {M} | {K} | {%} | {%} |
| L3（Repository 真实 DB）| {N} | {M} | {K} | {%} | {%} |
| L4（多 Story 协作）| {N} | {M} | {K} | {%} | {%} |
| **合计** | {N} | {M} | {K} | {%} | {%} |

**失败用例清单（如有）：**

| 用例 ID | 用例描述 | 失败原因 | 严重性 | 状态 |
|--------|---------|---------|--------|------|
| | | | 🔴/🟠/🟡 | Open/Fixed |

**测试真实性自检：**

| 检查项 | 状态 |
|--------|------|
| 无 `@Disabled` / `@Ignore` | ✅/❌ |
| 无 `assertTrue(true)` 永真 | ✅/❌ |
| 无 `catch (Exception e) {}` 吞噬 | ✅/❌ |
| 无全 Mock 替代（核心落库用真实 DB）| ✅/❌ |
| 无期望值=实际值 | ✅/❌ |
| 无 `Thread.sleep` 绕过 | ✅/❌ |
| 无凑覆盖率 | ✅/❌ |

### §五 已知问题（🔴 AI 自报，不藏不报）

> **🔴 必填：** 即便没有严重问题，也要列出 🟡/🟢 级问题。

| # | 问题 | 严重性 | 原因 | 后续动作 |
|---|------|--------|------|---------|
| 1 | | 🔴/🟠/🟡/🟢 | | |

### §六 产出物对账

| 产出物 | 实际路径 | 是否存在 | 与报告一致 |
|--------|---------|---------|----------|
| 源代码 | 工作目录 | □ | □ |
| 单元测试 | src/test | □ | □ |
| 集成测试 | src/test | □ | □ |
| DDL（如有）| db/migration | □ | □ |
| 配置文件变更 | application*.yml | □ | □ |

### §七 闸门自检

| 闸 | 名称 | 状态 | 备注 |
|----|------|------|------|
| 1 | 编译通过 | ✅/❌ | |
| 2 | 服务启动成功 | ✅/❌ | |
| 3 | 所有测试 Pass | ✅/❌ | |
| 4 | 测试真实性 0 命中 | ✅/❌ | |
| 5 | 真实 DB/HTTP 覆盖 | ✅/❌ | |
| 6 | 变更文件清单完整 | ✅/❌ | |
| 7 | 已知问题自报完整 | ✅/❌ | |

### §八 下一步建议

- 触发 Code Review SKILL → 生成 CodeReview 报告
- 如有 🔴 已知问题 → 触发 Coding 实时追溯链（先完善 Task/Story/DR）
- 触发 Story Update / Project Assets Update（如发现文档漂移）

### §九 异常路径触发（如有）

> 如果 Coding 过程中触发了异常路径 A1-A6，必须记录触发原因和处理结果。

| 异常路径 | 触发条件 | 处理结果 |
|---------|---------|---------|
| A1 问题记录 | {N} 条 | {简述} |
| A2 根因分析 | Task 错 / Story 错 / DR 错 / AI 犯蠢 | {简述} |
| A3 处理 | 改 Task / 改 Story / 改 DR / 改代码 | {简述} |
| A4 补充文档 | 写入 Supplement | ✅/❌ |
| A5 触发 Story Update | {是/否} | {原因} |
| A6 回到正常路径 | ✅/❌ | |

---

## 第三步：合理性自检

| 维度 | 必查项 | 状态 |
|------|--------|------|
| 客观性 | 无主观描述（✅/❌ 用数据替代）| ✅/❌ |
| 完整性 | 9 章节全填 | ✅/❌ |
| 可追溯性 | 每个声明附证据 | ✅/❌ |
| 与下游衔接 | 触发 Code Review | ✅/❌ |

---

## 第四步：生成 Coding 报告

按 [`templates/coding/be-coding-report-template.md`](../../templates/coding/be-coding-report-template.md) 模板汇总，输出 `{STORY-ID}-CodingReport-v{N}-r{M}.md`。

**🔴 强制：** 写入前先打印报告初稿（用 Read 显示），用户确认后再写入。

---

## 第五步：触发下游 SKILL

| 下游 | 触发 SKILL | 引用章节 |
|------|-----------|---------|
| Code Review | `code-review-skill.md` | 整体流程（Phase 3 ⑦） |

---

## 禁止事项

| # | 禁止 | 危害 | 正确做法 |
|---|------|------|---------|
| 1 | 禁止"测试通过✅"无证据 | 失真 | §四 测试结果 + 真实 DB/HTTP |
| 2 | 禁止隐藏已知问题 | 评审盲区 | §五 已知问题 AI 自报 |
| 3 | 禁止变更文件清单不按层级 | Code Review 困难 | §二 按项目资产 §3 层级 |
| 4 | 禁止"代码质量好"等主观词 | 不可验证 | §总则 标尺 1 |
| 5 | 禁止跳过产出物对账 | 报告失真 | §六 产出物对账 |
| 6 | 禁止未过 7 道闸就出报告 | 报告无效 | §七 闸门自检 |

---

## 执行清单

| # | 动作 | 产出 | 门禁 |
|---|------|--------|------|
| 1 | 读取 6 个输入文件 | 输入清单 | 6 文件全读 ✅ |
| 2 | 填充 §一 元信息 | 元信息表 | 字段齐 |
| 3 | 填充 §二 变更文件清单 | 清单表 | 按 §3 层级 |
| 4 | 填充 §三 编译+启动结果 | 编译/启动结果 | BUILD SUCCESS / 启动成功 |
| 5 | 填充 §四 测试结果 | 测试结果 | L1/L2/L3/L4 全填 + 测试真实性自检 7 项 |
| 6 | 填充 §五 已知问题 | 问题清单 | AI 自报不藏 |
| 7 | 填充 §六 产出物对账 | 对账表 | 5 类产出物全 ✅ |
| 8 | 填充 §七 闸门自检 | 闸门结果 | 7 闸门全 ✅ |
| 9 | 填充 §八 下一步建议 | 建议 | 触发 Code Review |
| 10 | 填充 §九 异常路径（如有）| 异常记录 | A1-A6 完整 |
| 11 | §第三步 合理性自检 | 自检报告 | 4 维度全 ✅ |
| 12 | §第四步 生成报告 | Coding 报告 | 用户确认初稿 |
| 13 | §第五步 触发 Code Review | 评审反馈 | Code Review SKILL 启动 |

---

## 维护

- **维护人：** 架构组 + 各项目 owner
- **更新频率：** 每次 Phase 2 ⑤ Coding 完成
- **同步对象：**
  - 与 `coding-skill.md` 协调（Coding 怎么写 → Coding 完成后出报告）
  - 与 `code-review-skill.md` 协调（报告是 Code Review 的输入）
  - 与 `ae-sdd-skill.md` 协调（AE 编排层角色 6 指针）
  - 与 `document-storage-skill.md` 协调（文档存放路径）
- **关键变化（2026-06-06 重大重构）：**
  - 🆕 新建独立 SKILL（之前只有 AE-skill 角色 6 的 5 行描述 + 空白模板）
  - 9 章节必填结构
  - 7 道闸自检（与 Code Review 闸门对齐）
  - 异常路径触发记录
