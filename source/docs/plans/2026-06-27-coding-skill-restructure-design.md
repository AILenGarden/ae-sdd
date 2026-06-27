# coding-skill 架构重构修订设计（v3.6.0 草案）

> **版本：** v3.6.0 草案（待评审）
> **日期：** 2026-06-27
> **作者：** 陈聪（用户驱动）+ ZCode（执笔）
> **状态：** 设计阶段，未落地
> **关联：**
> - 现状依据：`source/skills/phase2-coding/coding-skill.md`（2264 行，三类内容混合）
> - 已有插件机制：`source/standards/constraints/plugin-registry-spec.md`（v3.5.0，覆盖式）
> - 已有约束：`source/standards/constraints/*.md`（8 个固定文件名，Java/Spring 专用）
> - 思维引擎：`source/standards/thinking/be-coding-thinking-engine.md`（技术栈无关）

---

## 0. 一句话目标

把今天的 `coding-skill.md` 从"一个文件塞三件事"拆成**三件事、两层注册表**：
- SDD 流程归 SDD（`coding-sdd-skill.md`）
- Coding 能力归 Coding（`coding-skill.md`，技术栈无关 + 双注册表合并加载）
- 约束归约束（Java 技术栈约束从 ae-sdd 母版默认内置，可被项目层约束注册表覆盖/扩展）

---

## 1. 问题陈述（为什么要拆）

### 1.1 现状：一个文件 2264 行，三类内容混在一起

当前 `coding-skill.md` 实际承担了**三个不同维度的职责**，它们的生命周期、受众、复用性完全不同：

| 维度 | 内容 | 是否随 SDD 流程变 | 是否随技术栈变 | 是否随团队变 |
|------|------|:---:|:---:|:---:|
| **① SDD 流程** | 读 Story/Task、CodingModel 11 维决策、CodingPlan 14 门禁、Task 实现方案确认、实时追溯链（Task→Story→DR→AI犯蠢）、CodingReport/CodeReview 衔接 | ✅ 是 | ❌ 否 | ❌ 否 |
| **② Coding 思想指南** | 骨架展开规则、分层职责红线、生成规则（骨架填肉/包路径固定/约束优先）、经验检查清单、反模式防御 | ❌ 否 | ❌ 否（偏否） | ❌ 否 |
| **③ Java/项目约束** | `mvn compile` 父工程全量编译、pom 依赖完整性、SDK 包路径、`@NotBlank` 来源包、`Result<T>` 包路径、约束文件引用表 | ❌ 否 | ✅ 是（强 Java/Spring 绑定） | ✅ 是（icec-cloud 系） |

**后果：**
- 想把 Coding 能力用到 Go/Python/前端项目 → 必须忍受满屏 `mvn`、`pom.xml`、`@NotBlank`、`icec-cloud-commons`。
- 想换团队约束（如不用 icec-cloud-commons 改用别的公共库）→ 只能改母版或整体 `skill-override` 覆盖（v3.5.0 的覆盖式，覆盖了就把 SDD 流程和思想指南一起盖掉，得不偿失）。
- v3.5.0 的 `skill-extends`（章节合并）loader 未实现（留待 v3.6.0），且即使实现了也只解决"单外挂合并"，不解决"多外挂合并 + 约束注册表"两个诉求。

### 1.2 现有 plugin-registry 的能力边界（为什么不能直接复用）

| 现有能力 | 是否满足本需求 | 缺口 |
|---------|:---:|------|
| `skill-override`（整体替换） | ❌ | 覆盖了就把 SDD 流程/思想指南一起盖掉 |
| `skill-new`（新增独立 SKILL） | ⚠️ 部分 | 能加新能力，但不能"合并进 coding-skill 主体" |
| `skill-extends`（章节合并） | ❌ | v3.5.0 未实现；且只支持单外挂，不支持"多 plug 合并" |
| 约束加载 | ❌ | 现有是 8 个固定文件名，无"约束注册表"，无三层覆盖 |

**结论：** 需要 v3.6.0 新增**两类注册表**——skill 增强注册表（多外挂合并）+ 约束注册表（三层覆盖），二者都在 coding-skill 层加载。

---

## 2. 重构总览（三件事、两层注册表）

### 2.1 文件拆分

```
重构前（1 个文件）:
  source/skills/phase2-coding/coding-skill.md   ← SDD流程 + 思想指南 + Java约束 全混

重构后（3 个文件）:
  source/skills/phase2-coding/
  ├── coding-sdd-skill.md      ← 🆕 ① SDD 流程：读 Story/Task、CodingModel、CodingPlan、追溯链
  ├── coding-skill.md          ← ✂️ ② Coding 思想指南：骨架展开、分层红线、生成规则、反模式（技术栈无关）
  └── (约束不在这里，见下)
```

### 2.2 约束拆分与默认内置

```
重构前（约束全在 ae-sdd 母版，固定 8 文件名，Java 专用）:
  source/standards/constraints/
  ├── technology-stack.md      ← Java 8 / Spring Boot 1.5.7 / MyBatis-Plus / icec-cloud-*
  ├── project-structure.md
  ├── layered-arch.md
  ├── code-style.md
  ├── api.md
  ├── database.md
  ├── security.md
  └── testing.md

重构后（约束按"技术栈族"拆分，Java 作为默认内置，但可被约束注册表覆盖/扩展）:
  source/standards/constraints/
  ├── README.md                        ← 约束总览 + 新团队接入指引（更新）
  ├── constraint-registry-spec.md      ← 🆕 约束注册表 schema（对标 plugin-registry-spec）
  ├── java/                            ← 🆕 Java 技术栈约束族（母版默认内置）
  │   ├── README.md                    ← Java 约束族总览 + 装配说明
  │   ├── technology-stack.md          ← 原 technology-stack.md 迁入
  │   ├── project-structure.md         ← 原 project-structure.md 迁入
  │   ├── layered-arch.md              ← 原 layered-arch.md 迁入
  │   ├── code-style.md                ← 原 code-style.md 迁入
  │   ├── api.md                       ← 原 api.md 迁入
  │   ├── database.md                  ← 原 database.md 迁入（含慢SQL防范，见 §5）
  │   ├── security.md                  ← 原 security.md 迁入
  │   ├── testing.md                   ← 原 testing.md 迁入
  │   ├── distributed.md               ← 🆕 分布式专项（锁/注册/代理，见 §5）
  │   └── ...                          ← 后续可按技术点继续拆
  └── (其他技术栈族，如 go/、python/、frontend/，由社区/项目层提供，母版不强求)
```

> **关键设计：ae-sdd 母版默认只内置 Java 族。** 这是用户明确要求的"ae-sdd 可以先默认内置 Java 的技术栈约束"。其他技术栈由项目层约束注册表按需挂载，母版不打包。

### 2.3 双注册表（coding-skill 加载两个注册表）

```
┌─────────────────────────────────────────────────────────────────┐
│  coding-skill.md（技术栈无关的 Coding 思想指南）                  │
│                                                                  │
│  启动时加载两个注册表：                                            │
│                                                                  │
│  ① Skill 增强注册表（plug-coding-skill）                          │
│     └─ 把多个外挂 plug-coding-skill 的内容【追加合并】到主体       │
│     └─ 例：plug-coding-test-assertion / plug-coding-perf / ...   │
│                                                                  │
│  ② 约束注册表（constraints）                                      │
│     └─ 决定加载哪个技术栈族的约束（java / go / 项目自定义）        │
│     └─ 三层覆盖：L1 项目 > L2 全局 > L3 母版默认(java)            │
│                                                                  │
│  最终生效内容 = coding-skill 主体 + Σ plug-coding-skill + 约束族   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. 第一点拆分：coding-sdd-skill 与 coding-skill 边界

### 3.1 `coding-sdd-skill.md`（🆕 SDD 流程，归 ae-sdd）

**定位：** coding 阶段的 SDD 流程编排——"怎么从 Story/Task 走到代码、怎么出 CodingPlan、出错怎么追溯"。这一层与 ae-sdd 的 state.json、memory、门禁强绑定，离开 ae-sdd 流程无意义。

**迁入内容（从现 coding-skill.md 切出）：**

| 章节 | 迁入 coding-sdd-skill | 理由 |
|------|:---:|------|
| 📦 文档存放前置调用（document-storage API 矩阵） | ✅ | SDD 文档流程，非通用 Coding |
| 🧠 阶段记忆强制调用（ae-sdd memory enter/write/exit） | ✅ | ae-sdd 专属 |
| CodingSkill 对外调用契约（`CodingSkill.Plan` / `CodingSkill.Execute`） | ✅ | ae-sdd 编排契约 |
| 第零步：加载 CodingModel 11 维决策 | ✅ | SDD 决策框架（注意：思维引擎本身在 standards/thinking，是通用的；11 维表是 SDD 的执行入口） |
| 约束文件引用（get_constraints 加载） | ✅ | SDD 加载动作（但加载"什么"由约束注册表决定，见 §4） |
| 第一~四步：收集输入/读约束/读 Story/读 TestCase/读 Task | ✅ | SDD 文档消费流程 |
| 第五步：工程预检（pom/SDK 包路径） | ⚠️ 保留框架，Java 细节下沉约束 | 见 §3.3 |
| ④bis CodingPlan 输出（7 章节 + 14 门禁 + 5 步 SOP） | ✅ | SDD 计划门禁 |
| 异常路径：Coding 实时追溯链（Task→Story→DR→AI犯蠢） | ✅ | SDD 追溯，强 ae-sdd 绑定 |
| 测试真实性强制规范（8 类禁止 + 5 保障） | ✅（迁入，但标注 code-review-skill 为评审权威） | SDD 完成判定硬前置 |
| ⑥bis / ⑦bis 核查闸 | ✅（指针，定义已在 code-review-skill） | SDD 衔接 |
| 人工审核讲解模板（Code 节点） | ✅ | SDD 审核编排 |

### 3.2 `coding-skill.md`（✂️ Coding 思想指南，技术栈无关）

**定位：** 通用的"怎么把设计变成可靠代码"的 Coding 能力。不依赖 ae-sdd state.json、不依赖 Java、不依赖 icec-cloud。可被独立引用（如非 ae-sdd 项目也想用这套思想）。

**保留内容：**

| 章节 | 留在 coding-skill | 理由 |
|------|:---:|------|
| §6.1 生成规则（骨架填肉/包路径固定/字段类型严格/约束优先/冲突处理） | ✅ | 通用 Coding 规则 |
| §6.1.1 骨架展开规则（伪代码动词→代码：校验/查询/调用/转换/返回/抛异常/组装/发送） | ✅（动词→代码映射通用，具体 import 下沉约束） | 通用 |
| §6.1 分层职责红线（Domain/Application/Repository/Interfaces 归位口诀） | ✅ | DDD 通用（具体包名下沉约束） |
| 经验检查清单（15 项通用经验） | ✅（保留通用项，Java/icec 专属项下沉 §6.10→约束） | 通用 |
| 反模式防御（指向 be-coding-ai-anti-patterns.md） | ✅ | LLM 通用 |
| 禁止事项表 | ✅（保留通用项，移除 Java 专属） | 通用 |

**移除/下沉内容（不属于通用 Coding 能力）：**

| 内容 | 去向 |
|------|------|
| `mvn compile` 父工程全量编译、`mvn spring-boot:run`、health 端点校验 | → Java 约束族 `testing.md` / 新增 `build-verify.md` |
| pom 依赖完整性（Domain/Application/Infrastructure/Interfaces 必须依赖表） | → Java 约束族 `project-structure.md` |
| 第三方 SDK 包路径（融云 `io.rong`、`@NotBlank` 来源包、Feign 注解版本） | → Java 约束族 `code-style.md`（或项目资产 §6.10） |
| `Result<T>` / `ApiResult` / `@SkipAuth` 包路径 | → Java 约束族 / 项目资产 |
| `MetaObjectHandler` 审计字段自动填充 | → Java 约束族 `database.md` |
| icec-cloud-commons / icec-cloud-spi-common / casslog / panda | → Java 约束族 `technology-stack.md`（已是） |

### 3.3 第五步"工程预检"的特殊处理

第五步是混合体：框架通用（"确认工程存在/检查依赖/确认已有代码模式"），细节 Java（pom/m2/`@NotBlank`）。处理：

```
coding-sdd-skill.md 第五步（框架）:
  5.1 确认工程存在（通用：工作目录下查找工程路径，不存在则创建并注册）
  5.2 检查依赖完整性 → 【调用约束注册表加载的 project-structure 约束】逐项核对
  5.3 验证第三方 SDK 包路径 → 【调用约束注册表加载的 code-style 约束】核对
  5.4 确认已有代码模式（通用：扫描已有文件确认公共类包路径/代码风格）

coding-skill.md（不直接管第五步，但提供"骨架展开规则"供第五步的核对用）
Java 约束族 project-structure.md（具体到 pom modules/依赖矩阵）
Java 约束族 code-style.md（具体到 @NotBlank 来源包）
```

### 3.4 两个 SKILL 的调用关系

```
ae-sdd 主编排
    │
    ▼
coding-sdd-skill.md（SDD 流程：读 Story/Task → CodingModel → CodingPlan → 追溯）
    │
    │  在"按 Task 生成代码"时引用：
    ▼
coding-skill.md（Coding 能力：骨架展开规则 / 分层红线 / 反模式）
    │
    │  两者启动时都加载：
    ├─► Skill 增强注册表（plug-coding-skill 追加合并）
    └─► 约束注册表（java/go/项目自定义 约束族）
```

**frontmatter 触发词分工：**
- `coding-sdd-skill`：触发词 = "生成代码"/"写代码"/"实现 Story"/"根据 Task 实现"/"确认实现方案"/"Coding 遇到问题"（原 coding-skill 的 SDD 触发词）
- `coding-skill`：不设独立用户触发词（被 coding-sdd-skill 引用 + 可被非 ae-sdd 场景手动加载）；description 强调"通用 Coding 能力，技术栈无关"

---

## 4. 第二点：双注册表机制（Skill 增强注册表 + 约束注册表）

### 4.1 Skill 增强注册表（plug-coding-skill，多外挂追加合并）

#### 4.1.1 与现有 plugin-registry 的关系

| 机制 | 类型 | 合并方式 | v3.5.0 状态 | 本设计 |
|------|------|---------|:---:|------|
| 现有 `skill-override` | 覆盖 | 整体替换 | ✅ 已实现 | 保留（用于完全替换 coding-skill） |
| 现有 `skill-new` | 新增 | 独立新 SKILL | ✅ 已实现 | 保留 |
| 现有 `skill-extends` | 章节合并 | 单外挂合并 | ❌ 未实现（v3.6.0） | **本设计吸收并升级为"多外挂追加合并"** |
| **🆕 plug-coding-skill** | **能力增强** | **多外挂内容追加合并** | — | **v3.6.0 新增** |

**关键区别：** `skill-extends` 是"一个外挂的章节合并"；`plug-coding-skill` 是"N 个外挂的内容追加合并"——这是用户明确要的"看注册表内注册了哪些 plug，把它们全都加载进来合并在一起"。

#### 4.1.2 注册表 schema（plug-coding-skill 专用）

复用三层注册表骨架（L1 项目 > L2 全局 > L3 母版），新增 `type: coding-augment`：

```yaml
# <project>/.ae-sdd/plugins/registry.yaml（或 L2/L3）
schema_version: 2                     # 🆕 v3.6.0 bump 到 2（向后兼容 v1）
description: 项目级 Coding 能力增强
plugins:
  # ... 原有 skill-override / skill-new 保留 ...

  # 🆕 v3.6.0 新增类型：对 coding-skill 的能力增强（追加合并，非覆盖）
  - name: plug-coding-test-assertion
    type: coding-augment              # 🆕 新类型
    version: 0.1.0
    description: 增强 coding-skill 的测试断言能力（真实 DB/HTTP 强制 + 突变抽检）
    path: ./plugins/coding-augment/test-assertion.md
    target: coding-skill              # 🆕 增强目标（固定为 coding-skill）
    merge_strategy: append            # 🆕 append（追加章节）/ section-merge（章节级合并，v3.6.0 先实现 append）
    priority: 100                     # 🆕 同 target 多 plug 时的合并顺序（数字小先合并，默认 100）
    tags: [testing, java]             # 可选标签（用于条件加载，见 4.1.4）

  - name: plug-coding-perf-guard
    type: coding-augment
    version: 0.2.0
    description: 增强 coding-skill 的性能防线（慢SQL/N+1/循环IO 静态扫描模板）
    path: ./plugins/coding-augment/perf-guard.md
    target: coding-skill
    merge_strategy: append
    priority: 200
    tags: [performance]
```

#### 4.1.3 合并算法（append 策略，v3.6.0 先落地最简版）

```
coding-skill 加载时：

1. 读主体：source/skills/phase2-coding/coding-skill.md（或被 skill-override 覆盖后的版本）
2. 读三层注册表，收集所有 type=coding-augment 且 target=coding-skill 的 plug
3. 按 priority 升序排序（同 priority 按 name 字典序，保证确定性）
4. 依次把每个 plug 的内容【追加】到主体之后，每个 plug 前插入分隔标记：
   ────────────────────────────────────────
   <!-- 🔌 plug-coding-skill: {name} v{version} (from {layer}) -->
   ────────────────────────────────────────
   {plug 内容}
5. 最终合并文档 = 主体 + Σ plug（按 priority 顺序）

冲突处理（v3.6.0 简化版）：
- append 策略不处理章节内冲突——plug 之间内容重复由 plug 作者自责
- 多层同 name 冲突：按 L1>L2>L3 选胜者 + warn（与现有 plugin-registry 一致）
- 同层同 name：🔴 阻断（与现有一致）
```

> **v3.6.0 范围声明：** 先实现 `merge_strategy: append`（最简、零歧义）。`section-merge`（章节级智能合并，处理冲突）留待 v3.7.0，与原 plugin-registry 的 `skill-extends` 章节合并一并实现。

#### 4.1.4 条件加载（可选，v3.6.0 可不做）

plug 可带 `tags`，coding-skill 加载时可按上下文过滤（如当前技术栈=java → 只加载 tags 含 java 或无 tag 的 plug）。v3.6.0 先全量加载，过滤留待后续。

### 4.2 约束注册表（constraints，三层覆盖 + 技术栈族）

#### 4.2.1 与现有约束加载的区别

| 维度 | 现状（v3.5.x） | 重构后（v3.6.0） |
|------|---------|---------|
| 约束文件 | 8 个固定文件名，平铺在 `source/standards/constraints/` | 按"技术栈族"分目录（`java/`、`go/`...），族内仍是固定文件名 |
| 加载方式 | `get_constraints(projectKey)` 返回固定 8 个 | `get_constraints(projectKey)` 先查约束注册表确定"加载哪个族"，再读族内文件 |
| 覆盖能力 | 无（改母版即全改） | 三层覆盖：L1 项目 > L2 全局 > L3 母版默认（java） |
| 母版默认 | Java 约束直接是母版内容 | Java 约束在 `source/standards/constraints/java/`，作为 L3 默认 |

#### 4.2.2 约束注册表 schema

```yaml
# <project>/.ae-sdd/constraints/registry.yaml（L1 项目层）
schema_version: 1
description: 项目约束注册表——决定加载哪个技术栈族的约束
stack: java                          # 🆕 主技术栈族（决定 L3 默认加载 java/ 还是 go/）
                                     #   可选值：java / go / python / frontend / custom
overrides:                           # 🆕 三层覆盖（粒度到单个约束文件）
  - file: technology-stack.md        # 覆盖哪个约束文件
    path: ./constraints/java/technology-stack.md   # 用哪个文件覆盖（相对注册表目录）
    reason: 项目用 Spring Boot 2.x，与母版默认 1.5.7 不同
  - file: database.md
    path: ./constraints/java/database-strict.md
    reason: 项目要求更严格的慢SQL防范
extensions:                          # 🆕 扩展（追加约束文件，非 8 固定名）
  - file: distributed.md             # 新增约束文件名
    path: ./constraints/java/distributed.md
    reason: 项目大量分布式场景，需专项约束
```

**三层合成（对标 skill 注册表）：**

```
get_constraints(projectKey) 合成算法：

1. 确定 stack 族：
   L1 registry.stack > L2 registry.stack > L3 默认(java)
   （L1 未声明 stack → 用 L2；L2 也无 → 用 L3 java）

2. 确定每个约束文件的来源（8 固定 + extensions）：
   对每个约束文件 f：
   ├─ L1 overrides 命中 f → 用 L1 指定的 path
   ├─ L2 overrides 命中 f → 用 L2 指定的 path
   └─ 否则 → 用 {stack} 族内默认 path（L3 母版 source/standards/constraints/{stack}/{f}）

3. extensions（追加约束）：
   合并三层所有 extensions（按 file 去重，L1>L2>L3 优先），追加到约束清单

4. 返回 ConstraintList（含每个文件的 resolved path + 来源层 + reason）
```

#### 4.2.3 约束文件名约定（族内仍固定，便于 SKILL 按名引用）

族内保持 8 个固定文件名（SKILL 按文件名引用，不感知族路径）：
`technology-stack.md` / `project-structure.md` / `layered-arch.md` / `code-style.md` / `api.md` / `database.md` / `security.md` / `testing.md`

`extensions` 可追加任意文件名（如 `distributed.md` / `mq.md` / `cache.md`），SKILL 通过 `get_constraints()` 返回的清单按需加载。

---

## 5. 第三点：Java 技术栈约束重点补充（基于 thinking 模型）

用户原话："Java 本身的技术栈是很丰富的，每一块技术栈都有单独的实现标准……你来基于 thinking 模型重点补充。"

**补充原则：** 不重复 thinking engine 的 11 维风险决策树（那是通用的），而是把每个风险维度在 **Java/Spring 技术栈下** 的**具体实现标准、决策清单、反例**写清楚——让 AI 在 Java 项目里不需要自己推理"怎么做"，而是按约束执行。

### 5.1 补充清单（8 个约束文件 + 3 个新增专项）

| 文件 | 现状 | 重点补充方向（基于 thinking 模型维度映射） |
|------|:---:|------|
| `technology-stack.md` | ✅ 已有版本表 | 补"选型决策清单"——每个中间件何时该用/何时不该用（对应 thinking §2.1 技术选型 ADR） |
| `project-structure.md` | 待迁 | 补 pom modules 依赖矩阵 + 模块创建 SOP |
| `layered-arch.md` | 待迁 | 补 DDD 四层 + SPI/BFF 的 Java 包路径模板 + 跨层误引用扫描清单 |
| `code-style.md` | 待迁 | 补 Lombok/异常/枚举/日志三要素 + `@NotBlank`/`Result<T>` 来源包决策树 |
| `api.md` | 待迁 | 补 REST 错误码体系 + BFF/SPI 接口契约四维 |
| `database.md` | ✅ 已有 DDL | **重点补慢SQL防范（见 §5.2）** + MyBatis-Plus 用法 + EXPLAIN 验证 SOP |
| `security.md` | 待迁 | 补 @SkipAuth / 脱敏 / 审计日志 / 数据级权限（org_id/tenant_id） |
| `testing.md` | 待迁 | 补 JUnit4 + Mockito + H2/TestContainers 分层 + 真实 HTTP(RANDOM_PORT) |
| **`distributed.md`** 🆕 | — | **重点补分布式专项（见 §5.3）** |
| **`mq.md`** 🆕 | — | Kafka/courier 消费幂等 + DLQ + 顺序消息（对应 thinking 维度④可解耦） |
| **`cache.md`** 🆕 | — | Redis/Caffeine 穿透/击穿/雪崩 + 分布式锁 Lua（对应 thinking 维度②⑦） |

### 5.2 `database.md` 重点补充：慢 SQL 防范（用户点名）

> 对应 thinking engine 维度⑦性能瓶颈。把"防范慢SQL"从"原则口号"变成"逐条可执行的决策清单"。

**补充章节结构（追加到 java/database.md）：**

```markdown
## 七、慢 SQL 防范决策清单（🔴 写每条 SQL 前过一遍）

### 7.1 索引命中决策（写 WHERE 前必问）

| # | 决策点 | 判定 | 动作 |
|---|--------|------|------|
| 1 | WHERE 条件字段是否有索引？ | 否 → 🔴 阻断 | 建索引或改查询方案，禁止裸扫 |
| 2 | 组合索引字段顺序与 WHERE 顺序是否一致？ | 否 → 调整 WHERE 顺序匹配最左前缀 | 遵循"区分度最高字段最左" |
| 3 | 是否对索引字段用了函数/计算？ | 是 → 🔴 索引失效 | 改写为等价的无函数条件（如 `WHERE date(create_time)='2026-01-01'` → `WHERE create_time>='2026-01-01' AND create_time<'2026-01-02'`）|
| 4 | 是否对索引字段做了隐式类型转换？ | 是 → 🔴 索引失效 | 字段类型与参数类型严格一致（varchar 字段传数字会失效）|
| 5 | LIKE 是否左模糊/全模糊？ | 是 → 🔴 禁止 | 走 ES 或改右模糊；左模糊必走搜索引擎 |
| 6 | OR 条件两侧是否都有索引？ | 否 → 🔴 可能全表扫 | 改 UNION ALL 或确保两侧都走索引 |

### 7.2 联表决策

| # | 决策点 | 判定 | 动作 |
|---|--------|------|------|
| 1 | 联表数量 | > 3 → 🔴 阻断 | 拆查询 / 应用层组装 / 反范式冗余字段 |
| 2 | JOIN 字段是否有索引？ | 否 → 🔴 阻断 | 被关联字段必须建索引 |
| 3 | JOIN 字段类型是否一致？ | 否 → 🔴 隐式转换致索引失效 | 统一类型 |
| 4 | 是否可用覆盖索引避免回表？ | 能 → 优先覆盖索引 | SELECT 字段都在索引树内 |
| 5 | 大表 JOIN 小表 | driver 表选小表 | 小表驱动大表（MySQL 优化器一般会选，但复杂查询需手动 hint）|

### 7.3 函数与计算决策

| # | 决策点 | 判定 | 动作 |
|---|--------|------|------|
| 1 | SELECT/WHERE 是否含函数？ | 是 → 评估能否下推到应用层 | 能下推则下推（DB 算比应用算慢且占 CPU）|
| 2 | 聚合函数（SUM/COUNT/AVG）数据量 | 大 → 评估预聚合/汇总表 | 实时聚合禁用于大表，用定时预聚合 |
| 3 | 子查询 | 能改 JOIN 则改 JOIN | MySQL 子查询优化弱，优先 JOIN/EXISTS |

### 7.4 条件顺序与分页决策

| # | 决策点 | 判定 | 动作 |
|---|--------|------|------|
| 1 | WHERE 条件区分度顺序 | 区分度高的放前面 | 缩小扫描集 |
| 2 | 大分页（LIMIT 10000,10） | 是 → 🔴 改游标分页 | 基于主键/时间的游标分页，禁止 OFFSET 深翻 |
| 3 | 分页是否先 count | count=0 直接返回 | 避免无意义查询 |
| 4 | ORDER BY 字段是否在索引内 | 否 → filesort → 评估加索引 | 排序字段放组合索引最后 |

### 7.5 EXPLAIN 验证 SOP（写完 SQL 强制跑）

| EXPLAIN 字段 | 红线值 | 处置 |
|---|---|---|
| type | ALL / index（全表/全索引扫） | 🔴 阻断，加索引或改方案 |
| rows | 超过预估数据量 10% | 🟠 警告，优化条件 |
| Extra | Using filesort / Using temporary | 🟠 警告，优化排序/分组 |
| key | NULL（未走索引） | 🔴 阻断 |
| possible_keys 有但 key 为 NULL | 索引未被选中 | 检查条件写法（函数/类型转换致失效）|

> 🔴 门禁：核心读写 SQL 必须附 EXPLAIN 输出截图/文本，无 EXPLAIN = 未验证。

### 7.6 N+1 与循环内 IO（🔴 硬红线，对应 thinking 维度⑦）

- 循环内查 DB → 批量 IN 查询 + 内存组装
- 循环内调 Redis → pipeline 批量
- 循环内调 HTTP → CompletableFuture 并行 / 批量接口
- MyBatis 延迟加载致 N+1 → 显式 join 查询或 `@Options(fetchSize)`
```

### 5.3 `distributed.md` 重点补充：分布式专项（用户点名）

> 对应 thinking 维度①②③⑤⑧。Java 分布式实现标准。

**章节结构（新增 java/distributed.md）：**

```markdown
# Java 分布式实现约束

## 摘要
适用场景：涉及分布式锁 / 服务注册发现 / 分布式事务 / 远程代理 / 幂等 / 一致性的场景。
对应 thinking engine 维度：①原子性 ②并发安全 ③幂等 ⑤数据一致性 ⑧资源隔离。

## 一、分布式锁（对应 thinking 维度②）

### 1.1 选型决策
| 场景 | 方案 | 理由 |
|------|------|------|
| 单 Redis 实例 | Redis SET NX PX + Lua 释放 | 简单，CP 依赖单点 |
| 强一致要求 | Redisson + RedLock / ZK | RedLock 有争议，ZK 更稳但慢 |
| 防误删 | value=uuid，释放用 Lua 校验 | 禁止直接 DEL |

### 1.2 实现标准（Redis SET NX PX）
- 加锁：`SET key uuid NX PX {ttl}`，原子
- TTL：业务预估时长 × 3（兜底自动释放）
- 释放：Lua 脚本（GET 比较 uuid 一致才 DEL），禁止 Java 先 GET 后 DEL
- 加锁失败策略：用户实时操作 → 快速失败返回"请稍后重试"；后台任务 → 指数退避重试
- 看门狗（Redisson）：长任务用 watchdog 续期，但需评估续期失败兜底

### 1.3 禁止
- 禁止用 `SETNX` + `EXPIRE` 两步（非原子，宕机致死锁）
- 禁止直接 DEL 释放（可能删别人的锁）
- 禁止锁value 固定常量（无法防误删）

## 二、服务注册与发现

### 2.1 注册实现标准
- Spring Cloud：`@EnableDiscoveryClient` + Nacos/Eureka 配置
- 注册失败：启动阻断（不能静默降级为单机）
- 健康检查：`/actuator/health` 必须 UP 才注册
- 优雅下线：`spring.cloud.nacos.discovery.enabled=false` + 等待流量排空（非直接 kill）

### 2.2 服务发现消费
- Feign/OpenFeign：声明式调用，禁止手写 RestTemplate 拼地址
- 超时：连接 1s + 读 3s（对应 thinking 维度⑥）
- 负载均衡：Ribbon/LoadBalancer，禁止硬编码单实例地址

## 三、远程代理（Feign / RPC）

### 3.1 Feign 实现标准
- 注解：Spring Cloud Dalston 用 `org.springframework.cloud.netflix.feign.FeignClient`（版本敏感，见 code-style）
- fallback：`@HystrixCommand(fallbackMethod=...)` 或 Feign fallbackFactory
- 超时/重试：见 thinking 维度⑥，非幂等操作禁止自动重试
- 序列化：统一 JSON，禁止 JDK 序列化（跨服务兼容性差）

### 3.2 SPI 跨服务契约
- SPI 模块定义接口 + DTO，消费方依赖 SPI 模块，不依赖实现方
- 接口变更必须向后兼容（新增字段 optional / 废弃字段 @Deprecated / 破坏性变更走 /v2）
- ACL 防腐层：消费方在 Infrastructure 层包一层 Facade，Domain 不直接依赖外部 DTO

## 四、分布式事务（对应 thinking 维度①⑤）

### 4.1 选型决策（thinking 维度①决策树落地）
| 场景 | 方案 | 侵入 | 一致性 |
|------|------|:---:|------|
| 单库多表 | 本地 @Transactional | 低 | 强 |
| 跨库允许最终一致 | 本地消息表 + 补偿任务 | 中 | 最终 |
| 跨库要求强一致 | Seata AT | 低 | 强（同类库）|
| 跨异构强一致 | TCC | 高 | 强（慎用）|

### 4.2 实现标准
- 本地事务边界放 Application 层 `@Transactional`，禁止在 Domain/Repository
- 事务内禁止 HTTP/MQ 调用（大事务红线）→ 用 `TransactionSynchronizationManager.afterCommit()` 事务外发
- 跨库最终一致：先写 DB + 消息表（同事务），定时扫描消息表发送，失败重试 + DLQ
- 对账（thinking §⑤扩展）：核心资金 5min 对账，差异告警

## 五、幂等（对应 thinking 维度③）

### 5.1 实现标准
- 唯一业务键：唯一索引 + `INSERT IGNORE` / `ON DUPLICATE KEY UPDATE`
- 幂等表：MQ 消费前查幂等表，有则跳过
- 状态机前置：`WHERE status=#{expected}` 仅符合流转条件才执行
- 幂等键选择：业务单号 > 用户ID+操作+时间窗 > UUID

## 六、资源隔离（对应 thinking 维度⑧）

### 6.1 实现标准
- 线程隔离：批量/报表用独立线程池，禁止占用核心业务线程池
- DB 隔离：读写分离，统计查从库
- 拒绝策略：核心链路 AbortPolicy + DLQ，禁止 CallerRunsPolicy 让核心线程跑低优先级任务
- 分级队列：优先级队列 / 分 Topic
```

### 5.4 其他专项（mq.md / cache.md）补充要点

- **mq.md**：courier 封装 Kafka 禁止裸用 / 消费幂等（与 distributed.md 幂等交叉引用）/ DLQ + 人工介入告警 / 顺序消息用 Kafka 分区 / 消费手动 ack
- **cache.md**：Redis Key 命名 `{project}:{module}:{business}:{id}` / 穿透布隆过滤器+空值缓存 / 击穿互斥锁重建 / 雪崩随机 TTL / 分布式锁 Lua（与 distributed.md 交叉）/ Caffeine 本地 vs Redis 分布式不混用

---

## 6. 落地拆解（分 PR）

### 6.1 阶段划分

| 阶段 | PR | 内容 | 风险 |
|------|:---:|------|:---:|
| **P1 文档拆分** | v3.6.0-alpha | coding-skill.md → coding-sdd-skill.md + coding-skill.md（纯文档搬运 + 边界标注，不改逻辑） | 🟢 低 |
| **P2 约束族化** | v3.6.0-beta | constraints 平铺 → `java/` 族 + 约束注册表 schema + get_constraints 改造 | 🟠 中（改加载逻辑） |
| **P3 Skill 增强注册表** | v3.6.0-rc | `coding-augment` 类型 + 多外挂 append 合并 loader + 测试 | 🟠 中（新 loader） |
| **P4 Java 约束补充** | v3.6.0 | §5 的 database/distributed/mq/cache 重点补充 | 🟢 低（纯内容） |
| **P5 迁移与兼容** | v3.6.0 | 旧约束路径兼容映射 + update-graph + CHANGELOG | 🟠 中 |

### 6.2 兼容性保证（零破坏）

- 旧项目无约束注册表 → `get_constraints` fallback 到 L3 母版 `java/` 族（行为等价于今天）
- 旧项目无 skill 增强注册表 → coding-skill 只加载主体（行为等价于今天）
- 旧 `source/standards/constraints/*.md`（平铺）→ 迁入 `java/` 后，`get_constraints` 内部做路径兼容（旧调用不感知）
- schema_version: plugin-registry v1→v2（v1 视为无 coding-augment，兼容）；constraint-registry v1 起

### 6.3 测试

- `test_plugin_loader.py` 扩展 `coding-augment` 类型 + append 合并测试
- `test_constraints_loader.py`（新建）三层约束注册表合成测试
- `test_gates.py` / `test_paths.py` 回归（get_constraints 路径变化）

---

## 7. 待用户确认的决策点

| # | 决策点 | 选项 | 推荐 |
|---|--------|------|------|
| 1 | coding-sdd-skill 是否设独立用户触发词？ | A. 复用原 coding-skill 触发词 / B. 新增"SDD 编码"触发词 | A（用户无感） |
| 2 | 约束族目录命名 | A. `java/` / `go/` / B. `stack-java/` | A（简洁） |
| 3 | 母版是否内置非 Java 族？ | A. 只内置 java / B. 内置 java+go+python | A（用户明确"先默认 Java"） |
| 4 | plug-coding-skill 合并策略 v3.6.0 范围 | A. 只做 append / B. append + section-merge | A（KISS，section-merge 留 v3.7） |
| 5 | 约束注册表是否复用 `.ae-sdd/plugins/registry.yaml`？ | A. 同文件加 constraints 段 / B. 独立 `.ae-sdd/constraints/registry.yaml` | B（职责分离） |
| 6 | Java 约束补充的 mq.md/cache.md 是否独立成文件？ | A. 独立 / B. 并入 distributed.md | A（用户要"每一块技术栈都有单独标准"） |

---

## 8. 风险与对策

| 风险 | 对策 |
|------|------|
| 拆分后 coding-skill 过于抽象，Java 用户不知道去哪找 mvn 规则 | coding-skill 主体顶部加"约束加载指引"——明确"Java 编译/依赖规则在 java/ 约束族，由约束注册表加载" |
| 约束族化后 get_constraints 改造影响面大 | P2 先做路径兼容层，旧调用透明；充分回归 test_paths/test_gates |
| 多外挂 append 合并内容冲突（两个 plug 都讲测试断言） | v3.6.0 append 不处理冲突，由 plug 作者协调；文档明确"同主题 plug 应合并为一个" |
| thinking engine 11 维与 Java 约束重复 | 约束只写"Java 下的具体实现标准"，thinking 写"通用决策树"，交叉引用不抄 |
| 拆分增加文件数，维护成本上升 | 用 update-graph.json 管理依赖；README 总览导航 |

---

## 9. 不在本次范围

- 非 Java 技术栈族（go/python/frontend）的具体约束内容——由社区/项目层提供
- `section-merge` 章节级智能合并算法——留 v3.7.0
- plug-coding-skill 的 tags 条件加载过滤——留后续
- 约束注册表的 GUI 化向导——留后续
- 现有 plugin-registry `skill-extends` 的真正实现——与 section-merge 一并留 v3.7.0

---

## 附录 A：内容归属总表（现 coding-skill.md 2264 行 → 三去向）

| 现章节（行号区间） | 去向 |
|---|---|
| frontmatter + 📦文档存放 + 🧠阶段记忆（1-57） | coding-sdd-skill |
| 目标 + 整体流程 + CodingSkill 对外契约（59-178） | coding-sdd-skill |
| 第零步 CodingModel 11 维（180-204） | coding-sdd-skill（引用 thinking engine） |
| 约束文件引用 + 第一~四步（206-336） | coding-sdd-skill（框架）；Java 细节下沉 java/ 约束族 |
| 第五步工程预检（338-369） | coding-sdd-skill（框架）；pom/SDK 下沉 java/ 约束族 |
| §6.1 生成规则 + §6.1.1 骨架展开 + §6.2 Task 流程（372-547） | coding-skill（通用规则）；coding-sdd-skill 引用 |
| §6.2.1 Task 实现方案确认（465-535） | coding-sdd-skill（SDD 审核节点） |
| 第七~八步 编译/启动/测试（550-818） | coding-sdd-skill（框架）；mvn/health 下沉 java/ 约束族 |
| 第九步 全切面一致性核查（822-858） | coding-sdd-skill（指针，定义在 code-review-skill） |
| 完成标准 + 异常路径追溯链（862-1077） | coding-sdd-skill |
| 经验检查清单（1079-1102） | coding-skill（通用项）；Java 专属下沉 java/ 约束族 |
| 禁止事项 + 执行清单（1106-1161） | coding-skill（通用）；Java 专属下沉 |
| 人工审核讲解（1164-1239） | coding-sdd-skill |
| ④bis CodingPlan（1243-1496） | coding-sdd-skill |
| ④bis 实战 SOP 5 步（1499-1703） | coding-sdd-skill |
| 测试真实性规范（1706-1878） | coding-sdd-skill（指针，评审权威在 code-review-skill） |
| ⑥bis/⑦bis 闸（1884-1974） | coding-sdd-skill（指针） |
| 问题分层排查（1980-2054） | coding-sdd-skill |
| 实战闸沉淀 7 闸（2058-2264） | coding-sdd-skill（指针，定义在 code-review-skill）；通用静态扫描留 coding-skill |
