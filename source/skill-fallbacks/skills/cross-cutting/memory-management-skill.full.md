---
name: memory-management
description: |
  Entity-tree memory management (🆕 v3.10.3). Memory 存储编译后的 compact 文档，
  按业务实体树分层（prd/dr/story/testcase/coding/common）。子流程Agent 首次进入时
  主动编译源上下文 -> 写 memory；后续读上下文 = 读 memory；子流程结束删自己的 memory，
  common 保留。废弃了 v3.10.3 之前的 5 层原文索引 + enter/exit 生命周期门禁。
---

# Memory Management Skill (🆕 v3.10.3 Entity-Tree + Compiled Compact)

## 0. 核心变化（v3.10.3）

| 维度 | 旧（v3.10.2 及之前） | 新（v3.10.3） |
|---|---|---|
| 用途 | 5层各有用途(scratch/事件流/跨项目pattern/冷归档混入) | 只管上下文，存编译后的 compact 文档 |
| 内容 | 原文短索引(每条≤180字符 JSONL) | 编译后的 compact.md 文档（高密度表格/列表） |
| 分层 | phase×story×task 粗粒度(无PRD/DR维度) | prd/dr/story/testcase/coding/common 业务实体平级分层 |
| phase维度 | 5个phase各一jsonl文件 | 丢弃phase维度，按业务实体分层 |
| 隔离 | L1按story，L2 project全共享 | 各实体独立目录，common存项目级可复用约束 |
| 生命周期 | L1/L2永久留存，L0 session后可删 | 子流程结束删自己的(临时上下文)，common保留 |
| enter/exit | 生命周期门禁(check_state_transition) | 废弃，子流程启动=创建(编译)，结束=删除 |
| 编译 | 无（写入即原文） | 子流程Agent 首次进入主动编译源上下文 -> compact |

## 1. 目录结构

```
.ae-sdd/memory/
├── common/                  # 项目级可复用约束(必须轻)，跨子流程保留
│   └── context.compact.md
├── prd/{PRD-ID}/            # RA/PRD子流程的工作上下文
│   ├── boot.compact.md      # 锚点/当前阶段/下一步/产物路径
│   ├── context.compact.md   # 关键决策/约束/需求摘要
│   ├── pending.compact.md   # 待决项/存疑项
│   └── manifest.json        # 校验:source hash + fingerprint
├── dr/{DR-ID}/              # DR子流程的工作上下文(同上4文件)
├── story/{Story-ID}/        # Story子流程的工作上下文
├── testcase/{Story-ID}/     # TestCase子流程的工作上下文
└── coding/{Story-ID}/       # Coding子流程的工作上下文
```

**存储格式**：compact.md 文件（Markdown 表格/列表，可读可审查，符合编译器"不做密文"原则）+ manifest.json 校验。废弃 JSONL。

## 2. 生命周期

### 2.1 子流程启动 = 创建（编译）

子流程Agent 首次进入时，主动读源上下文（DR/Story/约束/模板，从 document-storage）-> 编译成 compact -> 写 memory：

```bash
ae-sdd memory create --entity-type story --entity-id STORY-001-BE \
  --sources "constraints=assets/constraints.md DR=doc.md" \
  --context-json '{"series_chain":["story-generate","story-review"],"current_series":"story-generate","next_step":"generate story doc","constraints":["BigDecimal","幂等"],"story_acs":[{"id":"AC-1","description":"...","status":"pending"}]}'
```

编译时自动提取项目级可复用约束 -> 写入 common（若 common 不存在则创建）。

### 2.2 子流程执行中 = 读/更新 memory

后续所有上下文读取 = 读 memory 的 compact 文档（不再喂源文档给 LLM）：

```bash
ae-sdd memory read --entity-type story --entity-id STORY-001-BE
```

关键决策落地 / 待决项变化时，增量更新对应 slice：

```bash
ae-sdd memory update --entity-type story --entity-id STORY-001-BE --slice pending --content "..."
```

### 2.3 子流程结束 = 删除自己的 memory

子流程Agent 交付物回传后，删自己的 memory（删临时上下文，留业务成果）：

```bash
ae-sdd memory clean --entity-type story --entity-id STORY-001-BE
```

common 保留给后续子流程使用。

### 2.4 从0重新开始 = clean-all

用户显式"放弃重新开始"时，删所有实体 memory（保留 common）：

```bash
ae-sdd memory clean-all
```

### 2.5 回归流程 = 先读无则建

回归流程（暂离后回归 / session 重启继续）时，先检查 memory 是否存在：
- **存在** -> 直接读 memory 续接（不重编译）
- **不存在** -> 创建(编译) memory

```bash
# 回归时检查
ae-sdd memory read --entity-type story --entity-id STORY-001-BE
# 若返回 "memory not found" -> 创建
ae-sdd memory create --entity-type story --entity-id STORY-001-BE --sources ...
```

> **回归不属于从0重新开始**，所以不先 clean-all，而是先读无则建。

## 3. common 层管理

common 是**项目级可复用约束**的唯一存储，跨所有子流程保留。

**必须轻，不可臃肿**：
- 只存项目级可复用约束（BigDecimal/幂等/禁大事务/架构规范/公共模板引用）
- 严禁存任何特定 PRD/DR/Story 的细节
- 编译时由 `memory_compiler.extract_common()` 从源上下文提取，自动去重
- 大小硬限制 `COMMON_MAX_CHARS = 2048` 字符，超出截断并告警

```bash
# 读 common
ae-sdd memory common read

# 更新 common（通常由 create_memory 自动完成，手动更新用于维护场景）
ae-sdd memory common update --content-file common-constraints.md

# 强制删除 common（仅在显式重置项目级约束时）
ae-sdd memory common clean
```

## 4. compact slice 说明

每个实体 memory 含 3 个 compact slice + manifest：

| slice | 内容 | 更新时机 |
|---|---|---|
| `boot.compact.md` | 锚点/当前系列/下一步/产物路径表 | 创建时 + 系列切换 + compact 前 snapshot |
| `context.compact.md` | DR锚点/Story AC/约束/接口契约/数据模型/资产引用 | 创建时 + 关键决策落地 |
| `pending.compact.md` | 待决项/失败历史/矫正计数/reviewLoop状态 | 创建时 + 待决项变化 + compact 前 snapshot |
| `manifest.json` | source hash + slice hash + fingerprint | 自动维护 |

compact 遵循"不做密文"原则：仍是可读 Markdown（表格/列表/JSON），不使用私有短码。

## 5. CLI Contract

```bash
# 创建（编译源上下文 -> compact -> 写 memory）
ae-sdd memory create --entity-type story --entity-id STORY-001-BE --sources "name=path ..." --context-json '{...}'

# 读
ae-sdd memory read --entity-type story --entity-id STORY-001-BE

# 更新 slice
ae-sdd memory update --entity-type story --entity-id STORY-001-BE --slice pending --content "..."

# 删单个实体（子流程结束）
ae-sdd memory clean --entity-type story --entity-id STORY-001-BE

# 删所有实体（从0重新开始，保留 common）
ae-sdd memory clean-all

# common 操作
ae-sdd memory common read
ae-sdd memory common update --content-file ...
ae-sdd memory common clean

# 搜索
ae-sdd memory search --query "transaction"

# 统计
ae-sdd memory summarize
```

## 6. 与 document-storage 的关系

| 存储 | 用途 | 生命周期 |
|---|---|---|
| **document-storage**（DR/Story/代码文档） | 业务成果永久存储 | 永久（除非显式归档） |
| **memory**（compact 文档） | 编译后的工作上下文 | 临时（子流程结束删除，common 保留） |

源上下文（document-storage 的 DR/Story/约束）是永久存储，memory 是编译后的工作上下文。
后续子流程需要上游上下文时，读 document-storage 原文 + common -> 重新编译成自己的 memory。

## 7. 过渡期兼容（v3.10.3）

- `memory_gate.py` 改为 passthrough（check_state_transition 永远 pass），批 3 彻底删除。
- `memory_store.locate_scope()` 保留旧参数（phase/story/task）过渡期兼容，内部转换为 entity_type/entity_id。
- `prompt_inject._inject_memory_block()` 改为从 memory 读 compact 注入；memory 不存在时不注入（不报错）。
- `--allow-empty-memory` CLI 参数保留但无效（memory gate 永远 pass）。
- 批 3 重写 prompt_inject/gate_intercept/CLI 后，过渡期兼容层可移除。
