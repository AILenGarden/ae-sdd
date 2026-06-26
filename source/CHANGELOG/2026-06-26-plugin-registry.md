# v3.5.0 — 三层 SKILL 注册表插件化体系（2026-06-26）

## 核心目标

把 ae-sdd 从"hard-coded SKILL 体系"升级为"内置 + 三层可外挂注册"的插件化框架。

每个项目团队 / 个人开发者 / ae-sdd 维护者，**各自在自己的 scope 内定制 CodingSKILL / 模板**，
互不污染、零 PR、零等待。

## 三层优先级链

| 层 | 路径 | scope | 典型使用人 | git |
|---|------|-------|-----------|-----|
| **L1 项目层** | `<project>/.ae-sdd/plugins/registry.yaml` | 单项目 | 项目 owner / Tech Lead | ❌ |
| **L2 用户全局层** | `~/.ae-sdd/plugins/registry.yaml` | 单用户跨项目 | 个人开发者 | ❌ |
| **L3 仓库根层** | `<ae-sdd-master>/plugins/registry.yaml` | ae-sdd 母版 | ae-sdd 团队发布 | ✅ |
| **L0 fallback** | `source/skills/` + `source/templates/` | ae-sdd 母版 | ae-sdd 团队 | ✅ |

**优先级：** L1 > L2 > L3 > L0。**零破坏保证：** 三层都未声明 → fallback 到内置 → 行为与 v3.4.x 完全一致。

## 新增文件

### 文档（4 个）

| 文件 | 职责 |
|------|------|
| `source/docs/plans/2026-06-26-plugin-registry-design.md` | 设计文档（D 方案权威说明） |
| `source/standards/constraints/plugin-registry-spec.md` | schema 规范（权威源） |
| `source/templates/project-assets/plugin-registry-template.yaml` | 三层通用注册表模板 |
| `source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md` | 加载协议 SOP + 用户注册流程引导 |

### Python 实现 + 测试（2 个）

| 文件 | 内容 |
|------|------|
| `tools/lib/plugin_loader.py` | 三层注册表加载器（YAML 子集解析 + 优先级合成 + 冲突检测 + 兼容性校验）。零外部依赖。 |
| `tools/tests/test_plugin_loader.py` | 35 个单元测试（YAML 解析 / 单层加载 / 三层合成 / 冲突检测 / fallback） |

### 示例（2 个）

| 文件 | 用途 |
|------|------|
| `plugins/_example-coding-style/SKILL.md` | 仓库根层 scaffolding 示例（不自动加载） |
| `plugins/_example-coding-style/README.md` | 示例说明 + 启用步骤 |

## 改动文件

| 文件 | 变更 |
|------|------|
| `source/SKILL.md` | frontmatter version `3.4.3` → `3.5.0` + 加 v3.5.0 描述行 + 路由决策算法新增 step 2.5「🔌 SKILL 注册表加载」 |
| `source/standards/update-graph.json` | 新增 UG-12（注册表 / 插件 SKILL / loader 改动依赖图） |
| `tools/lib/paths.py` | MASTER_VERSION `3.4.3` → `3.5.0` |
| `README.md` | 第 5 行版本行更新 + 新增 ## 🔌 SKILL 注册与外挂指南 章节 + Q7 |

## 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 注册表位置 | 三层叠加 | "每个人的 CodingSKILL 都不同" — 项目/团队/个人 三种 scope 都需要 |
| 注册 SKILL 形态 | 独立 SKILL 文件 + 模板 | 与 ae-sdd 自身 SKILL 习惯一致 |
| Schema 详细度 | 完整 plugin manifest | 包含 name/type/version/replaces/path/compatibility — 完整可校验 |
| 加载时机 | 路由 step 2.5（step 3 之前）| 对上层透明，路由算法不感知 |
| YAML 解析 | 自写极简（零依赖） | 跟 paths.py read_config 风格一致 |
| 冲突处理 | 多层冲突按优先级胜者 + warn | 不阻断，仅告警（用户体验优先）|
| fallback | 三层未命中 → 内置 | 零破坏，向后兼容 v3.4.x |

## 已知缺口 / 留待下个 PR

1. **`extends` 类型章节级合并** —— v3.5.0 schema 允许 `type: skill-extends`，但 loader 仍按 `skill-override` 整体替换处理。完整实现留待 v3.6.0。
2. **CLI `plugin` 子命令挂载** —— v3.5.0 只完成 Python 模块；CLI 注册到 `tools/bin/ae-sdd`（add_parser）留待 v3.5.1。
3. **依赖解析** —— `dependencies` 字段声明但 loader 不强制校验；v3.5.0 仅做提示。
4. **缓存** —— 每次加载 SKILL 重新读注册表；高频场景需要缓存。留待性能 profiling 后优化。
5. **GUI 化注册向导** —— CLI `plugin init` 是最小可用；后续考虑加交互式向导。

## 用户使用流程（5 步）

1. **选层** —— L1 项目 / L2 全局 / L3 仓库根
2. **生成注册表** —— `cp source/templates/project-assets/plugin-registry-template.yaml <目标路径>`
3. **填字段 + 写外挂 SKILL** —— 按 schema 填 plugins 列表 + 写 SKILL 文档
4. **验证** —— `ae-sdd plugin validate` + `ae-sdd plugin trace <skill>`
5. **测试** —— 实际跑 ae-sdd 流程，看是否命中外挂 SKILL

## 验证状态

- ✅ 35/35 单元测试通过（`tools/tests/test_plugin_loader.py`）
- ✅ YAML 解析器覆盖所有 schema 用法（list of dict / 嵌套 dict / list of scalar / literal block / comments / doc separator）
- ✅ 三层优先级合成正确（L1 > L2 > L3 > L0）
- ✅ 冲突检测正确（按 layer 数字 + name 排序选胜者）
- ✅ fallback 正确（三层未命中 → LAYER_BUILTIN）
- ✅ 校验规则 15 条全实现（schema_version/type/path/.. / replaces/provides/name 唯一性 / replaces 唯一性 / path 存在）

## 后续 PR 规划

- **v3.5.1** —— CLI `plugin` 子命令（list/validate/trace/init）挂载到 `tools/bin/ae-sdd`
- **v3.6.0** —— `extends` 类型章节级合并算法实现
- **v3.7.0** —— 注册表缓存 + 性能 profiling
- **v3.8.0** —— GUI 化注册向导（可选）