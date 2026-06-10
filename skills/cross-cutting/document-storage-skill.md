---
name: document-storage
description: 文档存放标准 SKILL — AE 体系的"横切依赖"标准，规范所有流程产出文档的存放路径、命名规则、版本号使用、重入流程处理、文档生命周期。任何 SKILL 在生成/更新文档前**必须先调用本 SKILL** 确定路径/命名/版本号。
---

# Document Storage — 文档存放标准 Skill（AE 体系横切依赖）

> **🔴 核心定位（2026-06-06 修订）：** 本 SKILL 是 AE 体系的"**横切依赖**"，**任何 SKILL 在生成/更新文档前都必须先调用本 SKILL** 确定：
> 1. 文档存哪里（路径模板 §2）
> 2. 文档怎么命名（命名规则 §3）
> 3. 是新建还是修改（重入 SOP §4）
> 4. 命名带不带版本号（版本号规则 §3.2）
>
> **触发场景：** 任何流程生成/更新/重入文档时**第一步**调用本 SKILL；不知道放哪/怎么命名/是不是新建时也查本 SKILL。
>
> **🔴 核心立场（2026-06-06 新建）：** 之前各流程产出的文档放得乱七八糟（Story 文档在 design/story/be/，CodingPlan 在 design/story/be/coding/，Coding 报告在 design/story/be/coding/，但跨 Story 的 UpdatePlan/ReviewLog 在 .auto-engineering/，项目资产在 skills/ae-sdd/project-assets/），**重入流程时不知道是新建还是修改**。本次新建本 SKILL 统一规范。
>
> **🔴 关键使用方式（2026-06-06 修订）：** 各 SKILL 在"§目标"之后、"§整体流程"之前，必须有"📦 文档存放前置调用"段，明确说明"本 SKILL 的每个输出文档按 document-storage-skill.md §X.Y 确定路径/命名/版本号"。

---

## 0. 目标

- AE 体系内所有流程产出文档**有明确路径**（按文档类型 + Story-ID）
- 文档**有统一命名规则**（新建带版本号 / 修改原地更新 / 重入有清晰判断）
- 文档**有清晰生命周期**（创建 / 修改 / 归档 / 撤销）
- 跨流程**有可追溯的引用**（Story 引用 CodePlan，CodePlan 引用项目资产，CodeReview 引用 Coding 报告）
- **重入流程时**有 SOP（哪些文档新建、哪些修改、哪些归档）

---

## 0.5 工程解耦定位原则（🆕 🔴 2026-06-10 硬约束）

> **原则：** ae-sdd SKILL 家族与工程代码**解耦**——SKILL 家族不知道工程在哪、工程不知道 SKILL 家族在哪。**document-storage-skill 是中间的"目录路由器"**，接收调用方的"意图"（哪个项目、哪个 STORY-ID、哪个微服务、什么任务），输出"文档/代码/产出物"的完整路径。

### 0.5.1 三维定位模型

AE 流程涉及 3 类不同维度的位置，必须分别定位：

| 维度 | 定位依据 | 谁消费 |
|------|---------|--------|
| **项目根** | `assets.md` §1 `gitPath` 字段 | 所有"项目级"操作（重任务文档根、流程状态、归档区）|
| **微服务根** | `{gitPath} + "/" + {serviceName}`（拼接约定） | 小/微任务的"工程根" |
| **Story 根** | `{项目根}/design/story/be/{STORY-ID}`（保持现状，不按 service 分层）| 重任务 Story/Task/Coding 文档 |

### 0.5.2 动态定位算法

```
调用方输入：{ projectKey, intent, storyId?, serviceName? }
    ↓
document-storage-skill.定位(projectKey, intent)
    ↓
1. 读 {projectKey}.assets.md §1 获取 gitPath
2. 校验 gitPath 存在性（文件系统可达）
3. 根据 intent 选择路径模板：
   ├─ "重任务 STORY-XXX" → 路径前缀 = {gitPath}/design/
   ├─ "小任务 ServiceX-任务" → 路径前缀 = {gitPath}/{serviceName}/.ae-task/
   └─ "微任务 ServiceX-任务" → 路径前缀 = {gitPath}/{serviceName}/.ae-plan/
4. 拼接具体文档路径
5. 返回：{完整路径, 文件名, 版本号, STORING 索引待更新项}
```

### 0.5.3 项目资产依赖

**document-storage-skill 强依赖** `assets.md`（作为"工程根"事实基线）：
- 项目级定位读 §1 `gitPath`
- 微服务级定位读 §2 `microservices[].name`（拼接命名约定）
- 路径规范读 §3-§5（分层映射 / 命名约定 / 包路径）

**调用方传入 `{projectKey}` 时，document-storage-skill 自动定位** `skills/ae-sdd/assets/{projectKey}/{projectKey}.assets.md`（原 §2.5 路径）

### 0.5.4 与 §2.6 小/微任务路径的关系

- **§2.6.2/§2.6.3 路径模板中的 `{工程根}`**：由 document-storage-skill 根据 `gitPath + serviceName` 动态填充
- **事务简称 `{服务缩写}`**：由 §2.6 命名规则 `icec-cloud-life-cs-service → cssv` 推导
- **隐藏目录 `.ae-task/` / `.ae-plan/`**：硬编码在 §2.6 路径模板中

### 0.5.5 硬约束

- ❌ SKILL 中硬编码 `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service` 这样的绝对路径（仅作示例，不作引用）
- ❌ SKILL 调用方传入"工程根"绝对路径（应传 `projectKey` + `serviceName`，由 document-storage-skill 推导）
- ❌ document-storage-skill 中包含 git 命令（不依赖外部工具）
- ❌ 项目资产文件中无 `gitPath` 字段（任何项目必须先建 assets.md 才能用 AE 流程）

---

## 0.6 动态定位 API（🆕 2026-06-10 文档化契约）

> 任何 SKILL 落地文档前调用本节 API 获取完整路径+元数据。

### 0.6.1 核心 API：`resolve_path()`

**输入参数：**

| 参数 | 类型 | 必填 | 含义 |
|------|------|------|------|
| `projectKey` | string | ✅ | 项目键（如 `icec-cloud-boss`）|
| `intent` | enum | ✅ | 文档意图（如 `STORY`、`TASK`、`CODING_PLAN`、`TASK_SMALL`、`PLAN_MICRO`）|
| `docType` | string | ✅ | 文档类型（如 `CodingReport`、`CodeReview`）|
| `storyId` | string | 条件 | `intent=STORY/TASK/...` 时必填 |
| `serviceName` | string | 条件 | `intent=TASK_SMALL/PLAN_MICRO` 时必填 |
| `taskName` | string | 条件 | `intent=TASK_SMALL/PLAN_MICRO` 时必填（事务简称）|
| `version` | object | 条件 | `intent` 含版本号时填（`{v: 1, r: 1}`）|

**输出：**

```typescript
interface ResolvedPath {
  fullPath: string;          // 完整路径
  dirPath: string;           // 目录路径（用于 mkdir）
  fileName: string;          // 文件名
  versionSuffix?: string;    // 版本号后缀（如 "-v1-r1"）
  scope: 'project' | 'service';  // 归属（项目级 / 服务级）
  storingIndexUpdate: {      // STORING.md 索引待更新项
    category: string;        // 6 大分类之一
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
5. 返回 ResolvedPath

### 0.6.2 工具 API：`get_git_path()`

**输入：** `projectKey`
**输出：** `string`（项目根绝对路径，从 assets.md 读取）

### 0.6.3 工具 API：`get_service_root()`

**输入：** `projectKey`, `serviceName`
**输出：** `string`（微服务根绝对路径 = `{gitPath}/{serviceName}`）

### 0.6.4 工具 API：`get_constraints()`（🆕 2026-06-10）

> **用途：** 取代 SKILL 内直接写死的 `constraints/` 路径引用。所有 SKILL 加载约束文档必须调用本 API，不得直接引用路径。

**输入：** `projectKey`
**输出：** `ConstraintList`（约束文档名称 → 完整路径的映射）

**行为：**
1. 读 `{projectKey}.assets.md` §1 获取 `gitPath`
2. 定位约束目录（约束文档随工程走：`{gitPath}/.ae-project/constraints/`；旧位置过渡期兼容：`skills/ae-sdd/standards/constraints/`）
3. 返回所有约束文档的名称 → 完整路径映射

**输出格式：**
```typescript
interface ConstraintList {
  [name: string]: string;  // name: "technology-stack", path: "/abs/path/technology-stack.md"
}
// 示例：
// { "technology-stack": "d:\\Item\\icec-cloud-boss\\.ae-project\\constraints\\technology-stack.md" }
```

**调用方不需要知道约束在哪里**——只需知道需要哪几项约束的 name。

### 0.6.5 工具 API：`get_assets()`（🆕 2026-06-10）

> **用途：** 取代 SKILL 内直接写死的 `assets/{projectKey}/icec-cloud-boss.assets.md` 路径。所有 SKILL 读取项目资产必须调用本 API。

**输入：** `projectKey`
**输出：** `AssetsRef`（项目资产文件的完整路径）

**行为：**
1. 定位 `{projectKey}.assets.md`（随工程走：`{gitPath}/.ae-project/assets.md`；旧位置过渡期兼容：`skills/ae-sdd/assets/{projectKey}/{projectKey}.assets.md`）
2. 返回路径

**输出格式：**
```typescript
interface AssetsRef {
  assetsPath: string;     // assets.md 完整路径
  updateLogPath: string;  // .update-log.md 完整路径
}
```

### 0.6.6 工具 API：`update_storing_index()`

**输入：** `projectKey`, `scope`（'project' or 'service'）, `entry`（索引项）
**输出：** `void`

**行为：** 自动更新 `skills/ae-sdd/{projectKey} Doc/STORING.md`（重任务）或 `{gitPath}/{serviceName}/.ae-task/Task-xxx/STORING.md`（小任务）或 `.ae-plan/Plan-xxx/STORING.md`（微任务）。

### 0.6.5 错误码

| 错误码 | 含义 | 恢复 |
|--------|------|------|
| `E001` | `assets.md` 不存在 | 提示运行 `project-assets-update-skill.md §3 生成动作` |
| `E002` | `gitPath` 字段为空 | 提示检查 assets.md §1 |
| `E003` | `gitPath` 路径不存在 | 提示检查文件系统 |
| `E004` | 微服务名不在 §2 列表 | 提示检查 assets.md §2 是否包含此微服务 |

---

## 1. 文档分类

| 分类 | 含义 | 例子 |
|------|------|------|
| **设计文档** | 描述"要做什么" | DR / Story / Task / CodePlan |
| **产出物文档** | 描述"做了什么" | Coding 报告 / 测试报告 / CodeReview 报告 |
| **Review 文档** | 描述"做得怎么样" | Story Review 报告 / Task Review 报告 / CodeReview UpdatePlan |
| **Update 文档** | 描述"修改了什么" | Story Update 补充文档 / Task Update 补充文档 / Project Assets 更新日志 |
| **基础设施文档** | AE 体系本身 | 项目资产 / SKILL 家族 / 文档存放标准 / 策略模板 |

---

## 2. 路径模板（按文档类型）

### 2.1 设计文档

| 文档类型 | 路径模板 | 例子 | `{projectKey}` 来自 |
|---------|---------|------|---------------------|
| DR（需求文档） | `design/dr/{projectKey}/DR-{YYYY-MM-DD}-{DR 标题简写}.md` | `design/dr/icec-cloud-boss/DR-2026-06-04-Boss用户列表查询接口.md` | `assets.md §1` |
| Story 文档 | `design/story/be/{STORY-ID}.md` | `design/story/be/STORY-001-BE.md` | （调用方传入） |
| Task 文档 | `design/story/be/task/{STORY-ID}/task-{N}-{X}-{任务简写}.md` | `design/story/be/task/STORY-001-BE/task-1-BossUserQuery值对象.md` | （调用方传入） |
| 统一版 CodePlan | `design/story/be/coding/{STORY-ID}/{STORY-ID}-CodingPlan.md` | `design/story/be/coding/STORY-001-BE/STORY-001-BE-CodingPlan.md` | （调用方传入） |
| 测试用例 | `design/testcase/be/{STORY-ID}/{STORY-ID}-testcase.md` | `design/testcase/be/STORY-001-BE/STORY-001-BE-testcase.md` | （调用方传入） |

### 2.2 产出物文档

| 文档类型 | 路径模板 | 例子 |
|---------|---------|------|
| Coding 报告 | `design/story/be/coding/{STORY-ID}/{STORY-ID}-CodingReport-v{N}-r{M}.md` | `design/story/be/coding/STORY-001-BE/STORY-001-BE-CodingReport-v1-r1.md` |
| 测试报告 | `design/testcase/be/{STORY-ID}/{STORY-ID}-Report-v{N}-r{M}.md` | `design/testcase/be/STORY-001-BE/STORY-001-BE-Report-v1-r1.md` |
| CodeReview 报告 | `design/story/be/coding/{STORY-ID}/{STORY-ID}-CodeReview-v{N}-r{M}.md` | `design/story/be/coding/STORY-001-BE/STORY-001-BE-CodeReview-v1-r1.md` |
| ⑦bis 对称性追溯矩阵 | `design/story/be/coding/{STORY-ID}/{STORY-ID}-追溯矩阵-v{N}-r{M}.md` | （独立成文件，因矩阵可能很大）|
| ⑥bis 一致性核查表 | 嵌入 CodeReview 报告 §零 | （不独立成文件）|
| Story-Writer/Task-Writer/CodeReviewer 报告 | `design/story/be/{阶段}/{STORY-ID}-阶段-WriterReport.md` | `design/story/be/story-gen/STORY-001-BE-Story-WriterReport.md` |

### 2.3 Review 文档

| 文档类型 | 路径模板 | 例子 |
|---------|---------|------|
| Story Review 报告 | `design/story/be/review/{STORY-ID}/{STORY-ID}-StoryReviewReport-r{N}.md` | `design/story/be/review/STORY-001-BE/STORY-001-BE-StoryReviewReport-r1.md` |
| Task Review 报告 | `design/story/be/task/{STORY-ID}/review/{STORY-ID}-TaskReview-r{N}.md` | （同上）|
| CodeReview UpdatePlan | 嵌入 CodeReview 报告 §十 | （不独立成文件）|
| Story Review UpdatePlan | `design/story/be/review/{STORY-ID}/{STORY-ID}-StoryReviewUpdatePlan-r{N}.md` | `design/story/be/review/STORY-001-BE/STORY-001-BE-StoryReviewUpdatePlan-r1.md` |
| 跨轮 Review 对比表 | `design/story/be/review/{STORY-ID}/{STORY-ID}-ReviewCompare-v1-to-v2.md` | （跨轮对比时用）|

### 2.4 Update 文档

| 文档类型 | 路径模板 | 例子 |
|---------|---------|------|
| Story 补充说明（Supplement）| `design/story/be/review/{STORY-ID}/{STORY-ID}-Supplement.md` | `design/story/be/review/STORY-001-BE/STORY-001-BE-Supplement.md` |
| Task 补充说明 | `design/story/be/task/{STORY-ID}/{STORY-ID}-Supplement.md` | 同上 |
| Story 变更追踪 | `design/story/be/change-log/{STORY-ID}-ChangeLog.md` | （记录 Story 历次变更的 diff 摘要）|
| Coding 历次变更文件清单 | 嵌入 Coding 报告 §二 | （不独立成文件）|

### 2.5 基础设施文档

| 文档类型 | 路径模板 | 例子 |
|---------|---------|------|
| 项目资产 | `skills/ae-sdd/project-assets/{projectKey}/{projectKey}.assets.md` | `skills/ae-sdd/project-assets/icec-cloud-boss/icec-cloud-boss.assets.md` |
| 项目资产更新日志 | `skills/ae-sdd/project-assets/{projectKey}/{projectKey}.update-log.md` | 同上目录 |
| 流程状态文件 | `.auto-engineering/{STORY-ID}/state.json` | `.auto-engineering/STORY-001-BE/state.json` |
| 流程运行日志 | `.auto-engineering/{STORY-ID}/run-log.md` | （记录每次状态变化）|

### 2.6 🆕 三类任务规模 × 文档路径（2026-06-10）

> **场景：** 不同任务规模的文档放不同根目录。重任务用 `design/` 相对路径（团队约定），小任务/微任务用**工程根目录**相对路径（与代码共存）。
>
> **🆕 2026-06-10 `{工程根}` 动态填充：** 由 `document-storage-skill.resolve_path()` 读取 `assets.md §1 gitPath` 字段自动推导：
> - 完整工程根 = `assets.md §1 gitPath`（如 `d:\Item\icec-cloud-boss`）
> - 微服务根 = `{gitPath} + "/" + {microservices[].name}`（如 `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service`）
>
> **调用方严禁硬编码**绝对路径传给本 SKILL；应传 `projectKey` + `serviceName`，由 §0.6 API 推导。

**事务简称命名规则（2026-06-10 用户确认）：**
- 格式：`{服务名缩写}-{任务简述}`
- 服务名缩写：去掉 `icec-cloud-` 前缀和 `-service`/`-bff` 后缀，保留核心
  - `icec-cloud-life-cs-service` → `cssv`
  - `icec-cloud-boss-user-service` → `usv`
  - `icec-cloud-boss-user-bff` → `ubff`
- 任务简述：业务名 / 功能名（2-3 个单词，尽量简短保留语义）
- 完整例子：`cssv-rongcloud-callback`、`usv-cache-preheat`、`ubff-user-export`

#### 2.6.1 重任务（套 Story 7 区模板能套满 4+ 区）

- **事务命名：** `{STORY-ID}`（团队约定）
- **文档归属：** 团队 `Story` + AE `{STORY-ID} Doc/`
- **根目录：** `design/` 相对路径
- **路径模板：** 见 §0.5 同工作流产出物归档原则 / §2.1-§2.5

#### 2.6.2 小任务（套 Story 7 区只能套 2-3 区）

- **事务命名：** `Task-{服务缩写}-{任务简述}`，例 `Task-cssv-rongcloud-callback`
- **文档归属：** 工程根 `.ae-task/` 隐藏目录
- **根目录：** `{工程根}/.ae-task/Task-{事务简称}/`（与代码共存，**隐藏**避免污染 IDE 视图）
- **完整例子：** `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service\.ae-task\Task-cssv-rongcloud-callback\`

| 文档类型 | 路径模板 | 例子 |
|---------|---------|------|
| Task 文档 | `{工程根}/.ae-task/Task-{事务简称}/Task.md` | `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service\.ae-task\Task-cssv-rongcloud-callback\Task.md` |
| 统一版 CodingPlan | `{工程根}/.ae-task/Task-{事务简称}/CodingPlan-v{N}.md` | `...\.ae-task\Task-cssv-rongcloud-callback\CodingPlan-v1.md` |
| CodingPlan ChangeLog | `{工程根}/.ae-task/Task-{事务简称}/CodingPlan-ChangeLog-v{N}-to-v{N+1}.md` | 同上 |
| Coding 报告 | `{工程根}/.ae-task/Task-{事务简称}/CodingReport-v{N}-r{M}.md` | `...\CodingReport-v1-r1.md` |
| CodeReview 报告 | `{工程根}/.ae-task/Task-{事务简称}/CodeReview-v{N}-r{M}.md` | `...\CodeReview-v1-r1.md` |
| ⑦bis 追溯矩阵 | `{工程根}/.ae-task/Task-{事务简称}/追溯矩阵-v{N}-r{M}.md` | 同上 |
| 测试报告 | `{工程根}/.ae-task/Task-{事务简称}/TestReport-v{N}-r{M}.md` | `...\TestReport-v1-r1.md` |
| 流程状态文件 | `{工程根}/.ae-task/Task-{事务简称}/state.json` | `...\state.json` |
| 流程运行日志 | `{工程根}/.ae-task/Task-{事务简称}/run-log.md` | `...\run-log.md` |
| STORING.md | `{工程根}/.ae-task/Task-{事务简称}/STORING.md` | 跨文档引用索引 |
| 归档 | `{工程根}/.ae-task/Task-{事务简称}/archive/{date}/` | r ≥ 4 旧报告 |

#### 2.6.3 微任务（套 Story 7 区套不出 0-1 区）

- **事务命名：** `Plan-{服务缩写}-{任务简述}`，例 `Plan-cssv-enum-fix`
- **文档归属：** 工程根 `.ae-plan/` 隐藏目录
- **根目录：** `{工程根}/.ae-plan/Plan-{事务简称}/`（与代码共存，**隐藏**避免污染 IDE 视图）
- **完整例子：** `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service\.ae-plan\Plan-cssv-enum-fix\`

| 文档类型 | 路径模板 | 例子 |
|---------|---------|------|
| CodingPlan（唯一设计文档）| `{工程根}/.ae-plan/Plan-{事务简称}/CodingPlan-v{N}.md` | `d:\Item\icec-cloud-boss\icec-cloud-life-cs-service\.ae-plan\Plan-cssv-enum-fix\CodingPlan-v1.md` |
| CodingPlan ChangeLog | `{工程根}/.ae-plan/Plan-{事务简称}/CodingPlan-ChangeLog-v{N}-to-v{N+1}.md` | 同上 |
| Coding 报告 | `{工程根}/.ae-plan/Plan-{事务简称}/CodingReport-v{N}-r{M}.md` | `...\CodingReport-v1-r1.md` |
| CodeReview 报告 | `{工程根}/.ae-plan/Plan-{事务简称}/CodeReview-v{N}-r{M}.md` | `...\CodeReview-v1-r1.md` |
| 测试报告 | `{工程根}/.ae-plan/Plan-{事务简称}/TestReport-v{N}-r{M}.md` | `...\TestReport-v1-r1.md` |
| 流程状态文件 | `{工程根}/.ae-plan/Plan-{事务简称}/state.json` | `...\state.json` |
| 流程运行日志 | `{工程根}/.ae-plan/Plan-{事务简称}/run-log.md` | `...\run-log.md` |
| STORING.md | `{工程根}/.ae-plan/Plan-{事务简称}/STORING.md` | 跨文档引用索引 |
| 归档 | `{工程根}/.ae-plan/Plan-{事务简称}/archive/{date}/` | r ≥ 4 旧报告 |

#### 2.6.4 三类对比

| 任务规模 | 事务名 | 文档归属 | 根目录 | 必出文档 |
|---------|--------|---------|--------|---------|
| **重任务** | `{STORY-ID}` | `{STORY-ID} Doc/` | `design/` 相对 | Story / Task / CodingPlan / Coding 报告 / CodeReview 报告 / 测试报告 |
| **小任务** | `Task-{服务缩写}-{任务简述}` | `{工程根}/.ae-task/Task-{简称}/` | 工程根相对（隐藏） | Task / CodingPlan / Coding 报告 / CodeReview 报告 / 测试报告 |
| **微任务** | `Plan-{服务缩写}-{任务简述}` | `{工程根}/.ae-plan/Plan-{简称}/` | 工程根相对（隐藏） | CodingPlan / Coding 报告 / CodeReview 报告 / 测试报告 |

**关键差异：**
- 重任务事务名是 `{STORY-ID}`（团队约定），小/微任务事务名是 `{服务缩写}-{任务简述}`
- 小任务有 Task 文档，微任务**只有 CodingPlan**
- 三类都过 14 条 CodingPlan 门禁 + TR-1~TR-7 全过程（流程深度不减）
- 小任务/微任务的 CodingPlan 必须独立产出 CodingModel 11 维决策 + 核心链路保护（**不引用 Story 章节**，因为没有 Story）
- **🆕 2026-06-10 隐藏目录约定：** 小任务/微任务用 `.ae-task/` `.ae-plan/` 前缀避免 IDE 显示/污染工程根视图；用 `ls -a` 可查看

**禁止：**
- ❌ 小任务/微任务文档放进 `design/` 相对路径
- ❌ 重任务文档放进工程根 `.ae-task/` / `.ae-plan/`
- ❌ 任务简写用全大写（必须小写连字符）
- ❌ 不同工程之间共享 `.ae-task/Task-{事务简称}/` 目录（按工程隔离）
- ❌ 在无 Story 上下文的微任务里编造 Story 引用

---

## 3. 命名规则

### 3.1 命名模板（统一格式）

```
{STORY-ID}-{文档类型标识}{?版本号后缀}.md
```

**字段说明：**

| 字段 | 是否必填 | 说明 |
|------|---------|------|
| `{STORY-ID}` | ✅ | Story 编号（如 `STORY-001-BE`）|
| `{文档类型标识}` | ✅ | 描述文档内容（如 `CodingReport` / `CodeReview` / `testcase`）|
| `{?版本号后缀}` | 视情况 | 见下表 |

### 3.2 版本号使用规则（🔴 关键：什么时候带/不带）

| 文档类型 | 命名规则 | 例子 | 原因 |
|---------|---------|------|------|
| **设计文档**（DR / Story / Task / CodePlan）| **不带版本号**（原地更新）| `STORY-001-BE.md` | 设计文档是"活的"，不断修改完善 |
| **任务级 CodePlan（嵌入 Task 文档）** | **不带版本号** | （嵌入 Task）| 任务级 CodePlan 跟 Task 文档一起更新 |
| **测试用例** | **不带版本号**（原地更新）| `STORY-001-BE-testcase.md` | 测试用例是"规范"，不断补充 |
| **Coding 报告** | **带版本号 `v{N}-r{M}`** | `STORY-001-BE-CodingReport-v1-r1.md` | 报告是"事件"，每轮 Coding 一份 |
| **测试报告** | **带版本号 `v{N}-r{M}`** | `STORY-001-BE-Report-v1-r1.md` | 同上 |
| **CodeReview 报告** | **带版本号 `v{N}-r{M}`** | `STORY-001-BE-CodeReview-v1-r1.md` | 同上 |
| **Writer/Reviewer Report** | **带 `r{N}`**（Review 轮次）| `STORY-001-BE-Story-WriterReport.md` | 1 个 Story 1 份，1 份 r1 |
| **Review UpdatePlan** | **带 `r{N}`** | `STORY-001-BE-StoryReviewUpdatePlan-r1.md` | 1 份/轮 Review |
| **Story Supplement** | **不带版本号**（原地累加）| `STORY-001-BE-Supplement.md` | 补充说明不断累加 |
| **跨轮 Review 对比表** | **带 `v1-to-v2`** | `STORY-001-BE-ReviewCompare-v1-to-v2.md` | 表示对比的 2 个版本 |
| **项目资产 + 更新日志** | **不带版本号**（在 .update-log.md 记录变更）| `icec-cloud-boss.assets.md` + `icec-cloud-boss.update-log.md` | 资产是"单一权威源"，更新通过日志追踪 |
| **流程状态文件** | **不带版本号** | `.auto-engineering/{STORY-ID}/state.json` | 状态实时变化 |

### 3.3 版本号含义

- `v{N}`：Story 版本（第 N 次大改 Story 后，N 递增）
- `r{M}`：Coding 轮次（第 M 轮 Coding，M 递增）
- 关系：v 改变时 r 重置为 1

---

## 4. 重入流程的文档处理（🔴 关键）

> **🔴 重入场景：** 用户说"重新跑 Coding"/"重入 Phase 2 ⑤"/"重做这一轮" 时，哪些文档新建、哪些修改、哪些归档？

### 4.1 重入 SOP（5 步判定）

**步骤 1：识别重入点**

| 重入点 | 触发动作 |
|--------|---------|
| 重入 Story 生成 | Story 文档原地修改（不带版本号）|
| 重入 Task 生成 | Task 文档原地修改（不带版本号）|
| 重入 Coding | **新增** Coding 报告（r 递增）|
| 重入 TestCase | 测试用例原地修改（不带版本号）|
| 重入 Test（运行测试）| **新增** 测试报告（r 递增）|
| 重入 CodeReview | **新增** CodeReview 报告（r 递增）|
| 重入 Story Review | **新增** Story Review 报告（r 递增）|

**步骤 2：判定每类文档的动作**

| 文档类型 | 重入时的动作 | 命名 |
|---------|------------|------|
| DR | 原地修改（不改版本号）| 同名 |
| Story | 原地修改（同上）| 同名 |
| Task | 原地修改（同上）| 同名 |
| 统一版 CodePlan | 原地修改（同上）| 同名 |
| 测试用例 | 原地修改（同上）| 同名 |
| Story Supplement | 原地累加（不删旧的）| 同名 |
| Coding 报告 | **新增**（r 递增）| `STORY-001-BE-CodingReport-v1-r2.md` |
| 测试报告 | **新增**（r 递增）| `STORY-001-BE-Report-v1-r2.md` |
| CodeReview 报告 | **新增**（r 递增）| `STORY-001-BE-CodeReview-v1-r2.md` |
| Story Review 报告 | **新增**（r 递增）| `STORY-001-BE-StoryReviewReport-r2.md` |
| Review UpdatePlan | **新增**（r 递增）| `STORY-001-BE-StoryReviewUpdatePlan-r2.md` |
| 跨轮 Review 对比表 | **新增**（按 v1-to-v2 命名）| `STORY-001-BE-ReviewCompare-v1-to-v2.md` |
| 项目资产 | 原地修改 + 写更新日志 | `icec-cloud-boss.assets.md` + `icec-cloud-boss.update-log.md` |
| 流程状态文件 | 原地修改 | `.auto-engineering/{STORY-ID}/state.json` |

**步骤 3：归档旧版本（如需）**

> **🔴 原则：** Coding/测试/CodeReview 报告**不归档**（r 递增保留历史），但 ≥ 3 轮的旧版本可归档。

| 轮数 | 处理 |
|------|------|
| r1, r2, r3 | 保留（不归档）|
| r4, r5, ... | 可归档到 `archive/{date}/` 子目录 |
| **归档命令** | `mv design/story/be/coding/{STORY-ID}/{STORY-ID}-CodingReport-v{N}-r4.md design/story/be/coding/{STORY-ID}/archive/2026-07-01/` |

**步骤 4：更新流程状态**

修改 `.auto-engineering/{STORY-ID}/state.json` 记录：
- `currentStep` 推进
- `codingRound` 递增
- `currentSubStep` 更新（如 "编码后" / "测试中" / "评审中"）

**步骤 5：检查交叉引用**

- Story 文档引用 CodePlan → CodePlan 重做时 Story 不变
- CodePlan 引用项目资产 → 项目资产更新时 CodePlan 可能需更新
- CodeReview 报告引用 Coding 报告 → Coding 报告重做时新 CodeReview 自动用新 Coding 报告

### 4.2 重入决策树

```
用户说"重入 XX 流程"
    ↓
识别重入点（Story / Task / Coding / Test / CodeReview ...）
    ↓
该文档类型是"原地更新"还是"新增"？
    ├─ 设计类（DR/Story/Task/CodePlan/测试用例/Supplement）→ 原地更新（同名）
    ├─ 事件类（Coding报告/测试报告/CodeReview报告/Review报告/UpdatePlan）→ 新增（r 递增）
    └─ 基础设施类（项目资产/状态文件）→ 原地更新 + 日志记录
    ↓
归档旧版本（≥ r4）→ 可选
    ↓
更新 state.json
    ↓
检查交叉引用
```

---

## 5. 文档状态码

> **🔴 与项目资产更新日志 §1 状态码对齐**

| 状态 | 含义 | 何时用 |
|------|------|--------|
| `🆕 initial` | 首次生成 | Story/Task/Coding 报告 第一次创建 |
| `⏳ pending` | 准备修改 | log 写好"待更新"条目，文档未改 |
| `✅ done` | 已完成 | 修改/新增已落地 |
| `🔴 blocked` | 卡住 | 评审/修改卡住，需用户决策 |
| `⬇️ downgraded` | 降级 | 长期未更新，自动降级 |
| `🗑️ archived` | 归档 | 旧版本归档到 archive/ |

---

## 6. 文档生命周期

```
生成（🆕 initial）
    ↓
使用 / 引用（其他文档引用此文档）
    ↓
修改（⏳ pending → ✅ done）
    ↓
（循环使用/修改）
    ↓
r ≥ 4 时可归档（🗑️ archived）
    ↓
项目撤销时（罕见）→ 整体删除
```

**关键约束：**
- 🔴 **设计类文档永不删除**（只修改，保留完整历史）
- 🔴 **事件类报告 r 递增不删除**（保留所有轮次）
- 🔴 **项目资产永不删除**（修改通过更新日志追踪）

---

## 7. 交叉引用规则

| 引用方 | 被引用方 | 引用方式 |
|--------|---------|---------|
| Story 文档 | CodePlan / 项目资产 / Story Review 报告 | 相对路径 + 锚点 |
| CodePlan | Story / Task / 项目资产 / 统一版 CodePlan | 相对路径 |
| Coding 报告 | Story / CodePlan / 测试报告 | 相对路径 |
| CodeReview 报告 | Coding 报告 / 测试报告 / Story / 项目资产 | 相对路径 |
| Story Review UpdatePlan | Story / 历轮 Story Review 报告 | 相对路径 |
| 项目资产 | Story / CodePlan / CodeReview 报告 | 相对路径 |

**🔴 引用必须用相对路径**（不是绝对路径），保证跨机器可读。

---

## 8. 与其他 SKILL 的衔接

| 上下游 SKILL | 衔接点 |
|------------|-------|
| `story-generate-skill.md` | §4 重入流程 → Story 文档原地更新 |
| `story-review-skill.md` | §4 重入流程 → Review 报告 r 递增 |
| `task-generate-skill.md` | §4 重入流程 → Task 文档原地更新 |
| `coding-skill.md` | §4 重入流程 → Coding 报告 r 递增 |
| `coding-report-skill.md` | §1 文档路径（按本 SKILL §2 路径模板）|
| `code-review-skill.md` | §1 文档路径（按本 SKILL §2 路径模板）|
| `testcase-generate-skill.md` | §1 文档路径（按本 SKILL §2 路径模板）|
| `project-assets-update-skill.md` | §1 文档路径（按本 SKILL §2 路径模板）|
| `ae-sdd-skill.md` | 状态文件 `state.json` 按本 SKILL §2.5 路径 |
| `ae-sdd-update-skill.md` | 边界判定表增补 1 行：文档存放标准 = 本 SKILL |

---

## 9. 禁止事项

| # | 禁止 | 危害 | 正确做法 |
|---|------|------|---------|
| 1 | 禁止文档放错位置（如 Coding 报告放 `.auto-engineering/`）| 找不着 | §2 路径模板 |
| 2 | 禁止事件类报告不带版本号 | 无法区分轮次 | §3.2 命名规则 |
| 3 | 禁止设计类文档带版本号 | 改名导致引用失效 | §3.2 命名规则 |
| 4 | 禁止跨流程文档互引时用绝对路径 | 跨机器失效 | §7 相对路径 |
| 5 | 禁止设计类文档删除历史 | 失追溯 | §6 生命周期 |
| 6 | 禁止事件类报告修改历史（必须 r 递增）| 失追溯 | §3.2 命名规则 |
| 7 | 禁止重入流程时不知道新建还是修改 | 文档乱 | §4 重入 SOP |
| 8 | 禁止 r ≥ 4 的旧版本不归档 | 目录爆炸 | §4.1 步骤 3 |

---

## 10. 执行清单

| # | 动作 | 产出 | 门禁 |
|---|------|--------|------|
| 1 | 识别文档类型 | 分类 | §1 5 类之一 |
| 2 | 查 §2 路径模板 | 路径 | 路径合规 |
| 3 | 查 §3 命名规则 | 名称 | 命名合规 |
| 4 | 判定重入时动作 | 新增/修改/归档 | §4 SOP |
| 5 | 更新 state.json（如有）| 状态文件 | 字段齐 |
| 6 | 检查交叉引用 | 引用链接 | 相对路径 |
| 7 | 触发下游 SKILL | 评审/Coding 等 | 已触发 |

---

## 11. 调用入口（🔴 横切依赖标准）

> **🔴 强制：** 任何生成/更新文档的 SKILL 在文档落地前**必须先调用本 SKILL**。本节是"调用入口"，列出"我应该读本 SKILL 的哪些章节"。

### 11.1 SKILL 间调用矩阵

| 调用方 SKILL | 生成的文档类型 | 必读本 SKILL 章节 |
|------------|--------------|------------------|
| `story-generate-skill.md` | Story 文档 | §2.1 路径模板 + §3.1/3.2 命名规则（设计类不带版本号）|
| `task-generate-skill.md` | Task 文档 | §2.1 路径模板 + §3.1/3.2 命名规则（设计类不带版本号）|
| `coding-report-skill.md` | Coding 报告 | §2.2 路径模板 + §3.2 命名规则（事件类带 v{N}-r{M}）+ §4.1 重入步骤（r 递增）|
| `code-review-skill.md` | CodeReview 报告 | §2.2 路径模板 + §3.2 命名规则（事件类带 v{N}-r{M}）+ §4.1 重入步骤 |
| `story-review-skill.md` | Story Review 报告 | §2.3 路径模板 + §3.2 命名规则（带 r{N}）|
| `coding-skill.md` | Coding 报告（由 coding-report-skill 生成）| 引用 coding-report-skill.md |
| `testcase-generate-skill.md` | 测试用例 | §2.1 路径模板 + §3.1/3.2 命名规则（设计类不带版本号）|
| `project-assets-update-skill.md` | 项目资产 + 更新日志 | §2.5 路径模板 + §3.2 命名规则（基础设施类不带版本号，更新走日志）|
| `proposal-skill.md` | Proposal | §2.6 路径模板 + §3.2 命名规则（事件类不带版本号，按 N 编号）|

### 11.2 标准调用段（🔴 各 SKILL 必加）

每个生成/更新文档的 SKILL 在 "§目标" 之后、"§整体流程" 之前，必须加以下段（**统一定位**）：

```markdown
## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 的每个输出文档在落地前**必须先调用 [`document-storage-skill.md`(./document-storage-skill.md)** 确定：
> 1. **路径**（§2 路径模板）：本 SKILL 产出的文档存哪里
> 2. **命名**（§3 命名规则）：本 SKILL 产出的文档怎么命名（特别是带不带版本号）
> 3. **重入判定**（§4 重入 SOP）：本 SKILL 文档重入时是新建还是修改
>
> **必读调用矩阵：** 参见 `document-storage-skill.md §11.1`（本 SKILL 在矩阵的哪一行）。
>
> **本 SKILL 输出文档与 document-storage-skill §X 的对应关系：**
> - {文档 1} → document-storage-skill §Y.Z
> - {文档 2} → document-storage-skill §Y.Z
> - ...

### 调用示例（按本 SKILL 实际文档类型填）

| 输出文档 | 路径模板 | 命名规则 | 重入时动作 |
|---------|---------|---------|----------|
| {文档名 1} | `{路径}` | {带/不带 版本号} | {新增/修改} |
| {文档名 2} | `{路径}` | {带/不带 版本号} | {新增/修改} |
```

### 11.3 调用时机（🔴 必在文档落地前调用）

```
SKILL 流程
    ↓
第零步：准入检查
    ↓
第一步：读取输入（含 §11 调用本 SKILL 查路径/命名/重入）
    ↓
第二步：内容生成
    ↓
第三步：合理性自检
    ↓
第四步：写入文档（落地前再次确认路径/命名/版本号）← 🔴 必查
    ↓
第五步：触发下游
```

### 11.4 不调用本 SKILL 的反模式

| 反模式 | 危害 | 正确做法 |
|--------|------|---------|
| ❌ 文档放错位置（自己起路径）| 找不着 | §11.1 调用矩阵 + §2 路径模板 |
| ❌ 事件类文档不带版本号 | 失轮次追溯 | §3.2 命名规则 |
| ❌ 设计类文档带版本号 | 引用失锚 | §3.2 命名规则 |
| ❌ 重入流程时不知道新建还是修改 | 文档漂移 | §4 重入 SOP |

### 11.5 API 化调用（🆕 2026-06-10）

> 各 SKILL 落地文档前**不再手写路径**，改为调用 `document-storage-skill.resolve_path()`。

#### 11.5.1 旧 vs 新对比

```python
# 旧方式（硬编码）
path = f"design/story/be/{story_id}/{story_id}-CodingReport-v{n}-r{m}.md"

# 新方式（动态定位）
from document_storage import resolve_path
resolved = resolve_path(
    projectKey="icec-cloud-boss",
    intent="CODING_REPORT",
    docType="CodingReport",
    storyId="STORY-001-BE",
    version={"v": 1, "r": 1}
)
full_path = resolved.fullPath  # 自动从 assets.md 推导
```

#### 11.5.2 调用矩阵（更新版）

| 调用方 SKILL | 旧：必读章节 | 新：调用 API |
|------------|------------|------------|
| story-generate-skill.md | §2.1 + §3.1/3.2 | `resolve_path(intent="STORY", projectKey, storyId)` |
| task-generate-skill.md | §2.1 + §3.1/3.2 | `resolve_path(intent="TASK", projectKey, storyId, taskId)` |
| coding-skill.md（重任务）| §2.2 + §3.2 | `resolve_path(intent="CODING_REPORT", ..., version)` |
| task-generate-skill.md（小任务）| §2.6.2 | `resolve_path(intent="TASK_SMALL", projectKey, serviceName, taskName)` |
| coding-skill.md（微任务）| §2.6.3 | `resolve_path(intent="PLAN_MICRO", projectKey, serviceName, taskName)` |

---

## 12. 维护

- **维护人：** 架构组
- **更新频率：** 每次新增流程或新建文档类型时
- **同步对象：**
  - 所有 SKILL 引用本文件作为"文档存放标准"（统一引用）
  - 与 `ae-sdd-update-skill.md` 协调（边界判定表增补 1 行）
- **关键变化（2026-06-06 新建）：**
  - 🆕 AE 体系第一个"横向基础设施"SKILL
  - 5 类文档分类（设计/产出物/Review/Update/基础设施）
  - 路径模板统一定义（按文档类型 + Story-ID）
  - 命名规则分两类：设计类不带版本号 / 事件类带版本号
  - 重入 SOP（5 步判定 + 决策树）
  - 文档状态码（与项目资产对齐）
  - 文档生命周期（5 阶段）
