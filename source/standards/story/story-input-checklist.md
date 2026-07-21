# Story 输入清单（SSOT）

> **本文件是 ae-sdd v3.9.3 新增的 Story 系列输入清单单一权威源（SSOT）。**
>
> **适用范围：** `story-generate-skill` / `story-review-skill` / `story-update-skill` 三个 Skill，以及与之相关的所有标准、模板、Writer prompt。
>
> **变更原则：** 任何新增/删除/修改 Story 系列输入项，**只改本文件**；其他文件通过引用本文件同步，禁止再次分散定义。

---

## 1. 为什么需要 SSOT

v3.9.2 之前，Story 系列输入清单散落在 4 个文件且互不一致：

| 文件 | 列出项 | 遗漏 |
|------|--------|------|
| `story-generate-skill.full.md` §第零步 | DR, PRD, 原型, 资产, 模板, 测试策略 | 项目约束 |
| `story-generate-skill.full.md` §第一步 | DR, PRD, 原型, 资产, **项目约束** | 测试策略、Story 审核标准 |
| `story-generation-standard.md` §1 | DR, PRD, 原型, 资产, 模板, 测试策略 | **项目约束、Story 审核标准、依赖 Story** |
| `story-review-checklist.md` §输入 | DR, PRD, 原型, Story, 模板, 资产, Supplement, Proposal | 项目约束、Story 审核标准、依赖 Story、章节级来源 |

本 SSOT 统一为 **13 项输入**，覆盖 4 个分类：**上游文档 / 项目级约束 / 依赖 Story / 模板与标准**。

---

## 2. 输入清单（13 项 · 4 类）

### A. 上游文档（4 项）

| # | 输入项 | 定位路径 / 加载 API | 用途 |
|---|--------|---------------------|------|
| A1 | **DR 文档** | `ae-sdd doc resolve --intent DR` 或 `design/` 下 `*DR*.md` | 业务规则、验收标准、接口契约、数据模型、异常场景、领域模型 |
| A2 | **PRD 文档** | `_find_prd_files`（rglob `*PRD*.md` / `*prd*.md` / `*需求*.md`）| 业务背景、用户场景、业务规则 |
| A3 | **产品原型** | 用户提供 或 `ae-sdd assets query "原型"` | UI 流程、状态、边界场景 |
| A4 | **历史 RA（如有）** | `ae-sdd doc resolve --intent RA` | 业务深度分析（如已有 RA，跳过重读） |

### B. 项目级约束（4 项）

| # | 输入项 | 定位路径 / 加载 API | 用途 |
|---|--------|---------------------|------|
| B1 | **项目资产（DDD 分层/命名/契约入口）** | `document-storage-skill.get_assets(projectKey)` + `ae-sdd assets read story-generate --project <projectKey>` | 命名约定、数据库规范、分层规则 |
| B2 | **项目约束** | `document-storage-skill.get_constraints(projectKey)` → `constraints/*.md` 名→路径映射 | 项目级硬约束（如事务、鉴权、限流规则） |
| B3 | **前端约束**（如涉及前端） | `constraints/frontend.md`（如有） | 前端组件库、Mock 平台、i18n 要求 |
| B4 | **依赖能力清单** | `ae-sdd assets query` 扫描已实现的能力 | 复用扫描、避免重复实现 |

### C. 依赖 Story（2 项）

| # | 输入项 | 定位路径 / 加载 API | 用途 |
|---|--------|---------------------|------|
| C1 | **本 Story 复用的其他 Story** | 元信息中"复用其他 Story / 模块的能力"表 → Story ID 列表 → `ae-sdd doc resolve --intent STORY --story-id {ID}` | 字段契约对齐、避免接口不一致 |
| C2 | **本 Story 阻塞的下游 Story**（如有） | 由 DR 反向推，或上游需求文档中明确 | 影响 Task 编排顺序 |

### D. 模板与标准（4 项）

| # | 输入项 | 定位路径 / 加载 API | 用途 |
|---|--------|---------------------|------|
| D1 | **Story 模板** | Document Storage `STORY_TEMPLATE`（返回 `path/source/content/sha256`） | 章节结构、顺序、ID 与主副层级 |
| D2 | **Story 撰写指南** | Document Storage `STORY_WRITING_GUIDE`（返回 `path/source/content/sha256`） | 各 section ID 的必填性、来源、写法与 Review 口径 |
| D3 | **Story 生成 / Review / 前端契约标准** | ae-sdd 当前 runtime 中的对应标准引用 | 阶段顺序、Review 与前端契约约束 |
| D4 | **测试策略** | 项目级测试约束与 ae-sdd 测试策略 | AC 覆盖基线 |

> **注：** D 类实际 4 项（Story 模板 / 生成标准 / 审核标准 / 测试策略），编号记为 D1-D4。

---

## 3. 输入加载 SOP（强制 4 步）

每个 Skill（generate / review / update）执行前，必须按以下 SOP 加载输入：

### Step 1：定位输入
- 遍历 §2 的输入，逐项通过声明的 Document Storage / assets API 定位
- 模板和指南禁止 rglob、手拼路径或直接读取固定文件
- 缺失项：标记 ❌ 并通过相应权威 API 重试
- 仍找不到：列入"待补输入清单"，禁止进入下一步

### Step 2：读取输入
- 按 §2 的加载 API 读取每项；模板和指南直接消费 Document Storage 响应中的 `content`
- 记录每项的权威来源、路径和 sha256（用于一致性校验）
- 提取每项的关键章节摘要（用于 AI 上下文压缩时保留）

### Step 3：输入完整性自检（prose checklist）
AI 必须按以下 13 项 ✅/❌ 自检，全部 ✅ 才能进入下一步：

```
[ ] A1 DR 文档已读取
[ ] A2 PRD 文档已读取
[ ] A3 产品原型已读取（或标注"无原型"）
[ ] A4 历史 RA 已读取（如有）
[ ] B1 项目资产已读取
[ ] B2 项目约束已读取
[ ] B3 前端约束已读取（如涉及前端）
[ ] B4 依赖能力清单已扫描
[ ] C1 依赖 Story 已加载并比对字段
[ ] C2 阻塞下游 Story 已识别（如有）
[ ] D1 Story 模板已加载
[ ] D2 Story 生成标准已加载
[ ] D3 Story 审核标准 + 前端契约标准已加载
[ ] D4 测试策略已加载
```

### Step 4：CLI 门禁验证（机械层）
运行 `ae-sdd gates check --only G-STORY-CTX`，未过 → **BLOCK**。

> 🔴 **G-STORY-CTX 在 v3.9.3 扩展**：除原有 `constraints/assets/DR/PRD` 4 类外，新增 `dependsStory` 与 `sourceTrace` 两项必查。详见 `tools/lib/gates.py:CONTEXT_GATE_REGISTRY["G-STORY-CTX"]`。

---

## 4. 设计来源标注规范（章节级）

> 🔴 **v3.9.3 新增红线：** 每个 AC / 字段 / 异常流程 / 数据模型字段必须标注上游来源，**精确到 PRD/DR 章节/段落**。

### 4.1 来源标注格式

```
📌 来源：{文档类型} {文档ID} §{章节号} {段落锚点}
```

**示例：**
- `📌 来源：DR DR-LIFE-001 §3.2 BR-05`
- `📌 来源：PRD PRD-LIFE-001 §4.1 场景 "用户登录失败"`
- `📌 来源：Story STORY-004-BE §接口契约 / DTO.userId`

### 4.2 各章节的来源标注要求

| 章节 | 来源标注对象 | 示例 |
|------|-------------|------|
| **元信息** | 来源 PRD / 来源 DR / 依赖 Story | `来源 PRD: PRD-001 §2.3` |
| **用户故事** | 角色 → PRD §用户场景；动作 → DR §业务规则 | `📌 角色：PRD §4.1` |
| **主流程** | 每步 → DR §业务流程 | `📌 步骤 2：DR §3.2 BR-05` |
| **异常流程** | 每行 → DR §异常场景 | `📌 用户不存在：DR §3.4 BR-12` |
| **AC 验收标准** | 每条 AC → DR §验收标准 / PRD §业务规则 | `📌 AC-001：DR §3.5 BR-15` |
| **接口契约 Request** | 每字段 → PRD/DR §字段定义 | `📌 userId：DR §4.2 字段表` |
| **接口契约 Response** | 每字段 → 服务端计算 / DB 字段 | `📌 nickname：DR §5.1 boss_user.nickname` |
| **数据模型** | 每字段 → DR §数据模型 | `📌 status：DR §5.1 boss_user.status` |
| **状态机** | 每状态 → DR §状态机 | `📌 WAITING：DR §6.1 状态定义` |
| **错误码** | 每错误码 → DR §错误码 / 业务规则 | `📌 11001：DR §7 错误码表` |
| **实现方案决策基线** | 每实现点 → 主流程 / DR / PRD | `📌 实现点 1：DR §3.2 BR-05` |
| **依赖 Story 复用** | 每复用项 → 依赖 Story §接口契约 | `📌 AgentStatusService.isOnline：STORY-004-BE §SPI` |

### 4.3 来源缺失判定（🔴 阻断型）

以下任一情况视为**来源缺失**，必须阻断，不得写入 Story：

1. 字段标注来源为"调用方传入"但未具体到调用路径
2. 来源指向不存在的文档/章节（如 DR §99）
3. 来源指向当前 Story 自身（循环引用）
4. 来源标注模糊（如"详见 PRD"无具体章节）
5. 复用项来源为"项目资产"但项目资产文件中未提及

### 4.4 跨文档字段对齐（🔴 阻断型）

每次填写完接口契约后，必须执行跨文档字段对齐检查：

```
[ ] PRD 中所有接口字段 → 与 Story Request 字段名/类型/必填一致
[ ] DR 中所有接口字段 → 与 Story Request 字段名/类型/必填一致
[ ] 依赖 Story 的出参字段 → 与本 Story Request 字段名/类型一致
[ ] 不一致项：标注为 🔴 阻断型，禁止继续
```

**字段对齐偏差示例（🔴 阻断）：**
- DR 声明 `userId: Long`，Story 写为 `userId: String`
- 依赖 Story 出参 `nickname: String(1-64)`，本 Story 入参写为 `nickname: String(1-128)`
- PRD 要求 `email` 必填，Story 未标注必填

---

## 5. 与 Skill/标准/模板的引用关系

| 上游文件 | 引用本 SSOT 的位置 | 用途 |
|----------|---------------------|------|
| `story-generate-skill.full.md` §第零步 | prose 自检表 + CLI 门禁 | 准入 |
| `story-generate-skill.full.md` §第一步 | 13 项定位与提取 | 读取 |
| `story-generate-skill.full.md` §第三步 bis | §4 来源标注规范 + §4.3 缺失判定 | 验证 |
| `story-review-skill.full.md` §第零步 | prose 自检表 + CLI 门禁 | 准入 |
| `story-review-skill.full.md` §A-E 检查 | §4 跨文档字段对齐 | 检查 |
| `story-update-skill.full.md` §第零步 | prose 自检表 | 准入 |
| `story-generation-standard.md` §1 | 指向本 SSOT | 标准 |
| `story-review-checklist.md` §输入 | 指向本 SSOT | 标准 |
| `tools/lib/gates.py` G-STORY-CTX | `required` 字段 + 13 项校验 | 门禁 |

---

## 6. 维护规则

1. **新增输入项**：在本文件 §2 表格追加一行，并同步 §3 自检表 + §5 引用关系
2. **删除输入项**：先评估影响范围，更新所有引用本 SSOT 的下游文件后再删除
3. **修改输入项**：同样需更新下游引用
4. **当前事实**：本 SSOT 只维护当前生效规则；设计语义写回 Design Ledger，验证事实由测试承载，任何时候都不写 changelog

---

## 7. 与其他 SSOT 的关系

| SSOT 文件 | 关系 |
|-----------|------|
| `tools/lib/paths.py:MASTER_VERSION` | 版本号 SSOT |
| `tools/lib/document_storage.py` | 文档与只读 Story 资源定位/正文/指纹 API SSOT |
| `STORY_TEMPLATE` | Story 章节结构、section ID 与主副层级 SSOT |
| `STORY_WRITING_GUIDE` | Story 章节撰写 SOP SSOT |
| `source/standards/story/story-generation-standard.md` | 7 阶段输出标准 SSOT（与本 SSOT 配套）|
| `source/standards/story/story-review-checklist.md` | Story Review 检查标准 SSOT（与本 SSOT 配套）|
| `source/standards/story/story-frontend-contract-standard.md` | 前端契约 6 维度 SSOT（与本 SSOT 配套）|
