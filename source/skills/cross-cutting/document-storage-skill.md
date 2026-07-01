---
name: document-storage
description: 文档存放横切 SKILL — 所有 SKILL 写入文档前必调。提供 ae-sdd-doc/ 统一目录、8 类流程分类、迭代目录、版本号、ChangeLog、关联性分析、gitignore 自动生成、存量迁移能力。当 SKILL 涉及"存放/保存/写入文档"时触发。
---

# Document Storage — 文档存放标准 Skill（AE 体系横切依赖）

> **🔴 核心定位：** 本 SKILL 是 AE 体系的"**横切依赖**"，**任何 SKILL 在生成/更新文档前都必须先调用本 SKILL** 确定：
> 1. 文档存哪里（统一目录 §1.2：`{工程根}/ae-sdd-doc/`）
> 2. 文档怎么命名（命名 + 版本号 §2）
> 3. 是新建还是修改（重入 SOP §5.1）
> 4. 属于哪个迭代（关联性分析 §6.3）
> 5. 是否需要写入 ChangeLog（§6.1）
> 6. 是否需要更新 .gitignore（§7）
>
> **触发场景：** 任何流程生成/更新/重入文档时**第一步**调用本 SKILL；不知道放哪/怎么命名/是不是新建/属于哪个迭代时也查本 SKILL。

> **📋 内容导航（本 SKILL 按以下章节组织，避免重复定义）：**
>
> | 主题 | 章节 |
> |------|------|
> | 8 类流程目录 + 路径模板 | §1 |
> | 命名规则 + 版本号 | §2 |
> | 工程解耦定位（五维模型）| §3 |
> | 动态定位 API 契约（14 个 API 全集 SSOT）| §4 |
> | 重入流程 + 文档生命周期 | §5 |
> | ChangeLog + 关联性分析 SSOT | §6 |
> | .gitignore 自动维护 | §7 |
> | 存量迁移 | §8 |
> | 横切调用规范（调用矩阵）| §9 |

> **🔴 兼容策略：** 旧路径（`design/`、`.ae-task/`、`.ae-plan/`、`.spec/iterations/`）保留但标记 deprecated，新文档**强制**使用新路径。详见 §1.6。

---

## 0. 目标

- AE 体系内所有流程产出文档**有明确路径**（按 8 类流程 + 统一目录）
- 文档**有统一命名规则**（事件类报告强制版本号 v{N}-r{M}）
- 文档**有 ChangeLog**（每次修改都记录，同级目录）
- 文档**有清晰迭代归属**（业务+逻辑关联性分析）
- 跨流程**有可追溯的引用**（相对路径 + 锚点）
- **重入流程时**有 SOP（事件类报告 r 递增）
- **不污染 git**（.gitignore 自动维护）

---

## 1. 文档分类与目录结构

### 1.1 文档分类（8 类）

| 分类 | 含义 | 目录 | 例子 |
|------|------|------|------|
| **PRD** | 产品需求 | `ae-sdd-doc/PRD/` | 用户管理 PRD |
| **RA** | 需求分析 | `ae-sdd-doc/RA/` | 用户列表查询 RA |
| **DR** | 设计需求 | `ae-sdd-doc/DR/` | Boss用户列表查询接口 DR |
| **Story** | 用户故事 | `ae-sdd-doc/Story/` | STORY-001-BE |
| **Task** | 任务拆分 | `ae-sdd-doc/Task/` | STORY-001-BE Task-1 |
| **Coding** | 编码计划 + 报告 | `ae-sdd-doc/Coding/` | STORY-001-BE-CodingPlan |
| **Test** | 测试用例 + 报告 | `ae-sdd-doc/Test/` | STORY-001-BE-testcase |
| **CR** | Code Review 报告 | `ae-sdd-doc/CR/` | STORY-001-BE-CodeReview-v1-r1 |

**两类文档的版本策略（🔴 SSOT，与代码 `_PATH_TEMPLATES` 对齐）：**

| 类别 | 包含 | 版本策略 |
|------|------|---------|
| **设计类文档** | PRD / RA / DR / Story / Task / CodingPlan / TestCase / Proposal | **原地更新，不带版本号**，历史追溯靠 ChangeLog（§6.1）|
| **事件类报告** | Coding 报告 / Test 报告 / CR 报告 / Story Review 报告 / 追溯矩阵 | **带 v{N}-r{M} 或 r{N} 版本号**，每轮新增、保留全部历史 |

### 1.2 统一根目录

> **🔴 强制：** 所有 AE 流程产出的文档**必须**写入 `{工程根}/ae-sdd-doc/` 目录。

**根目录定位：**
- `{工程根}` 由 `assets.md §1 gitPath` 字段决定
- 设计类文档路径基于 `docWorkspacePath`（§3.1 第四维，缺省=gitPath）
- 示例：`d:\Item\icec-cloud-boss\ae-sdd-doc\`

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
├── STORING.md              # 项目级文档索引（单一文件，§4.4 自动维护）
└── iterations/             # 迭代快照（事件类报告按日期归档）
    └── {YYYY-MM-DD}/
        └── {DocType}/
```

### 1.3 路径模板总表

> **🔴 SSOT：** 下表合并了原路径模板表与命名规则。**设计类不带版本号（原地更新），事件类带版本号（保留历史）**——与 `document_storage.py` `_PATH_TEMPLATES` 一致。

| 文档类型 | 路径模板 | 例子 | 版本策略 |
|---------|---------|------|---------|
| **PRD** | `{工程根}/ae-sdd-doc/PRD/{PRD-ID}.md` | `PRD-001-用户管理.md` | 不带版本号 |
| **RA** | `{工程根}/ae-sdd-doc/RA/{RA-ID}.md` | `RA-001-用户列表查询.md` | 不带版本号 |
| **DR** | `{工程根}/ae-sdd-doc/DR/{DR-ID}.md` | `DR-2026-06-04-Boss用户列表查询接口.md` | 不带版本号 |
| **Story 文档** | `{工程根}/ae-sdd-doc/Story/{STORY-ID}.md` | `STORY-001-BE.md` | 不带版本号 |
| **Story Supplement** | `{工程根}/ae-sdd-doc/Story/{STORY-ID}/{STORY-ID}-Supplement.md` | 原地累加 | 不带版本号 |
| **Task 文档** | `{工程根}/ae-sdd-doc/Task/{STORY-ID}/{TASK-ID}.md` | `task-1-BossUserQuery.md` | 不带版本号 |
| **CodingPlan** | `{工程根}/ae-sdd-doc/Coding/{STORY-ID}/{STORY-ID}-CodingPlan.md` | `STORY-001-BE-CodingPlan.md` | 不带版本号 |
| **Coding 报告** | `{工程根}/ae-sdd-doc/Coding/{STORY-ID}/{STORY-ID}-CodingReport-v{N}-r{M}.md` | `...CodingReport-v1-r1.md` | 带 v{N}-r{M} |
| **测试用例** | `{工程根}/ae-sdd-doc/Test/{STORY-ID}/{STORY-ID}-testcase.md` | `STORY-001-BE-testcase.md` | 不带版本号 |
| **测试报告** | `{工程根}/ae-sdd-doc/Test/{STORY-ID}/{STORY-ID}-Report-v{N}-r{M}.md` | `...Report-v1-r1.md` | 带 v{N}-r{M} |
| **CR 报告** | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{STORY-ID}-CodeReview-v{N}-r{M}.md` | `...CodeReview-v1-r1.md` | 带 v{N}-r{M} |
| **追溯矩阵** | `{工程根}/ae-sdd-doc/Coding/{STORY-ID}/{STORY-ID}-追溯矩阵-v{N}-r{M}.md` | （独立成文件）| 带 v{N}-r{M} |
| **Story Review** | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{STORY-ID}-StoryReviewReport-r{N}.md` | `...StoryReviewReport-r1.md` | 带 r{N} |
| **Review Proposal** | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{STORY-ID}-Proposal-{N}.md` | `...Proposal-1.md` | 按 N 编号 |
| **跨轮 Review 对比** | `{工程根}/ae-sdd-doc/CR/{STORY-ID}/{STORY-ID}-ReviewCompare-v1-to-v2.md` | `...ReviewCompare-v1-to-v2.md` | 带 v1-to-v2 |
| **项目资产** | 见 §1.4 | `icec-cloud-boss.assets.md` | 不带版本号 |

### 1.4 资产类路径模板（🔴 资产路径 SSOT）

> **核心设计：** 资产是"项目事实"（代码结构、命名、约束），应落在**项目工作区**（`{docWorkspacePath}/.ae-sdd/assets/`），不是技能包目录。一个工程一个文档，多业务线按 line 分组，再加一份工作区级索引。

**根目录定位：**
- `{docWorkspacePath}` 由 `assets.md §1 docWorkspacePath` 字段决定（缺省=gitPath）
- 资产根：`{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/`

| 资产要素 | 路径模板 | 命名 |
|---------|---------|------|
| **工作区级索引**（总览，1 份）| `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{workspaceKey}.assets.md` | 不带版本号 |
| **工程级子文件**（多业务线）| `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{line}/{工程名}/{工程名}.assets.md` | 不带版本号 |
| **工程级子文件**（单业务线，扁平）| `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{工程名}/{工程名}.assets.md` | 不带版本号 |
| **工作区级日志** | `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{workspaceKey}.update-log.md` | 原地累加 |
| **工程级日志** | `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/[{line}/]{工程名}/{工程名}.update-log.md` | 原地累加 |
| **待确认问题清单** | `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{workspaceKey}.pending-questions.md` | 原地维护 |

**life 项目实例化（多业务线示范，🔴 life 强制用此结构）：**

```
D:\Item\life\.ae-sdd\assets\life\                          ← workspaceKey = life
├── life.assets.md                                          ← 工作区级索引
├── life.update-log.md
├── 2c\                                                     ← line = 2c（21 工程）
│   ├── icec-cloud-life-cs\icec-cloud-life-cs.assets.md
│   ├── icec-cloud-life-im\icec-cloud-life-im.assets.md
│   └── ...
├── admin\                                                  ← line = admin（15 工程）
│   ├── icec-cloud-boss-user\icec-cloud-boss-user.assets.md
│   └── ...
└── common\                                                 ← line = common（公共线）
    ├── boss-common\boss-common.assets.md
    └── ...
```

**单业务线项目（扁平结构，向后兼容）：**

```
{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/
├── {workspaceKey}.assets.md            ← 索引
├── {工程名}\{工程名}.assets.md          ← 工程级子文件（无 line 层）
└── ...
```

**分组建目录的判定规则：**
- 项目含 2+ 业务线（如 life 的 2c+admin+common）→ **推荐**按 line 分组（life 实例**强制**）
- 单业务线项目 → 扁平结构，无需 line 层
- ae-sdd **不预设** line 取值，由项目工程结构决定；代码 `paths.discover_line_groups()` 自动适配

> ⚠️ **兼容旧位置：** 历史 v4.0 曾支持 `{docWorkspacePath}/assets/{key}/{module}/` 单层 + `.ae-sdd/assets/{key}.*.assets.md` 扁平，这两种 `paths.find_module_asset_files()` 仍自动发现（三路共存），不强制迁移。

### 1.5 迭代目录结构

> **🔴 强制：** 事件类报告落地前必须先调用 `choose_iteration()` 判定属于哪个迭代。

**迭代目录模板：**
```
{工程根}/ae-sdd-doc/iterations/
└── {YYYY-MM-DD}/
    └── {DocType}/
        ├── {doc-id}-v{N}-r{M}.md        ← 事件类报告（带版本号）
        └── ChangeLog/
            └── {doc-id}-changelog.md
```

> **注：** 迭代目录主要承载**事件类报告**（Coding 报告 / Test 报告 / CR 报告等）。设计类文档原地更新，不进迭代目录。

**完整例子（事件类报告）：**
```
d:\Item\icec-cloud-boss\ae-sdd-doc\iterations\
└── 2026-06-17\
    ├── Coding\
    │   ├── STORY-001-BE-CodingReport-v1-r1.md
    │   └── ChangeLog\
    │       └── STORY-001-BE-CodingReport-changelog.md
    └── CR\
        ├── STORY-001-BE-CodeReview-v1-r1.md
        └── ChangeLog\
            └── STORY-001-BE-CodeReview-changelog.md
```

**迭代命名规则：**
- 格式：`{YYYY-MM-DD}`（如 `2026-06-17`）
- 由 `choose_iteration()` API 自动判定（业务+逻辑关联性分析 §6.3）
- 可由调用方显式传入 `iterationDate` 覆盖

### 1.6 旧路径兼容层（⚠️ deprecated）

> **⚠️ 状态：** 旧路径**保留**（存量可读）但**新文档严禁写入**。新文档必须用 `ae-sdd-doc/`。

| 旧路径 | 状态 | 迁移目标 | 迁移 API |
|--------|------|---------|---------|
| `design/dr/{projectKey}/*.md` | ⚠️ deprecated | `ae-sdd-doc/DR/` 或 `iterations/{date}/DR/` | `migrate_old_docs()` |
| `design/story/be/*.md` | ⚠️ deprecated | `ae-sdd-doc/Story/` | `migrate_old_docs()` |
| `design/story/be/task/*/*.md` | ⚠️ deprecated | `ae-sdd-doc/Task/` | `migrate_old_docs()` |
| `design/story/be/coding/*/*.md` | ⚠️ deprecated | `ae-sdd-doc/Coding/` | `migrate_old_docs()` |
| `design/testcase/be/*/*.md` | ⚠️ deprecated | `ae-sdd-doc/Test/` | `migrate_old_docs()` |
| `{工程根}/.ae-task/Task-*/*.md` | ⚠️ deprecated | `ae-sdd-doc/Task/` | `migrate_old_docs()` |
| `{工程根}/.ae-plan/Plan-*/*.md` | ⚠️ deprecated | `ae-sdd-doc/Coding/` | `migrate_old_docs()` |
| `.spec/iterations/*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/{DocType}/` | `migrate_old_docs()` |
| `.auto-engineering/*/state.json` | ✅ 保留 | （状态文件不迁移）| - |

---

## 2. 命名与版本号规则

### 2.1 命名模板（统一格式）

```
{doc-id}{-v{N}-r{M} | -r{N}}.md
```

> **注：** 设计类文档命名仅 `{doc-id}.md`（不带版本号后缀），见 §1.1 两类文档版本策略。

**字段说明：**

| 字段 | 是否必填 | 说明 |
|------|---------|------|
| `{doc-id}` | ✅ | 文档唯一标识（如 `STORY-001-BE`、`DR-2026-06-04-Boss用户列表查询接口`）|
| `{v{N}-r{M}}` | 事件类报告必填 | 事件类报告（Coding 报告 / 测试报告 / CR 报告 / 追溯矩阵）|
| `{r{N}}` | Review 报告用 | Story Review 报告 |
| `{N}` | Proposal 用 | Review Proposal（1 份/轮）|

### 2.2 版本号使用规则（🔴 SSOT，与代码一致）

| 文档类型 | 命名规则 | 例子 | 原因 |
|---------|---------|------|------|
| **设计类文档**（PRD / RA / DR / Story / Task / CodingPlan / TestCase / Proposal）| **不带版本号**，原地更新 | `STORY-001-BE.md` | 原地更新，历史追溯靠 ChangeLog（§6.1）|
| **事件类报告**（Coding 报告 / 测试报告 / CR 报告 / 追溯矩阵）| **带 v{N}-r{M}** | `STORY-001-BE-CodingReport-v1-r1.md` | 报告是"事件"，每轮 Coding 一份，保留全部历史 |
| **Story Review 报告** | **带 r{N}** | `STORY-001-BE-StoryReviewReport-r1.md` | 1 个 Story 1 份，1 份 r1 |
| **Review Proposal** | **带 N** | `STORY-001-BE-Proposal-1.md` | 1 份/轮 Review |
| **跨轮 Review 对比表** | **带 v1-to-v2** | `STORY-001-BE-ReviewCompare-v1-to-v2.md` | 表示对比的 2 个版本 |
| **项目资产 + 更新日志** | **不带版本号** | `icec-cloud-boss.assets.md` + `icec-cloud-boss.update-log.md` | 资产是"单一权威源" |
| **流程状态文件** | **不带版本号** | `.auto-engineering/{STORY-ID}/state.json` | 状态实时变化 |
| **流程状态文件（PRD 级）**| **不带版本号** | `.auto-engineering/{PRD-ID}/state.json` | 状态实时变化；Story 完成 hook 写入 |
| **流程状态文件（PRD 级，handoff）**| **不带版本号** | `.auto-engineering/{PRD-ID}/summary.md` | `mavis session rotate --handoff-file` 时生成 |
| **流程状态文件（PRD 级，人类读）**| **不带版本号** | `.auto-engineering/{PRD-ID}/state.md` | `ae-sdd state prd-complete` 时一次性生成 |

> **🔴 PRD ID 命名规范（与 `SKILL.md §1.2` SSOT）：** 格式 `PRD-<业务域>-<序号>`（CS / IM / USER / LIFE + 3 位数字）。示例：`PRD-CS-001`。

> **📌 PRD 级 state.json schema 见附录 A。**

### 2.3 版本号含义

- **v{N}-r{M}**（事件类报告）：
  - `v{N}`：Story 版本（第 N 次大改 Story 后，N 递增）
  - `r{M}`：Coding 轮次（第 M 轮 Coding，M 递增）
  - 关系：v 改变时 r 重置为 1
- **r{N}**（Review 报告）：Review 轮次（第 N 轮）

### 2.4 版本号递增 SOP（🔴 限定事件类报告）

| 场景 | 动作 | 命名变化 |
|------|------|---------|
| **首次创建**（事件类报告）| v=1, r=1 | `STORY-001-BE-CodingReport-v1-r1.md` |
| **重入 Coding/Test/CR**（同 Story 版本）| r+1 | `STORY-001-BE-CodingReport-v1-r2.md` |
| **Story 大改后重入** | v+1, r=1 | `STORY-001-BE-CodingReport-v2-r1.md` |
| **旧版本** | **保留不删** | v1-r1 / v1-r2 全部保留 |
| **ChangeLog** | **追加新行** | 记录 `v1-r1 → v1-r2` 的修改项 |

> **设计类文档（PRD/RA/DR/Story/Task/CodingPlan/TestCase）无版本号递增**：原地更新，每次修改追加 ChangeLog 行追溯历史。

**自动版本号：**
- `save_doc()` API 自动调用 `get_latest_version()` 获取当前最大版本（仅事件类报告）
- 严禁手动跳过版本号

---

## 3. 工程解耦定位原则（🆕 🔴 2026-06-10 硬约束）

> **原则：** ae-sdd SKILL 家族与工程代码**解耦**——SKILL 家族不知道工程在哪、工程不知道 SKILL 家族在哪。**document-storage-skill 是中间的"目录路由器"**，接收调用方的"意图"（哪个项目、哪个 STORY-ID、哪个微服务、什么任务），输出"文档/代码/产出物"的完整路径。

### 3.1 五维定位模型（🆕 v4.1 四维→五维，新增"业务线根"；原 v3.4.0 为四维）

> **🆕 v4.1（2026-06-27，路径治理修订）：** v3.4.0 四维模型（项目根/微服务根/Story根/文档工作区根）解决了"工程目录≠文档目录"，但**资产**侧仍把"工程级子文件"定位甩给了 schema/代码，导致路径规则三处各写一套、改一处漂移两处（"路径偏移"的制度性根因）。本次新增第五维「业务线根」，并把**资产路径的单一权威源**收回本 SKILL。
>
> **历史：** v3.4.0（2026-06-25，建议书2）从三维升级为四维，新增"文档工作区根"。

AE 流程涉及 5 类不同维度的位置，必须分别定位：

| 维度 | 定位依据 | 谁消费 |
|------|---------|--------|
| **项目根** | `assets.md` §1 `gitPath` 字段 | 所有"项目级"操作（代码、构建、跑测试）|
| **微服务根** | `{gitPath} + "/" + {serviceName}`（拼接约定）| 微服务级文档 |
| **Story 根** | `{文档工作区根}/ae-sdd-doc/Story/{STORY-ID}`（新统一路径）| 重任务 Story/Task/Coding 文档 |
| **文档工作区根** | `assets.md` §1 `docWorkspacePath` 字段（可选，缺省=gitPath）| 工程目录与文档目录分离的项目（如 life）—— 设计类文档路径基于此维度 |
| 🆕 **业务线根** | `{docWorkspacePath}/assets/{workspaceKey}/{line}/` | 多业务线项目（如 life=2c/admin/common）—— 工程级资产子文件按业务线分组就近存放 |

**🆕 v4.1 业务线根（line）说明：**
- 仅"多业务线项目"启用（单业务线项目资产仍用扁平结构，无此维）
- line 取值由项目实际工程结构决定（ae-sdd **不预设**取值；life 实例 = `2c` / `admin` / `common`）
- 代码层 `paths.discover_line_groups()` 自动区分"module 目录"与"line 分组目录"，无需手填
- 详见 §3.3 资产依赖 + §1.4 资产类路径模板

**向后兼容：** `docWorkspacePath` 缺省时（assets.md §1 未填）回退到 `gitPath`，旧项目行为不变。仅当 assets.md §1 显式声明 `docWorkspacePath` 时，设计类文档路径基于该值。

### 3.2 动态定位算法

```
调用方输入：{ projectKey, intent, storyId?, serviceName? }
    ↓
document-storage-skill.定位(projectKey, intent)
    ↓
1. 读 {projectKey}.assets.md §1 获取 gitPath（+ 🆕 docWorkspacePath，缺省=gitPath）
2. 校验 gitPath 存在性（文件系统可达）← 🆕 v3.4.0 落地前强制触发 E003（非仅事后 health）
3. 根据 intent 选择路径模板 + 路径根：
   ├─ 设计类文档（DR/Story/Task/CodingPlan/测试用例/报告）→ 路径根 = docWorkspacePath（🆕）
   │   ├─ "重任务 STORY-XXX" → {docWorkspacePath}/ae-sdd-doc/Story/{STORY-ID}/ + 迭代目录
   │   ├─ "小任务 ServiceX-任务" → {docWorkspacePath}/ae-sdd-doc/Task/{事务简称}/
   │   └─ "微任务 ServiceX-任务" → {docWorkspacePath}/ae-sdd-doc/Coding/{事务简称}/
   └─ 工程类操作（代码、构建）→ 路径根 = gitPath
4. 拼接具体文档路径
5. 返回：{完整路径, 文件名, 版本号, ChangeLog 路径, STORING 索引待更新项}
```

### 3.3 项目资产依赖（🔴 资产路径单一权威源 — 🆕 v4.1）

> **🆕 v4.1（2026-06-27，SKILL 边界修复）：** 本节是**资产路径的唯一权威源（SSOT）**。`project-assets-schema.md` 只定义资产"装什么"（内容结构），`project-assets-update-skill.md` 只定义"怎么生成"（流程），**两者均不重复定义存放路径**——路径模板见本节 + §1.4。

**document-storage-skill 强依赖** `assets.md`（作为"工程根"事实基线）：
- 项目级定位读 §1 `gitPath`（工程目录，代码/构建根）
- 🆕 v3.4.0 文档工作区根读 §1 `docWorkspacePath`（可选，缺省=gitPath；设计类文档路径基于此）
- 微服务级定位读 §2 `microservices[].name`（拼接命名约定）
- 路径规范读 §3-§5（分层映射 / 命名约定 / 包路径）

**资产三要素定位（🆕 v4.1）：**

| 资产要素 | 路径 | 说明 |
|---------|------|------|
| **工作区级索引**（总览）| `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{workspaceKey}.assets.md` | 一份，聚合工程清单 + 跨工程依赖 + 指向下层 |
| **工程级子文件**（多业务线，🆕 v4.1）| `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{line}/{工程名}/{工程名}.assets.md` | 一个工程一个文档，按业务线分组 |
| **工程级子文件**（单业务线）| `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{工程名}/{工程名}.assets.md` | 无业务线分组时的扁平结构 |
| **工作区级日志** | `{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{workspaceKey}.update-log.md` | 累加 |
| **工程级日志** | 同目录下 `{工程名}.update-log.md` | 各工程独立 |

**业务线（line）自动发现规则（🆕 v4.1）：**
- 代码层 `paths.find_module_asset_files()` 经 `discover_line_groups()` 自动区分三种结构并共存发现：
  ① `{workspaceKey}/{line}/{工程}/`（多业务线，优先）② `{workspaceKey}/{工程}/`（单层）③ 旧扁平 `{workspaceKey}.{工程}.assets.md`
- ae-sdd **不预设** line 取值，由项目工程结构决定（life = `2c`/`admin`/`common`，见 §1.4 示范）
- 单业务线项目资产无 line 层，仍可正常工作（向后兼容）

> ⚠️ **本节取代旧表述：** 历史版本曾写"调用方传 projectKey 自动定位 `skills/ae-sdd/assets/{projectKey}/{projectKey}.assets.md`"——该表述把资产钉在技能包目录且无 line 维度，已废弃。资产应落在**项目工作区**（`{docWorkspacePath}/.ae-sdd/assets/`），不是技能包目录。详见 §1.4。

### 3.4 硬约束

- ❌ SKILL 中硬编码 `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service` 这样的绝对路径（仅作示例，不作引用）
- ❌ SKILL 调用方传入"工程根"绝对路径（应传 `projectKey` + `serviceName`，由 document-storage-skill 推导）
- ❌ document-storage-skill 中包含 git 命令（不依赖外部工具）
- ❌ 项目资产文件中无 `gitPath` 字段（任何项目必须先建 assets.md 才能用 AE 流程）
- ❌ 新文档写入 `design/`、`.ae-task/`、`.ae-plan/`、`.spec/iterations/` 等旧路径

---

## 4. 动态定位 API 契约（🆕 唯一 SSOT）

> 任何 SKILL 落地文档前调用本节 API 获取完整路径+元数据。**本节是 14 个 API 的唯一定义源**；§9 调用矩阵只做 SKILL→API 映射，不重复定义。

### 🆕 4.0 CLI 入口（v3.7.2 激活，推荐 LLM 使用）

> **本节 API 已封装为 `ae-sdd doc` CLI 子命令**（v3.7.2，2026-07-01）。LLM 和用户**优先通过 CLI 调用**，而非读 Python 函数签名手模拟。

| CLI 命令 | 封装的 API | 用途 |
|---------|-----------|------|
| `ae-sdd doc save --intent X --content-file F` | `save_doc()` | 一步到位存文档（resolve+写+版本+ChangeLog+STORING+gitignore+删草稿）|
| `ae-sdd doc resolve --intent X` | `resolve_path()` | 只推路径不写（查会写到哪）|
| `ae-sdd doc finalize --path P --intent X` | `finalize_doc()` | 已手写文件补版本号/ChangeLog/STORING（不覆盖内容）|

**完整 save 命令：**
```bash
ae-sdd doc save \
  --intent {INTENT} \           # 必填，取自 §4.10 intent 枚举表
  --story-id {STORY-ID} \        # intent=STORY/TASK/CODING 等时填
  --doc-id {DOC-ID} \            # raId/prdId/drId/taskId 等归一到此
  --content-file .ae-sdd/tmp/{doc-id}-draft.md \  # 必填，存完自动删除
  --version "v1-r1" \            # 事件类报告填（r 自动自增）；设计类不填
  --changelog-note "修改说明" \   # 可选，追加到 ChangeLog
  --keep-draft                   # 可选，保留草稿（默认删除）
```

> 下方 §4.1-§4.11 是 Python 函数签名的**代码层契约**（供 document_storage.py 实现对齐），LLM/用户实际调用走 CLI。

### 4.1 核心 API：`resolve_path()`

**输入参数：**

| 参数 | 类型 | 必填 | 含义 |
|------|------|------|------|
| `projectKey` | string | ✅ | 项目键（如 `icec-cloud-boss`）|
| `intent` | enum | ✅ | 文档意图（取自 §4.10 intent 枚举表）|
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
  changelogPath: string;     // ChangeLog 完整路径（文档同级目录）
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
1. 读工作区级索引资产 §1 获取 `gitPath`（+ `docWorkspacePath`，缺省=gitPath）—— 资产路径见 §1.4（`{docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/{workspaceKey}.assets.md`）
2. 校验 `gitPath` 存在性（文件系统可达）
3. 根据 `intent` 选路径模板（§1.3 路径模板总表）
4. 替换所有占位符
5. 调用 `choose_iteration()` 判定迭代归属（如未指定 `iterationDate`）
6. 拼接版本号
7. 返回 ResolvedPath

### 4.2 工具 API（定位原语）

| API | 输入 | 输出 | 用途 |
|-----|------|------|------|
| `get_git_path()` | `projectKey` | `string`（项目根绝对路径）| 从 assets.md §1 读取 gitPath |
| `get_service_root()` | `projectKey`, `serviceName` | `string`（微服务根 = `{gitPath}/{serviceName}`）| 微服务级定位 |
| `get_constraints()` | `projectKey` | `ConstraintList`（约束文档名 → 完整路径映射）| 取代 SKILL 内写死的 `constraints/` 路径。定位：`{gitPath}/constraints/` 或 `docWorkspace/constraints/`（二者其一）|
| `get_assets()` | `projectKey` | `AssetsRef`（项目资产文件路径列表）| 取代 SKILL 内写死的 `assets/{projectKey}/` 路径。复用 `paths.find_module_asset_files`（v4.1 支持 line 分组发现）|

### 4.3 统一保存 API：`save_doc()`

> **用途：** 统一文档保存入口（自动版本号 + ChangeLog + 目录创建 + .gitignore 维护）。

**输入：** `doc`（含 path / content / metadata）
**输出：** `SaveResult`（含 success / newVersion / changelogEntry）

**行为：**
1. resolve_path 推导路径
2. 🆕 RA 类型强检查（intent=RA 时调用 `check_ra_prerequisites`）— BUG / CONFIG intent 双重豁免
3. 若带版本号且未显式传 version → get_latest_version 自增（E007：重入必须递增）
4. 写文件（旧版本保留）
5. 追加 ChangeLog 行（文档同级目录，仅当传 changelog_note 时）
6. 首次写入 `ae-sdd-doc/` 时调用 `check_and_update_gitignore()`（§7.3）
7. 返回 SaveResult

### 4.4 索引维护 API：`update_storing_index()`

**输入：** `projectKey`, `scope`（'project' or 'service'）, `entry`（索引项）
**输出：** `void`

**行为：** 自动更新**单一** `ae-sdd-doc/STORING.md`（项目级）。幂等：同 fullPath 不重复追加。

> **注：** 小任务旧路径 `{gitPath}/{serviceName}/.ae-task/Task-xxx/STORING.md` 分支为后续兼容增强项，当前实现统一写项目级单一索引。

### 4.5 关联性 API

| API | 输入 | 输出 | 用途 |
|-----|------|------|------|
| `choose_iteration()` | `doc`（含 business / logic 标签）| `IterationChoice`（date / strength / reasoning）| 关联性分析，判定文档属于哪个迭代。判定规则见 §6.3 |
| `check_business_coherence()` | `doc`, `iteration` | `0 \| 1`（1 = 业务关联命中）| 业务关联判定。B1-B4 规则见 §6.3 |
| `check_logical_coherence()` | `doc`, `iteration` | `0 \| 1`（1 = 逻辑关联命中）| 逻辑关联判定。L1-L4 规则见 §6.3 |

> **判定规则的权威定义在 §6.3**，本节仅列 API 签名。

### 4.6 版本查询 API：`get_latest_version()`

**输入：** `doc_id`
**输出：** `{major: int, minor: int}`（最新版本号）

**行为：** 扫描文档目录下 `{doc-id}-v*.*.md` 取最大版本。

### 4.7 ChangeLog 读取 API：`get_changelog()`

**输入：** `changelog_path`
**输出：** `string`（ChangeLog 内容）

**行为：** 读取文档同级目录的 changelog 文件，返回内容（行列表）。不存在返回空。

### 4.8 .gitignore 维护 API：`check_and_update_gitignore()`

**输入：** `projectKey`（可选，不传则对当前项目）, `pattern`
**输出：** `bool`（是否新增）

**行为：**
1. 读取 `{gitPath}/.gitignore`
2. 检查是否已有 `# ae-sdd generated docs\nae-sdd-doc/` 段
3. 如无，幂等追加（保留原有内容）
4. 写回 `.gitignore`

### 4.9 存量迁移 API：`migrate_old_docs()`

**输入：** `projectKey`, `mode`（`'dry-run' | 'execute'`）
**输出：** `MigrationReport`

**行为：**
1. 扫描旧路径（`design/`、`.ae-task/`、`.ae-plan/`、`.spec/`）
2. 判定每个 .md 文件的目标新路径（按 §1.6 映射）
3. 移动到新路径
4. 生成 ChangeLog 标记"迁移自旧路径"
5. 默认 `mode='dry-run'`，需用户显式确认执行

### 4.10 intent 枚举表（🔴 save_doc / resolve_path 的 intent 参数必须取自此表）

> **SSOT：** 所有 SKILL 的 `save_doc(intent=...)` / `resolve_path(intent=...)` 调用，**intent 值必须是下表中已登记的枚举**。如需新增 intent，先在此表补行，再在调用方 SKILL 的 save_doc 矩阵中引用。
>
> **实现状态：** ✅ = 已在 `document_storage.py _PATH_TEMPLATES` 实现；📝 = 文档登记但代码未实现（属后续待办）。

| intent 值 | 文档类型 | 产出 SKILL | 命名规则 | 实现 |
|-----------|---------|-----------|---------|------|
| `PRD` | 产品需求文档 | requirement-analysis-skill | 不带版本号（原地更新）| ✅ |
| `ISSUE` | Issue 文档 | requirement-analysis-skill | 不带版本号（原地更新）| 📝 |
| `STORY` | Story 主文档 | story-generate-skill | 不带版本号（原地更新）| ✅ |
| `STORY_SUPPLEMENT` | Story 补充说明 | story-generate-skill | 不带版本号（原地累加）| 📝 |
| `DR` | DR 主文档 | dr-generate-skill | 不带版本号（原地更新）| ✅ |
| `DR_SUPPLEMENT` | DR 补充说明 | dr-update-skill | 不带版本号（原地累加）| 📝 |
| `STORY_REVIEW` | Story Review 报告 | story-review-skill | 带 r{N} | ✅ |
| `TESTCASE` | 测试用例文档 | testcase-generate-skill | 不带版本号（原地更新）| ✅ |
| `TESTCASE_COMPLIANCE_REPORT` | 测试用例合规性校验报告 | testcase-generate-skill | 带 r{N} | 📝 |
| `TESTCASE_REVIEW` | TestCase Review 报告 | testcase-review-skill 🆕 v3.7.0 | 带 r{N} | 📝 |
| `TASK` | Task 文档（含 Task-0）| task-generate-skill | 不带版本号（原地更新）| ✅ |
| `TASK_SUPPLEMENT` | Task 补充说明 | task-generate-skill | 不带版本号（原地累加）| 📝 |
| `TASK_WRITER_REPORT` | Task 撰写报告 | task-generate-skill | 带 r{N} | 📝 |
| `TASK_REVIEW` | Task Review 报告 | task-generate-skill | 带 r{N} | 📝 |
| `TASK_IMPL_PLAN` | Task 实现方案 | task-generate-skill | 不带版本号（原地覆盖）| 📝 |
| `CODING_PLAN` | 统一版 CodingPlan | task-generate-skill | 不带版本号（原地更新）| ✅ |
| `CODING_REPORT` | Coding 报告 | coding-report-skill | 带 v{N}-r{M} | ✅ |
| `TEST_REPORT` | 测试报告 | test-generate-skill / test-review-skill | 带 v{N}-r{M} | ✅ |
| `TRACE_MATRIX` | ⑦bis 全链路追溯矩阵 | coding-report-skill / code-review-skill | 带 v{N}-r{M} | ✅ |
| `CODE_REVIEW` | CodeReview 报告 | code-review-skill | 带 v{N}-r{M} | ✅ |
| `PROPOSAL` | Proposal 文档 | proposal-skill | 不带版本号（按 N 编号）| ✅ |
| `PROPOSAL_ARCHIVE` | Proposal 归档文档 | proposal-skill | 不带版本号（按 N 编号）| 📝 |
| `ASSETS` | 项目资产主体 + 更新日志 | project-assets-update-skill | 不带版本号（原地修改）| 📝 |
| `RA` | 需求分析文档 | requirement-analysis-skill | 不带版本号（原地更新）| ✅ |
| `RA_GENERATE_PLAN` | RA 生成计划 | requirement-analysis-skill | 带 r{N} | 📝 |
| `RA_IMPACT` | RA 修订影响分析报告 | requirement-analysis-skill | 带 r{N} | 📝 |
| `RA_REVERSE_ISSUES` | RA 反向问题登记 | requirement-analysis-skill | 不带版本号（原地累加）| 📝 |
| `STORY_GENERATE_PLAN` | Story 生成计划 | story-generate-skill | 带 r{N} | 📝 |
| `STORY_WRITER_REPORT` | Story 撰写报告 | story-generate-skill | 带 r{N} | 📝 |
| `REVIEW_COMPARE` | 跨轮 Review 对比表 | story-generate-skill | 带 v{N}-to-v{M} | 📝 |

### 4.11 错误码

| 错误码 | 含义 | 恢复 |
|--------|------|------|
| `E000` | 未知 intent（不在 §4.10 枚举表或代码未实现）| 提示检查 intent 拼写，或确认是否属 📝 未实现项 |
| `E001` | `assets.md` 不存在 | 提示运行 `project-assets-update-skill.md §3 生成动作` |
| `E002` | `gitPath` 字段为空 | 提示检查 assets.md §1 |
| `E003` | `gitPath` 路径不存在 | 提示检查文件系统。🆕 v3.4.0：**落地前强制触发**（resolve_path step 2，非仅事后 health）—— gitPath 无效即阻断文档落地，由 G-DOC-STORAGE 门禁兜底 |
| `E004` | 微服务名不在 §2 列表 | 提示检查 assets.md §2 |
| `E005` | 业务+逻辑都无关联（0/0）| 🔴 强制询问用户 |
| `E006` | 旧路径写入尝试 | 提示改用新路径 `ae-sdd-doc/` |
| `E007` | 版本号未递增 | 重入时必须 r 至少递增 |
| 🆕 `E008` | `docWorkspacePath` 声明但路径不存在 | 提示检查 assets.md §1 docWorkspacePath 字段（落地前强制触发，同 E003）|

---

## 5. 重入流程与文档演进

> **🔴 重入场景：** 用户说"重新跑 Coding"/"重入 Phase 2 ⑤"/"重做这一轮" 时，哪些文档新建、哪些修改、哪些归档？

### 5.1 重入 SOP（5 步判定）

**步骤 1：识别重入点**

| 重入点 | 触发动作 |
|--------|---------|
| 重入 Story 生成 | Story 文档**原地更新** + ChangeLog 追加 |
| 重入 Task 生成 | Task 文档**原地更新** + ChangeLog 追加 |
| 重入 Coding | **新增** Coding 报告（r 递增）|
| 重入 TestCase | 测试用例**原地更新** + ChangeLog 追加 |
| 重入 Test（运行测试）| **新增** 测试报告（r 递增）|
| 重入 CodeReview | **新增** CR 报告（r 递增）|
| 重入 Story Review | **新增** Story Review 报告（r 递增）|

**步骤 2：判定每类文档的动作**

| 文档类型 | 重入时的动作 | 命名 |
|---------|------------|------|
| PRD / RA / DR | 原地更新 + ChangeLog | `PRD-001.md`（不变）|
| Story / Task / CodingPlan / 测试用例 | 原地更新 + ChangeLog | 同名（不变）|
| Story Supplement | 原地累加（不删旧的）| 同名 |
| Coding 报告 | **新增**（r 递增）| `STORY-001-BE-CodingReport-v1-r2.md` |
| 测试报告 | **新增**（r 递增）| `STORY-001-BE-Report-v1-r2.md` |
| CR 报告 | **新增**（r 递增）| `STORY-001-BE-CodeReview-v1-r2.md` |
| Story Review 报告 | **新增**（r 递增）| `STORY-001-BE-StoryReviewReport-r2.md` |
| Review Proposal | **新增**（N 递增）| `STORY-001-BE-Proposal-2.md` |
| 跨轮 Review 对比表 | **新增**（按 v1-to-v2 命名）| `STORY-001-BE-ReviewCompare-v1-to-v2.md` |
| 项目资产 | 原地修改 + 写更新日志 | `icec-cloud-boss.assets.md` + `icec-cloud-boss.update-log.md` |
| 流程状态文件 | 原地修改 | `.auto-engineering/{STORY-ID}/state.json` |

**步骤 3：归档旧版本（如需）**

> **🔴 原则：** 设计类文档**永不归档**（原地更新，历史靠 ChangeLog）；事件类报告 r 递增保留历史，但 ≥ 3 轮的旧版本可归档。

| 轮数 | 处理 |
|------|------|
| v1-r1, v1-r2, v2-r1 | 保留（不归档）|
| v1-r1 / v1-r2 / ... v4-r1+ | 可归档到 `archive/{date}/` 子目录 |
| **归档命令** | `mv ae-sdd-doc/Coding/{STORY-ID}/{STORY-ID}-CodingReport-v1-r4.md ae-sdd-doc/Coding/{STORY-ID}/archive/2026-07-01/` |

**步骤 4：更新流程状态**

修改 `.auto-engineering/{STORY-ID}/state.json` 记录：
- `currentStep` 推进
- `codingRound` 递增
- `currentSubStep` 更新（如 "编码后" / "测试中" / "评审中"）

**步骤 5：检查交叉引用**

- Story 文档引用 CodePlan → CodePlan 重做时 Story 不变
- CodePlan 引用项目资产 → 项目资产更新时 CodePlan 可能需更新
- CR 报告引用 Coding 报告 → Coding 报告重做时新 CR 自动用新 Coding 报告

**重入决策树：**
```
用户说"重入 XX 流程"
    ↓
识别重入点（Story / Task / Coding / Test / CodeReview ...）
    ↓
该文档类型是"原地更新"还是"新增"？
    ├─ 设计类（PRD/RA/DR/Story/Task/CodePlan/测试用例/Supplement）→ 原地更新 + ChangeLog 追加
    ├─ 事件类（Coding报告/测试报告/CR报告/Review报告/Proposal）→ 新增（r 或 N 递增）
    └─ 基础设施类（项目资产/状态文件）→ 原地更新 + 日志记录
    ↓
事件类旧版本保留（不删）
    ↓
ChangeLog 追加新行
    ↓
更新 state.json
    ↓
检查交叉引用
```

### 5.2 文档状态码与生命周期

> **🔴 与项目资产更新日志 §1 状态码对齐**

| 状态 | 含义 | 何时用 |
|------|------|--------|
| `🆕 initial` | 首次生成 | Story/Task/Coding 报告 第一次创建 |
| `⏳ pending` | 准备修改 | log 写好"待更新"条目，文档未改 |
| `✅ done` | 已完成 | 修改/新增已落地 |
| `🔴 blocked` | 卡住 | 评审/修改卡住，需用户决策 |
| `⬇️ downgraded` | 降级 | 长期未更新，自动降级 |
| `🗑️ archived` | 归档 | 旧版本归档到 archive/ |

**生命周期流转：**
```
生成（🆕 initial）
    ↓
使用 / 引用（其他文档引用此文档）
    ↓
修改（⏳ pending → ✅ done，ChangeLog 追加）
    ↓
（循环使用/修改）
    ↓
事件类报告 r ≥ 3 时可归档（🗑️ archived）
    ↓
项目撤销时（罕见）→ 整体删除
```

**关键约束：**
- 🔴 **设计类文档永不删除**（原地更新，历史靠 ChangeLog 追溯）
- 🔴 **事件类报告 r 递增不删除**（保留所有轮次）
- 🔴 **项目资产永不删除**（修改通过更新日志追踪）
- 🔴 **ChangeLog 永不删除**（追加模式）

### 5.3 交叉引用规则

| 引用方 | 被引用方 | 引用方式 |
|--------|---------|---------|
| Story 文档 | CodePlan / 项目资产 / Story Review 报告 | 相对路径 + 锚点 |
| CodePlan | Story / Task / 项目资产 / 统一版 CodePlan | 相对路径 |
| Coding 报告 | Story / CodePlan / 测试报告 | 相对路径 |
| CR 报告 | Coding 报告 / 测试报告 / Story / 项目资产 | 相对路径 |
| Story Review Proposal | Story / 历轮 Story Review 报告 | 相对路径 |
| 项目资产 | Story / CodePlan / CR 报告 | 相对路径 |
| ChangeLog | 文档 | 相对路径 |

**🔴 引用必须用相对路径**（不是绝对路径），保证跨机器可读。

**跨迭代引用示例：**
```markdown
参见 [Story Review r1 报告](../2026-06-15/CR/STORY-001-BE-StoryReviewReport-r1.md)
```

---

## 6. ChangeLog 与迭代关联

### 6.1 ChangeLog 机制（🔴 强制）

> **🔴 强制：** 每个文档**必须**有 ChangeLog 文件，记录所有修改项。

**ChangeLog 位置（🔴 与代码一致：文档同级目录）：**

```
{文档所在目录}/{doc-stem}-changelog.md
```

**例子：**
```
d:\Item\icec-cloud-boss\ae-sdd-doc\Story\STORY-001-BE.md
d:\Item\icec-cloud-boss\ae-sdd-doc\Story\STORY-001-BE-changelog.md      ← 同级目录

d:\Item\icec-cloud-boss\ae-sdd-doc\iterations\2026-06-17\Coding\STORY-001-BE-CodingReport-v1-r1.md
d:\Item\icec-cloud-boss\ae-sdd-doc\iterations\2026-06-17\Coding\STORY-001-BE-CodingReport-changelog.md  ← 同级目录
```

> **注：** 事件类报告的 ChangeLog 文件名为 `{doc-id}-changelog.md`（去掉版本号后缀），同一 doc-id 的多版本共用一份 changelog。

**ChangeLog 格式：**

```markdown
# ChangeLog - {doc-id}

| 版本 | 日期 | 修改人 | 修改项 | 改动来源 |
|------|------|--------|--------|---------|
| v1-r1 | 2026-06-17 | cong.chen | 首次创建 | ae-sdd-skill Phase 2 |
| v1-r2 | 2026-06-18 | cong.chen | 修复编译错误 | 重入 Coding |
| v2-r1 | 2026-06-20 | cong.chen | Story 大改后重跑 | DR Update |
```

**ChangeLog 字段说明：**

| 字段 | 说明 |
|------|------|
| **版本** | 与文档版本号对齐（v{N}-r{M}，或设计类标"原地更新"）|
| **日期** | 修改日期（YYYY-MM-DD）|
| **修改人** | 操作者（JavaDoc 作者）|
| **修改项** | 一句话描述修改内容 |
| **改动来源** | 触发修改的来源（Phase / Review r{N} / 用户反馈 / DR Update）|

**save_doc() 自动追加：**

> **🔴 自动行为：** `save_doc()` API 在保存新版本文档时，若传入 `changelog_note`，**自动**追加一行到同级 ChangeLog，无需手动维护。

### 6.2 迭代目录命名

- 格式：`{YYYY-MM-DD}`（如 `2026-06-17`）
- **可由调用方显式传入**（如用户说"归到 6 月 17 日那批"）
- **未指定则自动判定**（业务+逻辑双轨关联性分析 §6.3）

### 6.3 关联性算法（🔴 唯一权威定义）

> 本节是关联性判定规则的 SSOT。§4.5 关联性 API 调用本节规则。

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

### 6.4 关联等级判定

| 业务 | 逻辑 | 等级 | 默认动作 |
|------|------|------|---------|
| 1 | 1 | **强关联** | 默认入当前迭代 |
| 1 | 0 | **中关联** | 默认入当前迭代 |
| 0 | 1 | **中关联** | 默认入当前迭代 |
| 0 | 0 | **无关联** | 🔴 **强制询问用户**（E005 错误码）|

### 6.5 choose_iteration() 流程

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

### 6.6 业务/逻辑标签采集

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

## 7. .gitignore 自动生成（🔴 强制）

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

## 8. 存量迁移

> **状态：** `migrate_old_docs()` API **已实现**，**默认不执行**，需用户显式确认。迁移目标映射见 §1.6。

### 8.1 migrate_old_docs() 行为

```python
migrate_old_docs(
    projectKey="icec-cloud-boss",
    mode="dry-run"  # 或 "execute"
)
```

**步骤：**
1. 扫描旧路径下的所有 .md 文件
2. 解析每个文件的 doc_id 和 doc_type
3. 判定每个文件的目标新路径（按 §1.6 映射）
4. **dry-run 模式：** 生成 MigrationReport（不实际移动）
5. **execute 模式：** 移动文件 + 生成 ChangeLog 标记"迁移自旧路径"

### 8.2 MigrationReport 格式

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
| 1 | design/dr/icec-cloud-boss/DR-001.md | ae-sdd-doc/DR/DR-001.md | DR-001 | DR |
| 2 | design/story/be/STORY-001-BE.md | ae-sdd-doc/Story/STORY-001-BE.md | STORY-001-BE | Story |
| ... | ... | ... | ... | ... |

## 注意事项
- 旧目录（design/）将保留（不删除）
- ChangeLog 初始行标记 "迁移自旧路径 design/"
- 需要用户确认后执行
```

### 8.3 默认不执行 + 用户确认

> **🔴 强制：** `migrate_old_docs()` 默认 `mode='dry-run'`，**必须**用户显式确认才执行 `mode='execute'`。

**用户确认模板：**
```
【存量迁移确认】
- 项目: {projectKey}
- 待迁移文件数: {N}
- 目标新路径: ae-sdd-doc/{DocType}/
- 旧目录保留: 是（不删除）
- ChangeLog 标记: 迁移自旧路径

请确认：
☐ 同意执行迁移
☐ 暂不执行（dry-run 模式）
```

---

## 9. 横切调用规范

### 9.1 调用矩阵（🔴 单一权威表）

> **本表合并了历史两个版本（旧 §15.1 + §15.5.2），覆盖全部调用方 SKILL。** API 签名见 §4，本表只做 SKILL→API 映射。

| 调用方 SKILL | 文档类型 | 调用 API |
|------------|---------|---------|
| `requirement-analysis-skill.md` | PRD / RA / RA 生成计划 / RA 影响分析 | `save_doc(intent="PRD"/"RA"/"RA_GENERATE_PLAN"/"RA_IMPACT")` |
| `story-generate-skill.md` | Story / Story Supplement / Story 生成计划 | `save_doc(intent="STORY"/"STORY_SUPPLEMENT"/"STORY_GENERATE_PLAN", version=None)` |
| `dr-generate-skill.md` | DR 主文档 | `save_doc(intent="DR", version=None)` |
| `dr-update-skill.md` | DR 补充说明 | `save_doc(intent="DR_SUPPLEMENT")` |
| `task-generate-skill.md` | Task / Task 补充 / CodingPlan | `save_doc(intent="TASK"/"TASK_SUPPLEMENT"/"CODING_PLAN", storyId, version=None)` |
| `coding-report-skill.md` | Coding 报告 / 追溯矩阵 | `save_doc(intent="CODING_REPORT"/"TRACE_MATRIX", storyId, version={v, r})` |
| `coding-skill.md` | （编码执行，报告交 coding-report-skill）| 引用 coding-report-skill.md |
| `testcase-generate-skill.md` | 测试用例 | `save_doc(intent="TESTCASE", storyId, version=None)` |
| `testcase-review-skill.md` | TestCase Review 报告 | `save_doc(intent="TESTCASE_REVIEW", storyId, version={r})` |
| `test-generate-skill.md` / `test-review-skill.md` | 测试报告 | `save_doc(intent="TEST_REPORT", storyId, version={v:N, r:M})` |
| `code-review-skill.md` | CR 报告 | `save_doc(intent="CODE_REVIEW", storyId, version={v, r})` |
| `story-review-skill.md` | Story Review 报告 | `save_doc(intent="STORY_REVIEW", storyId, version={r})` |
| `proposal-skill.md` | Proposal | `save_doc(intent="PROPOSAL", storyId)` |
| `project-assets-update-skill.md` | 项目资产 + 更新日志 | `save_doc(intent="ASSETS")` + `check_and_update_gitignore()` |

### 9.2 标准调用段（🔴 各 SKILL 必加）

每个生成/更新文档的 SKILL 在 "§目标" 之后、"§整体流程" 之前，必须加以下段（**统一定位**）：

```markdown
## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 的每个输出文档在落地前**必须先调用 [`document-storage-skill.md`](./document-storage-skill.md)** 确定：
> 1. **路径**（§1.3 路径模板）：本 SKILL 产出的文档存哪里（强制 `ae-sdd-doc/`）
> 2. **命名 + 版本号**（§2 命名规则）：本 SKILL 产出的文档怎么命名
> 3. **ChangeLog**（§6.1 ChangeLog 机制）：每次修改必须追加 ChangeLog 行
> 4. **关联性分析**（§6.3 关联性分析）：通过 `choose_iteration()` 判定属于哪个迭代
> 5. **.gitignore**（§7 .gitignore 自动生成）：首次写入时自动维护
>
> **必读调用矩阵：** 参见 `document-storage-skill.md §9.1`（本 SKILL 在矩阵的哪一行）。
>
> **本 SKILL 输出文档与 document-storage-skill §X 的对应关系：**
> - {文档 1} → document-storage-skill §1.3（路径）+ §2.2（命名）+ §6.1（ChangeLog）
> - {文档 2} → document-storage-skill §1.3（路径）+ §2.2（命名）+ §6.1（ChangeLog）
> - ...

### 调用示例（按本 SKILL 实际文档类型填）

| 输出文档 | 路径模板 | 命名规则 | ChangeLog | 重入时动作 |
|---------|---------|---------|----------|----------|
| {设计类文档名} | `ae-sdd-doc/{DocType}/{doc-id}.md` | 不带版本号 | 必填 | 原地更新 |
| {事件类报告名} | `ae-sdd-doc/{DocType}/{STORY-ID}/{doc-id}-v{N}-r{M}.md` | v{N}-r{M} | 必填 | r 递增 |
```

### 9.3 调用时机（🔴 必在文档落地前调用）

```
SKILL 流程
    ↓
第零步：准入检查
    ↓
第一步：读取输入（含 §9 调用本 SKILL 查路径/命名/重入/ChangeLog/关联性）
    ↓
第二步：内容生成
    ↓
第三步：合理性自检
    ↓
第四步：写入文档（落地前再次确认路径/命名/版本号/ChangeLog/关联性）← 🔴 必查
    ├─ 调 choose_iteration() 判定迭代
    ├─ 调 get_latest_version() 获取当前版本（事件类）
    ├─ 调 save_doc() 写入新版本 + 追加 ChangeLog
    └─ 首次写入时调 check_and_update_gitignore()
    ↓
第五步：触发下游
```

### 9.4 不调用本 SKILL 的反模式

| 反模式 | 危害 | 正确做法 |
|--------|------|---------|
| ❌ 文档放错位置（自己起路径）| 找不着 | §9.1 调用矩阵 + §1.3 路径模板 |
| ❌ 事件类文档不带版本号 | 失轮次追溯 | §2.2 命名规则 |
| ❌ 设计类文档带版本号 | 与代码不一致 | §2.2 命名规则（原地更新）|
| ❌ 重入流程时不知道新建还是修改 | 文档漂移 | §5.1 重入 SOP |
| ❌ 不写 ChangeLog | 失变更追踪 | §6.1 ChangeLog 机制 |
| ❌ 不调用 choose_iteration() | 文档漂移 | §6.3 关联性分析 |
| ❌ 不维护 .gitignore | 污染 git | §7 .gitignore 自动生成 |

---

## 10. 与其他 SKILL 的衔接

| 上下游 SKILL | 衔接点 |
|------------|-------|
| `story-generate-skill.md` | §📦 文档存放前置调用 → save_doc(intent="STORY") |
| `story-review-skill.md` | §📦 文档存放前置调用 → save_doc(intent="STORY_REVIEW") |
| `task-generate-skill.md` | §📦 文档存放前置调用 → save_doc(intent="TASK") |
| `coding-skill.md` | 编码执行，不直接产出事件报告；报告交 `coding-report-skill.md` |
| `coding-report-skill.md` | §1 文档路径（按本 SKILL §1.3 路径模板）|
| `test-generate-skill.md` | §输出 → save_doc(intent="TEST_REPORT") |
| `test-review-skill.md` | §输出 → save_doc(intent="TEST_REPORT") 追加独立复核章节 |
| `code-review-skill.md` | §📦 文档存放前置调用 → save_doc(intent="CODE_REVIEW") |
| `testcase-generate-skill.md` | §📦 文档存放前置调用 → save_doc(intent="TESTCASE") |
| `project-assets-update-skill.md` | §1 文档路径（按本 SKILL §1.4 路径模板）|
| `SKILL.md` | 状态文件 `state.json` 按本 SKILL §1.3 路径 |
| `ae-sdd-update-skill.md` | 边界判定表增补 1 行：文档存放标准 = 本 SKILL |

---

## 11. 禁止事项

| # | 禁止 | 危害 | 正确做法 |
|---|------|------|---------|
| 1 | 禁止文档放错位置（不写 `ae-sdd-doc/`）| 找不着 | §1.3 路径模板 |
| 2 | 禁止事件类报告不带版本号 | 无法区分轮次 | §2.2 命名规则 |
| 3 | 禁止设计类文档带版本号 | 与代码不一致 | §2.2 命名规则（原地更新）|
| 4 | 禁止跨流程文档互引时用绝对路径 | 跨机器失效 | §5.3 相对路径 |
| 5 | 禁止事件类报告修改历史（必须 r 递增）| 失追溯 | §5.1 重入 SOP |
| 6 | 禁止重入流程时不知道新建还是修改 | 文档乱 | §5.1 重入 SOP |
| 7 | 禁止不写 ChangeLog | 失变更追踪 | §6.1 ChangeLog 机制 |
| 8 | 禁止不调用 `choose_iteration()` | 文档漂移 | §6.3 关联性分析 |
| 9 | 禁止不维护 `.gitignore` | 污染 git | §7 .gitignore 自动生成 |
| 10 | 禁止未经用户确认执行 `migrate_old_docs()` | 误操作 | §8 存量迁移 |
| 11 | 禁止写入旧路径（design/、.ae-task/、.ae-plan/、.spec/iterations/）| 路径混乱 | §1.6 旧路径 deprecated |
| 12 | 禁止业务=0 ∧ 逻辑=0 时不问用户 | 归错迭代 | §6.4 关联等级判定 |
| 13 | 禁止手动跳过版本号 | 历史断裂 | §2.4 版本号递增 SOP |

---

## 12. 执行清单

| # | 动作 | 产出 | 门禁 |
|---|------|--------|------|
| 1 | 识别文档类型 | 8 类之一 | §1.1 8 类流程目录 |
| 2 | 采集业务/逻辑标签 | doc.business + doc.logic | §6.6 |
| 3 | 调用 `choose_iteration()` 判定迭代 | iterationDir | 强关联/中关联/无关联判定 |
| 4 | 调用 `get_latest_version()` 获取当前版本（事件类）| {v, r} | 不超过最大版本 |
| 5 | 计算新版本号（事件类）| newVersion | §2.4 递增 SOP |
| 6 | 查 §1.3 路径模板 | 路径 | 路径合规（必须 `ae-sdd-doc/`）|
| 7 | 查 §2 命名规则 | 名称 | 命名合规 |
| 8 | 创建目录（如需）| dirPath | mkdir -p |
| 9 | 写入文档 | 完整内容 | §5.1 旧版本保留（事件类）|
| 10 | 追加 ChangeLog | 新行 | §6.1 格式合规 |
| 11 | 更新 STORING.md | 索引 | §4.4 API |
| 12 | 首次写入时维护 .gitignore | 幂等追加 | §7 |
| 13 | 判定重入时动作 | 新增/修改/归档 | §5.1 SOP |
| 14 | 更新 state.json（如有）| 状态文件 | 字段齐 |
| 15 | 检查交叉引用 | 引用链接 | 相对路径 |
| 16 | 触发下游 SKILL | 评审/Coding 等 | 已触发 |

---

## 13. 维护

- **维护人：** 架构组
- **更新频率：** 每次新增流程或新建文档类型时
- **同步对象：**
  - 所有 SKILL 引用本文件作为"文档存放标准"（统一引用）
  - 与 `ae-sdd-update-skill.md` 协调（边界判定表增补 1 行）
- **关键变化（2026-07-01 结构重构）：**
  - 🔧 消除 `§0.x` 游离编号：§0.5 工程解耦→§3，§0.6 API 契约→§4
  - 🔧 3 处矛盾收敛（跟随代码方向）：
    - 设计类文档：明确"原地更新、不带版本号"（统一 §1.1/§2.2/§4.10）
    - ChangeLog：明确"文档同级目录"（§6.1，与 save_doc 一致）
    - STORING.md：明确"单一文件"（§4.4，与 update_storing_index 一致）
  - 🔧 三重描述收敛：API 契约单一 SSOT（§4）、关联性算法单一 SSOT（§6.3）、调用矩阵合并为单一表（§9.1）
  - 🔧 §3.5 PRD state.json schema 降级为附录 A（schema 读写归 state.py，本 SKILL 只管路径）
  - 🔧 intent 枚举表（§4.10）新增"实现状态"列（✅/📝），与代码 _PATH_TEMPLATES 对齐
- **历史关键变化：**
  - 2026-06-27 v4.1：五维定位模型（新增业务线根）+ 资产路径 SSOT
  - 2026-06-25 v3.4.0：四维定位模型（新增文档工作区根）
  - 2026-06-17：统一目录 `ae-sdd-doc/` + 8 类流程目录 + ChangeLog + 迭代目录 + .gitignore + 存量迁移
  - 2026-06-10：工程解耦定位原则（§0.5/§0.6 原型）

---

## 附录 A. PRD 级 `state.json` schema（参考）

> **⚠️ 定位声明：** 本附录仅作字段参考。schema 读写实现归 `state.py`（`prd_init`/`check_prd_4_layers` 等 PRD helper）；schema 定义 SSOT 归 `SKILL.md §1.3`。**本 SKILL 只负责 state.json 的路径**（见 §1.3），**不负责 schema**。如发生字段冲突以 `SKILL.md §1.3` 为准。

```json
{
  "$schema": "https://ae-sdd.dev/schema/prd-state-v1.0.0.json",
  "schemaVersion": "1.0.0",
  "prdId": "PRD-CS-001",
  "prdTitle": "客服系统 v1",
  "prdDocPath": "ae-sdd-doc/PRD/PRD-CS-001.md",
  "drId": "DR-CS-001",
  "storyIds": [
    {
      "storyId": "STORY-002-BE",
      "state": "completed",
      "taskIds": ["TASK-002-01", "TASK-002-02"],
      "codingPlanIds": ["CP-002-01", "CP-002-02"],
      "codeReviewReport": ".auto-engineering/STORY-002-BE/CR-r1.md",
      "sevenBisPassed": true,
      "userConfirmedAt": "2026-06-25T10:30:00Z",
      "completedAt": "2026-06-25T10:30:00Z"
    }
  ],

  "crossStoryDeps": [
    {
      "fromStory": "STORY-002-BE",
      "toStory": "STORY-007-FE",
      "depType": "api",
      "critical": true,
      "verifiedAt": null,
      "verifiedBy": null
    }
  ],

  "crossStoryResidualRisks": [
    {
      "riskId": "RISK-PRD-CS-001-001",
      "description": "...",
      "owner": "...",
      "severity": "🟠",
      "dueDate": "2026-07-15",
      "mitigationPlan": "..."
    }
  ],

  "sizeBudget": {
    "estimated": { "storyCount": 5, "taskCount": 18, "hours": 240 },
    "actual": { "storyCount": 6, "taskCount": 22, "hours": 310 },
    "variance": { "storyCountPct": 20, "taskCountPct": 22, "hoursPct": 29 }
  },

  "prdReview": {
    "confirmedAt": null,
    "confirmedBy": null,
    "storytoldAt": null,
    "openQuestions": []
  },

  <!-- 🟠 v3.5.12 删除 memoryLifecycle 字段：死设计重复
       实际 memory 门禁（memory_gate.py）读 memory_store.py 的独立 JSONL，
       完整记录 enter/write/exit 生命周期，不读 state.json 的 memoryLifecycle。
       保留此注释说明删除原因，避免后续误加回。 -->

  "runtimeHooks": {
    "mavis": { "compactCmd": "mavis session rotate", "args": ["--handoff-file", "{summary.md}"] },
    "claude-code": { "hookType": "UserPromptSubmit", "injectCmd": "..." },
    "codex": {
      "compactCmd": "codex plugin add ae-sdd-prd-state",
      "hookSupport": {
        "PostToolUse": "supported",
        "Stop": "supported",
        "PreToolUse": "fallback-to-PostToolUse-rollback",
        "UserPromptSubmit": "fallback-to-output-last-message"
      },
      "eventStream": "codex exec --json",
      "status": "hook-supported-with-fallback",
      "fallback": null
    }
  },

  "gateRegistry": {
    "G-PRD-1": "pending",
    "G-PRD-2": "pending",
    "G-PRD-3": "pending",
    "G-PRD-4": "pending"
  },

  "prdStatus": "in_progress | prd_complete_pending_user | awaiting_compact | compacted | prd_aborted",
  "lastUpdated": "2026-06-25T10:30:00Z",
  "compactHistory": []
}
```

**字段职责矩阵（5 核心 + 3 runtime 字段）：**

| 类别 | 字段 | 写入方 | 读取方 |
|------|------|--------|--------|
| **核心标识** | `prdId` / `prdTitle` / `prdDocPath` / `drId` | `ae-sdd state prd-init` | 所有 |
| **Story 聚合** | `storyIds[]` | Story 完成 hook | G-PRD-1 闸 |
| **跨 Story 依赖** | `crossStoryDeps[]` | Story 完成 hook | G-PRD-3 闸 |
| **残留风险** | `crossStoryResidualRisks[]` | AI + 用户协作 | G-PRD-3 闸 |
| **规模预算** | `sizeBudget` | 聚合自 Story | 🔍 人工审核点 5 |
| **PRD 审核** | `prdReview` | 用户确认后（`state.prdReview`）| G-PRD-4 闸 |
| **memory lifecycle** | ~~`memoryLifecycle`~~（🟠 v3.5.12 删除：死设计重复，实际门禁读 `memory_store.py` JSONL）| `ae-sdd memory enter/write/exit` | `ae-sdd state write` 前置校验（读 memory_store 不读此字段）|
| **runtime 适配** | `runtimeHooks` | 项目实例化时一次 | `ae-sdd runtime compact` |
| **闸注册表** | `gateRegistry` | G-PRD-* 闸 hook | PRD 完成判定 CLI |

**`prdStatus` 枚举扩展：**

| 值 | 含义 | 触发条件 |
|----|------|---------|
| `in_progress` | 进行中 | 默认 |
| `prd_complete_pending_user` | 4 层 AND 全过，等用户确认收尾 | G-PRD-1·2·3 全 pass |
| `awaiting_compact` | 用户已确认，等 compact 钩子触发 | `prdReview.confirmedAt` 非空 |
| `compacted` | compact 完成，可进入下一个 PRD | `summary.md` 已生成 |
| `prd_aborted` | 异常终止（保留现场，不删 state.json）| 人工触发或门禁失败 |

**字段演进策略：** 所有新字段均为 **optional + 默认值**，旧 PRD 级 state.json 缺字段不报错（v3.3.0 兼容策略）。
