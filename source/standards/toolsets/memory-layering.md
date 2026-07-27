# ae-sdd Memory Layering Standard (🆕 v3.10.3 Entity-Tree)

## 1. Purpose

ae-sdd memory 存储编译后的 compact 文档，按业务实体树分层。memory 是子流程Agent
的工作上下文容器，不是原文索引或日志。

**v3.10.3 核心变化**：从"5层原文索引 + enter/exit 生命周期门禁"重构为
"业务实体树 + 编译文档容器"。

## 2. 业务实体树分层

memory 按业务实体平级分层，不按 phase 维度分：

| 实体类型 | 目录 | 归属子流程 | 生命周期 |
|---|---|---|---|
| `common` | `memory/common/default/` | 跨所有子流程 | 永久保留（跨子流程复用） |
| `prd` | `memory/prd/{PRD-ID}/` | RA/PRD 子流程 | 子流程结束删除 |
| `dr` | `memory/dr/{DR-ID}/` | DR 子流程 | 子流程结束删除 |
| `story` | `memory/story/{Story-ID}/` | Story 子流程 | 子流程结束删除 |
| `testcase` | `memory/testcase/{Story-ID}/` | TestCase 子流程 | 子流程结束删除 |
| `coding` | `memory/coding/{Story-ID}/` | Coding 子流程 | 子流程结束删除 |

**隔离规则**：
- 每个实体独立目录，互不干扰。
- 同层按任务隔离：PRD 有 PRD 的独立上下文，DR 有 DR 的独立上下文，Story 有 Story 的独立上下文。
- 一个 PRD 内有多个 DR，DR 内有多个 Story -- 各自独立 memory 目录。

**common 层**：
- 只存项目级可复用约束（BigDecimal/幂等/禁大事务/架构规范）。
- 必须轻，不可臃肿（`COMMON_MAX_CHARS = 2048` 字符硬限制）。
- 严禁存任何特定 PRD/DR/Story 的细节。
- 编译时由 `memory_compiler.extract_common()` 自动提取，去重。
- 子流程结束后 common 保留给后续子流程使用。

## 3. 子流程生命周期

### 3.1 启动 = 创建（编译）

子流程Agent 首次进入时，主动读源上下文（从 document-storage）-> 编译成 compact -> 写 memory。

### 3.2 执行中 = 读/更新 memory

- 读上下文 = 读 memory 的 compact 文档（不再喂源文档给 LLM）。
- 关键决策落地 / 待决项变化 -> 增量更新对应 slice。
- compact 前 -> snapshot 到 memory（`pre_compact_snapshot`）。

### 3.3 结束 = 删除自己的 memory

子流程Agent 交付物回传后，删自己的 memory（删临时上下文，留业务成果）。common 保留。

### 3.4 从0重新开始 = clean-all

用户显式"放弃重新开始"时，删所有实体 memory（保留 common）。

### 3.5 回归流程 = 先读无则建

回归流程（暂离后回归 / session 重启继续）时：先检查 memory 是否存在 -> 有则读，无则建。
**回归不属于从0重新开始**，不先 clean-all。

## 4. compact slice 结构

每个实体 memory 含 3 个 compact slice + manifest：

| slice | 文件 | 内容 |
|---|---|---|
| boot | `boot.compact.md` | 锚点/当前系列/下一步/产物路径表 |
| context | `context.compact.md` | DR锚点/Story AC/约束/接口契约/数据模型/资产引用 |
| pending | `pending.compact.md` | 待决项/失败历史/矫正计数/reviewLoop状态 |
| manifest | `manifest.json` | source hash + slice hash + fingerprint（校验用） |

**设计原则**（与 runtime 编译器一致）：
- 不做密文：compact 仍是可读 Markdown（表格/列表/JSON），不使用私有短码。
- 高密度：去水词、表格化、引用符号化。
- 确定性：同一输入编译两次结果完全一致（无时间戳/随机数）。

## 5. 旧分层废弃（v3.10.3）

以下旧分层概念已废弃：

| 旧概念 | 废弃原因 | 替代 |
|---|---|---|
| L0 scratch（会话草稿） | 混入事件流，不注入上下文 | 废弃，事件流不需要 memory 存储 |
| L1 task（Story/task 级） | 按 phase 分文件，无 PRD/DR 维度 | story/testcase/coding 实体目录 |
| L2 project（项目级） | 无 story 维度，全共享 | common 层（有大小限制） |
| L3 pattern（跨项目） | 非"上下文管理"用途 | 迁到项目资产（document-storage 约束类） |
| L4 archive（冷归档） | 非"上下文管理"用途 | 迁到独立归档目录 `.ae-sdd/archive/` |
| enter/exit 生命周期门禁 | 新体系用创建/删除管理生命周期 | create_memory / clean_memory |
| JSONL 原文索引 | 无编译，原文短索引 | compact.md 编译文档 |
| `_validate_compact_memory` 校验 | 针对原文索引 | 编译器保证 compact 质量 |
| `promote` 跨层提升 | 5层体系废弃 | common 自动提取 |

## 6. 与 document-storage 的关系

| 存储 | 用途 | 生命周期 |
|---|---|---|
| document-storage（DR/Story/代码文档） | 业务成果永久存储 | 永久 |
| memory（compact 文档） | 编译后的工作上下文 | 临时（子流程结束删除，common 保留） |

源上下文是永久存储，memory 是编译后的工作上下文。后续子流程需要上游上下文时，
读 document-storage 原文 + common -> 重新编译成自己的 memory。

## 7. 过渡期兼容

- memory 门禁在状态迁移上为 passthrough（永远 pass）。
- memory scope 定位保留旧参数（phase/story/task）过渡期兼容。
- `--allow-empty-memory` CLI 参数保留但无效。
