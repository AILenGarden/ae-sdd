# Sonar Issue Fix Rule Registry

## 目的

本文件是 `sonar-issue-fix-skill` 的硬编码规则 SSOT。条目描述独立、可审计的保守编辑配方，不包含 Sonar 分析器实现，也不尝试在本地重做规则判定。Sonar issue 是“应修哪里”的权威输入；本注册表只在严格前置条件下把已确认 issue 转成最小 edit。

## 注册表 Schema

每个可自动应用条目必须包含：

| 字段 | 要求 |
| --- | --- |
| `ruleKey` | 完整语言限定 key |
| `status` | `enabled` / `disabled` |
| `analyzerEvidence` | 官方规则/文档链接、观察到的 analyzer 版本或兼容范围 |
| `riskClass` | 只允许 `syntax-local` 条目自动执行 |
| `preconditions` | issue、文件、范围和语法的全部前置条件 |
| `edit` | 有界 `TextEdit(range, newText)` 生成方法 |
| `negativeExamples` | 必须 skip 的负例 |
| `verification` | 语法/compile/test/Sonar 复扫要求 |
| `owner` | 维护责任与最近复核日期 |

缺一字段、版本未知且无法证明兼容、负例未覆盖或验证命令不可用时，条目视为 `disabled`，issue 降级到 `reasoned`/`manual`。

## 已启用条目

### `java:S1128` - Remove unused imports

| 字段 | 当前契约 |
| --- | --- |
| `ruleKey` | `java:S1128` |
| `status` | `enabled` |
| `analyzerEvidence` | [Sonar Java rule S1128](https://rules.sonarsource.com/java/RSPEC-1128/)；官方 SonarJava 行为公开显示该规则支持 quick fix，但本配方不复制其实现 |
| `riskClass` | `syntax-local` |
| `owner` | ae-sdd maintainers；首次复核 2026-07-14 |

前置条件（全部满足）：

1. issue 的 `ruleKey` 必须精确等于 `java:S1128`，目标是仓库内、非 generated/vendor/cache 的 `.java` 文件。
2. 当前文件 SHA-256 必须等于 EditPlan 的 `baseSha256`，且 issue range 能唯一定位到单条 unused import declaration。
3. 被删除文本去掉行尾后，必须是单条 `import qualified.Name;` 或 `import static qualified.Name.member;`；不得把同一行上的其他语句纳入。
4. import 声明及其行尾不得带 `//`、`/* */`、注解、抑制说明或需要保留的人工注释。
5. 删除范围只能覆盖该 import 声明和紧随其后的一个原始换行符；文件无末尾换行时只删除声明。不得顺手整理空行或重新排序其他 import。
6. range 不与同批其他 edit 重叠；若删除会让任何后续旧坐标失效，应用后先复扫再处理下一批。

Edit：构造一条 `TextEdit`，`range` 为上述完整 import 行，`newText` 为 `""`。保留文件原有 UTF-8/BOM 与 CRLF/LF 风格。

负例与 skip 条件：

- 只有 `quickFix=true` 提示，没有实际 issue range 或无法唯一映射到 import 行：`skip`。
- range 横跨两条 import、类声明、package 声明或注释：`skip`。
- import 行含 trailing comment、block comment 或 suppression 说明：`skip`，转 `reasoned`。
- 文件 hash 陈旧、路径逃逸、符号链接离开仓库、edit 重叠：整条计划 `skip`。
- 同一 issue 要改多个文件，或当前应用工具不能保证多文件原子性：整批 `skip`。
- analyzer 版本发生语义漂移，或规则不再表示 unused import：将条目标记 `disabled` 后降级。

验证：Java 语法/compile 成功；执行受影响模块的 focused test；用相同 analyzer 配置重跑 Sonar，原 issue 消失且无新增 Blocker/Critical 问题。任一步不可执行都只能报 `unverified`，不能报 `fixed`。

## 强制人工类别

以下类别即使上游显示 quick fix，也默认 `manual`，除非未来独立安全评审明确建立更窄契约：

- security/taint 漏洞与 Security Hotspot。
- 认证、授权、会话、密码学、密钥和敏感数据处理。
- 并发、锁、异步时序和资源生命周期。
- 事务边界、幂等性和跨系统一致性。
- 公共 API、序列化、数据库 schema 或持久化语义。

不得盲改这些问题；输出根因证据、风险、负责人和所需测试即可。

## Analyzer 漂移策略

注册项不会仅因 `ruleKey` 相同而永久兼容。每次应用都记录 analyzer 名称/版本和规则元数据：

1. 版本落在已验证范围且语义未变，继续核对全部前置条件。
2. 版本未知或 rule title/type/scope 与证据不一致，降级 `reasoned`。
3. 出现一次错误编辑、错误定位或复扫回归，立即 `disabled`，补负例和测试后才可恢复。
4. 新规则必须先有真实正例、负例、EditPlan、防陈旧测试和复扫证据，不能从 SonarJava 源码复制实现后直接登记。

## 许可证与秘密信息

SonarJava 当前许可证为 `Sonar Source-Available License v1.0`。本注册表只能引用官方规则页面、公开文档和独立观察结果；不得复制或移植 SonarJava 分析器/quick-fix 代码，尤其不得让非程序化 AI 摄取其实现后生成等价配方。

Sonar token/令牌必须来自环境变量/env 或既有秘密存储。示例、日志、测试夹具和报告只能使用明显的假值，不得提交真实凭据。
