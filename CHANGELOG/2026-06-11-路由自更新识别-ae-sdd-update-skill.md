# 2026-06-11 路由增强：自更新识别 → ae-sdd-update-skill + 项目资产经验文档

## 变更摘要

1. 在 ae-sdd-skill.md 智能路由中增加「自更新识别」能力（步骤 1.5 短路）
2. 在项目资产生成流程中新增「团队惯用实现方式（经验文档）」步骤（§10）

## 变更 1：路由自更新识别

| 文件 | 变更位置 | 变更内容 |
|------|---------|---------|
| `skills/orchestration/ae-sdd-skill.md` | §基础路由表（L83 后） | 新增一行路由：触发词 → `ae-sdd-update-skill.md`（自身维护） |
| `skills/orchestration/ae-sdd-skill.md` | §路由决策算法（步骤 1 → 2 之间） | 新增步骤 1.5「自更新识别」，优先级高于步骤 2，命中即短路 |
| `plugins/ae-sdd/skills/orchestration/ae-sdd-skill.md` | 同上 | 同步修改 |

## 触发关键词

- "修改 SKILL" / "更新 SKILL" / "新增 SKILL" / "重构 SKILL"
- "SKILL 边界" / "SKILL 维护"
- "优化 ae-sdd" / "改 ae-sdd"
- "ae-sdd skill" + 任意变更动词

## 路由行为

- 命中 → 路由到 `ae-sdd-update-skill.md`
- 跳过步骤 2-5（不走业务流程节点匹配），直接按 update-skill 的 5 步流程执行
- 未命中 → 正常进入步骤 2 关键词匹配

## 触发原因

用户要求增强路由能力，避免自身维护类任务误入业务流程路由。

## 影响范围

- `ae-sdd-skill.md` §智能路由表 + §路由决策算法
- 不影响现有 6 大节点路由 / 4 类需求路由 / 重入路由

## 验证方式

- 输入 "优化 ae-sdd skill" → 应路由到 ae-sdd-update-skill
- 输入 "修改 SKILL" → 应路由到 ae-sdd-update-skill
- 输入 "开始 Coding" → 应正常路由到 coding-skill（不命中自更新）

## Reviewer

用户

---

## 变更 2：项目资产增加经验文档（§10 团队惯用实现方式）

### 详细变更

| 文件 | 变更位置 | 变更内容 |
|------|---------|---------|
| `standards/project-assets/project-assets-schema.md` | §10（新增章节） | 新增「团队惯用实现方式（经验文档）」：9 分类结构 + 记录格式 + 准入门禁 + 排除规则 + 与其他章节关系 |
| `standards/project-assets/project-assets-schema.md` | §9 探查 SOP（步骤 8→9→10） | 步骤 8 改写为 §11 缺口；新增步骤 9「提炼团队惯用实现方式」6 步 SOP；原步骤 9 改为步骤 10 |
| `standards/project-assets/project-assets-schema.md` | §8 Code Plan 输入索引 | 新增 §13 惯用模式对齐引用 §10；§5 引用扩展含 §10 |
| `standards/project-assets/project-assets-schema.md` | 附录 A JSON Schema | `required` 新增 `teamPatterns`；`properties` 新增 `teamPatterns` 引用 |
| `standards/project-assets/project-assets-schema.md` | §11（原 §10 改编号） | 缺口章节从 §10 → §11 |
| `skills/cross-cutting/project-assets-update-skill.md` | §2 整体流程 | 动作 1 新增"提炼团队惯用实现方式" |
| `skills/cross-cutting/project-assets-update-skill.md` | §3.2 SOP 表 | 9 步 → 10 步，新增步骤 9 |
| `skills/cross-cutting/project-assets-update-skill.md` | §3.4 门禁 | 新增 3 条 §10 经验文档门禁 |
| `skills/cross-cutting/project-assets-update-skill.md` | §10 执行清单 | 动作 1 新增经验文档步骤 |

### 核心设计

- **准入门禁 4 条：** ≥2 处一致使用 / 符合 constraints/ / 无已知 BUG / 可脱离业务复用
- **排除规则 5 条：** 孤例 / 违反约束 / 技术债 / 样板代码 / 过度设计
- **9 分类必须各出 ≥1 条经验：** 跨层 / Domain / Application / Infrastructure / Interfaces / 异常处理 / 并发幂等 / 测试 / 配置集成
- **每条经验必须有 ≥2 个真实文件出处 + 标注对齐 constraints/ 条款**

### 触发原因

用户要求在生成项目资产时阅读现有实现、总结惯用方式、过滤后形成经验文档。

### 影响范围

- 项目资产 schema 新增 §10 + §11 编号变更
- 项目资产 update-skill 生成流程 9 步 → 10 步
- CodingPlan 编写时新增"惯用模式对齐"参考维度
