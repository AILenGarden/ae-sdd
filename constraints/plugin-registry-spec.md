# Plugin Registry Schema 规范

## 摘要

本文件是 ae-sdd `registry.yaml` 的人读 schema SSOT。released runtime 的机器实现归属 `ae-sdd-integrations`，内容安全扫描归属 `ae-sdd-scanners`；禁止调用 Python loader/scanner。
适用场景：解析、校验、缓存、覆盖、新增或迁移 SKILL/template plugin。

当前 `schema_version: 1`。

---

## 一、分层与优先级

| 层 | 路径 | scope | git |
| --- | --- | --- | --- |
| L1 项目层 | `<project>/.ae-sdd/plugins/registry.yaml` | 单项目 | 否 |
| L2 用户全局层 | `~/.ae-sdd/plugins/registry.yaml` | 当前 OS 用户跨项目 | 否 |
| L3 仓库根层 | `<ae-sdd-master>/plugins/registry.yaml` | ae-sdd 发行源 | 是 |
| L0 built-in | `source/skills/` + `source/templates/` | 内置 fallback | 是 |

优先级固定为 `L1 > L2 > L3 > L0`。同一 `name`、`replaces` target 或 `provides` key 跨层冲突时选择最高层，并返回结构化 warning；禁止依赖文件系统遍历顺序。

L0 是数据内容 fallback，不是 Python runtime fallback。

## 二、顶层 schema

```yaml
schema_version: 1
description: optional text
plugins:
  - name: example-coding
    type: skill-override
    version: 1.0.0
    description: Replace the built-in coding contract.
    replaces: source/skills/phase2-coding/coding-skill.md
    path: ./example-coding/SKILL.md
```

| 字段 | 类型 | 必填 | 约束 |
| --- | --- | --- | --- |
| `schema_version` | integer | 是 | 当前只能为 `1` |
| `description` | string | 否 | 有界文本，不参与业务身份 |
| `plugins` | array | 是 | 可为空；顺序不决定跨层优先级 |

未知顶层字段在 schema v1 必须拒绝，避免拼写错误静默生效。

## 三、plugin 字段

### 3.1 公共字段

| 字段 | 类型 | 必填 | 约束 |
| --- | --- | --- | --- |
| `name` | string | 是 | `[a-z0-9-]+`，层内唯一 |
| `type` | enum | 是 | `skill-override`, `template-override`, `skill-new`, `template-new` |
| `version` | semver string | 是 | 完整 semver，不接受浮动 range |
| `description` | string | 是 | 非空，最多 120 Unicode scalar values |
| `path` | relative path | 是 | registry-relative，见 §4 |
| `author` | string | 否 | 人/团队显示信息 |
| `tags` | array[string] | 否 | 每项有长度上限，canonical 排序后参与 digest |
| `compatibility.ae_sdd_version` | semver range | 否 | 不满足时 warning/deny 由运行模式决定 |
| `dependencies` | array[string] | 否 | plugin name；schema v1 只校验格式/存在性并给 warning，不自动执行依赖代码 |

未知 plugin 字段必须拒绝。任何 extension 需升级 schema/protocol capability，不能靠宽松 YAML map 偷渡。

### 3.2 type-specific 字段

| type | 必须 | 禁止 | 语义 |
| --- | --- | --- | --- |
| `skill-override` | `replaces` | `provides` | 整体替换一个 built-in SKILL |
| `template-override` | `replaces` | `provides` | 整体替换一个 built-in template |
| `skill-new` | `provides` | `replaces` | 新增一个路由 key |
| `template-new` | `provides` | `replaces` | 新增一个模板 key |

`replaces` 必须精确命中 L0 inventory 的 canonical built-in path；`provides` 必须符合 `[a-z0-9-]+` 且在同层唯一。

`skill-extends` 不属于 schema v1。历史文档中“声明后按 override 处理”的歧义必须在 compatibility manifest 标为 breaking-fix；Rust released loader 不得静默改写未知 type。

## 四、路径解析与内容边界

- `path` 使用 `/` 分隔符，必须是相对路径；禁止绝对路径、drive/UNC prefix、`..`、空 segment 和 NUL/control character。
- 解析基准固定为当前 registry 文件所在目录，不以 process cwd 为基准。
- 先 canonicalize registry directory 与已存在 plugin file，再验证文件仍在 registry-approved plugin root 内；必须拒绝 symlink、junction/reparse-point 越界。
- `path` 必须指向普通文件，不得是目录、device、pipe 或 socket；单文件默认最大 1 MiB。
- load/使用/commit 前复核 metadata 与 SHA-256；watcher event 只用于 cache invalidation，不能代替内容验证。
- plugin 只提供 SKILL/template 文本数据；禁止把 registry path 当 native library、executable、shell script 或动态 Rust/Python module 执行。

## 五、解析、合并与 digest

1. 以固定 L1→L2→L3→L0 顺序发现 registry；不存在的层跳过，语法错误的已存在 registry 不得静默忽略。
2. 每层先完成 YAML/schema/path/content scan，再进入 merge；无效 plugin 不得产生半条目。
3. 同层 `name`、`replaces` 或 `provides` 重复必须阻断并报告两条 source location。
4. 跨层冲突按优先级选 winner，记录 winner/ignored layer/path/digest 的 warning。
5. compatibility、dependency missing/cycle 在 schema v1 返回结构化 finding；automation policy 可将 finding 升级为阻断。
6. 生成 deterministic `registryDigest`：包含 schema、层、canonical plugin fields、content SHA-256、built-in inventory digest 和 policy digest，禁止包含 absolute user path/mtime。
7. cache key 必须包含 registryDigest 与 runtime build/protocol；任一 registry/plugin/built-in/policy 变化都使旧 cache 失效。

## 六、校验规则

| # | 校验 | 失败 |
| --- | --- | --- |
| 1 | UTF-8/YAML 可解析、无 duplicate YAML key/anchor expansion abuse | 阻断 |
| 2 | `schema_version == 1`，`plugins` 为 array | 阻断 |
| 3 | 必填字段存在且类型正确，未知字段为 0 | 阻断 |
| 4 | name/provides/type/version/description 合法 | 阻断 |
| 5 | type-specific replaces/provides 互斥正确 | 阻断 |
| 6 | path 相对、无越界、普通文件、存在、大小合规 | 阻断 |
| 7 | replaces 命中 built-in inventory | 阻断 |
| 8 | 同层 name/replaces/provides 唯一 | 阻断 |
| 9 | 跨层冲突 deterministic winner | warning + winner |
| 10 | compatibility range 与 runtime version | warning；strict automation 可阻断 |
| 11 | dependency name 存在、无显式 self-cycle | warning；strict automation 可阻断 |
| 12 | content scanner 完成并按来源层策略裁决 | 见 §7 |

校验结果必须是 typed findings，包含 ruleId、severity、layer、plugin name、脱敏 relative source、remediation 和 input digest；禁止只打印字符串后继续加载。

## 七、内容安全扫描

Rust scanner 至少保留并版本化以下规则：

| 规则 | 等级 | 检测意图 |
| --- | --- | --- |
| PC-001 | BLOCKER | 无差别/越界递归删除 |
| PC-002 | BLOCKER | 任意命令执行、eval/exec/shell bypass |
| PC-003 | BLOCKER | 下载并直接执行远程脚本 |
| PC-004 | WARN | 绕过 ae-sdd Gate/approval 的指令 |
| PC-005 | WARN | 疑似硬编码 password/secret/api key/token |
| PC-006 | INFO | 内网 IP/环境耦合信息 |
| PC-007 | BLOCKER | 过度文件权限或提权指令 |
| PC-008 | WARN | `--no-verify`、force push 或绕过审计 |

来源层裁决：

| 层 | BLOCKER | WARN/INFO | scanner error |
| --- | --- | --- | --- |
| L1 项目 | warning + audit；automation 必须显式 project-owner confirmation 才可继续 | warning | automation 阻断；交互模式需确认并记录 |
| L2 用户全局 | 阻断 | warning | 阻断 |
| L3 仓库 | release CI 扫描阻断；runtime 校验已签 digest | release finding | release 阻断 |
| L0 built-in | release CI 扫描阻断；runtime 校验 built-in digest | release finding | release 阻断 |

- scanner panic、timeout 或未支持 encoding 不能当 clean；必须产生 `PLUGIN_SCAN_ERROR`。
- scanner 不执行 plugin 中的任何命令、macro 或 code block，只做有界静态解析。
- PC rule 变更必须更新 policy digest、golden corpus 与 compatibility classification。

## 八、稳定错误

至少提供：`PLUGIN_REGISTRY_SYNTAX`, `PLUGIN_SCHEMA_UNSUPPORTED`, `PLUGIN_FIELD_INVALID`, `PLUGIN_DUPLICATE_NAME`, `PLUGIN_TARGET_CONFLICT`, `PLUGIN_PATH_ESCAPE`, `PLUGIN_CONTENT_TOO_LARGE`, `PLUGIN_TARGET_MISSING`, `PLUGIN_COMPATIBILITY_MISMATCH`, `PLUGIN_DEPENDENCY_INVALID`, `PLUGIN_SCAN_BLOCKED`, `PLUGIN_SCAN_ERROR`。

错误必须关联 registryDigest/input digest；禁止泄露其他用户 registry 的 absolute path 或 plugin 内容正文。

## 九、兼容、迁移与测试

- Rust loader 必须用现有三层 fixture 与 Python oracle 做 shadow differential；差异逐项标记 preserve/breaking-fix，不能在 canary 期间双写或动态回退。
- fixture 覆盖合法四种 type、空 registry、未知字段/type、duplicate YAML key、同/跨层冲突、path traversal、symlink/junction、missing target、semver、dependency、1 MiB 边界和 PC-001~008。
- property test 必须证明 merge 结果不依赖 directory/YAML map 遍历顺序；相同规范化输入产生相同 registryDigest。
- release artifact 必须扫描 Python plugin loader/scanner execution entry 数量为 0。
- schema major 变更必须新增版本和 migration 文档；禁止原地改变 schema v1 已发布字段语义。

## 十、禁止事项

- 禁止动态 import/exec plugin、调用 Python loader、通过 shell 执行 plugin 内容。
- 禁止用 mtime、directory order 或首个匹配文件决定 winner。
- 禁止在 validation/scan 失败后回退到低层同 target 并假装安全；失败层的显式配置必须可见并按 policy 阻断。
- 禁止把 L1/L2 registry 或 plugin 正文写入 root ContextProjection、普通日志或跨 workspace cache。
