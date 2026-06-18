---
name: document-storage
description: 文档存放横切 SKILL — 所有 SKILL 写入文档前必调。提供 ae-sdd-doc/ 统一目录、8 类流程分类、迭代目录、版本号、ChangeLog、关联性分析、gitignore 自动生成、存量迁移能力。当 SKILL 涉及"存放/保存/写入文档"时触发。
---

# Document Storage — 文档存放标准 Skill（AE 体系横切依赖）

> **🔴 核心定位（2026-06-17 重大重构）：** 本 SKILL 是 AE 体系的"**横切依赖**"，**任何 SKILL 在生成/更新文档前都必须先调用本 SKILL** 确定：
> 1. 文档存哪里（统一目录 §2.1：`{工程根}/ae-sdd-doc/`）
> 2. 文档怎么命名（命名 + 版本号 §3）
> 3. 是新建还是修改（重入 SOP §4 + 版本号递增 §3.2）
> 4. 属于哪个迭代（关联性分析 §6）
> 5. 是否需要写入 ChangeLog（§5）
> 6. 是否需要更新 .gitignore（§7）
>
> **触发场景：** 任何流程生成/更新/重入文档时**第一步**调用本 SKILL；不知道放哪/怎么命名/是不是新建/属于哪个迭代时也查本 SKILL。
>
> **🔴 关键变化（2026-06-17）：**
> 1. **统一目录升级为 `ae-sdd-doc/`**（替代零散的 `design/`、`.ae-task/`、`.ae-plan/`）
> 2. **强制 8 类流程目录**（PRD / RA / DR / Story / Task / Coding / Test / CR）
> 3. **强制版本号机制**（每个文档带 v{major}.{minor}，旧版本保留）
> 4. **强制 ChangeLog 独立文件**（`iterations/{date}/{DocType}/ChangeLog/{doc-id}-changelog.md`）
> 5. **强制迭代目录**（`iterations/{YYYY-MM-DD}/` + 业务/逻辑双轨关联性分析）
> 6. **自动 .gitignore 维护**（避免污染 git）
> 7. **存量迁移 API**（`migrate_old_docs()`，默认不执行）
>
> **🔴 兼容策略：** 旧路径（`design/`、`.ae-task/`、`.ae-plan/`、`.spec/iterations/`）保留但标记 deprecated，新文档**强制**使用新路径。

---

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 文档是 AE 体系的"**横切依赖**"——任何其他 SKILL 在"§目标"之后、"§整体流程"之前，必须有"📦 文档存放前置调用"段，并在落地文档前调用本 SKILL 的 API。

### 统一目录结构

```
{工程根}/ae-sdd-doc/
├── PRD/                    # 产品需求文档
├── RA/                     # 需求分析（Requirement Analysis）
├── DR/                     # 设计需求（Design Requirement）
├── Story/                  # Story 文档
├── Task/                   # Task 文档
├── Coding/                 # 编码计划 / 编码报告
├── Test/                   # 测试用例 / 测试报告
├── CR/                     # Code Review 报告
└── iterations/             # 迭代快照（按日期归档）
    └── {YYYY-MM-DD}/
        ├── {DocType}/      # 某次迭代的某类文档
        │   └── {doc-id}-v{major}.{minor}.md
        └── {DocType}/ChangeLog/
            └── {doc-id}-changelog.md
```

### 8 类流程目录对应表

| 目录 | 文档类型 | 路径模板 | 命名规则 |
|------|---------|---------|---------|
| `PRD/` | 产品需求 | `{工程根}/ae-sdd-doc/PRD/{PRD-ID}.md` | 不带版本号（原地更新）|
| `RA/` | 需求分析 | `{工程根}/ae-sdd-doc/RA/{RA-ID}.md` | 不带版本号 |
| `DR/` | 设计需求 | `{工程根}/ae-sdd-doc/DR/{DR-ID}.md` | 不带版本号 |
| `Story/` | Story 文档 | `{工程根}/ae-sdd-doc/Story/{STORY-ID}.md` | 不带版本号 |
| `Task/` | Task 文档 | `{工程根}/ae-sdd-doc/Task/{STORY-ID}/{TASK-ID}.md` | 不带版本号 |
| `Coding/` | CodingPlan + 报告 | `{工程根}/ae-sdd-doc/Coding/{STORY-ID}/{DOC-NAME}.md` | CodingPlan 不带版本号 / 报告带 v{N}-r{M} |
| `Test/` | 测试用例 + 报告 | `{工程根}/ae-sdd-doc/Test/{STORY-ID}/{DOC-NAME}.md` | 用例不带版本号 / 报告带 v{N}-r{M} |
| `CR/` | Code Review 报告 | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{DOC-NAME}.md` | 报告带 v{N}-r{M} |

### 关联性分析（业务 + 逻辑双轨）

| 关联类型 | 子规则 | 命中条件 |
|---------|--------|---------|
| **业务关联** | B1 业务域匹配 | 文档涉及同一业务域（如"用户管理""订单管理"）|
| **业务关联** | B2 业务场景匹配 | 文档涉及同一业务场景（如"用户注册""订单支付"）|
| **业务关联** | B3 业务实体匹配 | 文档涉及同一业务实体（如"User""Order"）|
| **业务关联** | B4 业务规则匹配 | 文档涉及同一业务规则（如"幂等要求""权限校验"）|
| **逻辑关联** | L1 代码调用 | 文档涉及代码间的调用关系（同方法/同接口）|
| **逻辑关联** | L2 数据流 | 文档涉及同一数据流（如同一字段的读写）|
| **逻辑关联** | L3 状态流转 | 文档涉及同一状态机的流转 |
| **逻辑关联** | L4 组件复用 | 文档涉及同一组件/工具类/常量 |

**关联等级判定：**

| 业务 | 逻辑 | 等级 | 默认动作 |
|------|------|------|---------|
| 1 | 1 | **强关联** | 默认入当前迭代 |
| 1 | 0 | **中关联** | 默认入当前迭代 |
| 0 | 1 | **中关联** | 默认入当前迭代 |
| 0 | 0 | **无关联** | 🔴 强制询问用户 |

### 必读 API 清单

| API | 用途 | 调用时机 |
|-----|------|---------|
| `save_doc(doc)` | 保存文档（自动版本号 + ChangeLog + 目录创建）| 文档生成/更新时 |
| `choose_iteration(doc)` | 判定文档属于哪个迭代（业务+逻辑双轨分析）| save_doc 之前 |
| `get_latest_version(doc_id)` | 获取文档最新版本号 | 重入判定时 |
| `get_changelog(doc_id)` | 读取文档历史变更 | Review/对比时 |
| `check_and_update_gitignore()` | 维护 .gitignore（幂等追加 `ae-sdd-doc/`）| 项目初始化时 |
| `migrate_old_docs()` | 存量文档迁移（旧路径 → 新路径）| 用户显式确认后 |

### 与旧路径的兼容策略

| 旧路径 | 状态 | 兼容策略 |
|--------|------|---------|
| `design/` | ⚠️ deprecated | 新文档**不**写入；存量保留可读；migrate_old_docs 一次性迁移 |
| `.ae-task/` | ⚠️ deprecated | 同上 |
| `.ae-plan/` | ⚠️ deprecated | 同上 |
| `.spec/iterations/` | ⚠️ deprecated | 同上 |
| `.auto-engineering/` | ⚠️ deprecated | 状态文件保留此处（不迁移） |
| `ae-sdd-doc/` | ✅ **强制新路径** | 所有新文档必须写入 |

---

## 0. 目标

- AE 体系内所有流程产出文档**有明确路径**（按 8 类流程 + 统一目录）
- 文档**有统一命名规则**（强制版本号 v{major}.{minor}）
- 文档**有独立 ChangeLog**（每次修改都记录）
- 文档**有清晰迭代归属**（业务+逻辑关联性分析）
- 跨流程**有可追溯的引用**（相对路径 + 锚点）
- **重入流程时**有 SOP（版本号递增 + 旧版本保留）
- **不污染 git**（.gitignore 自动维护）

---

## 0.5 工程解耦定位原则（🆕 🔴 2026-06-10 硬约束）

> **原则：** ae-sdd SKILL 家族与工程代码**解耦**——SKILL 家族不知道工程在哪、工程不知道 SKILL 家族在哪。**document-storage-skill 是中间的"目录路由器"**，接收调用方的"意图"（哪个项目、哪个 STORY-ID、哪个微服务、什么任务），输出"文档/代码/产出物"的完整路径。

### 0.5.1 三维定位模型

AE 流程涉及 3 类不同维度的位置，必须分别定位：

| 维度 | 定位依据 | 谁消费 |
|------|---------|--------|
| **项目根** | `assets.md` §1 `gitPath` 字段 | 所有"项目级"操作（统一目录根）|
| **微服务根** | `{gitPath} + "/" + {serviceName}`（拼接约定）| 微服务级文档 |
| **Story 根** | `{项目根}/ae-sdd-doc/Story/{STORY-ID}`（新统一路径）| 重任务 Story/Task/Coding 文档 |

### 0.5.2 动态定位算法

```
调用方输入：{ projectKey, intent, storyId?, serviceName? }
    ↓
document-storage-skill.定位(projectKey, intent)
    ↓
1. 读 {projectKey}.assets.md §1 获取 gitPath
2. 校验 gitPath 存在性（文件系统可达）
3. 根据 intent 选择路径模板：
   ├─ "重任务 STORY-XXX" → 路径前缀 = {gitPath}/ae-sdd-doc/Story/ + 迭代目录
   ├─ "小任务 ServiceX-任务" → 路径前缀 = {gitPath}/ae-sdd-doc/Task/ + 迭代目录
   └─ "微任务 ServiceX-任务" → 路径前缀 = {gitPath}/ae-sdd-doc/Coding/ + 迭代目录
4. 拼接具体文档路径
5. 返回：{完整路径, 文件名, 版本号, ChangeLog 路径, STORING 索引待更新项}
```

### 0.5.3 项目资产依赖

**document-storage-skill 强依赖** `assets.md`（作为"工程根"事实基线）：
- 项目级定位读 §1 `gitPath`
- 微服务级定位读 §2 `microservices[].name`（拼接命名约定）
- 路径规范读 §3-§5（分层映射 / 命名约定 / 包路径）

**调用方传入 `{projectKey}` 时，document-storage-skill 自动定位** `skills/ae-sdd/assets/{projectKey}/{projectKey}.assets.md`

### 0.5.4 硬约束

- ❌ SKILL 中硬编码 `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service` 这样的绝对路径（仅作示例，不作引用）
- ❌ SKILL 调用方传入"工程根"绝对路径（应传 `projectKey` + `serviceName`，由 document-storage-skill 推导）
- ❌ document-storage-skill 中包含 git 命令（不依赖外部工具）
- ❌ 项目资产文件中无 `gitPath` 字段（任何项目必须先建 assets.md 才能用 AE 流程）
- ❌ 新文档写入 `design/`、`.ae-task/`、`.ae-plan/`、`.spec/iterations/` 等旧路径

---

## 0.6 动态定位 API（🆕 2026-06-17 文档化契约）

> 任何 SKILL 落地文档前调用本节 API 获取完整路径+元数据。

### 0.6.1 核心 API：`resolve_path()`

**输入参数：**

| 参数 | 类型 | 必填 | 含义 |
|------|------|------|------|
| `projectKey` | string | ✅ | 项目键（如 `icec-cloud-boss`）|
| `intent` | enum | ✅ | 文档意图（如 `STORY`、`TASK`、`CODING_PLAN`、`CODING_REPORT`）|
| `docType` | string | ✅ | 文档类型（如 `Story`、`CodingReport`、`CodeReview`）|
| `storyId` | string | 条件 | `intent=STORY/TASK/...` 时必填 |
| `serviceName` | string | 条件 | `intent` 含微服务时填 |
| `taskName` | string | 条件 | `intent=TASK_SMALL/PLAN_MICRO` 时必填（事务简称）|
| `version` | object | 条件 | `intent` 含版本号时填（`{major: 1, minor: 0}`）|
| `iterationDate` | string | ❌ | 迭代日期（`YYYY-MM-DD`，不传则自动判定）|

**输出：**

```typescript
interface ResolvedPath {
  fullPath: string;          // 完整路径（含版本号）
  dirPath: string;           // 目录路径（用于 mkdir）
  fileName: string;          // 文件名
  versionSuffix?: string;    // 版本号后缀（如 "-v1.0"）
  changelogPath: string;     // ChangeLog 完整路径
  iterationDir: string;      // 迭代目录
  scope: 'project' | 'service';  // 归属（项目级 / 服务级）
  storingIndexUpdate: {      // STORING.md 索引待更新项
    category: string;        // 8 大分类之一
    docType: string;
    fullPath: string;
  };
}
```

**行为：**
1. 读 `skills/ae-sdd/assets/{projectKey}/{projectKey}.assets.md` §1 获取 `gitPath`
2. 校验 `gitPath` 存在性（文件系统可达）
3. 根据 `intent` 选路径模板（§2 路径模板）
4. 替换所有占位符
5. 调用 `choose_iteration()` 判定迭代归属（如未指定 `iterationDate`）
6. 拼接版本号
7. 返回 ResolvedPath

### 0.6.2 工具 API：`get_git_path()`

**输入：** `projectKey`
**输出：** `string`（项目根绝对路径，从 assets.md 读取）

### 0.6.3 工具 API：`get_service_root()`

**输入：** `projectKey`, `serviceName`
**输出：** `string`（微服务根绝对路径 = `{gitPath}/{serviceName}`）

### 0.6.4 工具 API：`get_constraints()`

> **用途：** 取代 SKILL 内直接写死的 `constraints/` 路径引用。

**输入：** `projectKey`
**输出：** `ConstraintList`（约束文档名称 → 完整路径的映射）

### 0.6.5 工具 API：`get_assets()`

> **用途：** 取代 SKILL 内直接写死的 `assets/{projectKey}/` 路径。

**输入：** `projectKey`
**输出：** `AssetsRef`（项目资产文件的完整路径）

### 0.6.6 工具 API：`update_storing_index()`

**输入：** `projectKey`, `scope`（'project' or 'service'）, `entry`（索引项）
**输出：** `void`

**行为：** 自动更新 `ae-sdd-doc/{DocType}/STORING.md`（项目级）或 `{gitPath}/{serviceName}/.ae-task/Task-xxx/STORING.md`（小任务，兼容旧路径）。

### 0.6.7 工具 API：`save_doc()`

> **用途：** 统一文档保存入口（自动版本号 + ChangeLog + 目录创建 + .gitignore 维护）。

**输入：** `doc`（含 path / content / metadata）
**输出：** `SaveResult`（含 success / newVersion / changelogEntry）

**行为：**
1. 检查 `get_latest_version(doc_id)` 获取当前最新版本
2. 根据 doc.metadata 判定 version 增量
3. 写新版本文件（旧版本保留）
4. 追加 ChangeLog 行
5. 更新 STORING.md 索引
6. 返回 SaveResult

### 0.6.8 工具 API：`choose_iteration()`

> **用途：** 关联性分析，判定文档属于哪个迭代。

**输入：** `doc`（含 business / logic 标签）
**输出：** `IterationChoice`（含 `date` / `strength` / `reasoning`）

**行为：**
1. 扫描现有迭代目录 `ae-sdd-doc/iterations/*/`
2. 对每个现有迭代，调用 `check_business_coherence()` 和 `check_logical_coherence()`
3. 应用 §关联性等级判定 表
4. 返回最强关联的迭代；如无关联 → 强制问用户

### 0.6.9 工具 API：`check_business_coherence()`

**输入：** `doc`, `iteration`
**输出：** `0 | 1`（1 = 业务关联命中）

**判定规则：**
- B1 业务域匹配：doc.businessDomain == iteration.docs[].businessDomain
- B2 业务场景匹配：doc.scenario ∈ iteration.docs[].scenarios
- B3 业务实体匹配：doc.entities ∩ iteration.docs[].entities ≠ ∅
- B4 业务规则匹配：doc.rules ∩ iteration.docs[].rules ≠ ∅

**任一命中返回 1，否则返回 0。**

### 0.6.10 工具 API：`check_logical_coherence()`

**输入：** `doc`, `iteration`
**输出：** `0 | 1`（1 = 逻辑关联命中）

**判定规则：**
- L1 代码调用：doc.calls ∩ iteration.docs[].calls ≠ ∅
- L2 数据流：doc.dataFlow ∩ iteration.docs[].dataFlow ≠ ∅
- L3 状态流转：doc.stateMachine == iteration.docs[].stateMachine
- L4 组件复用：doc.components ∩ iteration.docs[].components ≠ ∅

**任一命中返回 1，否则返回 0。**

### 0.6.11 工具 API：`get_latest_version()`

**输入：** `doc_id`
**输出：** `{major: int, minor: int}`（最新版本号）

**行为：** 扫描 `ae-sdd-doc/iterations/*/{DocType}/{doc_id}-v*.md` 取最大版本。

### 0.6.12 工具 API：`get_changelog()`

**输入：** `doc_id`
**输出：** `string`（ChangeLog 全文）

**行为：** 读取 `ae-sdd-doc/iterations/*/{DocType}/ChangeLog/{doc_id}-changelog.md` 并合并所有迭代版本。

### 0.6.13 工具 API：`check_and_update_gitignore()`

**输入：** `projectKey`（可选，不传则对当前项目）
**输出：** `void`

**行为：**
1. 读取 `{gitPath}/.gitignore`
2. 检查是否已有 `# ae-sdd generated docs\nae-sdd-doc/` 段
3. 如无，幂等追加（保留原有内容）
4. 写回 `.gitignore`

### 0.6.14 工具 API：`migrate_old_docs()`

**输入：** `projectKey`, `mode`（`'dry-run' | 'execute'`）
**输出：** `MigrationReport`

**行为：**
1. 扫描旧路径（`design/`、`.ae-task/`、`.ae-plan/`、`.spec/iterations/`）
2. 判定每个 .md 文件的目标新路径
3. 移动到 `ae-sdd-doc/iterations/{date}/{DocType}/{doc-id}-v1.0.md`
4. 生成 ChangeLog 标记"迁移自旧路径"
5. 默认 `mode='dry-run'`，需用户显式确认执行

### 0.6.15 错误码

| 错误码 | 含义 | 恢复 |
|--------|------|------|
| `E001` | `assets.md` 不存在 | 提示运行 `project-assets-update-skill.md §3 生成动作` |
| `E002` | `gitPath` 字段为空 | 提示检查 assets.md §1 |
| `E003` | `gitPath` 路径不存在 | 提示检查文件系统 |
| `E004` | 微服务名不在 §2 列表 | 提示检查 assets.md §2 |
| `E005` | 业务+逻辑都无关联（0/0）| 🔴 强制询问用户 |
| `E006` | 旧路径写入尝试 | 提示改用新路径 `ae-sdd-doc/` |
| `E007` | 版本号未递增 | 重入时必须 major/minor 至少一个递增 |

---

## 1. 文档分类

| 分类 | 含义 | 8 类流程目录对应 | 例子 |
|------|------|----------------|------|
| **PRD** | 产品需求 | `ae-sdd-doc/PRD/` | 用户管理 PRD |
| **RA** | 需求分析 | `ae-sdd-doc/RA/` | 用户列表查询 RA |
| **DR** | 设计需求 | `ae-sdd-doc/DR/` | Boss用户列表查询接口 DR |
| **Story** | 用户故事 | `ae-sdd-doc/Story/` | STORY-001-BE |
| **Task** | 任务拆分 | `ae-sdd-doc/Task/` | STORY-001-BE Task-1 |
| **Coding** | 编码计划 + 报告 | `ae-sdd-doc/Coding/` | STORY-001-BE-CodingPlan |
| **Test** | 测试用例 + 报告 | `ae-sdd-doc/Test/` | STORY-001-BE-testcase |
| **CR** | Code Review 报告 | `ae-sdd-doc/CR/` | STORY-001-BE-CodeReview-v1-r1 |

**设计文档**（PRD / RA / DR / Story / Task / CodingPlan / TestCase）：原地更新，不带版本号
**事件类报告**（Coding 报告 / Test 报告 / CR 报告）：带 v{N}-r{M} 版本号

---

## 2. 路径模板

### 2.1 统一根目录（🆕 2026-06-17）

> **🔴 强制：** 所有 AE 流程产出的文档**必须**写入 `{工程根}/ae-sdd-doc/` 目录。

**根目录定位：**
- `{工程根}` 由 `assets.md §1 gitPath` 字段决定
- 示例：`d:\Item\icec-cloud-boss\ae-sdd-doc\`

### 2.2 8 类流程目录路径模板

| 文档类型 | 路径模板 | 例子 | 命名 |
|---------|---------|------|------|
| **PRD** | `{工程根}/ae-sdd-doc/PRD/{PRD-ID}.md` | `d:\Item\icec-cloud-boss\ae-sdd-doc\PRD\PRD-001-用户管理.md` | 不带版本号 |
| **RA** | `{工程根}/ae-sdd-doc/RA/{RA-ID}.md` | `d:\Item\icec-cloud-boss\ae-sdd-doc\RA\RA-001-用户列表查询.md` | 不带版本号 |
| **DR** | `{工程根}/ae-sdd-doc/DR/{DR-ID}.md` | `d:\Item\icec-cloud-boss\ae-sdd-doc\DR\DR-2026-06-04-Boss用户列表查询接口.md` | 不带版本号 |
| **Story 文档** | `{工程根}/ae-sdd-doc/Story/{STORY-ID}.md` | `d:\Item\icec-cloud-boss\ae-sdd-doc\Story\STORY-001-BE.md` | 不带版本号 |
| **Task 文档** | `{工程根}/ae-sdd-doc/Task/{STORY-ID}/{TASK-ID}.md` | `d:\Item\icec-cloud-boss\ae-sdd-doc\Task\STORY-001-BE\task-1-BossUserQuery.md` | 不带版本号 |
| **CodingPlan** | `{工程根}/ae-sdd-doc/Coding/{STORY-ID}/{STORY-ID}-CodingPlan.md` | `d:\Item\icec-cloud-boss\ae-sdd-doc\Coding\STORY-001-BE\STORY-001-BE-CodingPlan.md` | 不带版本号 |
| **Coding 报告** | `{工程根}/ae-sdd-doc/Coding/{STORY-ID}/{STORY-ID}-CodingReport-v{N}-r{M}.md` | `...CodingReport-v1-r1.md` | 带 v{N}-r{M} |
| **测试用例** | `{工程根}/ae-sdd-doc/Test/{STORY-ID}/{STORY-ID}-testcase.md` | `d:\Item\icec-cloud-boss\ae-sdd-doc\Test\STORY-001-BE\STORY-001-BE-testcase.md` | 不带版本号 |
| **测试报告** | `{工程根}/ae-sdd-doc/Test/{STORY-ID}/{STORY-ID}-Report-v{N}-r{M}.md` | `...Report-v1-r1.md` | 带 v{N}-r{M} |
| **CR 报告** | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{STORY-ID}-CodeReview-v{N}-r{M}.md` | `...CodeReview-v1-r1.md` | 带 v{N}-r{M} |
| **追溯矩阵** | `{工程根}/ae-sdd-doc/Coding/{STORY-ID}/{STORY-ID}-追溯矩阵-v{N}-r{M}.md` | （独立成文件）| 带 v{N}-r{M} |
| **Story Review** | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{STORY-ID}-StoryReviewReport-r{N}.md` | `...StoryReviewReport-r1.md` | 带 r{N} |
| **Review UpdatePlan** | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{STORY-ID}-StoryReviewUpdatePlan-r{N}.md` | `...UpdatePlan-r1.md` | 带 r{N} |
| **跨轮 Review 对比** | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{STORY-ID}-ReviewCompare-v1-to-v2.md` | `...ReviewCompare-v1-to-v2.md` | 带 v1-to-v2 |
| **Story Supplement** | `{工程根}/ae-sdd-doc/Story/{STORY-ID}/{STORY-ID}-Supplement.md` | 原地累加 | 不带版本号 |

### 2.3 迭代目录结构（🆕 2026-06-17 强制）

> **🔴 强制：** 任何文档落地前必须先调用 `choose_iteration()` 判定属于哪个迭代。

**迭代目录模板：**
```
{工程根}/ae-sdd-doc/iterations/
└── {YYYY-MM-DD}/
    ├── {DocType}/
    │   ├── {doc-id}-v{major}.{minor}.md
    │   ├── {doc-id}-v{major}.{minor}.md
    │   └── ChangeLog/
    │       └── {doc-id}-changelog.md
    └── {DocType}/
        └── ...
```

**完整例子：**
```
d:\Item\icec-cloud-boss\ae-sdd-doc\iterations\
└── 2026-06-17\
    ├── Story\
    │   ├── STORY-001-BE-v1.0.md
    │   ├── STORY-002-BE-v1.0.md
    │   └── ChangeLog\
    │       ├── STORY-001-BE-changelog.md
    │       └── STORY-002-BE-changelog.md
    ├── DR\
    │   ├── DR-2026-06-04-Boss用户列表查询接口-v1.0.md
    │   └── ChangeLog\
    │       └── DR-2026-06-04-Boss用户列表查询接口-changelog.md
    └── CR\
        ├── STORY-001-BE-CodeReview-v1.0-r1.md
        └── ChangeLog\
            └── STORY-001-BE-CodeReview-changelog.md
```

**迭代命名规则：**
- 格式：`{YYYY-MM-DD}`（如 `2026-06-17`）
- 由 `choose_iteration()` API 自动判定（业务+逻辑关联性分析）
- 可由调用方显式传入 `iterationDate` 覆盖

### 2.4 旧路径兼容层（⚠️ deprecated）

> **⚠️ 状态：** 旧路径**保留**（存量可读）但**新文档严禁写入**。新文档必须用 `ae-sdd-doc/`。

| 旧路径 | 状态 | 迁移目标 | 迁移 API |
|--------|------|---------|---------|
| `design/dr/{projectKey}/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/DR/` | `migrate_old_docs()` |
| `design/story/be/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Story/` | `migrate_old_docs()` |
| `design/story/be/task/*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Task/` | `migrate_old_docs()` |
| `design/story/be/coding/*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Coding/` | `migrate_old_docs()` |
| `design/testcase/be/*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Test/` | `migrate_old_docs()` |
| `{工程根}/.ae-task/Task-*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Task/` | `migrate_old_docs()` |
| `{工程根}/.ae-plan/Plan-*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Coding/` | `migrate_old_docs()` |
| `.spec/iterations/*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/{DocType}/` | `migrate_old_docs()` |
| `.auto-engineering/*/state.json` | ✅ 保留 | （状态文件不迁移）| - |

---

## 3. 命名规则

### 3.1 命名模板（统一格式）

```
{doc-id}{-v{major}.{minor} | -v{N}-r{M} | -r{N}}.md
```

**字段说明：**

| 字段 | 是否必填 | 说明 |
|------|---------|------|
| `{doc-id}` | ✅ | 文档唯一标识（如 `STORY-001-BE`、`DR-2026-06-04-Boss用户列表查询接口`）|
| `{v{major}.{minor}}` | 视情况 | 设计类文档每次升级（major 或 minor 递增）|
| `{v{N}-r{M}}` | 视情况 | 事件类报告（Coding 报告 / 测试报告 / CR 报告）|
| `{r{N}}` | 视情况 | Review 报告 / UpdatePlan |

### 3.2 版本号使用规则（🔴 关键：什么时候带/不带）

| 文档类型 | 命名规则 | 例子 | 原因 |
|---------|---------|------|------|
| **设计文档**（PRD / RA / DR / Story / Task / CodingPlan / TestCase）| **带 v{major}.{minor}** | `STORY-001-BE-v1.0.md` → 升级到 `STORY-001-BE-v1.1.md` | 设计文档升级要保留历史，旧版本 v1.0 不删 |
| **事件类报告**（Coding 报告 / 测试报告 / CR 报告）| **带 v{N}-r{M}** | `STORY-001-BE-CodingReport-v1-r1.md` | 报告是"事件"，每轮 Coding 一份 |
| **Writer/Reviewer Report** | **带 r{N}** | `STORY-001-BE-Story-WriterReport-r1.md` | 1 个 Story 1 份，1 份 r1 |
| **Review UpdatePlan** | **带 r{N}** | `STORY-001-BE-StoryReviewUpdatePlan-r1.md` | 1 份/轮 Review |
| **跨轮 Review 对比表** | **带 v1-to-v2** | `STORY-001-BE-ReviewCompare-v1-to-v2.md` | 表示对比的 2 个版本 |
| **项目资产 + 更新日志** | **不带版本号** | `icec-cloud-boss.assets.md` + `icec-cloud-boss.update-log.md` | 资产是"单一权威源" |
| **流程状态文件** | **不带版本号** | `.auto-engineering/{STORY-ID}/state.json` | 状态实时变化 |

### 3.3 版本号含义

- **v{major}.{minor}**（设计文档）：
  - `major`：第 N 次大改（如需求重大调整）
  - `minor`：第 N 次小改（如章节增补、错误修复）
  - 升级规则：major 递增时 minor 归零；minor 递增时 major 不变
- **v{N}-r{M}**（事件类报告）：
  - `v{N}`：Story 版本（第 N 次大改 Story 后，N 递增）
  - `r{M}`：Coding 轮次（第 M 轮 Coding，M 递增）
  - 关系：v 改变时 r 重置为 1

### 3.4 版本号递增 SOP（🔴 2026-06-17 强制）

| 场景 | 动作 | 命名变化 |
|------|------|---------|
| **首次创建** | major=1, minor=0 | `STORY-001-BE-v1.0.md` |
| **小改**（错误修复、章节增补）| minor+1 | `STORY-001-BE-v1.1.md` |
| **大改**（需求重大调整）| major+1, minor=0 | `STORY-001-BE-v2.0.md` |
| **旧版本** | **保留不删** | v1.0 / v1.1 全部保留 |
| **ChangeLog** | **追加新行** | 记录 `v1.0 → v1.1` 的修改项 |

**自动版本号：**
- `save_doc()` API 自动调用 `get_latest_version()` 获取当前最大版本
- 根据 doc.metadata.changeType（`major` / `minor`）递增
- 严禁手动跳过版本号

---

## 4. 重入流程的文档处理（🔴 关键）

> **🔴 重入场景：** 用户说"重新跑 Coding"/"重入 Phase 2 ⑤"/"重做这一轮" 时，哪些文档新建、哪些修改、哪些归档？

### 4.1 重入 SOP（5 步判定）

**步骤 1：识别重入点**

| 重入点 | 触发动作 |
|--------|---------|
| 重入 Story 生成 | Story 文档**新增版本**（v{major}.{minor} 递增）|
| 重入 Task 生成 | Task 文档**新增版本** |
| 重入 Coding | **新增** Coding 报告（r 递增）|
| 重入 TestCase | 测试用例**新增版本** |
| 重入 Test（运行测试）| **新增** 测试报告（r 递增）|
| 重入 CodeReview | **新增** CR 报告（r 递增）|
| 重入 Story Review | **新增** Story Review 报告（r 递增）|

**步骤 2：判定每类文档的动作**

| 文档类型 | 重入时的动作 | 命名 |
|---------|------------|------|
| PRD | 新增 minor 版本 | `PRD-001-v1.0.md` → `PRD-001-v1.1.md` |
| RA | 新增 minor 版本 | 同上 |
| DR | 新增 minor 版本 | 同上 |
| Story | 新增 minor 版本 | `STORY-001-BE-v1.0.md` → `STORY-001-BE-v1.1.md` |
| Task | 新增 minor 版本 | 同上 |
| CodingPlan | 新增 minor 版本 | 同上 |
| 测试用例 | 新增 minor 版本 | 同上 |
| Story Supplement | 原地累加（不删旧的）| 同名 |
| Coding 报告 | **新增**（r 递增）| `STORY-001-BE-CodingReport-v1-r2.md` |
| 测试报告 | **新增**（r 递增）| `STORY-001-BE-Report-v1-r2.md` |
| CR 报告 | **新增**（r 递增）| `STORY-001-BE-CodeReview-v1-r2.md` |
| Story Review 报告 | **新增**（r 递增）| `STORY-001-BE-StoryReviewReport-r2.md` |
| Review UpdatePlan | **新增**（r 递增）| `STORY-001-BE-StoryReviewUpdatePlan-r2.md` |
| 跨轮 Review 对比表 | **新增**（按 v1-to-v2 命名）| `STORY-001-BE-ReviewCompare-v1-to-v2.md` |
| 项目资产 | 原地修改 + 写更新日志 | `icec-cloud-boss.assets.md` + `icec-cloud-boss.update-log.md` |
| 流程状态文件 | 原地修改 | `.auto-engineering/{STORY-ID}/state.json` |

**步骤 3：归档旧版本（如需）**

> **🔴 原则：** 设计文档**永不归档**（版本号保留全部历史），事件类报告 r 递增保留历史，但 ≥ 3 轮的旧版本可归档。

| 轮数 | 处理 |
|------|------|
| v1.0, v1.1, v2.0 | 保留（不归档）|
| v1.0 / v1.1 / ... v4.0+ | 可归档到 `archive/{date}/` 子目录 |
| **归档命令** | `mv ae-sdd-doc/iterations/{date}/Coding/{STORY-001-BE-CodingReport-v1-r4.md} ae-sdd-doc/iterations/{date}/Coding/archive/2026-07-01/` |

**步骤 4：更新流程状态**

修改 `.auto-engineering/{STORY-ID}/state.json` 记录：
- `currentStep` 推进
- `codingRound` 递增
- `currentSubStep` 更新（如 "编码后" / "测试中" / "评审中"）

**步骤 5：检查交叉引用**

- Story 文档引用 CodePlan → CodePlan 重做时 Story 不变
- CodePlan 引用项目资产 → 项目资产更新时 CodePlan 可能需更新
- CR 报告引用 Coding 报告 → Coding 报告重做时新 CR 自动用新 Coding 报告

### 4.2 重入决策树

```
用户说"重入 XX 流程"
    ↓
识别重入点（Story / Task / Coding / Test / CodeReview ...）
    ↓
该文档类型是"原地更新"还是"新增"？
    ├─ 设计类（PRD/RA/DR/Story/Task/CodePlan/测试用例/Supplement）→ 新增版本号（v{major}.{minor} 递增）
    ├─ 事件类（Coding报告/测试报告/CR报告/Review报告/UpdatePlan）→ 新增（r 递增）
    └─ 基础设施类（项目资产/状态文件）→ 原地更新 + 日志记录
    ↓
旧版本保留（不删）
    ↓
ChangeLog 追加新行
    ↓
更新 state.json
    ↓
检查交叉引用
```

---

## 5. ChangeLog 机制（🆕 🔴 2026-06-17 强制）

> **🔴 强制：** 每个文档**必须**有独立 ChangeLog 文件，记录所有修改项。

### 5.1 ChangeLog 位置

```
{工程根}/ae-sdd-doc/iterations/{YYYY-MM-DD}/{DocType}/ChangeLog/{doc-id}-changelog.md
```

**例子：**
```
d:\Item\icec-cloud-boss\ae-sdd-doc\iterations\2026-06-17\Story\ChangeLog\STORY-001-BE-changelog.md
```

### 5.2 ChangeLog 格式

```markdown
# ChangeLog - {doc-id}

| 版本 | 日期 | 修改人 | 修改项 | 改动来源 |
|------|------|--------|--------|---------|
| v1.0 | 2026-06-17 | cong.chen | 首次创建 | ae-sdd-skill Phase 1 ① |
| v1.1 | 2026-06-18 | cong.chen | 修复 AC-005 边界条件错误 | Story Review r1 |
| v1.2 | 2026-06-19 | cong.chen | 补充 ①bis 维度 4 错误码体系 | 用户反馈 |
| v2.0 | 2026-06-20 | cong.chen | 重大需求调整：新增 XX 场景 | DR Update |
```

### 5.3 ChangeLog 字段说明

| 字段 | 说明 |
|------|------|
| **版本** | 与文档版本号对齐（v{major}.{minor} 或 v{N}-r{M}）|
| **日期** | 修改日期（YYYY-MM-DD）|
| **修改人** | 操作者（JavaDoc 作者）|
| **修改项** | 一句话描述修改内容 |
| **改动来源** | 触发修改的来源（Phase / Review r{N} / 用户反馈 / DR Update）|

### 5.4 save_doc() 自动追加

> **🔴 自动行为：** `save_doc()` API 在保存新版本文档时，**自动**追加一行到 ChangeLog，无需手动维护。

```python
# 自动行为示例
save_doc(doc={
    "doc_id": "STORY-001-BE",
    "new_version": "v1.1",
    "change_type": "minor",
    "change_description": "修复 AC-005 边界条件错误",
    "change_source": "Story Review r1"
})
# → 自动追加到 STORY-001-BE-changelog.md
```

### 5.5 ChangeLog 合并（多迭代）

> **场景：** 同一文档可能在多个迭代中修改（iterations/2026-06-17/、iterations/2026-06-18/）。

**`get_changelog()` API 自动合并：**
- 扫描 `ae-sdd-doc/iterations/*/{DocType}/ChangeLog/{doc-id}-changelog.md`
- 按时间排序合并所有变更行
- 返回完整 ChangeLog

---

## 6. 迭代目录 + 关联性分析（🆕 🔴 2026-06-17 核心新增）

> **🔴 强制：** 任何文档落地前必须先调用 `choose_iteration()` 判定属于哪个迭代。

### 6.1 迭代目录命名

- 格式：`{YYYY-MM-DD}`（如 `2026-06-17`）
- **可由调用方显式传入**（如用户说"归到 6 月 17 日那批"）
- **未指定则自动判定**（业务+逻辑双轨关联性分析）

### 6.2 关联性算法（业务关联 0/1 + 逻辑关联 0/1，不加权）

#### 业务关联（B1-B4，任一命中=1）

| 子规则 | 含义 | 例子 |
|--------|------|------|
| **B1 业务域匹配** | 文档涉及同一业务域 | "用户管理" / "订单管理" / "客服系统" |
| **B2 业务场景匹配** | 文档涉及同一业务场景 | "用户注册" / "订单支付" / "工单分配" |
| **B3 业务实体匹配** | 文档涉及同一业务实体 | "User" / "Order" / "Ticket" |
| **B4 业务规则匹配** | 文档涉及同一业务规则 | "幂等要求" / "权限校验" / "状态流转" |

**判定算法：**
```python
def check_business_coherence(doc, iteration):
    for other_doc in iteration.docs:
        if doc.businessDomain == other_doc.businessDomain:  # B1
            return 1
        if doc.scenario in other_doc.scenarios:  # B2
            return 1
        if doc.entities & other_doc.entities:  # B3
            return 1
        if doc.rules & other_doc.rules:  # B4
            return 1
    return 0
```

#### 逻辑关联（L1-L4，任一命中=1）

| 子规则 | 含义 | 例子 |
|--------|------|------|
| **L1 代码调用** | 文档涉及代码间的调用关系 | 同方法 / 同接口 / 同 Feign Client |
| **L2 数据流** | 文档涉及同一数据流 | 同一字段的读写链 / 同一表 |
| **L3 状态流转** | 文档涉及同一状态机的流转 | 同一状态枚举 / 同一 transition |
| **L4 组件复用** | 文档涉及同一组件/工具类/常量 | 同一工具类 / 同一枚举 / 同一常量 |

**判定算法：**
```python
def check_logical_coherence(doc, iteration):
    for other_doc in iteration.docs:
        if doc.calls & other_doc.calls:  # L1
            return 1
        if doc.dataFlow & other_doc.dataFlow:  # L2
            return 1
        if doc.stateMachine == other_doc.stateMachine:  # L3
            return 1
        if doc.components & other_doc.components:  # L4
            return 1
    return 0
```

### 6.3 关联等级判定

| 业务 | 逻辑 | 等级 | 默认动作 |
|------|------|------|---------|
| 1 | 1 | **强关联** | 默认入当前迭代 |
| 1 | 0 | **中关联** | 默认入当前迭代 |
| 0 | 1 | **中关联** | 默认入当前迭代 |
| 0 | 0 | **无关联** | 🔴 **强制询问用户**（E005 错误码）|

### 6.4 choose_iteration() 流程

```
新文档准备落地（save_doc）
    ↓
1. 扫描现有迭代：ae-sdd-doc/iterations/*/
    ↓
2. 对每个现有迭代，调用 check_business_coherence() + check_logical_coherence()
    ↓
3. 应用关联等级判定
    ├─ 强关联 (1,1) → 直接归入
    ├─ 中关联 (1,0) 或 (0,1) → 直接归入
    └─ 无关联 (0,0) → 🔴 强制问用户（E005）
    ↓
4. 用户确认 → 归入
    ↓
5. 用户新建迭代 → 归入新迭代 iterations/{YYYY-MM-DD}/
```

### 6.5 业务/逻辑标签采集

> **调用方需在 save_doc 之前为文档打上业务/逻辑标签：**

```python
doc = {
    "doc_id": "STORY-002-BE",
    "doc_type": "Story",
    "business": {
        "domain": "用户管理",
        "scenario": "用户列表查询",
        "entities": ["User", "Role"],
        "rules": ["权限校验", "分页约束"]
    },
    "logic": {
        "calls": ["BossUserManagementService.list", "BossUserRepository.findByQuery"],
        "dataFlow": ["boss_user.id", "boss_user.status"],
        "stateMachine": None,
        "components": ["PagedModels", "ApiResult"]
    }
}
```

**调用方责任：** 为每个文档正确打标签，关联性分析依赖标签准确性。

---

## 7. .gitignore 自动生成（🆕 🔴 2026-06-17 强制）

> **🔴 强制：** `ae-sdd-doc/` 目录**必须**被 git 忽略（避免污染代码仓库）。

### 7.1 check_and_update_gitignore() 行为

**幂等追加：**
```
# ae-sdd generated docs
ae-sdd-doc/
```

**完整 .gitignore 段：**
```gitignore
# ae-sdd generated docs
ae-sdd-doc/
```

### 7.2 幂等性保证

| 场景 | 行为 |
|------|------|
| 段不存在 | 追加新段 |
| 段已存在 | 不重复追加（幂等）|
| `.gitignore` 不存在 | 创建并写入段 |
| 工程原有其他配置 | 完全保留，不破坏 |

### 7.3 调用时机

| 时机 | 调用方 |
|------|--------|
| **项目初始化**（首次使用 AE 流程）| `project-assets-update-skill.md` 初始化时 |
| **首次 save_doc** | `save_doc()` API 内部自动检查+追加 |
| **手动触发** | 用户显式调用 `check_and_update_gitignore()` |

**🔴 自动行为：** `save_doc()` API 在第一次写入 `ae-sdd-doc/` 时**自动**调用 `check_and_update_gitignore()`。

---

## 8. 存量迁移 API（🆕 2026-06-17 准备就绪）

> **状态：** `migrate_old_docs()` API **已实现**，**默认不执行**，需用户显式确认。

### 8.1 迁移目标

| 旧路径 | 新路径 |
|--------|--------|
| `design/dr/{projectKey}/*.md` | `ae-sdd-doc/iterations/{date}/DR/{doc-id}-v1.0.md` |
| `design/story/be/*.md` | `ae-sdd-doc/iterations/{date}/Story/{doc-id}-v1.0.md` |
| `design/story/be/task/*/*.md` | `ae-sdd-doc/iterations/{date}/Task/{STORY-ID}/{doc-id}-v1.0.md` |
| `design/story/be/coding/*/*.md` | `ae-sdd-doc/iterations/{date}/Coding/{STORY-ID}/{doc-id}-v1.0.md` |
| `design/testcase/be/*/*.md` | `ae-sdd-doc/iterations/{date}/Test/{STORY-ID}/{doc-id}-v1.0.md` |
| `{工程根}/.ae-task/Task-*/*.md` | `ae-sdd-doc/iterations/{date}/Task/{STORY-ID}/{doc-id}-v1.0.md` |
| `{工程根}/.ae-plan/Plan-*/*.md` | `ae-sdd-doc/iterations/{date}/Coding/{STORY-ID}/{doc-id}-v1.0.md` |
| `.spec/iterations/*/*.md` | `ae-sdd-doc/iterations/{date}/{DocType}/{doc-id}-v1.0.md` |

### 8.2 migrate_old_docs() 行为

```python
migrate_old_docs(
    projectKey="icec-cloud-boss",
    mode="dry-run"  # 或 "execute"
)
```

**步骤：**
1. 扫描旧路径下的所有 .md 文件
2. 解析每个文件的 doc_id 和 doc_type
3. 判定每个文件的目标新路径（按 §8.1 映射）
4. **dry-run 模式：** 生成 MigrationReport（不实际移动）
5. **execute 模式：** 移动文件 + 生成 ChangeLog 标记"迁移自旧路径"

### 8.3 MigrationReport 格式

```markdown
# Migration Report - {projectKey} - {YYYY-MM-DD}

## 扫描结果
- design/dr/: 3 个文件
- design/story/be/: 12 个文件
- design/story/be/task/: 8 个文件
- design/story/be/coding/: 15 个文件
- .ae-task/: 0 个文件
- .ae-plan/: 0 个文件
- 合计: 38 个文件

## 迁移计划
| 序号 | 源路径 | 目标路径 | doc_id | doc_type |
|------|--------|---------|--------|---------|
| 1 | design/dr/icec-cloud-boss/DR-001.md | ae-sdd-doc/iterations/2026-06-17/DR/DR-001-v1.0.md | DR-001 | DR |
| 2 | design/story/be/STORY-001-BE.md | ae-sdd-doc/iterations/2026-06-17/Story/STORY-001-BE-v1.0.md | STORY-001-BE | Story |
| ... | ... | ... | ... | ... |

## 注意事项
- 旧目录（design/）将保留（不删除）
- ChangeLog 初始行标记 "迁移自旧路径 design/"
- 需要用户确认后执行
```

### 8.4 默认不执行 + 用户确认

> **🔴 强制：** `migrate_old_docs()` 默认 `mode='dry-run'`，**必须**用户显式确认才执行 `mode='execute'`。

**用户确认模板：**
```
【存量迁移确认】
- 项目: {projectKey}
- 待迁移文件数: {N}
- 目标新路径: ae-sdd-doc/iterations/{date}/{DocType}/
- 旧目录保留: 是（不删除）
- ChangeLog 标记: 迁移自旧路径

请确认：
☐ 同意执行迁移
☐ 暂不执行（dry-run 模式）
```

---

## 9. 文档状态码

> **🔴 与项目资产更新日志 §1 状态码对齐**

| 状态 | 含义 | 何时用 |
|------|------|--------|
| `🆕 initial` | 首次生成 | Story/Task/Coding 报告 第一次创建（v1.0）|
| `⏳ pending` | 准备修改 | log 写好"待更新"条目，文档未改 |
| `✅ done` | 已完成 | 修改/新增已落地 |
| `🔴 blocked` | 卡住 | 评审/修改卡住，需用户决策 |
| `⬇️ downgraded` | 降级 | 长期未更新，自动降级 |
| `🗑️ archived` | 归档 | 旧版本归档到 archive/ |

---

## 10. 文档生命周期

```
生成（v1.0，🆕 initial）
    ↓
使用 / 引用（其他文档引用此文档）
    ↓
小改（v1.1，⏳ pending → ✅ done，ChangeLog 追加）
    ↓
大改（v2.0，⏳ pending → ✅ done，ChangeLog 追加）
    ↓
（循环使用/修改）
    ↓
v ≥ 4 时可归档（🗑️ archived）
    ↓
项目撤销时（罕见）→ 整体删除
```

**关键约束：**
- 🔴 **设计类文档永不删除**（版本号保留全部历史）
- 🔴 **事件类报告 r 递增不删除**（保留所有轮次）
- 🔴 **项目资产永不删除**（修改通过更新日志追踪）
- 🔴 **ChangeLog 永不删除**（追加模式）

---

## 11. 交叉引用规则

| 引用方 | 被引用方 | 引用方式 |
|--------|---------|---------|
| Story 文档 | CodePlan / 项目资产 / Story Review 报告 | 相对路径 + 锚点 |
| CodePlan | Story / Task / 项目资产 / 统一版 CodePlan | 相对路径 |
| Coding 报告 | Story / CodePlan / 测试报告 | 相对路径 |
| CR 报告 | Coding 报告 / 测试报告 / Story / 项目资产 | 相对路径 |
| Story Review UpdatePlan | Story / 历轮 Story Review 报告 | 相对路径 |
| 项目资产 | Story / CodePlan / CR 报告 | 相对路径 |
| ChangeLog | 文档 | 相对路径 |

**🔴 引用必须用相对路径**（不是绝对路径），保证跨机器可读。

**跨迭代引用示例：**
```markdown
参见 [Story Review r1 报告](../2026-06-15/CR/STORY-001-BE-StoryReviewReport-r1.md)
```

---

## 12. 与其他 SKILL 的衔接

| 上下游 SKILL | 衔接点 |
|------------|-------|
| `story-generate-skill.md` | §📦 文档存放前置调用 → save_doc(intent="STORY") |
| `story-review-skill.md` | §📦 文档存放前置调用 → save_doc(intent="STORY_REVIEW") |
| `task-generate-skill.md` | §📦 文档存放前置调用 → save_doc(intent="TASK") |
| `coding-skill.md` | §📦 文档存放前置调用 → save_doc(intent="CODING_REPORT") |
| `coding-report-skill.md` | §1 文档路径（按本 SKILL §2 路径模板）|
| `code-review-skill.md` | §📦 文档存放前置调用 → save_doc(intent="CODE_REVIEW") |
| `testcase-generate-skill.md` | §📦 文档存放前置调用 → save_doc(intent="TESTCASE") |
| `project-assets-update-skill.md` | §1 文档路径（按本 SKILL §2 路径模板）|
| `ae-sdd-skill.md` | 状态文件 `state.json` 按本 SKILL §2.4 路径 |
| `ae-sdd-update-skill.md` | 边界判定表增补 1 行：文档存放标准 = 本 SKILL |

---

## 13. 禁止事项

| # | 禁止 | 危害 | 正确做法 |
|---|------|------|---------|
| 1 | 禁止文档放错位置（不写 `ae-sdd-doc/`）| 找不着 | §2 路径模板 |
| 2 | 禁止事件类报告不带版本号 | 无法区分轮次 | §3.2 命名规则 |
| 3 | 禁止设计类文档不带版本号 | 无法追溯历史 | §3.2 命名规则 |
| 4 | 禁止跨流程文档互引时用绝对路径 | 跨机器失效 | §11 相对路径 |
| 5 | 禁止设计类文档删除历史版本 | 失追溯 | §3.4 + §10 生命周期 |
| 6 | 禁止事件类报告修改历史（必须 r 递增）| 失追溯 | §4.1 重入 SOP |
| 7 | 禁止重入流程时不知道新建还是修改 | 文档乱 | §4 重入 SOP |
| 8 | 禁止不写 ChangeLog | 失变更追踪 | §5 ChangeLog 机制 |
| 9 | 禁止不调用 `choose_iteration()` | 文档漂移 | §6 关联性分析 |
| 10 | 禁止不维护 `.gitignore` | 污染 git | §7 .gitignore 自动生成 |
| 11 | 禁止未经用户确认执行 `migrate_old_docs()` | 误操作 | §8 存量迁移 |
| 12 | 禁止写入旧路径（design/、.ae-task/、.ae-plan/、.spec/iterations/）| 路径混乱 | §2.4 旧路径 deprecated |
| 13 | 禁止业务=0 ∧ 逻辑=0 时不问用户 | 归错迭代 | §6.3 关联等级判定 |
| 14 | 禁止手动跳过版本号 | 历史断裂 | §3.4 版本号递增 SOP |

---

## 14. 执行清单

| # | 动作 | 产出 | 门禁 |
|---|------|--------|------|
| 1 | 识别文档类型 | 8 类之一 | §1 8 类流程目录 |
| 2 | 采集业务/逻辑标签 | doc.business + doc.logic | §6.5 |
| 3 | 调用 `choose_iteration()` 判定迭代 | iterationDir | 强关联/中关联/无关联判定 |
| 4 | 调用 `get_latest_version()` 获取当前版本 | {major, minor} 或 {v, r} | 不超过最大版本 |
| 5 | 计算新版本号 | newVersion | §3.4 递增 SOP |
| 6 | 查 §2 路径模板 | 路径 | 路径合规（必须 `ae-sdd-doc/`）|
| 7 | 查 §3 命名规则 | 名称 | 命名合规（带版本号）|
| 8 | 创建目录（如需）| dirPath | mkdir -p |
| 9 | 写入文档 | 完整内容 | §3.4 旧版本保留 |
| 10 | 追加 ChangeLog | 新行 | §5 格式合规 |
| 11 | 更新 STORING.md | 索引 | §11.6 API |
| 12 | 首次写入时维护 .gitignore | 幂等追加 | §7 |
| 13 | 判定重入时动作 | 新增/修改/归档 | §4 SOP |
| 14 | 更新 state.json（如有）| 状态文件 | 字段齐 |
| 15 | 检查交叉引用 | 引用链接 | 相对路径 |
| 16 | 触发下游 SKILL | 评审/Coding 等 | 已触发 |

---

## 15. 调用入口（🔴 横切依赖标准）

> **🔴 强制：** 任何生成/更新文档的 SKILL 在文档落地前**必须先调用本 SKILL**。本节是"调用入口"，列出"我应该读本 SKILL 的哪些章节"。

### 15.1 SKILL 间调用矩阵

| 调用方 SKILL | 生成的文档类型 | 必读本 SKILL 章节 |
|------------|--------------|------------------|
| `story-generate-skill.md` | Story 文档 | §2.2 路径模板 + §3.1/3.2 命名规则 + §5 ChangeLog + §6 关联性 |
| `task-generate-skill.md` | Task 文档 | §2.2 路径模板 + §3.1/3.2 命名规则 + §5 ChangeLog + §6 关联性 |
| `coding-report-skill.md` | Coding 报告 | §2.2 路径模板 + §3.2 命名规则（事件类带 v{N}-r{M}）+ §4.1 重入步骤（r 递增）+ §5 ChangeLog |
| `code-review-skill.md` | CR 报告 | §2.2 路径模板 + §3.2 命名规则（事件类带 v{N}-r{M}）+ §4.1 重入步骤 + §5 ChangeLog |
| `story-review-skill.md` | Story Review 报告 | §2.2 路径模板 + §3.2 命名规则（带 r{N}）+ §5 ChangeLog |
| `coding-skill.md` | Coding 报告（由 coding-report-skill 生成）| 引用 coding-report-skill.md |
| `testcase-generate-skill.md` | 测试用例 | §2.2 路径模板 + §3.1/3.2 命名规则 + §5 ChangeLog + §6 关联性 |
| `project-assets-update-skill.md` | 项目资产 + 更新日志 | §2.4 路径模板 + §3.2 命名规则（基础设施类不带版本号，更新走日志）+ §7 .gitignore |
| `proposal-skill.md` | Proposal | §2.2 路径模板 + §3.2 命名规则（事件类不带版本号，按 N 编号）+ §5 ChangeLog |

### 15.2 标准调用段（🔴 各 SKILL 必加）

每个生成/更新文档的 SKILL 在 "§目标" 之后、"§整体流程" 之前，必须加以下段（**统一定位**）：

```markdown
## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 的每个输出文档在落地前**必须先调用 [`document-storage-skill.md`](./document-storage-skill.md)** 确定：
> 1. **路径**（§2 路径模板）：本 SKILL 产出的文档存哪里（强制 `ae-sdd-doc/`）
> 2. **命名 + 版本号**（§3 命名规则）：本 SKILL 产出的文档怎么命名（强制带 v{major}.{minor} 或 v{N}-r{M}）
> 3. **ChangeLog**（§5 ChangeLog 机制）：每次修改必须追加 ChangeLog 行
> 4. **关联性分析**（§6 关联性分析）：通过 `choose_iteration()` 判定属于哪个迭代
> 5. **.gitignore**（§7 .gitignore 自动生成）：首次写入时自动维护
>
> **必读调用矩阵：** 参见 `document-storage-skill.md §15.1`（本 SKILL 在矩阵的哪一行）。
>
> **本 SKILL 输出文档与 document-storage-skill §X 的对应关系：**
> - {文档 1} → document-storage-skill §Y.Z（路径）+ §3.2（命名）+ §5（ChangeLog）
> - {文档 2} → document-storage-skill §Y.Z（路径）+ §3.2（命名）+ §5（ChangeLog）
> - ...

### 调用示例（按本 SKILL 实际文档类型填）

| 输出文档 | 路径模板 | 命名规则 | ChangeLog | 重入时动作 |
|---------|---------|---------|----------|----------|
| {文档名 1} | `ae-sdd-doc/iterations/{date}/{DocType}/{doc-id}-v{major}.{minor}.md` | v{major}.{minor} | 必填 | 新增版本 |
| {文档名 2} | `ae-sdd-doc/iterations/{date}/{DocType}/{doc-id}-v{N}-r{M}.md` | v{N}-r{M} | 必填 | r 递增 |
```

### 15.3 调用时机（🔴 必在文档落地前调用）

```
SKILL 流程
    ↓
第零步：准入检查
    ↓
第一步：读取输入（含 §15 调用本 SKILL 查路径/命名/重入/ChangeLog/关联性）
    ↓
第二步：内容生成
    ↓
第三步：合理性自检
    ↓
第四步：写入文档（落地前再次确认路径/命名/版本号/ChangeLog/关联性）← 🔴 必查
    ├─ 调 choose_iteration() 判定迭代
    ├─ 调 get_latest_version() 获取当前版本
    ├─ 调 save_doc() 写入新版本 + 追加 ChangeLog
    └─ 首次写入时调 check_and_update_gitignore()
    ↓
第五步：触发下游
```

### 15.4 不调用本 SKILL 的反模式

| 反模式 | 危害 | 正确做法 |
|--------|------|---------|
| ❌ 文档放错位置（自己起路径）| 找不着 | §15.1 调用矩阵 + §2 路径模板 |
| ❌ 事件类文档不带版本号 | 失轮次追溯 | §3.2 命名规则 |
| ❌ 设计类文档不带版本号 | 失变更追溯 | §3.2 命名规则 |
| ❌ 重入流程时不知道新建还是修改 | 文档漂移 | §4 重入 SOP |
| ❌ 不写 ChangeLog | 失变更追踪 | §5 ChangeLog 机制 |
| ❌ 不调用 choose_iteration() | 文档漂移 | §6 关联性分析 |
| ❌ 不维护 .gitignore | 污染 git | §7 .gitignore 自动生成 |

### 15.5 API 化调用（🆕 2026-06-17 强化版）

> 各 SKILL 落地文档前**不再手写路径**，改为调用 `document-storage-skill` 的 API。

#### 15.5.1 旧 vs 新对比

```python
# 旧方式（硬编码到 design/）
path = f"design/story/be/{story_id}/{story_id}-CodingReport-v{n}-r{m}.md"

# 新方式（动态定位 + 强制新路径）
from document_storage import save_doc, choose_iteration

# 1. 判定迭代
iteration = choose_iteration(doc={
    "doc_id": "STORY-001-BE",
    "doc_type": "CodingReport",
    "business": {"domain": "用户管理", ...},
    "logic": {"calls": [...], ...}
})

# 2. 保存文档（自动版本号 + ChangeLog）
result = save_doc(doc={
    "doc_id": "STORY-001-BE-CodingReport",
    "doc_type": "CodingReport",
    "iteration_date": iteration.date,
    "version": {"v": 1, "r": 1},
    "content": "...",
    "change_type": "new",
    "change_description": "首次创建"
})
# → 自动写入 ae-sdd-doc/iterations/2026-06-17/Coding/STORY-001-BE-CodingReport-v1-r1.md
# → 自动追加 ChangeLog 行
# → 首次写入时自动维护 .gitignore
```

#### 15.5.2 调用矩阵（更新版）

| 调用方 SKILL | 旧：必读章节 | 新：调用 API |
|------------|------------|------------|
| `story-generate-skill.md` | §2.2 + §3.1/3.2 + §5 + §6 | `save_doc(intent="STORY", version={major, minor})` |
| `task-generate-skill.md` | §2.2 + §3.1/3.2 + §5 + §6 | `save_doc(intent="TASK", storyId, taskId, version={major, minor})` |
| `coding-skill.md`（重任务）| §2.2 + §3.2 + §5 | `save_doc(intent="CODING_REPORT", storyId, version={v, r})` |
| `code-review-skill.md` | §2.2 + §3.2 + §5 | `save_doc(intent="CODE_REVIEW", storyId, version={v, r})` |
| `story-review-skill.md` | §2.2 + §3.2 + §5 | `save_doc(intent="STORY_REVIEW", storyId, version={r})` |
| `testcase-generate-skill.md` | §2.2 + §3.1/3.2 + §5 | `save_doc(intent="TESTCASE", storyId, version={major, minor})` |
| `project-assets-update-skill.md` | §2.4 + §3.2 + §7 | `save_doc(intent="ASSETS", ...)` + `check_and_update_gitignore()` |

#### 15.5.3 完整调用流程示例（Story 文档）

```python
# 1. 第零步：读取项目资产获取工程根
git_path = get_git_path(projectKey="icec-cloud-boss")

# 2. 准备 Story 文档
doc = {
    "doc_id": "STORY-002-BE",
    "doc_type": "Story",
    "business": {
        "domain": "用户管理",
        "scenario": "用户列表查询",
        "entities": ["User", "Role"],
        "rules": ["权限校验", "分页约束"]
    },
    "logic": {
        "calls": ["BossUserManagementService.list"],
        "dataFlow": ["boss_user.id"],
        "components": ["PagedModels", "ApiResult"]
    },
    "content": "# STORY-002-BE ..."
}

# 3. 判定迭代
iteration = choose_iteration(doc)
# → 检查现有迭代
# → 找到强关联迭代 (1,1)：2026-06-15 (Story 阶段)
# → 归入

# 4. 获取最新版本
latest = get_latest_version("STORY-002-BE")
# → 无 → v1.0

# 5. 保存文档
result = save_doc(doc={
    **doc,
    "iteration_date": iteration.date,  # "2026-06-15"
    "version": {"major": 1, "minor": 0},
    "change_type": "new",
    "change_description": "首次创建",
    "change_source": "story-generate-skill Phase 1 ①"
})
# → 写入 ae-sdd-doc/iterations/2026-06-15/Story/STORY-002-BE-v1.0.md
# → 追加 ChangeLog
# → 首次写入时维护 .gitignore
```

---

## 16. 维护

- **维护人：** 架构组
- **更新频率：** 每次新增流程或新建文档类型时
- **同步对象：**
  - 所有 SKILL 引用本文件作为"文档存放标准"（统一引用）
  - 与 `ae-sdd-update-skill.md` 协调（边界判定表增补 1 行）
- **关键变化（2026-06-17 重大重构）：**
  - 🆕 统一目录升级为 `ae-sdd-doc/`（替代零散的 `design/`、`.ae-task/`、`.ae-plan/`、`.spec/iterations/`）
  - 🆕 强制 8 类流程目录（PRD / RA / DR / Story / Task / Coding / Test / CR）
  - 🆕 强制版本号机制（每个文档带 v{major}.{minor}，旧版本保留）
  - 🆕 强制 ChangeLog 独立文件（`iterations/{date}/{DocType}/ChangeLog/{doc-id}-changelog.md`）
  - 🆕 强制迭代目录（`iterations/{YYYY-MM-DD}/` + 业务/逻辑双轨关联性分析）
  - 🆕 自动 .gitignore 维护（避免污染 git）
  - 🆕 存量迁移 API（`migrate_old_docs()`，默认不执行）
  - ⚠️ 旧路径（`design/`、`.ae-task/`、`.ae-plan/`、`.spec/iterations/`）保留但标记 deprecated
  - 📝 历史关键变化（2026-06-10 工程解耦定位原则）：保留 §0.5 / §0.6 完整内容
