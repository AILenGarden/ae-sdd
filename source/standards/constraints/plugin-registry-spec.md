# Plugin Registry Schema 规范（🆕 v3.5.0）

> **权威源：** 本文件是 registry.yaml 的 schema 权威文档。`tools/lib/plugin_loader.py` 是机器可读实现；本文件是人读视图。
>
> **适用范围：** ae-sdd v3.5.0+ 三层注册表（L1 项目层 / L2 全局层 / L3 仓库根层）。
>
> **schema_version：** `1`（v3.5.0 起固定）。

---

## 一、三层注册表定义

| 层 | 路径（按优先级降序） | scope | 典型使用人 | git |
|---|---------------------|-------|-----------|-----|
| **L1 项目层** | `<project>/.ae-sdd/plugins/registry.yaml` | 单项目 | 项目 owner / Tech Lead | ❌ |
| **L2 全局层** | `~/.ae-sdd/plugins/registry.yaml` | 单用户跨项目 | 个人开发者 | ❌ |
| **L3 仓库根层** | `<ae-sdd-master>/plugins/registry.yaml` | ae-sdd 母版 | ae-sdd 团队发布 | ✅ |
| **L0 fallback** | `source/skills/` + `source/templates/` | ae-sdd 母版 | ae-sdd 团队 | ✅ |

**优先级链：** L1 > L2 > L3 > L0（三层都未命中时 fallback 到内置 SKILL）。

---

## 二、registry.yaml 顶层 schema

```yaml
schema_version: 1            # 必填；当前固定 "1"
description: <text>          # 可选；注册表说明（多行用 |）
plugins: [<plugin>, ...]     # 必填；插件清单（数组，可空）
```

### 2.1 顶层字段表

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `schema_version` | int | ✅ | 当前固定 1 |
| `description` | string | ❌ | 注册表说明 |
| `plugins` | array | ✅ | 插件清单（数组可空 `[]`） |

---

## 三、plugin 字段 schema

### 3.1 公共字段（所有 type 必填/可选）

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 全局唯一（跨层）；kebab-case；`[a-z0-9-]+` |
| `type` | enum | ✅ | `skill-override` / `template-override` / `skill-new` / `template-new` |
| `version` | semver string | ✅ | 语义化版本（如 `0.1.0`） |
| `author` | string | ❌ | 作者名 / 团队名 |
| `description` | string | ✅ | 一句话说明（≤120 字符） |
| `path` | relative path | ✅ | 外挂 SKILL/模板的相对路径（相对注册表所在目录） |
| `tags` | array[string] | ❌ | 分类标签 |

### 3.2 type-相关字段

| type | replaces（必填） | provides（禁用） | 备注 |
|------|------------------|------------------|------|
| `skill-override` | ✅ | ❌ | 覆盖内置 SKILL |
| `template-override` | ✅ | ❌ | 覆盖内置模板 |
| `skill-new` | ❌ | ✅ | 新增 SKILL |
| `template-new` | ❌ | ✅ | 新增模板 |

### 3.3 可选元数据

| 字段 | 类型 | 说明 |
|------|------|------|
| `compatibility.ae_sdd_version` | semver range | 兼容性要求（如 `>=3.5.0`） |
| `dependencies` | array[string] | 依赖的其他插件 name 列表（v3.5.0 仅声明，不强制校验）|

---

## 四、path 解析规则

| 规则 | 说明 |
|------|------|
| 相对基准 | 相对注册表所在目录（如 `L1` 是 `<project>/.ae-sdd/plugins/registry.yaml`，则 `./plugins/foo/SKILL.md` 指 `<project>/.ae-sdd/plugins/foo/SKILL.md`） |
| 分隔符 | 统一用 `/`（YAML 跨平台约定，loader 内部处理 Windows 反斜杠） |
| 禁止 `..` | 路径中禁止出现 `..`（loader 校验，防止越权读注册表外文件） |
| 必须存在 | loader 校验 path 必须指向真实存在的文件，否则阻断 |

---

## 五、覆盖类型语义

### 5.1 replaces（整体替换）

```yaml
- name: boss-coding-style
  type: skill-override
  replaces: source/skills/phase2-coding/coding-skill.md  # 内置路径
  path: ./plugins/boss-coding/SKILL.md
```

**加载行为：** 命中后，loader 返回外挂 path，整个 SKILL 内容替换内置。

### 5.2 provides（新增）

```yaml
- name: boss-finance-coding
  type: skill-new
  provides: boss-finance-coding-skill    # 新 SKILL 的引用 key
  path: ./plugins/boss-finance/SKILL.md
```

**加载行为：** SKILL 路由层看到这个 key 时（如用户输入 "用 finance coding"）→ loader 返回外挂 path。

### 5.3 extends（v3.5.0 暂未实现，留待 v3.6.0）

```yaml
- name: boss-coding-ext
  type: skill-extends                   # ⚠️ v3.5.0 schema 允许声明但 loader 按 skill-override 处理
  replaces: source/skills/phase2-coding/coding-skill.md
  path: ./plugins/boss-coding/extensions.md
```

**v3.5.0 行为：** loader 把 type=`skill-extends` 视为 `skill-override`，整体替换。后续 v3.6.0 实现章节级合并。

---

## 六、冲突处理

### 6.1 多层冲突

同一内置 SKILL `S` 被多层注册表同时覆盖：

```
L1 项目层: replaces S → ./boss-coding/SKILL.md
L2 全局层: replaces S → ./personal-coding/SKILL.md
L3 仓库根层: replaces S → ./official-coding/SKILL.md
```

**处理：**
1. 按优先级选胜者（**L1 胜出**，加载 boss-coding）
2. 🟡 输出冲突告警：`WARN: plugin 'personal-coding' (L2) 与 'boss-coding' (L1) 都覆盖了 coding-skill.md；L1 胜出，L2 被忽略`
3. 不阻断流程

### 6.2 单层冲突（同一注册表内 name 重复）

**处理：🔴 阻断** —— 注册表 YAML 解析时检测到同一注册表内 `name` 重复 → 报错指出哪个 name 重复、哪两行。

### 6.3 单层冲突（同一注册表内 replaces 同一 target）

```
plugins:
  - name: a-coding
    replaces: source/skills/phase2-coding/coding-skill.md
  - name: b-coding
    replaces: source/skills/phase2-coding/coding-skill.md  # 同一注册表内重复覆盖
```

**处理：🔴 阻断** —— 同一注册表内同一 target 被多次覆盖 → 报错要求用户二选一。

---

## 七、校验规则汇总

loader 跑 sanity check 时的校验项：

| # | 校验项 | 失败行为 |
|---|--------|---------|
| 1 | YAML 语法可解析 | 🔴 阻断 |
| 2 | `schema_version` 存在且 = 1 | 🔴 阻断 |
| 3 | 每个 plugin 的 `name/type/version/description/path` 存在 | 🔴 阻断 |
| 4 | `type` ∈ {skill-override, template-override, skill-new, template-new} | 🔴 阻断 |
| 5 | `name` 符合 `[a-z0-9-]+` 正则 | 🔴 阻断 |
| 6 | `version` 是合法 semver | 🔴 阻断 |
| 7 | `type=skill-override/template-override` 时 `replaces` 存在 | 🔴 阻断 |
| 8 | `type=skill-new/template-new` 时 `provides` 存在 | 🔴 阻断 |
| 9 | `path` 不含 `..` | 🔴 阻断 |
| 10 | `path` 指向文件真实存在 | 🔴 阻断 |
| 11 | `replaces` 指向的内置路径真实存在（builtins 校验） | 🔴 阻断 |
| 12 | 同一注册表内 `name` 唯一 | 🔴 阻断 |
| 13 | 同一注册表内 `replaces` 唯一 | 🔴 阻断 |
| 14 | 多层冲突时按优先级选胜者 + 告警 | 🟡 告警（不阻断） |
| 15 | `compatibility.ae_sdd_version` 不满足 | 🟡 告警（不阻断） |

---

## 八、向后兼容

- v3.4.x 行为完全保留：未启用插件的项目 loader 全部走 fallback 路径，行为与 v3.4.3 完全一致。
- schema_version = 1 是当前唯一版本；未来 schema 变更时按 semver major bump 处理（v4.0.0 起 schema_version = 2）。

---

## 九、参考示例

完整三层注册表示例见 `templates/project-assets/plugin-registry-template.yaml`。