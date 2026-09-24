# DR-001 - ALSpec 运行时内容清理与流程产物迁移

## 1. 元信息与设计意图

```yaml
spec_id: DR-001
spec_type: dr
scope: project
project_id: al-spec
status: draft
version: 0.1.0
owner: AILenGarden
updated: 2026-09-07
```

设计意图：让 `al-spec` 的运行时内容只承担规格写作能力，把变更历史、维护说明和原 `ae-sdd` 流程产物移出 Skill 与模板，降低上下文噪声和误导性。

证据范围：

- `skills/dr/SKILL.md`
- `skills/story/SKILL.md`
- `skills/testcase/SKILL.md`
- `templates/global/*`
- `templates/project/*`
- `framework/*`
- `standards/*`
- `CHANGELOG/*`
- `docs/*`
- `dist/*`

## 2. 目标、范围与非目标

### 目标

- Skill 只描述上下文加载、写作规则、质量检查和审查规则。
- 模板只提供规格输出结构、字段、示例和填写提示。
- 运行时内容不包含 changelog、修订日志、评审记录或“本次变更”说明。
- 运行时内容不包含 `ae-sdd` 阶段、门禁、交接、存储协议和自动编排产物。
- 源目录、注册表和发布包保持一致。

### 范围内

- 清理三个全局 Skill 中的历史记录、内部实现说明和非写作边界声明。
- 清理全局模板和项目模板中的 Skill 加载说明、流程说明和规范归属说明。
- 将原有流程产物移出运行时目录，按维护参考、迁移记录或历史归档分类保存。
- 修正 registry、插件描述、加载契约、维护文档和测试，使其反映新的运行时模型。
- 重建并验证发布产物。

### 范围外

- 不新增 PRD、CodingPlan 或其他规格类型。
- 不改变 DR、Story、TestCase 的核心语义和章节结构，除非该章节属于流程产物。
- 不删除有审计价值的历史 changelog；历史记录只迁移位置或标注为非运行时资料。
- 不改变用户项目仓库中的业务代码、测试代码或项目约束文件。
- 不新增 `al-spec` 产品 CLI。

## 3. 现状问题

| ID | 问题 | 影响 |
|---|---|---|
| REQ-001 | Skill 或模板中混入历史、修订、流程和注册实现说明 | Agent 把维护信息误当成写作规则，增加上下文噪声 |
| REQ-002 | `ae-sdd` 的阶段、门禁、交接、存储和自动 handoff 产物与运行时内容混杂 | 可能重新引入文档上下游依赖和流程门槛 |
| REQ-003 | 模板承担 Skill 加载说明和规范归属说明 | 模板不能作为纯输出结构复用 |
| REQ-004 | 源目录与 `dist` 发布副本存在旧项目级 Skill 和旧 registry | 安装后行为可能与源目录不一致 |
| REQ-005 | 维护脚本被误解为产品 CLI | 用户对插件能力边界产生错误预期 |

## 4. 设计决策

### DEC-001 - 运行时与维护资料分离

- 选定方案：`skills/` 和 `templates/` 只保留运行时必需内容；历史、迁移、校验和格式修复资料放在 `standards/`、`docs/`、`CHANGELOG/` 或明确的维护归档目录。
- 拒绝方案：把所有资料继续放在 Skill 或模板中。
- 接受代价：维护者需要通过额外目录查阅历史和迁移背景。

### DEC-002 - 删除奇怪的边界声明，保留必要的写作约束

- 选定方案：删除“唯一 Skill”“不是项目级 Skill”“不产生第二套规范”等插件内部结构声明，以及泛化的职责边界口号；保留会影响文档质量的约束，例如范围、责任归属、不可编造事实和可观察验收。
- 拒绝方案：完全删除所有边界相关内容。
- 接受代价：部分插件结构约束不再由 Skill 自述，改由 registry 和加载契约维护。

### DEC-003 - 原 `ae-sdd` 流程产物只保留为维护参考

- 选定方案：从运行时 Skill、模板和插件入口中移除 Phase、RA intake、plan-first、gate、review loop、save_doc、resolve_path、状态文件、自动 handoff 等内容；若有保留价值，则迁移到非运行时维护资料并标注来源。
- 拒绝方案：把流程产物继续嵌入规格 Skill，依靠文字提醒避免 Agent 执行。
- 接受代价：旧流程用户需要重新适应独立规格模型。

### DEC-004 - 不提供产品 CLI

- 选定方案：不新增 `al-spec` CLI。现有 `scripts/registry.py`、`scripts/validate_registry.py` 和 Markdown 工具仅作为仓库维护脚本；文档必须明确其维护属性。
- 拒绝方案：把维护脚本包装成正式 CLI。
- 接受代价：注册表维护和校验仍需通过开发环境脚本执行。

### DEC-005 - 三类规格统一使用全局 Skill 与模板映射

- 选定方案：DR、Story、TestCase 均使用全局类型 Skill；项目差异通过 `project_templates` 映射选择一个项目模板，无匹配时使用全局模板。
- 拒绝方案：为 life 项目继续保留 `life-dr`、`life-story`、`life-testcase` 项目级 Skill。
- 接受代价：项目级特殊写作规范必须表达为模板字段、项目证据或维护参考，不能通过第二个 Skill 覆盖。

## 5. 目标运行时结构

```text
al-spec/
├── .codex-plugin/plugin.json       # 插件元数据与能力说明
├── skills/
│   ├── dr/SKILL.md                 # 全局 DR 写作规范
│   ├── story/SKILL.md              # 全局 Story 写作规范
│   └── testcase/SKILL.md           # 全局 TestCase 写作规范
├── templates/
│   ├── global/                     # 全局兜底模板
│   └── project/                    # 项目模板
├── framework/                     # registry 与加载契约
├── standards/                     # 维护参考，不作为运行时前置上下文
├── docs/                          # 设计、迁移和维护文档
├── CHANGELOG/                     # 历史变更记录
└── scripts/                       # 维护脚本，不是产品 CLI
```

运行时加载规则：

1. 解析规格类型和项目 ID。
2. 加载对应的全局 Skill。
3. 有精确项目模板映射时加载项目模板，否则加载全局模板。
4. 不同时加载全局模板和项目模板。
5. 不从模板推断新的 Skill 或流程依赖。

## 6. 迁移与清理要求

| 对象 | 处理方式 | 结果 |
|---|---|---|
| Skill 内 changelog、revision diary、review transcript | 删除 | 不再进入 Agent 运行时上下文 |
| 模板内 Skill 加载说明、规范归属说明 | 删除 | 模板只保留输出结构 |
| `ae-sdd` Phase、gate、handoff、save_doc、resolve_path 等 | 从运行时移出 | 需要时仅在维护参考中查阅 |
| 旧项目级 `life-*` Skill | 删除注册和运行时文件 | life 差异只由项目模板表达 |
| 旧 `dist` 副本 | 不直接复用，按当前源目录重建 | 发布包与源目录一致 |
| registry/校验/格式脚本 | 保留为维护脚本并明确定位 | 不宣称为产品 CLI |
| 历史 CHANGELOG | 保留历史事实，必要时补充迁移说明 | 不加载为 Skill 上下文 |

## 7. 非功能要求

- **语义密度：** Skill 删除重复解释后，保留会改变 Agent 决策的规则。
- **可发现性：** registry 中每个全局类型必须有有效 Skill 和模板路径。
- **可复现性：** 项目模板选择必须由精确 `project_id` 映射决定。
- **兼容性：** 不改变已有三类规格 ID、模板核心章节和独立规格原则。
- **可维护性：** 维护脚本、历史记录和迁移资料不得成为运行时必需上下文。
- **发布一致性：** 源目录、manifest 和 zip 必须包含同一套 Skill、模板和 registry。

## 8. 验收标准

### AC-001 - Skill 内容清理

- Given 三个全局 Skill 文件，When 检查其正文，Then 不包含 changelog、revision diary、review transcript、历史修订叙述或插件内部注册实现说明。
- 证据：三个 `skills/*/SKILL.md` 的文本扫描结果。

### AC-002 - 模板纯结构化

- Given 全局和项目模板，When 检查模板正文，Then 不包含 Skill 加载指令、规范归属说明、流程门禁或运行时 Skill 引用。
- 证据：模板扫描测试和人工审阅。

### AC-003 - 流程产物隔离

- Given `ae-sdd` 相关内容，When 检查运行时 Skill、模板和插件入口，Then 不出现 Phase、RA intake、gate、handoff、save_doc、resolve_path、状态文件或自动流程触发说明。
- 证据：关键字扫描和运行时上下文审阅。

### AC-004 - 项目模板选择

- Given `project_id: icec-cloud-life`，When 选择 DR、Story 或 TestCase 模板，Then 分别得到对应的 `templates/project/life-*.md`，且不加载项目级 `life-*` Skill。
- 证据：registry 结构、路径存在性检查和选择测试。

### AC-005 - 全局兜底

- Given 未知或无映射项目，When 选择 DR、Story 或 TestCase 模板，Then 使用对应的 `templates/global/*.md`。
- 证据：registry/加载契约测试。

### AC-006 - 无产品 CLI

- Given 插件 manifest 和发布目录，When 检查入口定义，Then 不存在 `al-spec` 命令、console script、bin 或其他产品 CLI 入口；维护脚本只在开发文档中标记为维护工具。
- 证据：manifest 检查和脚本定位说明。

### AC-007 - 发布一致性

- Given 干净重建的发布目录，When 运行 registry、Skill、模板和包内容校验，Then 源目录与发布包一致，且发布包不包含旧项目级 Skill。
- 证据：release manifest、zip 内容清单和安装后验证结果。

### AC-008 - 现有测试不回归

- Given 当前测试集，When 运行完整测试和校验，Then 所有测试通过，且 `git diff --check` 无错误。
- 证据：测试输出、registry validation、Skill validation 和 diff 检查结果。

## 9. 风险与未决问题

| ID | 风险/问题 | 影响 | 负责人 | 状态 |
|---|---|---|---|---|
| Q-001 | 历史 changelog 是否需要移动到独立 archive 目录，还是继续保留在 `CHANGELOG/` | 影响目录整洁度，不影响运行时 | 维护者 | open |
| Q-002 | 现有 `scripts/` 是否保留在正式发布包中 | 影响发布包大小和维护者体验 | 维护者 | open |
| Q-003 | 是否需要为发布包增加安装后读取/选择模板的行为测试 | 影响投产可信度 | 维护者 | open |

## 10. 回滚

若清理后的运行时内容导致规格生成行为回归，回滚到上一个已验证发布包；不恢复已删除的流程依赖到 Skill 中。历史资料和迁移记录独立保留，回滚只切换发布版本，不改变用户项目代码。
