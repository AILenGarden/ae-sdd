---
name: project-assets-update
description: 项目资产更新 SKILL — 维护 {projectKey}.assets.md，生成 7 层索引（大纲/模块/字段/组件/API/反向/读取 API），支持按需加载和增量更新。当需要"查看/更新项目资产"或新工程初始化时触发。
---

# Project Assets Update — 项目资产目录生成/更新/审计 SKILL

> **本质：** 项目资产是 ④bis CodePlan ⑤ Coding ⑦ CodeReview 阶段的"项目事实基线"。**项目资产是活的** — 新增微服务/修改分层/补缺口都必须增量更新；每月必须做一次双源一致性审计。
>
> **核心难题：** 项目资产 = ae-sdd 体系的"全局上下文"，**所有 SKILL 都需要它**，但全文加载太重。
> **本 SKILL 的解决方案：** 7 层索引（大纲/模块/字段/组件/API/反向/读取 API）让任何 SKILL **像 ES 一样按需读取**，不加载全文也能精准定位。
>
> **本 SKILL 覆盖 3 个动作 + 1 个读取 API 层：**
> 1. **生成** — 新项目启动 / 首次构建项目资产（含索引层一次性生成）
> 2. **更新** — 新增微服务 / 修改分层 / 补缺口（**增量更新索引项**）
> 3. **审计** — 每月一次例行审计（双源一致性 + 缺口进度 + 索引有效性）
> 4. **🆕 索引读取** — 提供 `assets.*()` API，让其他 SKILL 按需拉数据
>
> **关系：**
> - **本 SKILL（怎么做）** = 流程/触发/门禁
> - [`project-assets-schema.md`](../../standards/project-assets/project-assets-schema.md)（**是什么**）= 结构定义/数据模型
> - [`project-assets-template.md`](../../standards/project-assets/project-assets-template.md)（**怎么起步**）= starter 模板
> - [`templates/project-assets/project-assets-update-log-template.md`](../../templates/project-assets/project-assets-update-log-template.md)（**记录什么**）= 更新日志模板
> - **④bis CodePlan SOP 步骤 1 "读取项目资产"** = 已迁入本 SKILL（见 §6）
> - **🆕 索引读取 API** = 见 §G（任何 SKILL 可调用 `assets.module(name)` / `assets.search(keyword)` 等）

---

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 生成/更新/读取的项目资产在写入或访问磁盘前**必须先调用 [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)** 确定：
> 1. **路径**（§2.5 路径模板）：
>    - 主体：`skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md`
>    - 日志：`skills/ae-sdd/project-assets/{projectKey}/{projectKey}.update-log.md`
> 2. **命名**（§3.1/3.2 命名规则）：**基础设施类文档不带版本号**（资产是单一权威源，更新通过日志追踪）
> 3. **重入判定**（§4 重入 SOP）：项目资产**永不修改文件名**（只通过更新日志记录变更）
> 4. **🆕 索引层归属**：索引项**写入资产主体文件**（与 §0-§10 同文件），便于一次性 grep 定位；不另开索引文件（避免双源漂移）

| 输出文档 | 路径模板 | 命名规则 | 重入时动作 |
|---------|---------|---------|----------|
| 项目资产主体（含索引层） | `skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md` | 不带版本号 | 原地修改（变更加日志）|
| 项目资产更新日志 | `skills/ae-sdd/project-assets/{projectKey}/{projectKey}.update-log.md` | 不带版本号 | 原地累加（每变更 1 条）|

> 🔴 **关键：** 项目资产是"单一权威源"，**永不通过文件名识别版本**，所有变更通过 `{projectKey}.update-log.md` 追踪。
> 🆕 **索引层与内容层同文件**：§A-G 索引项与 §0-§10 内容同处一文件，确保"一处改、索引与内容同步"。

---

## 0. 目标

- 让每个项目的代码事实（微服务清单 / DDD 分层 / 命名约定 / 工程约束 / 契约入口）有**单一权威源**
- 让项目资产**随项目演进**而非一次写定
- 让 ④bis CodePlan / ⑤ Coding / ⑦ CodeReview 始终基于**最新事实**
- 与 `constraints/`（规则层）保持**双源一致性**
- 🆕 **让任何 SKILL 都能像 ES 一样按需读取**（不加载全文）

---

## 1. 触发条件

| 触发词 | 动作 |
|--------|------|
| "生成项目资产" / "构建项目资产" / "首次建立项目资产" | → §3 生成（含 §A-G 索引层）|
| "更新项目资产" / "新增微服务" / "修改分层" / "补缺口" | → §4 更新（含索引项增量更新）|
| "审计项目资产" / "双源一致性" / "每月审计" | → §5 审计（含索引有效性检查）|
| "读取项目资产" / "加载项目资产" / "按需加载" | → §6 读取（其他 SKILL 调用）|
| 🆕 `assets.outline()` / `assets.module(name)` / `assets.search(kw)` | → §G 索引读取 API（被其他 SKILL 直接调用）|

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
  → 列 §11 缺口
  → 🆕 提炼团队惯用实现方式（§10 经验文档）
  → 🆕 生成 7 层索引（§A 大纲 / §B 模块 / §C 字段 / §D 组件 / §E API / §F 反向）
  → 审计 + 写 §1 lastAuditedAt
  → 写更新日志（首次生成 = initial entry）

动作 2：更新
  识别变更类型（新增微服务 / 修改分层 / 补缺口 / 修复错误）
  → 跑对应探查命令
  → 增量更新项目资产对应章节
  → 🆕 同步维护对应索引项（§B/§C/§D/§E/§F）
  → 跑双源一致性检查（如果涉及 §6 工程约束）
  → 写更新日志条目（含"哪些索引项已更新"）

动作 3：审计
  读更新日志 → 确认所有"已补"项已落入资产
  → 跑双源一致性脚本（§6 工程约束 vs constraints/）
  → 跑缺口进度（§10 已补项数 / 总缺口数）
  → 🆕 跑索引有效性（§A-G 索引项是否与 §0-§10 内容一致）
  → 更新 §1 lastAuditedAt
  → 输出审计报告

动作 4：读取（被其他 SKILL 调用）
  ① 检查资产存在性
  ② 按阶段选读章节（§6.2 表格）
  ③ 校验 §1 lastAuditedAt 有效期
  ④ 🆕 调用 §G 索引 API 按需加载

🆕 动作 5：索引读取（被其他 SKILL 调用）
  assets.outline() → 拉资产总览
  assets.module(name) → 拉单个模块详情
  assets.table(name) → 拉单表字段清单
  assets.component(name) → 拉公共组件位置
  assets.api(method) → 拉 API 契约
  assets.search(keyword) → 关键词反向定位
```

---

## 3. 动作 1：生成（首次或新项目）

### 3.1 触发场景

- 新项目启动（git 仓库初始化后）
- 老项目首次构建项目资产
- 完全重构导致项目结构巨变

### 3.2 12 步 SOP（详见 `project-assets-schema.md §9`，新增第 10 步生成索引层）

| 步骤 | 动作 | 产物 |
|------|------|------|
| 1 | 读 CLAUDE.md + AGENTS.md + README（优先级 CLAUDE.md > README） | §1 元信息 + §3/§5/§6 初稿 |
| 2 | 读 constraints/ 8 个 .md | §6 工程约束初稿 |
| 3 | 跑 mvn dependency:tree 列 SPI 依赖 | §2 dependsOnSpi |
| 4 | 抽典型类（每层 1-2 个） | §4 典型类名 |
| 5 | 抽命名约定（7 类各 5 个类名） | §5 |
| 6 | 抽跨服务契约（§7.2 命令清单 grep） | §7 |
| 7 | 写 §3 抽象分层 → 项目分层映射 | §3 |
| 8 | 列 §11 缺口 | §11 |
| 9 | 提炼团队惯用实现方式（阅读现有代码 → 交叉验证 → 约束过滤 → 形成经验文档） | §10 经验文档 |
| **10** 🆕 | **生成 7 层索引**（§A 大纲 / §B 模块 / §C 字段 / §D 组件 / §E API / §F 反向） | **§A-§F 索引章节** |
| 11 | 审计 + 写 §1 lastAuditedAt | §1 |
| 12 | 写更新日志 initial 条目 | log |

### 3.3 输出物

- `skills/ae-sdd/project-assets/{project-key}/{project-key}.assets.md`（按 schema 12 节 + §A-§F 索引层填写，含 §10 经验文档）
- `skills/ae-sdd/project-assets/{project-key}/{project-key}.update-log.md`（更新日志，首次条目 = initial）

### 3.4 门禁

- 🔴 12 节**全部**填写（无"待定"项；不知则写"未探查 + 计划探查时间"）
- 🔴 至少填 1 个典型类的 §4 包路径（无典型类 = 项目无代码 = 不可生成）
- 🔴 §10 经验文档**每个分类至少 1 条经验**（9 分类 × ≥1 条 = 最少 9 条）
- 🔴 §10 每条经验**有 ≥2 个真实出处**（文件路径:行号）
- 🔴 §10 每条经验**标注对齐的 constraints/ 条款**（无对齐 = 不入经验文档）
- 🔴 §11 缺口列表**非空**（即使探查全面也要记录"已确认无缺口"）
- 🆕 §A 大纲生成（≤ 2 页，列全 §0-§11 + §A-§G 一级标题）
- 🆕 §B 模块索引表格生成（每行 1 个微服务，列：module/概述/主表/入口/Service/文档位置）
- 🆕 §C 字段索引表格生成（每行 1 个字段，列：表名/字段/类型/业务含义/关联模块）
- 🆕 §D 组件索引表格生成（每行 1 个公共组件，列：组件名/功能/路径/调用方）
- 🆕 §E API 索引表格生成（每行 1 个 API 契约，列：Feign/SPI/方法/入参/出参）
- 🆕 §F 反向索引表格生成（每行 1 个关键词，列：关键词/出现位置）
- 🆕 §G 读取 API 函数模板生成（伪代码 + 自然语言协议）
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
- 🆕 新增表 / 字段（如新建 `boss_user_ext` 表）
- 🆕 新增公共组件（如抽一个 `XxxUtil`）
- 🆕 新增跨服务 API（如新增 SPI 接口）

### 4.2 5 步 SOP

#### 步骤 1：识别变更类型

| 变更类型 | 影响章节 | 影响索引 | 优先级 |
|---------|---------|---------|--------|
| 新增微服务 | §1 元信息 / §2 微服务清单 / §10 缺口 | §B 模块 / §F 反向 | 🟠 |
| 修改分层（拆/合） | §3 抽象分层映射 / §4 DDD 落点 | §B 模块 / §D 组件 | 🟠 |
| 新增工程约束（如新依赖） | §6 工程约束 | - | 🟠 |
| 🆕 新增表 / 字段 | §6.5 数据库规范 | §C 字段 / §F 反向 | 🟠 |
| 🆕 新增公共组件 | §4 DDD 落点 / §6.3 代码风格 | §D 组件 / §F 反向 | 🟡 |
| 🆕 新增跨服务 API | §7 跨服务契约 | §E API / §F 反向 | 🟠 |
| 错误修复（如包路径写错） | 对应章节 + §10 缺口 | 对应索引项同步修正 | 🟡 |
| 补缺口（端口/ServiceProviderConstants 抽全） | §2 / §7 / §10 | §B 模块 / §E API | 🟡 |
| 命名约定变化 | §5 | §F 反向 | 🟢 |

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

# 🆕 新增表/字段
find . -name "*.sql" -not -path "*/target/*" | xargs grep -l "{table_name}"
# 或 SHOW CREATE TABLE {table_name}

# 🆕 新增公共组件
find . -name "{XxxUtil}.java" -not -path "*/target/*"
grep -rn "@Autowired.*{XxxUtil}\|@Resource.*{XxxUtil}" --include="*.java" --include="*.java"

# 🆕 新增跨服务 API
find {new-spi-path}/ -name "*Service.java" -not -path "*/target/*"
```

#### 步骤 3：🔴 触发 Proposal（不直接改项目资产）

> **🔴 【2026-06-06 重大重构】** 本步骤内容替换为 Proposal 指针。
> 之前"直接增量更新项目资产"的做法已废弃，统一走 Proposal 流程。

- **触发** [`proposal-skill.md` §第二步](../cross-cutting/proposal-skill.md)，渠道标识 = 4（Project Assets 漂移）
- **Proposal 文档路径**：`documentStorage.resolve_path(intent="PROPOSAL", storyId={projectKey}, version={N}, title={标题})`（按 `document-storage-skill.md §2.6` 🆕 2026-06-17 修复 P1-3）
- **走 5 步流程**（proposal-skill.md §第五步）：改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test
- **不在本 SKILL 直接改项目资产**（避免"上游漂移 → 下游 SKILL 各自改"的重复维护）

> 🔴 **关键：** 项目资产是"单一权威源"，**必须通过 Proposal 走完整流程修改**，禁止任何 SKILL（包括本 SKILL）直接改项目资产。
> 🆕 **索引层同步：** Proposal 落地时必须**同时更新**对应索引项（§B/§C/§D/§E/§F），否则 §G 读取 API 会返回过期数据。

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
🆕 **日志条目必须列出"哪些索引项已同步"**（如：§B 模块索引新增 1 行 / §C 字段索引新增 3 行）。

### 4.3 门禁

- 🔴 更新前先在 log 写"待更新"条目（**先记录再修改**，防"修完忘记录"）
- 🔴 修改章节与 log 条目"变动章节"列**一一对应**
- 🆕 修改章节与 log 条目"变动索引项"列**一一对应**
- 🔴 §1 `lastAuditedAt` 更新为本次日期
- 🔴 如果涉及 §6，§6 引用与 constraints/ **双源一致**
- 🔴 不在更新时删除缺口项（缺口项要打"✅ 已补 {date}"或保留在原位）

---

## 5. 动作 3：审计（每月例行）

### 5.1 触发场景

- 每月 1 号例行审计
- 大版本变更前（如年度升级）
- 跨项目模板对齐前

### 5.2 7 步 SOP（新增第 4 步索引有效性）

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

#### 步骤 4：🆕 跑索引有效性检查

| 检查项 | 算法 | 失败处理 |
|--------|------|---------|
| §B 模块索引 = §2 微服务清单 | 对比行数（应一致） | 增量补全缺失行 |
| §C 字段索引覆盖所有主表 | §C 行数 ≥ §2 微服务数 × 5（主表平均字段数） | 增量补全 |
| §D 组件索引 = §4 DDD 落点中典型类数 | 对比行数 | 增量补全 |
| §E API 索引 = §7 跨服务契约接口数 | 对比行数 | 增量补全 |
| §F 反向索引关键词 ≥ 20 | 统计 §F 行数 | 增量补充 |
| 索引项位置引用是否过期 | grep 引用的 `§X.Y` 是否在 §0-§10 存在 | 修正过期引用 |

#### 步骤 5：跑"知识衰减"检查

| 衰减类型 | 判定 |
|---------|------|
| 包路径与代码不一致 | grep §4 包路径模板，匹配度 < 80% 视为漂移 |
| 命名约定与代码不一致 | grep §5 命名模板，匹配度 < 70% 视为漂移 |
| 微服务清单与代码不一致 | 对比 §2 与 `find . -name "pom.xml" -path "*/icec-cloud-*"` |
| 🆕 索引项与实际代码不一致 | grep §B/§C/§D 引用的路径/类名，匹配度 < 90% 视为漂移 |

#### 步骤 6：输出审计报告

输出到 `{projectKey}.update-log.md` 末尾"审计报告"章节：

```markdown
## 审计报告 - {YYYY-MM-DD}

| 指标 | 值 | 状态 |
|------|---|------|
| 双源一致性（§6 引用 vs constraints/） | {N}/{total} | ✅/⚠️ |
| 缺口进度 | {已补}/{总数} ({百分比}%) | 🟠/🟡/🟢 |
| 🆕 索引有效性（§A-G 一致性） | {一致项}/{总项} ({百分比}%) | ✅/⚠️ |
| 知识衰减（包路径/命名/微服务清单/索引） | {N} 处漂移 | ✅/🔴 |
| 与上次审计的变更数 | {N} 条 | - |
| 建议 | {1-3 条具体建议} | - |
```

#### 步骤 7：更新 §1 lastAuditedAt

- 更新为本次审计日期
- 在 §0 摘要补"上次审计发现"链接到审计报告

### 5.3 门禁

- 🔴 **双源一致性 100%**（§6 引用与 constraints/ 不允许漂移）
- 🟠 **缺口进度有推进**（不允许连续 3 个月缺口数不变）
- 🟠 **知识衰减 < 5 处**（> 5 处视为项目资产已腐化，触发"重新生成"）
- 🆕 **索引有效性 ≥ 95%**（< 95% 触发"补索引"动作）
- 🔴 **审计报告必须写**到 log 末尾

---

## 6. 动作 4：读取（被其他 SKILL 调用）— 原 ④bis SOP 步骤 1

> **本节是原 `coding-skill.md` ④bis SOP 步骤 1 的迁入位置。** ④bis SOP 步骤 1 改为 1 行指针 → 加载本节。

### 6.1 触发场景

- ④bis CodePlan 编写开始时
- ⑤ Coding 实施前再次校验
- ⑦ CodeReview 对照时
- 🆕 Story Review 上下文加载时
- 🆕 TestCase 编写前事实核对时

### 6.2 4 步操作

#### 步骤 1：检查项目资产是否存在

```bash
# 目标路径（按 projectKey 定位）
ls "skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md"
```

- 存在 → 步骤 2
- 不存在 → **禁止**继续 ④bis，**先执行本 SKILL §3 生成动作**

#### 步骤 2：读取项目资产核心章节

| 阶段 | 必读章节 | 选读章节 | 🆕 索引 API（替代全文加载）|
|------|---------|---------|---------------------------|
| ④bis CodePlan | §3 分层映射 / §4 DDD 落点 / §5 命名约定 | §6 工程约束 / §7 契约入口 | `assets.outline()` 拉大纲 + `assets.module(name)` 按需拉模块 |
| ⑤ Coding | §4 DDD 落点 / §5 命名约定 | §6 工程约束 / §7 契约入口 | `assets.search(keyword)` 关键词反查 |
| ⑦ CodeReview | §6 工程约束 / §10 缺口 | §4 DDD 落点 | `assets.table(name)` 字段对齐核对 |
| 🆕 Story Review | §4 DDD 落点 / §6 工程约束 | §7 契约入口 | `assets.api(method)` 跨服务契约核对 |
| 🆕 TestCase | §6.7 测试规范 / §4 DDD 落点 | §5 命名约定 | `assets.component(name)` 测试工具定位 |

🆕 **优先调用 §G 索引 API**（避免全文加载 ~2000 行），需要详细章节时再回退到全文读取。

#### 步骤 3：确认 §1 lastAuditedAt 在合理范围

- 上次审计 < 30 天 → 资产可信，直接使用
- 上次审计 30-90 天 → 资产**可能过期**，建议跑一次增量更新（§4）
- 上次审计 > 90 天 → 资产**已过期**，**禁止**直接使用，先跑审计（§5）

#### 步骤 4：在 CodePlan 头部写"项目资产已就绪"声明

```markdown
项目资产路径: skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md
项目资产版本: v{N} (lastAuditedAt: {YYYY-MM-DD})
本次引用章节: §3, §4, §5, §6
🆕 索引调用记录: assets.outline() + assets.module("boss-user") + assets.search("AppService")
```

### 6.3 门禁

- 🔴 **缺项目资产 = ④bis 整 Plan 打回**（强制）
- 🔴 **§1 lastAuditedAt > 90 天 = 资产过期，禁止使用**（强制）
- 🟠 **§1 lastAuditedAt > 30 天 = 建议先跑 §4 更新**（推荐）
- 🆕 **未通过 §G 索引 API 按需加载 = 视为"粗暴使用资产"**（推荐改用 API）

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

### A5：🆕 索引项与实际代码严重不一致

- **触发：** §B/§C/§D 引用的路径/类名匹配度 < 50%
- **动作：** 触发"重建索引层"（保留 §0-§10 内容，仅重建 §A-§G）
- **记录：** 审计报告标注"索引漂移严重 → 触发索引重建"

---

## 8. 与其他 SKILL 的衔接

| 上游 SKILL | 衔接点 |
|-----------|--------|
| `ae-sdd-skill.md` | 流程编排定义本 SKILL 在 ④bis / ⑤ / ⑦ 阶段被调用 |
| `coding-skill.md` ④bis SOP | 步骤 1 "读取项目资产" **已迁入本 SKILL §6**（coding-skill 中只保留指针） |
| `code-review-skill.md` | ⑦ CodeReview 阶段按需引用 §6 工程约束做对照 |
| `dr-update-skill.md` / `story-update-skill.md` / `task-generate-skill.md` | 这些 SKILL 改动如果影响项目结构（如新增工程），**必须**联动触发本 SKILL §4 更新 |
| 🆕 `story-review-skill.md` | Story Review 阶段调用 `assets.module(name)` 拉模块详情 |
| 🆕 `testcase-generate-skill.md` | TestCase 阶段调用 `assets.component(name)` 拉测试组件 |
| 🆕 任何 SKILL 需查代码事实 | 调用 `assets.search(keyword)` 反向定位 |

---

## §A 资产大纲生成 SOP（🆕 索引层 1/7）

### A.1 目标

生成 1-2 页**资产总览**，让任何 SKILL 5 秒内了解"这个项目有什么"。

### A.2 触发场景

- 项目资产生成时（第 10 步）
- 审计时（确认 §A 与 §0-§10 一致）

### A.3 输出模板

```markdown
## §A 资产大纲（Outline）

> **调用：** `assets.outline()` — 拉取本总览
> **用途：** 5 秒了解项目范围 + 一级目录速查

### A.1 项目速览

| 维度 | 值 |
|------|---|
| 微服务数 | {N} |
| 主表数 | {N} |
| 公共组件数 | {N} |
| 跨服务 API 数 | {N} |
| 业务域 | {boss / life / ...} |
| 上次审计 | {YYYY-MM-DD} |
| 索引关键词数 | {N} |

### A.2 一级目录速查

| 章节 | 标题 | 一句话说明 |
|------|------|-----------|
| §0 | 摘要与使用场景 | 何时查 / 谁负责 / 与 constraints 关系 |
| §1 | 项目资产元信息 | projectKey / gitPath / 端口段 |
| §2 | 微服务清单 | {N} 个微服务的职责/端口/SPI 依赖 |
| §3 | 抽象分层映射 | 4 层 DDD → 本项目工程模块 |
| §4 | DDD 内部分层落点 | 类角色 → 精确包路径 |
| §5 | 命名约定 | 7 类命名模板 + 反例 |
| §6 | 工程约束 | 8 类 constraints 映射 |
| §7 | 跨服务契约入口 | 11 个 SPI 服务名清单 |
| §8 | Code Plan 输入索引 | CodePlan 章节 → 资产章节引用 |
| §9 | 探查 SOP | 9 步研究流程 |
| §10 | 项目资产缺口 | 待补充项 |
| §11 | 附录 | JSON 实例 |
| 🆕 §A | 资产大纲（本节）| 总览 |
| 🆕 §B | 模块索引 | {N} 行微服务索引 |
| 🆕 §C | 字段索引 | {N} 行数据库字段索引 |
| 🆕 §D | 组件索引 | {N} 行公共组件索引 |
| 🆕 §E | API 索引 | {N} 行跨服务契约索引 |
| 🆕 §F | 关键词反向索引 | {N} 行关键词定位 |
| 🆕 §G | 资产读取 API | 调用协议 |
```

### A.4 门禁

- 🔴 A.1 项目速览 7 字段**全部填写**
- 🔴 A.2 一级目录速查**覆盖 §0-§11 + §A-§G 全部 19 个章节**

---

## §B 模块索引 SOP（🆕 索引层 2/7）

### B.1 目标

按"微服务"维度建立索引，让 ④bis/⑤/⑦ 阶段**1 次 grep 定位 1 个微服务的全部信息**。

### B.2 触发场景

- 项目资产生成时（第 10 步）
- 新增/删除微服务时（§4 更新）
- 任何 SKILL 查"某微服务的入口在哪"时（§G 读取）

### B.3 输出模板

```markdown
## §B 模块索引（Module Index）

> **调用：** `assets.module("{name}")` — 拉取单个模块详情
> **用途：** 1 次定位某个微服务的入口 / 主表 / 关键类 / 文档位置

| module | 概述路径 | 主表 | 入口 Controller | 关键 Service | 文档 |
|--------|---------|------|---------------|-------------|------|
| `icec-cloud-boss-user` | §2 行 50 | `boss_user / boss_role / boss_menu` | `BossUserServiceImpl` (§4) | `BossUserAppService` (§4) | §4 + §6.4 |
| `icec-cloud-life-cs` | §2 行 58 | `cs_ticket / cs_session` | `BossCsTicketServiceImpl` (§4) | `BossCsTicketAppService` (§4) | §4 + §6.4 |
| `icec-cloud-life-im` | §2 行 59 | `im_message / im_session` | `BossImMessageServiceImpl` (§4) | `BossImMessageAppService` (§4) | §4 + §6.4 |
| ... (1 行 / 微服务) |

**调用示例：**
```python
# 拉 icec-cloud-boss-user 模块详情
detail = assets.module("icec-cloud-boss-user")
# 返回: {overview: §2 行 50, mainTables: [...], entryController: "...", keyService: "...", docs: [...]}
```
```

### B.4 门禁

- 🔴 每行 = 1 个微服务（与 §2 微服务清单一一对应）
- 🔴 概述路径、主表、入口、Service、文档**5 列**全部填写
- 🟠 主表列至少列 1 张（如有主子表则列主要 1-3 张）

---

## §C 字段索引 SOP（🆕 索引层 3/7）

### C.1 目标

按"数据库字段"维度建立索引，让任何 SQL 引用 / DR 字段对齐 / 验证性测试**1 次 grep 定位 1 个字段**。

### C.2 触发场景

- 项目资产生成时（第 10 步，扫所有主表 + 关系表）
- 新增/修改表或字段时（§4 更新）
- 验证字段是否在所有关联表中存在时（§G 读取）

### C.3 输出模板

```markdown
## §C 字段索引（Table/Field Index）

> **调用：** `assets.table("{name}")` — 拉取单表所有字段；`assets.table.search("{keyword}")` — 关键词反查
> **用途：** 验证字段存在 / 字段含义核对 / 跨表关联分析

### C.1 主表字段

| 表名 | 字段 | 类型 | 业务含义 | 关联模块 |
|------|------|------|---------|---------|
| `boss_user` | `id` | bigint | 主键 | boss-user |
| `boss_user` | `user_name` | varchar(64) | 用户名 | boss-user |
| `boss_user` | `password` | varchar(128) | BCrypt 哈希密码 | boss-user |
| `boss_user` | `cellphone` | varchar(20) | 手机号（脱敏） | boss-user |
| `boss_user` | `status` | tinyint(1) | 0=禁用 1=启用 | boss-user |
| `boss_user` | `created_by` | bigint | 创建人 | common |
| `boss_user` | `created_date` | datetime | 创建时间 | common |
| `boss_user` | `last_updated_by` | bigint | 最后修改人 | common |
| `boss_user` | `last_updated_date` | datetime | 最后修改时间 | common |
| `boss_user` | `deleted_flag` | tinyint(1) | 逻辑删除标记 | common |
| ... (1 行 / 字段) |

### C.2 关系表字段（仅列关键外键字段）

| 表名 | 字段 | 类型 | 业务含义 | 关联模块 |
|------|------|------|---------|---------|
| `boss_user_role` | `user_id` | bigint | 用户 ID | boss-user |
| `boss_user_role` | `role_id` | bigint | 角色 ID | boss-user |
| `boss_role_menu` | `role_id` | bigint | 角色 ID | boss-user |
| `boss_role_menu` | `menu_id` | bigint | 菜单 ID | boss-user |
| ... |

### C.3 历史表/审计表/事件表（如有）

| 表名 | 字段 | 类型 | 业务含义 | 关联模块 |
|------|------|------|---------|---------|
| `boss_user_log` | `id` | bigint | 主键 | boss-user |
| `boss_user_log` | `user_id` | bigint | 用户 ID | boss-user |
| `boss_user_log` | `action` | varchar(32) | 操作类型 | boss-user |
| `boss_user_log` | `operator_id` | bigint | 操作人 | common |
| ... |

**调用示例：**
```python
# 查 cellphone 字段在哪些表
result = assets.table.search("cellphone")
# 返回: [{table: "boss_user", field: "cellphone", type: "varchar(20)", meaning: "手机号（脱敏）"}, ...]

# 拉 boss_user 表所有字段
fields = assets.table("boss_user")
# 返回: [{field: "id", type: "bigint", meaning: "主键"}, ...]
```
```

### C.4 门禁

- 🔴 主表所有字段**全部录入**（不漏字段；漏字段 = 后续验证盲区）
- 🔴 字段类型**与 DB 实际一致**（审计时跑 `SHOW CREATE TABLE` 对比）
- 🟠 业务含义**简明**（≤ 20 字）
- 🟠 关系表/历史表**优先列外键字段和审计字段**

---

## §D 组件索引 SOP（🆕 索引层 4/7）

### C.1 目标

按"公共组件"维度建立索引，让 CodeReview / Coding 阶段**1 次 grep 找到"现成的工具类"避免重复造轮子**。

### D.2 触发场景

- 项目资产生成时（第 10 步）
- 新增公共组件时（§4 更新）
- 编写新功能前查"有没有现成的"时（§G 读取）

### D.3 输出模板

```markdown
## §D 组件索引（Component Index）

> **调用：** `assets.component("{name}")` — 拉取单个组件详情
> **用途：** 编码前查复用、CodeReview 查"是否自己造轮子"

| 组件名 | 功能 | 路径 | 调用方 |
|--------|------|------|--------|
| `TokenService` | JWT Token 创建/解析/刷新/删除 | `icec-cloud-boss-security/.../service/TokenService.java` | 所有 BFF |
| `@SkipAuth` | 跳过认证注解 | `icec-cloud-boss-security/.../annotation/SkipAuth.java` | 所有 BFF Controller |
| `@RequiresPermissions` | 方法级权限校验 | `icec-cloud-boss-security/.../annotation/RequiresPermissions.java` | 所有 BFF Controller |
| `AccessUserInfoContext` | 请求上下文（**BFF 禁用**）| `boss-common/.../context/AccessUserInfoContext.java` | BFF 层禁用 |
| `ApiResult<T>` | 统一返回包装 | `boss-common/.../result/ApiResult.java` | 所有 Controller |
| `PagedModels<T>` | 分页返回 | `boss-common/.../result/PagedModels.java` | 所有分页接口 |
| `PageRequest<T>` | 分页请求包装 | `boss-common/.../request/PageRequest.java` | 所有分页接口 |
| `JsonUtils` | JSON 序列化 | `com.casstime.commons.utils.JsonUtils` | 所有 Service |
| `DesensitizeUtils.handleCellphone` | 手机号脱敏 | `com.casstime.commons.utils.DesensitizeUtils` | 所有展示层 |
| `BCryptUtil.matches` | 密码校验 | `com.casstime.commons.utils.BCryptUtil` | 登录服务 |
| `MybatisPlusConfig` | MyBatis-Plus 配置 | `icec-cloud-boss-user-infrastructure/.../config/MybatisPlusConfig.java` | 所有 Service |
| `BaseMapper<T>` | MyBatis-Plus 基础 Mapper | `com.baomidou.mybatisplus.core.mapper.BaseMapper` | 所有 Mapper |
| `ApplicationEventPublisher` | 事件发布 | `org.springframework.context.ApplicationEventPublisher` | 跨域事件 |
| `KafkaDomainEventPublisher` | 领域事件发布（MQ） | `icec-cloud-boss-user-infrastructure/.../messaing/publisher/KafkaDomainEventPublisher.java` | boss-user 事件 |
| `RoleOperationLoggable` | 角色操作日志能力 | `icec-cloud-boss-user-bff/.../operationlog/capability/RoleOperationLoggable.java` | boss-user-bff |
| ... |

**调用示例：**
```python
# 查"有没有现成的手机号脱敏工具"
detail = assets.component("DesensitizeUtils")
# 返回: {name, function, path, callers, version, deprecation}
```
```

### D.4 门禁

- 🔴 每个公共组件 4 列**全部填写**
- 🟠 调用方至少列 1 个**真实调用方**（文件路径:行号）
- 🟠 标注**版本/废弃状态**（如 `@Deprecated` 需在"功能"列标"已废弃"）

---

## §E API 索引 SOP（🆕 索引层 5/7）

### E.1 目标

按"跨服务契约"维度建立索引，让 ④bis/Story Review 阶段**1 次 grep 找到"调哪个 SPI、走什么方法、入参出参"**。

### E.2 触发场景

- 项目资产生成时（第 10 步）
- 新增/修改跨服务 API 时（§4 更新）
- Story Review 对照契约时（§G 读取）

### E.3 输出模板

```markdown
## §E API 索引（API Index）

> **调用：** `assets.api("{method}")` — 拉取单个 API 详情
> **用途：** 跨服务契约核对 / DR 接口契约引用 / Story Review 对照

| Feign/SPI | 服务 | 方法 | 入参 | 出参 |
|----------|------|------|------|------|
| `BossUserService` | `boss-user-service` | `getUserById(Long userId)` | `Long userId` | `ApiResult<BossUserDTO>` |
| `BossUserService` | `boss-user-service` | `listUsers(BossUserQueryRequest req)` | `BossUserQueryRequest` | `ApiResult<PagedModels<BossUserDTO>>` |
| `BossUserManagementService` | `boss-user-service` | `createUser(BossUserCreateRequest req)` | `BossUserCreateRequest` | `ApiResult<BossUserDTO>` |
| `BossMenuService` | `boss-user-service` | `listMenus(BossMenuQueryRequest req)` | `BossMenuQueryRequest` | `ApiResult<PagedModels<BossMenuDTO>>` |
| `BossRoleService` | `boss-user-service` | `listRoles(BossRoleQueryRequest req)` | `BossRoleQueryRequest` | `ApiResult<PagedModels<BossRoleDTO>>` |
| `CsTicketService` | `life-cs-service` | `getTicketById(Long ticketId)` | `Long ticketId` | `ApiResult<CsTicketDTO>` |
| `ImMessageService` | `life-im-service` | `sendMessage(ImMessageRequest req)` | `ImMessageRequest` | `ApiResult<ImMessageDTO>` |
| ... |

**调用示例：**
```python
# 查"谁提供 getUserById 接口"
detail = assets.api("getUserById")
# 返回: {spi: "BossUserService", service: "boss-user-service", method: "getUserById(Long)", in: "Long userId", out: "ApiResult<BossUserDTO>", docs: "§7 行 437"}
```
```

### E.4 门禁

- 🔴 每个 Feign 接口的方法**全部录入**（不漏方法）
- 🟠 入参/出参**标注类型 + 字段名**（不只写类型）
- 🟠 服务名用 Nacos 注册名（与 §7.1 ServiceProviderConstants 一致）

---

## §F 关键词反向索引 SOP（🆕 索引层 6/7）

### F.1 目标

按"业务关键词"维度建立反向索引，让任何 SKILL **1 次 grep 定位"项目里所有出现 X 的地方"**。

### F.2 触发场景

- 项目资产生成时（第 10 步）
- 新增业务概念时（§4 更新）
- 验证"项目里有没有 X" / "X 在哪些表/哪些模块出现"时（§G 读取）

### F.3 输出模板

```markdown
## §F 关键词反向索引（Reverse Index）

> **调用：** `assets.search("{keyword}")` — 拉取关键词所有出现位置
> **用途：** 跨章节定位、验证"项目里有没有 X"、业务关联分析

| 关键词 | 出现位置（§X.Y / 文件路径:行号）|
|--------|------------------------------|
| `AppService` | §4 / §5 / §6.3 / §6.10 |
| `@Transactional` | §4 / §6.3 / §6.10 |
| `Facade` | §4 / §6.9 |
| `FeignClient` | §4 / §6.9 / §7 |
| `Converter` | §4 / §5 |
| `BossUser` | §2 / §4 / §5 / §6.4 / §7 |
| `ServiceProviderConstants` | §4 / §7.1 / §7.2 |
| `security_context` | §6.6 / §6.9 |
| `@SkipAuth` | §4 / §6.6 / §6.9 |
| `@RequiresPermissions` | §4 / §6.6 |
| `ApiResult` | §4 / §6.3 / §6.4 |
| `PagedModels` | §4 / §6.4 |
| `PageRequest` | §4 / §6.4 |
| `BCrypt` | §6.6 / §6.10 |
| `deleted_flag` | §6.5 / §C.1 |
| `created_by` | §6.5 / §C.1 |
| `cellphone` | §6.6 / §C.1 |
| `TokenService` | §4 / §6.6 / §6.9 / §D |
| `...` | ... |

**调用示例：**
```python
# 查"项目里所有出现 AppService 的地方"
positions = assets.search("AppService")
# 返回: [{section: "§4", line: "114"}, {section: "§5", line: "196"}, {file: "...", line: 50}, ...]

# 查"项目里有没有 X"（返回空 = 项目里没有 X）
exists = assets.search("MySpecialConcept")
```
```

### F.4 门禁

- 🔴 至少 **20 个关键词**（不足 = 索引粒度太粗）
- 🆕 出现位置必须**精确到 §X.Y 或文件路径:行号**（不写"§4" 这种粗粒度）
- 🟠 关键词覆盖：核心类名（AppService / FeignClient）+ 核心字段名（cellphone / status）+ 核心常量名（ServiceProviderConstants / security_context）

---

## §G 资产读取 API 实现 SOP（🆕 索引层 7/7）

### G.1 目标

把 §A-F 的索引层**封装为可调用的 API**，让其他 SKILL 不需要懂 Markdown 解析也能按需加载。

### G.2 触发场景

- 任何 SKILL 需查项目事实时（**优先调用本 API，而非全文加载**）
- ae-sdd 整体"按需加载"能力落地时

### G.3 API 协议

> **实现方式：** 自然语言协议 + 伪代码。实际执行时由调用 SKILL 解析 Markdown 表格或通过 Grep 工具实现。

| API | 入参 | 返回 | 实现思路 |
|-----|------|------|---------|
| `assets.outline()` | 无 | §A 全部内容 | 读 §A 章节 |
| `assets.module(name)` | `name: str` | 该 module 的 §2 行 + §4 行 + §6.4 错误码 + §7.1 契约 | grep `\| \`{name}\` \|` in §B + 跳转 |
| `assets.table(name)` | `name: str` | 该表的 §C.1 字段清单 | grep `\| \`{name}\` \|` in §C.1 |
| `assets.table.search(keyword)` | `keyword: str` | 所有表含该字段的行 | grep 关键词 in §C.1 + §C.2 + §C.3 |
| `assets.component(name)` | `name: str` | 该组件的 §D 行 | grep `\| \`{name}\` \|` in §D |
| `assets.api(method)` | `method: str` | 该方法的 §E 行 | grep `\| \`{method}(` in §E |
| `assets.search(keyword)` | `keyword: str` | §F 中所有匹配行 | grep `\| \`{keyword}\` \|` in §F |
| `assets.sections(§X.Y)` | `section: str` | 该章节全文 | 读 §X.Y 章节 |
| `assets.last_audited()` | 无 | §1 lastAuditedAt | grep `lastAuditedAt` in §1 |

### G.4 调用示例（伪代码）

```python
# 示例 1：④bis CodePlan 阶段拉模块详情
def plan_module_detail(projectKey, moduleName):
    outline = assets.outline(projectKey)            # 拉 §A
    module = assets.module(projectKey, moduleName)  # 拉 §B 行
    if assets.last_audited(projectKey) < 30 days:  # 校验时效
        return {
            "overview": outline,
            "module": module,
            "tables": assets.table.search(projectKey, "id"),  # 拉该模块主表
            "components": assets.component.search(projectKey, moduleName),  # 拉模块内组件
        }

# 示例 2：Story Review 阶段跨服务契约核对
def review_api_contract(projectKey, methodName):
    api = assets.api(projectKey, methodName)  # 拉 §E 行
    return {
        "spi": api.spi,
        "service": api.service,
        "method": api.method,
        "in": api.in,
        "out": api.out,
        "docs": api.docs,
    }

# 示例 3：CodeReview 阶段验证字段存在
def verify_field_exists(projectKey, tableName, fieldName):
    fields = assets.table(projectKey, tableName)  # 拉 §C 行
    return any(f.field == fieldName for f in fields)
```

### G.5 关联性分析整合

| 关联性类型 | 实现路径 |
|-----------|---------|
| **B1 业务域关联** | `assets.module(name)` 返回的 `module` + `tables` + `components` 共同表达 |
| **B2 业务场景关联** | `assets.search(keyword)` 跨章节定位 |
| **B3 业务实体关联** | `assets.table(name)` + `assets.search(entityName)` |
| **B4 业务规则关联** | `assets.search(ruleName)` + `assets.sections(§6.X)` |
| **L1 代码调用关联** | `assets.component(name).callers` 字段 |
| **L2 数据流关联** | `assets.table(name).relations` 字段（如 boss_user_role.user_id 关联 boss_user.id）|
| **L3 状态流转关联** | `assets.search("status")` + `assets.sections(§4.5)` |
| **L4 组件复用关联** | `assets.component(name).callers` 字段 |

### G.6 门禁

- 🔴 §G.3 API 协议表 9 个 API **全部定义**（缺一不可）
- 🆕 §G.4 调用示例**至少 3 个**（覆盖 ④bis / Story Review / CodeReview 3 个阶段）
- 🆕 §G.5 关联性分析覆盖 B1-B4 + L1-L4 **8 种**

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
| 8 | 🆕 禁止 §A-G 索引项与 §0-§10 内容不一致 | ❌ §B 列 22 个微服务，§2 实际 20 个 |
| 9 | 🆕 禁止 §F 反向索引少于 20 个关键词 | ❌ 只列 5 个关键词 |
| 10 | 🆕 禁止索引项写"§4" 这种粗粒度位置 | ❌ 必须写"§4.5.1"或"文件路径:行号" |
| 11 | 🆕 禁止任何 SKILL 全文加载资产而不优先用 §G API | ❌ ④bis 直接 `Read` 整个 .assets.md |

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
- [ ] 列 §11 缺口（必须非空）
- [ ] 提炼团队惯用实现方式（§10 经验文档）：选取 ≥2 个样本服务 → 逐层扫描 → 交叉验证 → 约束过滤 → 形成 ≥9 条经验
- [ ] 🆕 生成 §A 资产大纲（A.1 项目速览 7 字段 + A.2 一级目录速查 19 章）
- [ ] 🆕 生成 §B 模块索引（每行 1 个微服务，5 列齐全）
- [ ] 🆕 生成 §C 字段索引（主表全字段 + 关系表/历史表关键字段）
- [ ] 🆕 生成 §D 组件索引（每行 1 个公共组件，4 列齐全）
- [ ] 🆕 生成 §E API 索引（每行 1 个跨服务方法，5 列齐全）
- [ ] 🆕 生成 §F 反向索引（≥20 个关键词，位置精确到 §X.Y 或文件:行号）
- [ ] 🆕 生成 §G 读取 API（9 个 API + 3 个调用示例 + 8 种关联性分析）
- [ ] 写 §1 lastAuditedAt
- [ ] 写更新日志 initial 条目

### 动作 2：更新

- [ ] 识别变更类型（新增微服务 / 修改分层 / 补缺口 / 修复错误 / 约束变动 / 🆕 新增表字段 / 新增组件 / 新增 API）
- [ ] 在 log 写"待更新"条目（**先记录再修改**）
- [ ] 跑对应探查命令
- [ ] 增量更新项目资产对应章节
- [ ] 🆕 同步维护对应索引项（§B/§C/§D/§E/§F 视变更类型而定）
- [ ] 🆕 log 条目新增"变动索引项"列
- [ ] 如果涉及 §6，跑双源一致性检查
- [ ] 更新 §1 lastAuditedAt
- [ ] 写 log 条目"已补"状态

### 动作 3：审计

- [ ] 读更新日志，确认"待更新"项已落实
- [ ] 跑双源一致性脚本
- [ ] 跑缺口进度统计
- [ ] 🆕 跑索引有效性检查（§B=§2、§C 覆盖主表、§D=§4、§E=§7、§F≥20）
- [ ] 跑知识衰减检查（包路径/命名/微服务清单/🆕 索引项）
- [ ] 输出审计报告到 log 末尾（🆕 含索引有效性指标）
- [ ] 更新 §1 lastAuditedAt

### 动作 4：读取（被其他 SKILL 调用）

- [ ] 检查项目资产是否存在
- [ ] 读取项目资产核心章节
- [ ] 确认 §1 lastAuditedAt 在合理范围
- [ ] 🆕 优先调用 §G 索引 API 按需加载
- [ ] 在 CodePlan 头部写"项目资产已就绪"声明（🆕 含索引调用记录）

### 动作 5：🆕 索引读取（被其他 SKILL 调用）

- [ ] 调用 `assets.outline()` 拉资产总览
- [ ] 调用 `assets.module(name)` 拉模块详情
- [ ] 调用 `assets.table(name)` 拉表字段
- [ ] 调用 `assets.component(name)` 拉组件位置
- [ ] 调用 `assets.api(method)` 拉 API 契约
- [ ] 调用 `assets.search(keyword)` 反向定位

---

## 维护

- **维护人：** 架构组 + 各项目 owner
- **更新频率：** 触发"生成/更新/审计"时立即执行
- **同步对象：** ① 与 `project-assets-schema.md` 配套 ② ④bis CodePlan 步骤 1 已迁入本 SKILL §6 ③ 🆕 §A-G 索引层为 7 项新内容
- **双源一致性审计：** 每月动作 3 中执行
- **🆕 索引层维护：** §A-G 索引项随 §0-§10 内容同步维护（生成时一次性建立；更新时增量维护；审计时验证一致性）
