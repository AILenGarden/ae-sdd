# Plugin Registry 设计方案（🆕 v3.5.0 — 2026-06-26）

> **定位：** 把 ae-sdd 当前 hard-coded 的 SKILL 体系升级为"内置 + 三层可外挂注册"的插件化框架。设计哲学：**渐进增强 + 三层分级 + 强制 fallback**。
>
> **为什么需要：** 当前 `source/skills/` 是固定的内置 SKILL 集合，"每个人的 CodingSKILL 都不同"只能 fork 母版改——成本高、PR 阻塞、生态分裂。本方案让项目 owner / 个人开发者 / ae-sdd 团队**各自在自己的 scope 内定制**，互不污染。
>
> **v3.5.0 起，本文件即 plugin registry 体系的唯一权威设计文档。**

---

## 一、设计目标

| # | 目标 | 验收标准 |
|---|------|---------|
| 1 | **可外挂注册** | 任何 SKILL（内置节点或新增节点）可被项目 / 全局 / 仓库根三层覆盖或新增 |
| 2 | **三层分级** | 不同 scope 的人在自己的层定制，互不干扰：ae-sdd 团队 → 仓库根层；项目 owner → 项目层；个人开发者 → 全局层 |
| 3 | **强制 fallback** | 三层都未注册 → 自动 fallback 到内置 SKILL，保证 ae-sdd 默认行为不变 |
| 4 | **可观测** | 任何 SKILL 加载都能追溯"从哪层注册表来"（`ae-sdd plugin trace <SKILL>`） |
| 5 | **可校验** | 注册时自动跑 sanity check（path 存在 / frontmatter 完整 / replaces 目标存在），防止伪注册 |
| 6 | **零破坏** | 未启用插件的项目行为与 v3.4.3 完全一致（向后兼容） |

---

## 二、三层优先级链

### 2.1 三层定义

| 层 | 注册表路径 | scope | 典型使用人 | 是否 git tracked |
|---|-----------|-------|-----------|----------------|
| **L1 项目层** | `<project>/.ae-sdd/plugins/registry.yaml` | 单项目 | 项目 owner / Tech Lead | ❌（项目本地，不入仓） |
| **L2 用户全局层** | `~/.ae-sdd/plugins/registry.yaml` | 单用户跨项目 | 个人开发者（"我的 CodingSKILL 偏好") | ❌（per-user，不入仓） |
| **L3 仓库根层** | `<ae-sdd-repo>/plugins/registry.yaml` | ae-sdd 母版 / 团队发布 | ae-sdd 维护者（官方扩展） | ✅（git tracked，仅 scaffolding） |
| **L0 内置 fallback** | `source/skills/` + `source/templates/` | ae-sdd 母版 | ae-sdd 团队 | ✅（git tracked，SSOT） |

### 2.2 优先级合成算法

```
加载 SKILL S 时：

1. 收集所有层的注册表（缺失层跳过，不报错）：
   ┌─ L1 项目层  ─ .ae-sdd/plugins/registry.yaml
   ├─ L2 全局层  ─ ~/.ae-sdd/plugins/registry.yaml
   ├─ L3 仓库根层 ─ <master>/plugins/registry.yaml
   └─ L0 内置   ─ source/skills/...

2. 从 L1 到 L3 依次查询 S 的覆盖记录（replaces 或 provides）：
   ├─ 命中 → 加载该层的 path 指向的 SKILL 文档
   └─ 未命中 → 继续下一层

3. 三层都未命中：
   └─ fallback 到 L0 内置 SKILL（v3.4.3 默认行为）

4. 多层同时覆盖同一 S（冲突）：
   └─ 按优先级选胜者 + 🟡 输出冲突告警（不阻断，但记录在 plugin_loader 日志）
       例：项目层 "我的 CodingStyle" 覆盖了 coding-skill.md
           全局层 "个人 CodingStyle" 也覆盖了 coding-skill.md
           → 加载项目层（胜出），全局层被忽略 + 告警
```

### 2.3 优先级举例

```
场景 A：项目 owner 在 .ae-sdd/plugins/registry.yaml 注册 my-coding-style 替换 coding-skill.md
   └─ 加载 coding-skill 时 → 命中 L1 → 加载外挂版
   └─ 不管 L2/L3 怎么写，都被 L1 屏蔽

场景 B：个人开发者只写全局层 ~/.ae-sdd/plugins/registry.yaml
   └─ 该用户的任意项目加载 coding-skill 时 → 命中 L2 → 加载外挂版
   └─ 切换项目时无需重新配置

场景 C：ae-sdd 团队发版时更新仓库根 plugins/registry.yaml
   └─ 所有用户安装新版本后 → 命中 L3 → 加载团队版
   └─ 用户可在自己层覆盖团队版
```

### 2.4 覆盖类型语义

| 类型 | 含义 | 用途 |
|------|------|------|
| `replaces` | 1:1 整体替换内置 SKILL 或模板 | 项目定制整套 CodingSKILL |
| `extends` | 在内置 SKILL 基础上**追加**章节（不替换主体） | 项目附加"团队约定"段 |
| `provides` | **新增**一个 SKILL（不替换任何内置项） | 项目专属 SKILL（如 `coding-skill-finance.md`） |

---

## 三、注册表 schema

### 3.1 顶层 schema

```yaml
# registry.yaml v1
schema_version: 1          # 必填，固定 "1"
description: |             # 可选，注册表说明（多行）
  <项目 / 用户 / 团队> 的 ae-sdd 插件注册表

plugins:                   # 必填，插件清单
  - name: <plugin-name>   # 必填，全局唯一（建议 kebab-case）
    type: <plugin-type>    # 必填：skill-override | template-override | skill-new | template-new
    version: <semver>      # 必填，semver（如 0.1.0）
    author: <name>         # 可选
    description: <text>    # 必填，一句话说明
    
    # ── 覆盖 / 新增 ──
    # type=skill-override / template-override 时：
    replaces: <path-to-builtin-skill>      # 必填，被覆盖的内置路径
    path: <path-to-plugin-skill>           # 必填，外挂 SKILL 路径（相对注册表所在目录）
    
    # type=skill-new / template-new 时：
    # provides: <skill-key>                 # 必填，新增 SKILL 的引用 key
    # path: <path-to-plugin-skill>          # 必填
    
    # ── 元数据 ──
    compatibility:                          # 可选
      ae_sdd_version: ">=3.5.0"            # semver range
    dependencies:                           # 可选，依赖其他插件
      - <other-plugin-name>
    tags: [team-style, finance]            # 可选，分类标签
```

### 3.2 type 枚举

| 值 | 含义 | replaces 必填 | path 必填 | provides 必填 |
|---|------|---------------|-----------|---------------|
| `skill-override` | 覆盖内置 SKILL | ✅ | ✅ | ❌ |
| `template-override` | 覆盖内置模板 | ✅ | ✅ | ❌ |
| `skill-new` | 新增 SKILL | ❌ | ✅ | ✅ |
| `template-new` | 新增模板 | ❌ | ✅ | ✅ |

### 3.3 path 解析规则

- path 是**相对路径**，相对注册表所在目录
- 支持子目录引用：`./plugins/my-coding/SKILL.md`
- 禁止 `..` 跳出注册表所在目录（防止越权）
- 路径分隔符 Windows/Linux 都用 `/`（YAML 跨平台约定）

### 3.4 完整示例

```yaml
schema_version: 1
description: icec-cloud-boss 项目 CodingSKILL 定制

plugins:
  # 例 1：覆盖内置 CodingSKILL
  - name: boss-coding-style
    type: skill-override
    version: 0.1.0
    author: "EDY"
    description: boss 项目团队的 CodingSKILL 约定（TDD + DDD 风格）
    replaces: source/skills/phase2-coding/coding-skill.md
    path: ./plugins/boss-coding/SKILL.md
    compatibility:
      ae_sdd_version: ">=3.5.0"
    tags: [team-style, tdd, ddd]
  
  # 例 2：覆盖内置模板
  - name: boss-codingplan-template
    type: template-override
    version: 0.1.0
    description: boss 项目 CodingPlan 模板（加重试/幂等章节）
    replaces: source/templates/coding/coding-plan-template.md
    path: ./plugins/boss-coding/coding-plan.md
  
  # 例 3：新增 SKILL（项目专属）
  - name: boss-finance-coding
    type: skill-new
    version: 0.1.0
    description: 财务领域 CodingSKILL（精度/舍入/对账）
    provides: boss-finance-coding-skill
    path: ./plugins/boss-finance/SKILL.md
    dependencies: [boss-coding-style]   # 依赖例 1
```

---

## 四、加载协议（Agent SOP）

### 4.1 加载时机

在 `source/SKILL.md §路由决策算法` step 3（"加载对应 SKILL"）**之前**插入：

```
2.5 【🔌 SKILL 注册表加载】(🆕 v3.5.0)

   加载目标 SKILL = S（如 coding-skill.md）：

   1. 调用 tools/lib/plugin_loader.py 的 load_skill(S) API：
      └─ 该函数会按 §2.2 优先级链合成三层注册表
      └─ 命中任何一层 → 返回该层指向的 path
      └─ 三层都未命中 → fallback 到内置 source/skills/... 路径

   2. AI Agent 读取返回的 path 指向的 SKILL 文档：
      └─ 路径已替换为外挂版 → 读外挂 SKILL
      └─ fallback → 读内置 SKILL（与 v3.4.3 行为一致）

   3. （可选）记录 trace 到对话日志：
      └─ "已加载 coding-skill from L1 项目层（.ae-sdd/plugins/registry.yaml）"
```

### 4.2 fallback 默认行为

三层注册表**全部缺失** → 视为未启用插件 → 行为与 v3.4.3 完全一致（读内置 SKILL）。**这是零破坏保证**。

### 4.3 加载失败处理

| 失败场景 | 行为 |
|---------|------|
| 注册表 YAML 语法错误 | 🔴 阻断 + 报错到对话（指出错误层 + 错误位置）|
| replaces 路径不存在 | 🔴 阻断 + 报错（防止伪注册） |
| path 路径不存在 | 🔴 阻断 + 报错 |
| compatibility 不满足 | 🟡 警告 + 仍加载（不阻断，但告知） |
| 多层冲突（同一 target 被覆盖多次）| 🟡 警告 + 按优先级选胜者 |

---

## 五、CLI 工具命令（🔌 新增 plugin 子命令）

### 5.1 命令列表

```bash
# 查看所有已注册插件（合并三层）
ae-sdd plugin list

# 校验三层注册表 + 每个插件 sanity check
ae-sdd plugin validate

# 查看某 SKILL 的加载路径（含三层合并 trace）
ae-sdd plugin trace <skill-key>

# 从模板生成新注册表（项目层）
ae-sdd plugin init --layer project

# 从模板生成新注册表（用户全局层）
ae-sdd plugin init --layer global
```

### 5.2 输出格式示例（`plugin list`）

```
🔌 ae-sdd Plugin Registry (3 plugins loaded)

L1 项目层 (.ae-sdd/plugins/registry.yaml): 1 plugin
  ✅ boss-coding-style v0.1.0 → overrides source/skills/phase2-coding/coding-skill.md

L2 全局层 (~/.ae-sdd/plugins/registry.yaml): 1 plugin
  ✅ personal-tdd-style v0.2.0 → overrides source/skills/phase2-coding/coding-skill.md
  ⚠️ 与项目层 boss-coding-style 冲突 → 项目层胜出

L3 仓库根层 (<master>/plugins/registry.yaml): 1 plugin
  ✅ official-coding-v2 v0.1.0 → extends source/skills/phase2-coding/coding-skill.md
```

---

## 六、Loader SKILL 形态

### 6.1 文件位置

- 文档：`source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md`（独立 SKILL 包，与 review-loop-skill 平级）
- 实现：`tools/lib/plugin_loader.py`（Python 模块，CLI 调用 + 未来其他 runtime 调用）

### 6.2 SKILL 内容大纲

```
# ae-sdd-plugin-loader — SKILL 注册表加载协议

> 核心职责：在 ae-sdd 主编排层加载 SKILL 之前，按三层优先级合成注册表，决定实际加载路径。

## §1 三层定义（pointer 到 plugin-registry-spec.md）
## §2 优先级合成算法（pointer 到 §2.2）
## §3 加载协议 SOP（pointer 到 §4.1）
## §4 失败处理（pointer 到 §4.3）
## §5 CLI 命令使用（pointer 到 §5）
## §6 与其他 SKILL 关系
```

### 6.3 注册 SKILL 引导流程（给用户 + Agent 看的）

`source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md` 同时承担两个角色：
1. **对 Agent**：加载协议 SOP（按协议执行加载）
2. **对用户**：注册流程引导（"你想注册一个 CodingSKILL？看这里"）

README.md 也独立写一份简版注册指南（用户文档）。

---

## 七、与现有体系的关系

| 现有体系 | 关系 |
|---------|------|
| G-00 项目资产门卫 | **正交**：G-00 检查项目资产，plugin loader 检查插件注册表 |
| G-DOC-STORAGE 文档落地门卫 | plugin SKILL 文档本身也要走 G-DOC-STORAGE（如果项目内） |
| `source/skills/` 子 SKILL | 注册表 replaces 时指向它们，作为 fallback 默认值 |
| `source/templates/` 模板 | 注册表 replaces 时也可指向它们（同上） |
| `update-graph.json` UG-08 | 任何 source/ 改动触发；本方案新增 UG-12 专门管注册表/loader |
| dist/ae-sdd/ | dist 是 ae-sdd 母版的分发包；插件不入 dist（用户层和项目层都不进 dist） |
| `plugins/`（仓库根）| v3.4.3 起空着；本方案用作 L3 仓库根层注册表位置 + scaffolding 示例 |

---

## 八、已知缺口 / 留待下个 PR

1. **extends 类型的合并算法未实现**：当前 loader 只支持 replaces（整体替换）和 provides（新增）；extends（追加章节）需要更复杂的章节合并逻辑——v3.5.0 只在 schema 中允许声明 `type: skill-extends`，但 loader 仍按"整体替换"处理。完整实现留待 v3.6.0。
2. **依赖解析**：dependencies 字段已声明但 loader 不强制校验；v3.5.0 只做提示。
3. **热更新**：当前每次加载 SKILL 都要重新读注册表；高频场景需要缓存。留待性能 profiling 后优化。
4. **GUI 化注册向导**：CLI `plugin init` 是最小可用；后续考虑加交互式向导。

---

## 九、实施清单

详见 todos 跟踪：

| # | 文件 | 类型 | 优先级 |
|---|------|------|--------|
| 1 | `source/docs/plans/2026-06-26-plugin-registry-design.md` | 新增（本文件） | P0 |
| 2 | `source/standards/constraints/plugin-registry-spec.md` | 新增 | P0 |
| 3 | `source/templates/project-assets/plugin-registry-template.yaml` | 新增 | P0 |
| 4 | `tools/lib/plugin_loader.py` | 新增 | P0 |
| 5 | `tools/tests/test_plugin_loader.py` | 新增 | P0 |
| 6 | `source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md` | 新增 | P0 |
| 7 | `source/SKILL.md` §🔌 SKILL 注册表加载 + version 3.4.3→3.5.0 | 改动 | P0 |
| 8 | `source/standards/update-graph.json` UG-12 | 改动 | P0 |
| 9 | `plugins/_example-coding-style/SKILL.md` + `README.md` | 新增 | P1 |
| 10 | `README.md` ## 🔌 SKILL 注册与外挂指南 + 版本行 | 改动 | P0 |
| 11 | `tools/lib/paths.py` MASTER_VERSION 更新 | 改动 | P0 |
| 12 | `source/CHANGELOG/2026-06-26-plugin-registry.md` | 新增 | P1 |

---

## 十、版本策略

- **v3.5.0**（本次新增）
- 兼容性：v3.4.x 行为完全保留（fallback 默认）
- 破坏性变更：无