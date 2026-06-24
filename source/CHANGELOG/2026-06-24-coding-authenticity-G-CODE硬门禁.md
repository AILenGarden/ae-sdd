# 2026-06-24 | ae-sdd v3.2.1 — Coding 真实性 G-CODE 硬门禁

> **性质**：Coding 能力工具层加固。把 CodingModel §6 AI Coding 反模式库从“思维层清单”推进到“可执行扫描 + 门禁阻断 + CLI 入口 + 分发同步”。
>
> **影响范围**：新增 1 个扫描器 + 1 个门禁 + 1 个 CLI 入口 + 1 组测试 + 构建/同步脚本更新 + README/思维引擎文档修正。

## 背景

此前 CodingModel v1.5 已新增 AI Coding 反模式库（AP-1~AP-6），但主要停留在文档要求：

- LLM 需要主动扫描反模式；
- CodeReview 需要人工逐项检查；
- gates.py 只有 CodingPlan G-08 与测试真实性 G-09，没有独立的“代码真实性/反模式”门禁。

这导致 Coding 能力仍存在“文档强、工具弱”的缺口。RA 侧已经有 `ra_authenticity_scan.py` + G-RA-1~4，Coding 侧也需要同等级的运行时扫描。

## 改动 1：新增 `scripts/coding_authenticity_scan.py`

新增无第三方依赖的 Coding 真实性扫描器，输出契约对齐 `test_authenticity_scan.py` / `ra_authenticity_scan.py`：

```json
{
  "root": "...",
  "status": "PASS|FAIL",
  "codeFiles": 12,
  "codingReports": 1,
  "reportStats": {
    "codeFiles": 12,
    "codingReports": 1,
    "blockerFindings": 0,
    "warnFindings": 2
  },
  "findings": []
}
```

当前覆盖 AP-1~AP-6 中可静态命中的部分：

| 反模式 | 扫描信号 |
|---|---|
| AP-1 幻觉 API | 已知幻觉参数/不存在注解参数，如 `@TransactionalEventListener(... fallback=...)` |
| AP-2 抄旧代码 | 旧式 API/版本错位信号，如 `WebSecurityConfigurerAdapter` |
| AP-3 过度设计 | `Strategy/Factory/Builder/Template/Abstract/Base` 抽象命名密集出现（WARN） |
| AP-4 注释撒谎 | 生产代码残留 `TODO/FIXME/XXX`、`@SuppressWarnings` |
| AP-5 默认值陷阱 | 硬编码 secret/token/url/timeout/retry/TTL |
| AP-6 上下文漂移 | Coding 报告引用不存在的代码文件 |

BLOCKER 命中会使门禁失败；WARN 要求 CodeReview 解释。

## 改动 2：新增 `G-CODE-1`

`tools/lib/gates.py` 新增：

- `G-CODE-1 Coding 真实性扫描通过`
- `_locate_coding_scanner()`
- `check_gcode1()`
- `check_all(... only="G-CODE-1")` 特判

GATE_REGISTRY 从 18 项扩展为 19 项：

```text
14 主门禁 + 4 G-RA + 1 G-CODE = 19
```

状态切换门禁也同步增强：

```text
进入 code-reviewed 前：
G-00 + G-09 + G-CODE-1 + G-10 + G-11
```

## 改动 3：新增 CLI 入口

`tools/bin/ae-sdd` 新增：

```bash
ae-sdd gate coding-required [--project <dir>] [--json]
```

该命令用于 Coding 完成 / CodeReview 前独立运行 G-CODE-1。

`ae-sdd health` 新增 G-CODE 扫描器可用性检查，避免安装包缺脚本时静默降级。

## 改动 4：doc 转实例 / 分发工具同步

`scripts/build_dist.py` 已把运行时脚本列表从：

```text
test_authenticity_scan.py
ra_authenticity_scan.py
```

扩展为：

```text
test_authenticity_scan.py
ra_authenticity_scan.py
coding_authenticity_scan.py
```

原因：`G-CODE-1` 在分发/安装环境中运行时必须能定位 `scripts/coding_authenticity_scan.py`。如果只改母版脚本而不改 build-dist，Codex/Claude 实际加载的实例包会缺脚本，门禁只能 stub 跳过。

同时 `scripts/dev_sync.py --watch` 从只监听 `source/` 扩展为监听：

```text
source/
tools/
scripts/
```

原因：v3.2 起运行时能力不只在 `source/`，还包括 `tools/` 和 `scripts/`。继续只监听 `source/` 会导致脚本/CLI 更新后不会自动 build + install。

## 改动 5：文档事实修正

修正 `be-coding-thinking-engine.md` 中错误的门禁编号说明：

- G-08 = CodingPlan 14 门禁
- G-09 = 测试真实性扫描
- G-CODE-1 = Coding 真实性扫描
- G-RA-4 = RA 真实性扫描

同时将“CodeReview / dr-review 阶段扫代码”修正为“Coding / CodeReview 阶段扫代码”。`dr-review` 属于设计评审，不应承担代码反模式扫描职责。

## 验证方式

```bash
python -m pytest tools/tests/test_gates.py -q
python scripts/coding_authenticity_scan.py --root <sample-project> --format json
python tools/bin/ae-sdd gate coding-required --help
python scripts/build_dist.py
```

## 后续建议

G-CODE-1 当前是保守静态扫描，重点拦截可确定风险。后续可以继续增强：

- 结合语言 AST 或编译器输出减少误伤；
- 增加 CodingPlan ↔ 代码改动矩阵；
- 增加 Coding 报告 ↔ git diff ↔ 测试文件三方对账；
- 将 WARN 要求写入 CodeReview 报告模板。
