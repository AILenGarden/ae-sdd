---
name: ae-sdd-plugin-loader
description: |
  ae-sdd 三层 SKILL 注册表加载协议（🆕 v3.5.0）。
  在 ae-sdd 主编排层加载任何 SKILL 之前，按"项目层 > 全局层 > 仓库根层 > 内置 fallback"
  的优先级合成三层注册表，决定实际加载路径。Agent 收到 `/ae-sdd` 后涉及 SKILL 路由时
  必须先加载本 SKILL 确认加载协议。
  同时承担"用户注册 CodingSKILL"引导职责：用户说"注册插件 / 注册 SKILL / 外挂 CodingStyle"
  时加载本 SKILL 引导生成 registry.yaml + 外挂 SKILL。
---

# ae-sdd Plugin Loader — 三层 SKILL 注册表加载协议

> **核心职责：** 在 ae-sdd 主编排层加载任何 SKILL 之前，按三层优先级合成注册表，决定实际加载路径。
>
> **双重身份：**
> 1. **对 Agent** —— 加载协议 SOP（按协议执行加载）
> 2. **对母版维护者 / 项目 owner / 个人开发者** —— 注册 CodingSKILL 的流程引导（"你想挂载自己的 CodingSKILL？看这里"）
>
> 注：ae-sdd 母版**不接受**外部 CodingSKILL 贡献 PR——所有 CodingSKILL 定制都在本地或项目层完成。

---

## §1 三层注册表定义

| 层 | 路径 | scope | 典型使用人 | git |
|---|------|-------|-----------|-----|
| **L1 项目层** | `<project>/.ae-sdd/plugins/registry.yaml` | 单项目 | 项目 owner / Tech Lead | ❌ |
| **L2 全局层** | `~/.ae-sdd/plugins/registry.yaml` | 单用户跨项目 | 个人开发者 | ❌ |
| **L3 仓库根层** | `<ae-sdd-master>/plugins/registry.yaml` | ae-sdd 母版 | ae-sdd 团队发布 | ✅ |
| **L0 fallback** | `source/skills/` + `source/templates/` | ae-sdd 母版 | ae-sdd 团队 | ✅ |

**优先级链：** L1 > L2 > L3 > L0。三层都未命中 → fallback 到内置 SKILL（v3.4.x 默认行为，零破坏）。

> **完整定义：** 见 [`source/standards/constraints/plugin-registry-spec.md`](../../standards/constraints/plugin-registry-spec.md)
>
> **设计文档：** 见 [`source/docs/plans/2026-06-26-plugin-registry-design.md`](../../docs/plans/2026-06-26-plugin-registry-design.md)

---

## §2 加载协议 SOP（Agent 用）

### 2.1 加载时机

在 `source/SKILL.md §路由决策算法` step 3（"加载对应 SKILL"）**之前**插入：

```
2.5 【🔌 SKILL 注册表加载】(🆕 v3.5.0)

   加载目标 SKILL = S（如 coding-skill.md）：

   1. 调用 crates/ae-sdd-integrations/src/jobs/plugin 的 resolve_skill(S, ade_sdd, master) API：
      └─ 该函数会按 §2.2 优先级链合成三层注册表
      └─ 命中任何一层 → 返回该层指向的 resolved_path
      └─ 三层都未命中 → fallback 到内置 source/skills/... 路径
      └─ 多层冲突 → 按优先级选胜者 + 警告

   2. AI Agent 读取返回的 resolved_path 指向的 SKILL 文档：
      └─ 命中某层 → 读外挂 SKILL
      └─ fallback → 读内置 SKILL（与 v3.4.x 行为一致）

   3. （可选）记录 trace 到对话日志：
      └─ "已加载 coding-skill from L1 项目层（.ae-sdd/plugins/registry.yaml）"
      └─ 或 "已加载 coding-skill from L0 内置 fallback"
```

### 2.2 三层优先级合成算法

```
加载 SKILL S 时：

1. 收集所有层的注册表（缺失层跳过，不报错）：
   ┌─ L1 项目层  ─ .ae-sdd/plugins/registry.yaml
   ├─ L2 全局层  ─ ~/.ae-sdd/plugins/registry.yaml
   ├─ L3 仓库根层 ─ <master>/plugins/registry.yaml
   └─ L0 内置   ─ source/skills/...

2. 收集所有匹配 S 的 plugin（按 replaces 或 provides）：
   ├─ 命中 → 加入候选列表
   └─ 未命中 → 不进候选

3. 候选按 layer 排序（数字越小优先级越高）：
   ├─ L1 = 1, L2 = 2, L3 = 3
   └─ 第一个（layer 最小）= 胜者

4. 候选为空：
   └─ fallback 到 L0 内置 SKILL

5. 候选 > 1（多层冲突）：
   └─ 按优先级选胜者 + 🟡 输出冲突告警（不阻断）
   └─ 告警内容："plugin 'X' (L1) 与 ['Y' (L2), 'Z' (L3)] 都覆盖了 'S'；L1 胜出"
```

### 2.3 fallback 默认行为

三层注册表**全部缺失** → 视为未启用插件 → 行为与 v3.4.x 完全一致（读内置 SKILL）。**零破坏保证**。

### 2.4 加载失败处理

| 失败场景 | 行为 |
|---------|------|
| 注册表 YAML 语法错误 | 🔴 阻断 + 报错到对话（指出错误层 + 错误位置）|
| replaces 路径不存在 | 🔴 阻断 + 报错（防止伪注册） |
| path 路径不存在 | 🔴 阻断 + 报错 |
| compatibility 不满足 | 🟡 警告 + 仍加载（不阻断，但告知） |
| 多层冲突 | 🟡 警告 + 按优先级选胜者 |

---

## §3 注册流程引导（母版维护者 / 项目 owner / 个人开发者 用）

> **触发场景：** **母版维护者** / **项目 owner** / **个人开发者** 说"注册插件"、"注册 SKILL"、"外挂 CodingStyle"、"项目 CodingSKILL 怎么定制"、"覆盖内置 SKILL"。
>
> **Agent 加载本 SKILL 后按以下流程引导用户：**

### 3.1 Step 1 — 确认注册层 + 使用方身份

**Agent 主动询问使用方身份**（不是社区贡献者——ae-sdd 母版不接收外部 SKILL 贡献）：

| 使用方 | 推荐层 | 路径 |
|-------|-------|------|
| **项目 owner / Tech Lead** 团队约定（项目成员共享）| **L1 项目层** | `<project>/.ae-sdd/plugins/registry.yaml` |
| **个人开发者** 跨项目偏好（如"我所有项目都用 TDD"）| **L2 全局层** | `~/.ae-sdd/plugins/registry.yaml` |
| **ae-sdd 母版维护者** 发布官方扩展 | **L3 仓库根层** | `<ae-sdd-master>/plugins/registry.yaml` |

**默认推荐 L1**（项目 owner 是核心使用方，跟 ae-sdd 强项目级绑定一致）。

### 3.2 Step 2 — 生成注册表

**运行**：
```bash
# 项目层
ae-sdd plugin init --layer project
# 全局层
ae-sdd plugin init --layer global
```

**生成内容**（拷贝自 `source/templates/project-assets/plugin-registry-template.yaml`）：
- schema_version: 1
- description: 用户填写
- plugins: 插件清单（先用注释示例填充，让用户改）

**用户填字段**：
- `name`（必填，kebab-case）
- `type`（必填，4 选 1）
- `version`（必填，semver）
- `description`（必填）
- `path`（必填，相对注册表所在目录）
- `replaces`（skill-override / template-override 必填）
- `provides`（skill-new / template-new 必填）

### 3.3 Step 3 — 写外挂 SKILL

**用户在外挂 path 指向的位置写 SKILL 文档**（同内置 SKILL 格式）：
- `name` (frontmatter)
- `description` (frontmatter)
- 正文（Markdown）

**示例：**
```yaml
# registry.yaml
plugins:
  - name: my-coding-style
    type: skill-override
    version: 0.1.0
    description: my TDD + DDD coding
    replaces: source/skills/phase2-coding/coding-skill.md
    path: ./my-coding/SKILL.md
```

```markdown
<!-- my-coding/SKILL.md -->
---
name: my-coding-style
description: 团队 TDD + DDD CodingSKILL
---

# My Coding Style (TDD + DDD)

（具体 CodingSKILL 内容）
```

### 3.4 Step 4 — 验证

**运行**：
```bash
# 校验三层注册表 + 每个 plugin sanity check
ae-sdd plugin validate

# 查看某 SKILL 的加载路径
ae-sdd plugin trace coding-skill.md
```

**预期输出**（validate 通过）：
```
🔌 ae-sdd Plugin Registry (1 plugin loaded)

L1 项目层 (.ae-sdd/plugins/registry.yaml): 1 plugin
  ✅ my-coding-style v0.1.0 → overrides source/skills/phase2-coding/coding-skill.md

✅ 校验通过
```

**预期输出**（trace）：
```
🔍 trace: source/skills/phase2-coding/coding-skill.md
  → 命中 L1-project: my-coding-style
  → resolved: /path/to/.ae-sdd/plugins/my-coding/SKILL.md
```

### 3.5 Step 5 — 测试

**运行实际流程**：
- 用户说"开始 Coding" → 触发 coding-skill 加载
- Agent 加载本 SKILL → 按 §2.1 SOP 调 plugin loader
- 命中 L1 → 读外挂 SKILL
- 流程继续

**如果未生效**：
- `ae-sdd plugin trace coding-skill.md` 看是不是真的命中
- `ae-sdd plugin validate` 看是不是有错

---

## §4 CLI 命令

### 4.1 `ae-sdd plugin list`

列出所有已注册插件（合并三层 + 冲突检测）。

### 4.2 `ae-sdd plugin validate`

校验三层注册表 + 每个 plugin 的 sanity check（path 存在 / frontmatter 完整 / replaces 目标存在）。

### 4.3 `ae-sdd plugin trace <target>`

查看某 SKILL（`replaces` 内置路径或 `provides` key）的加载路径 + 命中层 + 冲突告警。

### 4.4 `ae-sdd plugin init --layer {project|global}`

从模板生成新注册表（项目层或全局层）。

> **CLI 实现状态：** 解析实现在 `crates/ae-sdd-integrations/src/jobs/plugin`；`plugin list/trace/validate` 子命令已挂载。

---

## §5 与其他 SKILL 的关系

| SKILL | 关系 |
|-------|------|
| [`source/SKILL.md`](../../SKILL.md) | 主编排层，§路由决策算法 step 2.5 调用本 SKILL |
| [`source/skills/orchestration/ae-sdd-update-skill.md`](../orchestration/ae-sdd-update-skill.md) | 修改本 SKILL 时由 update-skill 引导 |
| [`source/standards/constraints/plugin-registry-spec.md`](../../standards/constraints/plugin-registry-spec.md) | schema 权威文档（本 SKILL 内容指针到这里）|
| [`source/docs/plans/2026-06-26-plugin-registry-design.md`](../../docs/plans/2026-06-26-plugin-registry-design.md) | 设计文档（本 SKILL 内容指针到这里）|
| [`source/templates/project-assets/plugin-registry-template.yaml`](../../templates/project-assets/plugin-registry-template.yaml) | 注册表模板（init 命令从这个拷贝） |

---

## §6 已知缺口 / 留待下个 PR

1. **`extends` 类型章节级合并未实现** —— v3.5.0 loader 把 `type=skill-extends` 当 `skill-override` 处理（整体替换）。完整实现留待 v3.6.0。
   > **🆕 v3.6.1 替代方案已落地**：语言/项目编码适配器的"叠加"不走未实现的 skill-extends，改由**共有 [`coding-skill.md` §13 注册加载协议](../phase2-coding/coding-skill.md)** 用现有 `skill-new` 机制实现——适配器以 `provides: coding-adapter-{lang}` 注册，AI 运行时读共有 + 适配器两份文件叠加。母版 L3 已注册首例 [`java3d-coding-skill`](../../plugins/registry.yaml)（`plugins/registry.yaml`）。
2. **依赖解析** —— dependencies 字段声明但 loader 不强制校验；v3.5.0 仅做提示。
3. **缓存** —— 每次加载 SKILL 重新读注册表；高频场景需要缓存。留待性能 profiling 后优化。
4. **GUI 化注册向导** —— CLI `plugin init` 是最小可用；后续考虑加交互式向导。

---

## §7 实施历史

- **v3.5.0（2026-06-26）**：新建本 SKILL。完成 plugin loader + 单元测试 + 注册表模板 + 设计文档 + schema 规范。CLI 子命令留待 v3.5.1。