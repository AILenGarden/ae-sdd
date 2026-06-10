---
name: project-assets-update
description: 项目资产目录 SKILL — 覆盖"生成/更新/审计"3 个动作。新项目启动时走探查 SOP 生成；新增微服务/修改分层/补缺口时增量更新；每月一次双源一致性审计。④bis CodePlan SOP 步骤 1"读取项目资产"已迁入本 SKILL。当用户说"生成项目资产"/"更新项目资产"/"审计项目资产"/"补项目资产缺口"时触发。
---

# Project Assets Update — 项目资产目录生成/更新/审计 SKILL

> **本质：** 项目资产是 ④bis CodePlan ⑤ Coding ⑦ CodeReview 阶段的"项目事实基线"。**项目资产是活的** — 新增微服务/修改分层/补缺口都必须增量更新；每月必须做一次双源一致性审计。
>
> **本 SKILL 覆盖 3 个动作：**
> 1. **生成** — 新项目启动 / 首次构建项目资产
> 2. **更新** — 新增微服务 / 修改分层 / 补缺口
> 3. **审计** — 每月一次例行审计（双源一致性 + 缺口进度）
>
> **关系：**
> - **本 SKILL（怎么做）** = 流程/触发/门禁
> - [`project-assets-schema.md`](../../standards/project-assets/project-assets-schema.md)（**是什么**）= 结构定义/数据模型
> - [`project-assets-template.md`](../../standards/project-assets/project-assets-template.md)（**怎么起步**）= starter 模板
> - [`templates/project-assets/project-assets-update-log-template.md`../../templates/project-assets/project-assets-update-log-template.md)（**记录什么**）= 更新日志模板
> - **④bis CodePlan SOP 步骤 1 "读取项目资产"** = 已迁入本 SKILL（见 §X）

---

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 生成的项目资产在写入磁盘前**必须先调用 [`document-storage-skill.md`(../cross-cutting/document-storage-skill.md)** 确定：
> 1. **路径**（§2.5 路径模板）：
>    - 主体：`skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md`
>    - 日志：`skills/ae-sdd/project-assets/{projectKey}/{projectKey}.update-log.md`
> 2. **命名**（§3.1/3.2 命名规则）：**基础设施类文档不带版本号**（资产是单一权威源，更新通过日志追踪）
> 3. **重入判定**（§4 重入 SOP）：项目资产**永不修改文件名**（只通过更新日志记录变更）

| 输出文档 | 路径模板 | 命名规则 | 重入时动作 |
|---------|---------|---------|----------|
| 项目资产主体 | `skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md` | 不带版本号 | 原地修改（变更加日志）|
| 项目资产更新日志 | `skills/ae-sdd/project-assets/{projectKey}/{projectKey}.update-log.md` | 不带版本号 | 原地累加（每变更 1 条）|

> 🔴 **关键：** 项目资产是"单一权威源"，**永不通过文件名识别版本**，所有变更通过 `{projectKey}.update-log.md` 追踪。

---

## 0. 目标

- 让每个项目的代码事实（微服务清单 / DDD 分层 / 命名约定 / 工程约束 / 契约入口）有**单一权威源**
- 让项目资产**随项目演进**而非一次写定
- 让 ④bis CodePlan / ⑤ Coding / ⑦ CodeReview 始终基于**最新事实**
- 与 `constraints/`（规则层）保持**双源一致性**

---

## 1. 触发条件

| 触发词 | 动作 |
|--------|------|
| "生成项目资产" / "构建项目资产" / "首次建立项目资产" | → §3 生成 |
| "更新项目资产" / "新增微服务" / "修改分层" / "补缺口" | → §4 更新 |
| "审计项目资产" / "双源一致性" / "每月审计" | → §5 审计 |
| "读取项目资产" / "加载项目资产" | → §6 读取（其他 SKILL 调用） |

---

## 2. 整体流程

```
动作 1：生成
  读 CLAUDE.md + AGENTS.md + README（优先级 CLAUDE.md > README）
  → 读 constraints/ 8 个 .md
  → 跑 mvn dependency:tree 列 SPI 依赖
  → 抽典型类（每层 1-2 个）
  → 抽命名约定（7 类各 5 个类名）
  → 抽跨服务契约（grep 关键文件）
  → 写 §3 抽象分层 → 项目分层映射
  → 列 §10 缺口
  → 审计 + 写 §1 lastAuditedAt
  → 写更新日志（首次生成 = initial entry）

动作 2：更新
  识别变更类型（新增微服务 / 修改分层 / 补缺口 / 修复错误）
  → 跑对应探查命令
  → 增量更新项目资产对应章节
  → 跑双源一致性检查（如果涉及 §6 工程约束）
  → 写更新日志条目

动作 3：审计
  读更新日志 → 确认所有"已补"项已落入资产
  → 跑双源一致性脚本（§6 工程约束 vs constraints/）
  → 跑缺口进度（§10 已补项数 / 总缺口数）
  → 更新 §1 lastAuditedAt
  → 输出审计报告
```

---

## 3. 动作 1：生成（首次或新项目）

### 3.1 触发场景

- 新项目启动（git 仓库初始化后）
- 老项目首次构建项目资产
- 完全重构导致项目结构巨变

### 3.2 9 步 SOP（详见 `project-assets-schema.md §9`）

| 步骤 | 动作 | 产物 |
|------|------|------|
| 1 | 读 CLAUDE.md + AGENTS.md + README（优先级 CLAUDE.md > README） | §1 元信息 + §3/§5/§6 初稿 |
| 2 | 读 constraints/ 8 个 .md | §6 工程约束初稿 |
| 3 | 跑 mvn dependency:tree 列 SPI 依赖 | §2 dependsOnSpi |
| 4 | 抽典型类（每层 1-2 个） | §4 典型类名 |
| 5 | 抽命名约定（7 类各 5 个类名） | §5 |
| 6 | 抽跨服务契约（§7.2 命令清单 grep） | §7 |
| 7 | 写 §3 抽象分层 → 项目分层映射 | §3 |
| 8 | 列 §10 缺口 | §10 |
| 9 | 审计 + 写 §1 lastAuditedAt + 写更新日志 | §1 + log |

### 3.3 输出物

- `skills/ae-sdd/project-assets/{project-key}/{project-key}.assets.md`（按 schema 12 节填写）
- `skills/ae-sdd/project-assets/{project-key}/{project-key}.update-log.md`（更新日志，首次条目 = initial）

### 3.4 门禁

- 🔴 12 节**全部**填写（无"待定"项；不知则写"未探查 + 计划探查时间"）
- 🔴 至少填 1 个典型类的 §4 包路径（无典型类 = 项目无代码 = 不可生成）
- 🔴 §10 缺口列表**非空**（即使探查全面也要记录"已确认无缺口"）
- 🔴 `lastAuditedAt` 已写
- 🔴 更新日志 initial 条目已写

---

## 4. 动作 2：更新（增量修改）

### 4.1 触发场景

- 新增微服务（如 STORY-001 引入 new-domain-service）
- 修改分层（如把 -web-api 拆为 -interfaces）
- 补缺口（如探查中发现某端口未读到，现补齐）
- 修复错误（如发现某包路径与代码不符）
- constraints/ 改动（如 technology-stack 升级）→ 项目资产 §6 同步

### 4.2 5 步 SOP

#### 步骤 1：识别变更类型

| 变更类型 | 影响章节 | 优先级 |
|---------|---------|--------|
| 新增微服务 | §1 元信息 / §2 微服务清单 / §10 缺口 | 🟠 |
| 修改分层（拆/合） | §3 抽象分层映射 / §4 DDD 落点 | 🟠 |
| 新增工程约束（如新依赖） | §6 工程约束 | 🟠 |
| 错误修复（如包路径写错） | 对应章节 + §10 缺口 | 🟡 |
| 补缺口（端口/ServiceProviderConstants 抽全） | §2 / §7 / §10 | 🟡 |
| 命名约定变化 | §5 | 🟢 |

#### 步骤 2：跑对应探查命令

按变更类型跑对应命令（详见 `project-assets-schema.md §7.2` 契约抽取命令清单）：

```bash
# 新增微服务
find . -name "bootstrap*.yml" | xargs grep "spring.application.name"
mvn dependency:tree -pl {new-module} -Dincludes="*:icec-cloud-*-spi"

# 修改分层
find {module}/src/main -type d -name "{new-layer}" | head -5

# 补缺口（端口/服务名）
grep -rn "ServiceProviderConstants" {spi-path}/ --include="*.java"
```

#### 步骤 3：🔴 触发 Proposal（不直接改项目资产）

> **🔴 【2026-06-06 重大重构】** 本步骤内容替换为 Proposal 指针。
> 之前"直接增量更新项目资产"的做法已废弃，统一走 Proposal 流程。

- **触发** [`proposal-skill.md` §第二步(../cross-cutting/proposal-skill.md)，渠道标识 = 4（Project Assets 漂移）
- **Proposal 文档路径**：`design/proposal/{projectKey}-Proposal-{N}-{标题}.md`（按 `document-storage-skill.md §2.6`）
- **走 5 步流程**（proposal-skill.md §第五步）：改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test
- **不在本 SKILL 直接改项目资产**（避免"上游漂移 → 下游 SKILL 各自改"的重复维护）

> 🔴 **关键：** 项目资产是"单一权威源"，**必须通过 Proposal 走完整流程修改**，禁止任何 SKILL（包括本 SKILL）直接改项目资产。

#### 步骤 4：跑双源一致性检查（如果涉及 §6 工程约束）

```bash
# 自动化脚本建议（每月审计时跑）
diff <(grep "^### 6\." {projectKey}.assets.md) <(ls constraints/*.md)
```

检查项：
- §6 引用的 constraints/ 文件名**未缺失**（不出现"6.X 文件不存在"）
- §6 描述与 constraints/ 实际内容**未漂移**（人工 spot check）

#### 步骤 5：写更新日志条目

**强制：** 每次更新都必须在 `{projectKey}.update-log.md` 写一条记录。

格式见 `templates/project-assets/project-assets-update-log-template.md`。

### 4.3 门禁

- 🔴 更新前先在 log 写"待更新"条目（**先记录再修改**，防"修完忘记录"）
- 🔴 修改章节与 log 条目"变动章节"列**一一对应**
- 🔴 §1 `lastAuditedAt` 更新为本次日期
- 🔴 如果涉及 §6，§6 引用与 constraints/ **双源一致**
- 🔴 不在更新时删除缺口项（缺口项要打"✅ 已补 {date}"或保留在原位）

---

## 5. 动作 3：审计（每月例行）

### 5.1 触发场景

- 每月 1 号例行审计
- 大版本变更前（如年度升级）
- 跨项目模板对齐前

### 5.2 6 步 SOP

#### 步骤 1：读更新日志

确认所有"待更新"项已落实；"已补"项已打 ✅。

#### 步骤 2：跑双源一致性脚本

```bash
# 1) §6 引用完整性
grep "^### 6\." {projectKey}.assets.md | awk '{print $2}' | sort -u
# 对比 constraints/ 目录文件名
ls constraints/*.md | xargs -n1 basename

# 2) §6 描述与 constraints/ 实际内容一致性
# 人工 spot check 3-5 条
```

#### 步骤 3：跑缺口进度

| 指标 | 算法 |
|------|------|
| 缺口总数 | §10 表格行数 |
| 已补数 | §10 "✅ 已补 {date}" 行数 |
| 剩余数 | 缺口总数 - 已补数 |
| 进度 | 已补数 / 缺口总数 |

#### 步骤 4：跑"知识衰减"检查

| 衰减类型 | 判定 |
|---------|------|
| 包路径与代码不一致 | grep §4 包路径模板，匹配度 < 80% 视为漂移 |
| 命名约定与代码不一致 | grep §5 命名模板，匹配度 < 70% 视为漂移 |
| 微服务清单与代码不一致 | 对比 §2 与 `find . -name "pom.xml" -path "*/icec-cloud-*"` |

#### 步骤 5：输出审计报告

输出到 `{projectKey}.update-log.md` 末尾"审计报告"章节：

```markdown
## 审计报告 - {YYYY-MM-DD}

| 指标 | 值 | 状态 |
|------|---|------|
| 双源一致性（§6 引用 vs constraints/） | {N}/{total} | ✅/⚠️ |
| 缺口进度 | {已补}/{总数} ({百分比}%) | 🟠/🟡/🟢 |
| 知识衰减（包路径/命名/微服务清单） | {N} 处漂移 | ✅/🔴 |
| 与上次审计的变更数 | {N} 条 | - |
| 建议 | {1-3 条具体建议} | - |
```

#### 步骤 6：更新 §1 lastAuditedAt

- 更新为本次审计日期
- 在 §0 摘要补"上次审计发现"链接到审计报告

### 5.3 门禁

- 🔴 **双源一致性 100%**（§6 引用与 constraints/ 不允许漂移）
- 🟠 **缺口进度有推进**（不允许连续 3 个月缺口数不变）
- 🟠 **知识衰减 < 5 处**（> 5 处视为项目资产已腐化，触发"重新生成"）
- 🔴 **审计报告必须写**到 log 末尾

---

## 6. 动作 4：读取（被其他 SKILL 调用）— 原 ④bis SOP 步骤 1

> **本节是原 `coding-skill.md` ④bis SOP 步骤 1 的迁入位置。** ④bis SOP 步骤 1 改为 1 行指针 → 加载本节。

### 6.1 触发场景

- ④bis CodePlan 编写开始时
- ⑤ Coding 实施前再次校验
- ⑦ CodeReview 对照时

### 6.2 4 步操作

#### 步骤 1：检查项目资产是否存在

```bash
# 目标路径（按 projectKey 定位）
ls "skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md"
```

- 存在 → 步骤 2
- 不存在 → **禁止**继续 ④bis，**先执行本 SKILL §3 生成动作**

#### 步骤 2：读取项目资产核心章节

按 ④bis / ⑤ Coding / ⑦ CodeReview 阶段不同，按需读取：

| 阶段 | 必读章节 | 选读章节 |
|------|---------|---------|
| ④bis CodePlan | §3 分层映射 / §4 DDD 落点 / §5 命名约定 | §6 工程约束 / §7 契约入口 |
| ⑤ Coding | §4 DDD 落点 / §5 命名约定 | §6 工程约束 / §7 契约入口 |
| ⑦ CodeReview | §6 工程约束 / §10 缺口 | §4 DDD 落点 |

#### 步骤 3：确认 §1 lastAuditedAt 在合理范围

- 上次审计 < 30 天 → 资产可信，直接使用
- 上次审计 30-90 天 → 资产**可能过期**，建议跑一次增量更新（§4）
- 上次审计 > 90 天 → 资产**已过期**，**禁止**直接使用，先跑审计（§5）

#### 步骤 4：在 CodePlan 头部写"项目资产已就绪"声明

```markdown
项目资产路径: skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md
项目资产版本: v{N} (lastAuditedAt: {YYYY-MM-DD})
本次引用章节: §3, §4, §5, §6
```

### 6.3 门禁

- 🔴 **缺项目资产 = ④bis 整 Plan 打回**（强制）
- 🔴 **§1 lastAuditedAt > 90 天 = 资产过期，禁止使用**（强制）
- 🟠 **§1 lastAuditedAt > 30 天 = 建议先跑 §4 更新**（推荐）

---

## 7. 异常路径

### A1：项目资产生成中途发现项目结构本身有缺陷

- **触发：** 探查 §3-§6 时发现项目分层违反 `constraints/project-structure.md` 红线
- **动作：** 暂停项目资产生成 → 输出"项目结构缺陷清单"给用户 → 用户确认后再继续
- **记录：** 缺陷清单写入 `{projectKey}.update-log.md` 的"异常条目"章节

### A2：双源一致性审计发现严重漂移

- **触发：** §6 描述与 constraints/ 实际内容偏差 > 50%
- **动作：** 触发"重新生成"动作（§3），而不是增量更新（§4）
- **记录：** 审计报告标注"严重漂移 → 触发重新生成"

### A3：缺口长期未补（连续 3 个月）

- **触发：** 缺口进度连续 3 个月不变
- **动作：** 自动提议降级为"低优先级缺口"或拆分到下个季度
- **记录：** 审计报告标注"长期未补缺口清单 + 降级建议"

### A4：用户手动撤销项目资产

- **触发：** 用户说"删除项目资产"
- **动作：** 备份当前 → 移动到 `project-assets/{projectKey}/.archive/{date}/` → 更新 log
- **禁止：** 直接 `rm`（防误删）

---

## 8. 与其他 SKILL 的衔接

| 上游 SKILL | 衔接点 |
|-----------|--------|
| `ae-sdd-skill.md` | 流程编排定义本 SKILL 在 ④bis / ⑤ / ⑦ 阶段被调用 |
| `coding-skill.md` ④bis SOP | 步骤 1 "读取项目资产" **已迁入本 SKILL §6**（coding-skill 中只保留指针） |
| `code-review-skill.md` | ⑦ CodeReview 阶段按需引用 §6 工程约束做对照 |
| `dr-update-skill.md` / `story-update-skill.md` / `task-generate-skill.md` | 这些 SKILL 改动如果影响项目结构（如新增工程），**必须**联动触发本 SKILL §4 更新 |

---

## 9. 禁止事项

| # | 禁止 | 反例 |
|---|------|------|
| 1 | 禁止"修完项目资产再补日志" | ❌ 直接修改 §4 包路径不写 log |
| 2 | 禁止在更新时大改结构 | ❌ 把 §3 抽象分层从 4 类改成 5 类（应触发"重新生成"） |
| 3 | 禁止在 log 中混用"待更新"/"已补"语义 | ❌ "已补" 行无 ✅ 标记 / "待更新" 行无日期 |
| 4 | 禁止 §6 引用未存在的 constraints 文件 | ❌ "### 6.10 xxx" 但 constraints/ 没有 xxx.md |
| 5 | 禁止跳过 §1 lastAuditedAt 更新 | ❌ 修改了 §4 但 lastAuditedAt 还是上个月的 |
| 6 | 禁止把项目资产作为"通用"模板 | ❌ 把 icec-cloud-boss 项目资产复制到非 boss 类项目用 |
| 7 | 禁止"用 default value 凑数" | ❌ "未读到" 不写计划补齐时间 |

---

## 10. 执行清单（🔴 逐项执行，禁止跳过）

> 用户说"生成项目资产"/"更新项目资产"/"审计项目资产"时，按本表 1:1 映射到 TodoWrite。

### 动作 1：生成

- [ ] 读 CLAUDE.md + AGENTS.md + README（优先级 CLAUDE.md > README）
- [ ] 读 constraints/ 8 个 .md
- [ ] 跑 mvn dependency:tree 列 SPI 依赖
- [ ] 抽典型类（每层 1-2 个）
- [ ] 抽命名约定（7 类各 5 个类名）
- [ ] 抽跨服务契约（§7.2 命令清单）
- [ ] 写 §3 抽象分层 → 项目分层映射
- [ ] 列 §10 缺口（必须非空）
- [ ] 写 §1 lastAuditedAt
- [ ] 写更新日志 initial 条目

### 动作 2：更新

- [ ] 识别变更类型（新增微服务 / 修改分层 / 补缺口 / 修复错误 / 约束变动）
- [ ] 在 log 写"待更新"条目（**先记录再修改**）
- [ ] 跑对应探查命令
- [ ] 增量更新项目资产对应章节
- [ ] 如果涉及 §6，跑双源一致性检查
- [ ] 更新 §1 lastAuditedAt
- [ ] 写 log 条目"已补"状态

### 动作 3：审计

- [ ] 读更新日志，确认"待更新"项已落实
- [ ] 跑双源一致性脚本
- [ ] 跑缺口进度统计
- [ ] 跑知识衰减检查（包路径/命名/微服务清单）
- [ ] 输出审计报告到 log 末尾
- [ ] 更新 §1 lastAuditedAt

### 动作 4：读取（被其他 SKILL 调用）

- [ ] 检查项目资产是否存在
- [ ] 读取项目资产核心章节
- [ ] 确认 §1 lastAuditedAt 在合理范围
- [ ] 在 CodePlan 头部写"项目资产已就绪"声明

---

## 维护

- **维护人：** 架构组 + 各项目 owner
- **更新频率：** 触发"生成/更新/审计"时立即执行
- **同步对象：** ① 与 `project-assets-schema.md` 配套 ② ④bis CodePlan 步骤 1 已迁入本 SKILL §6
- **双源一致性审计：** 每月动作 3 中执行
