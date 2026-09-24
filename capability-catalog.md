# ae-sdd 能力清单

> 版本：0.1.0
> 更新日期：2026-09-11
> 状态：重做基线

## 1. 目标

本清单定义 `ae-sdd` 当前托管的最小能力集合。它是能力发现和调用边界说明，不是流程状态机、Gate 注册表或 daemon 配置。

## 2. 能力总览

| 能力 ID | 主要职责 | 典型输出 | 可独立调用 |
| --- | --- | --- | --- |
| `al-ra` | 需求理解、澄清、事实与边界分析 | Requirements Analysis | 是，推荐作为新需求入口 |
| `al-spec` | 编写和审查类型化 Spec | DR、Story、TestCase 等 Spec | 是 |
| `al-coding` | 编码设计、实现、代码审查和实现验证 | 代码、变更说明、验证结果 | 是，前提是输入充分 |
| `al-knowledge` | ae-sdd 内置：查询、构建、核查和维护项目知识 | OKF 概念、来源证据、工程关系与派生视图 | 经 ae-sdd 调用 |
| `db-operator` | 严格只读数据库查询和结构检查 | 查询结果、Schema 事实、数据库证据 | 是 |

### 运维入口

| 管理流程 | 职责 | 参考文档 |
| --- | --- | --- |
| 安装/更新 | 统一安装、更新入口及能力；复用已有独立安装 | `ae-sdd/references/install.md` |
| 卸载 | 按安装记录卸载托管文件；保留独立能力和用户修改 | `ae-sdd/references/uninstall.md` |

它们是参考文档中的管理流程，不注册为 Skill 或插件。用户统一使用 ae-sdd；插件只暴露该入口，并随包携带 al-knowledge 的内部说明、脚本和模板，不要求单独安装或调用知识 Skill。其他四项能力保留现有安装方式，由 ae-sdd 选择调用。

各模块的唯一维护源位于 `D:\al-agent-workspace\al-skills`，路径由 registry.yaml 显式登记。al-knowledge 的源码保持独立目录；构建器 `scripts/bundle_knowledge.py` 将其生成到 `skills/ae-sdd/capabilities/al-knowledge/`，内部入口为 `CAPABILITY.md`。生成副本与其归属清单纳入插件，禁止手工双维护；发布前执行构建器 `--verify`。

安装和卸载共用 `ae-sdd/references/installation-contract.md` 中的来源映射、归属记录、更新和恢复合同。两者先展示具体 dry-run；已有授权覆盖操作和目标时继续执行，仅对未决范围或破坏性冲突请求决定。安装记录只说明哪些文件由本套件部署，不记录研发任务状态。安装后的清单位于 `ae-sdd/references/capability-catalog.md`，由本文件复制生成。

安装/更新通过 `scripts/manage_agents.py` 将 `references/agents-guidance.md` 中的 ae-sdd 入口写入明确选定的 AGENTS.md 及登记镜像，记录标记与哈希；卸载先移除该托管段落，再移除插件。通用 ALSDD、Git 和其他用户规则不归插件所有。未标记段落默认保留，只有明确授权且原段落摘要匹配时才接管；标记/哈希冲突保留并报告。联动由 ae-sdd 安装/卸载流程执行，单独的宿主插件按钮或 CLI 不运行此流程。

## 3. 能力边界

### al-ra

- 负责：目标、范围、角色、行为、规则、数据流、约束、异常、风险和开放问题。
- 方式：与用户持续交互；关键事实未经确认不得当作最终需求。
- 不负责：替用户决定实现细节，不负责修改业务代码。

### al-spec

- 负责：依据已确认需求和项目现状撰写或审查 Spec。
- Spec 类型：DR、Story、TestCase，以及后续明确登记的类型。
- 不负责：把 Spec 类型强制串成固定流水线，不负责中央流程编排。

### al-coding

- 负责：实现方案、代码修改、实现级 Review、测试和结果说明。
- 前提：应明确输入 Spec、需求事实或用户授权的直接修改范围。
- 不负责：维护全局任务状态或替代项目知识库。

### al-knowledge

- 负责：项目技术栈、业务语义、架构理由、实现惯例，以及 Spec、接口和调用链的证据关系；支持查询、核查、增量维护和影响分析。
- 存储：OKF v0.2 概念文档及 ae-sdd 工程 profile；索引和追溯树派生。独立调用其他能力可按需消费，缺库直接回源，不强制先建库。
- 归属：ae-sdd 内置能力；详细存储模型和模板由 al-knowledge 模块维护，ae-sdd 按任务加载包内资源。
- 不负责：代写 Spec 或直接修改业务实现。

### db-operator

- 负责：注册数据库的只读查询、Schema 检查、`SHOW`/`DESCRIBE`/安全 `EXPLAIN` 和事实提取。
- 安全边界：拒绝写入、管理操作、凭据访问和绕过安全客户端的直接连接。
- 输出：可被 RA、Spec、Coding 或 Knowledge 消费的数据库事实和证据。

## 4. 组合建议

这些是建议，不是强制流程：

```text
新需求：       al-ra -> al-spec? -> al-coding? -> al-knowledge?
架构调整：     al-ra -> al-spec:dr -> al-coding? -> al-knowledge
行为需求：     al-ra -> al-spec:story -> al-coding? -> al-knowledge
测试设计：     al-ra -> al-spec:testcase
小型修复：     al-ra -> al-coding
数据库调查：   db-operator -> al-ra/al-spec/al-coding/al-knowledge
```

Agent 可以跳过不适用能力；用户只需使用 ae-sdd，由入口按上下文选择能力，不要求逐个报出内部能力名。

## 5. 共同输入原则

能力调用优先读取：

1. 用户提供的当前任务和约束；
2. 项目现有文档与代码；
3. `.al-knowledge` 中的项目事实；
4. 其他能力已经产生并明确引用的产物；
5. `db-operator` 返回的数据库事实。

缺少关键上下文时，应说明缺口并请求补充，不得用猜测填充。

## 6. 明确不属于当前能力清单

当前不保留以下独立能力或基础设施：

- daemon、Gate、门禁、phase、route 状态机；
- 固定流程编排器、中央事件总线、全局任务注册表；
- 独立 Docs、Review、Test、Report、Evidence、Git、Memory、Hook 能力；
- 统一上下文代理或全局项目缓存。

Review、Test 和报告可以由 `al-coding` 或 Agent 在具体任务中完成；项目知识由 `al-knowledge` 维护。
