---
name: test-tool-score
description: |
  test-tool 能力测试自动评分 skill。当用户说 "/test-tool-score"、"评分 test-tool"、
  "给 test-tool 打分"、"评估 ae-sdd 能力测试"、"scoring test-tool" 时触发。
  自动采集 cargo fmt/clippy/test 输出、ae-sdd state.json/evidence/runtime-stats、
  代码静态统计，按 EVALUATION.md 四维度评分卡计算分数，落地符合 metrics.schema.json
  的 metrics-<STORY-ID>-run<N>.json。支持多次 run 横向对比。
trigger_when:
  - "/test-tool-score"
  - "评分 test-tool"
  - "给 test-tool 打分"
  - "评估 ae-sdd 能力测试"
  - "scoring test-tool"
  - "score test-tool"
  - "test-tool 评分"
---

# test-tool-score Skill

## 用途

为 `demo/test-tool/` 这道 ae-sdd 能力标准测试题自动评分。一次完整的 ae-sdd 流程跑完后
（Route → RA → DR → Story → CodingPlan → [用户批准] → Coding → Test → Review），
调用本 skill 即可：

1. **采集**：自动跑 `cargo fmt --check` / `cargo clippy` / `cargo test`、读
   `.auto-engineering/<story>/state.json`、`.auto-engineering/<story>/evidence/manifest.json`、
   `.ae-sdd/runtime-stats/<date>.jsonl`、grep 代码统计。
2. **评分**：按 `demo/test-tool/EVALUATION.md` 四维度评分卡（A 流程合规性 35% +
   B 能力项覆盖度 25% + C 完成度与质量 25% + D 效率指标 15%）自动计算。
3. **落地**：生成符合 `demo/test-tool/metrics.schema.json` 的
   `demo/test-tool/metrics-<STORY-ID>-run<N>.json`。
4. **对比**：若已有历史 metrics JSON，自动生成横向对比表（总分 + 派生指标）。

## 强制前置（操作员手动提供，skill 不能自动抓）

skill 无法从仓库自动读取以下数据，必须由操作员在调用时提供（或后续手动补填）：

| 数据 | 来源 | 必填 |
| ---- | ---- | ---- |
| Story ID | 如 `STORY-DEMO-TEST-TOOL-001` | ✅ |
| run index | 第几次跑（1, 2, 3...） | ✅ |
| 总耗时 / 各阶段耗时 | 操作员记录的时间戳 | ✅ |
| token input / output | ZCode UI 或客户端日志 | ✅ |
| 对话轮次数 | 操作员计数 | ✅ |
| 跑前 state.revision 基线 | 跑前 `ae-sdd state` 输出 | ✅ |
| 模型名 / 单价 | 用于成本估算 | 可选（无则跳过 cost） |
| prompt cache 命中 token | 客户端日志 | 可选 |

## 工作流（必须按顺序执行）

### Step 0：前置检查

```bash
# 1. 确认在仓库根
test -f "D:/Item/ae-sdd/Cargo.toml" || echo "ERROR: 必须在 ae-sdd 仓库根运行"

# 2. 确认 demo/test-tool/ 存在
test -d "D:/Item/ae-sdd/demo/test-tool" || echo "ERROR: demo/test-tool 不存在"

# 3. 确认有 ae-sdd state 目录（至少一个 Story）
ls "D:/Item/ae-sdd/.auto-engineering/" | grep -i "STORY-DEMO-TEST-TOOL" || echo "WARN: 未找到 STORY-DEMO-TEST-TOOL state 目录"
```

### Step 1：向操作员收集强制前置数据

用 AskUserQuestion 或直接对话，收集：
- Story ID（必须匹配 `^STORY-DEMO-TEST-TOOL-\d{3}$`）
- run index
- 总耗时（分钟）
- 8 个 phase 耗时
- token input/output
- 对话轮次
- 跑前 state.revision
- （可选）模型名、单价、缓存 token

### Step 2：自动采集

```bash
python "D:/Item/ae-sdd/demo/test-tool/score-skill/scripts/collect.py" \
    --repo "D:/Item/ae-sdd" \
    --story-id "STORY-DEMO-TEST-TOOL-001" \
    --operator "user:cc" \
    --ae-sdd-version "3.14.0" \
    --host-agent "ZCode" \
    --model-id "glm-4.5" \
    --total-minutes 75 \
    --phase-route 2 --phase-ra 8 --phase-dr 10 --phase-story 12 \
    --phase-coding-plan 5 --phase-coding 20 --phase-test 10 --phase-review 8 \
    --tokens-input 1200000 --tokens-output 80000 --tokens-cached 0 \
    --turn-count 45 \
    --baseline-revision 74 \
    --input-price 0.5 --output-price 1.5 \
    --started-at "2026-07-25T10:00:00Z" \
    --finished-at "2026-07-25T11:15:00Z" \
    --output "D:/Item/ae-sdd/demo/test-tool/.collected.json"
```

`collect.py` 会自动跑：
- `cd demo/test-tool && cargo fmt --check`
- `cd demo/test-tool && cargo clippy --all-targets -- -D warnings`
- `cd demo/test-tool && cargo test --no-fail-fast` （解析通过/失败的 AC）
- `cd demo/test-tool && cargo test -- --list` （统计测试用例数）
- `git ls-files` + `cloc`/`tokei`（若有）或 `wc -l`（统计 LOC）
- `grep` 统计 unsafe/unwrap/todo
- 读 `.auto-engineering/<story>/state.json`
- 读 `.auto-engineering/<story>/evidence/manifest.json`
- 读 `.ae-sdd/runtime-stats/<date>.jsonl`
- 计算关键文件的 sha256

输出：`.collected.json`（中间产物，含所有原始采集数据）。

### Step 3：评分

```bash
python "D:/Item/ae-sdd/demo/test-tool/score-skill/scripts/score.py" \
    --collected "D:/Item/ae-sdd/demo/test-tool/.collected.json" \
    --evaluation "D:/Item/ae-sdd/demo/test-tool/EVALUATION.md" \
    --output "D:/Item/ae-sdd/demo/test-tool/metrics-STORY-DEMO-TEST-TOOL-001-run1.json"
```

`score.py` 会按 `EVALUATION.md` 的评分规则，把 `.collected.json` 转成最终 metrics JSON：
- 维度 A：检查 state.json 的 routeDecision / history / executionPlan.approved / evidence 三 gate
- 维度 B：检查能力项触发证据
- 维度 C：必修 AC 通过情况 + 边界覆盖 + 契约对齐 + 工程质量 + 选修组完成情况
- 维度 D：时长分档 + 所有派生指标计算
- 计算综合总分、grade、capabilityTier
- 落地 `metrics-<STORY-ID>-run<N>.json`

### Step 4：校验

```bash
python "D:/Item/ae-sdd/demo/test-tool/score-skill/scripts/validate.py" \
    --metrics "D:/Item/ae-sdd/demo/test-tool/metrics-STORY-DEMO-TEST-TOOL-001-run1.json" \
    --schema "D:/Item/ae-sdd/demo/test-tool/metrics.schema.json"
```

校验生成的 metrics JSON 是否符合 schema。失败则提示哪里不符。

### Step 5：横向对比（可选）

若 `demo/test-tool/` 下已有多份 `metrics-*.json`，自动汇总对比表：

```
| run | STORY-ID | 总分 | A | B | C | C_ceiling | D | 总耗时 | input tok | output tok | LOC | 测试通过率 | 成本(USD) |
| --- | -------- | ---- | -- | -- | -- | --------- | -- | ------ | --------- | ---------- | --- | ---------- | --------- |
| 1   | ...001   | 82.5 | 88 | 92 | 78 | 18        | 80 | 75     | 1.2M      | 80K        | 850 | 100%       | 0.72      |
| 2   | ...002   | 85.0 | 90 | 95 | 80 | 22        | 80 | 70     | 1.1M      | 75K        | 920 | 100%       | 0.68      |

关键派生指标：
| run | 分/分钟 | 分/百万tok | 分/美元 | LOC/分钟 | AC/分钟 | BLOCKER占比 | 缓存命中率 |
| --- | ------- | ---------- | ------- | -------- | ------- | ----------- | ---------- |
| 1   | 1.10    | 64.5       | 114.6   | 11.3     | 0.16    | 0%          | 0%         |
| 2   | 1.21    | 72.7       | 125.0   | 13.1     | 0.18    | 0%          | 0%         |
```

### Step 6：清理中间产物

```bash
rm -f "D:/Item/ae-sdd/demo/test-tool/.collected.json"
```

## 输出文件命名规则

`metrics-<STORY-ID>-run<N>.json`，例如：
- `metrics-STORY-DEMO-TEST-TOOL-001-run1.json`
- `metrics-STORY-DEMO-TEST-TOOL-002-run1.json`
- `metrics-STORY-DEMO-TEST-TOOL-001-run2.json`（同一 Story 重跑）

## 失败处理

| 失败场景 | 处理 |
| -------- | ---- |
| Story state 目录不存在 | 报错并提示检查 Story ID；可能 ae-sdd 流程没走完 |
| `cargo test` 失败 | 仍然评分，但维度 C 的 AC 项计 0 分；在 notes 里标注 |
| `cargo fmt` / `clippy` 失败 | 计入维度 C 工程质量扣分；继续评分 |
| evidence/manifest.json 缺失 | 维度 A 的 A11 红线触发 → A=0；继续评分但总分受影响 |
| runtime-stats jsonl 不存在 | 跳过 cliInvocations / gateBlocks 统计，填 0 |
| schema 校验失败 | 报错并指出哪个字段不符；不落地 metrics JSON |

## 安装方式

本 skill 写在仓库内 `demo/test-tool/score-skill/`，**需要手动安装到用户目录**才能被 ZCode 自动识别：

```bash
# Windows
cp -r "D:/Item/ae-sdd/demo/test-tool/score-skill" "C:/Users/EDY/.zcode/skills/test-tool-score"

# 或建符号链接（推荐，便于随仓库更新）
cmd //c "mklink /D \"C:\\Users\\EDY\\.zcode\\skills\\test-tool-score\" \"D:\\Item\\ae-sdd\\demo\\test-tool\\score-skill\""
```

安装后重启 ZCode，输入 `/test-tool-score` 或自然语言「评分 test-tool」即可触发。

## 依赖

- Python 3.10+（仓库已用 Python 跑 ae-sdd CLI，假定已装）
- 仅标准库（json/pathlib/subprocess/hashlib/argparse/datetime/re）
- `cargo` / `rustfmt` / `cargo-clippy`（仓库 toolchain 已固定 1.97.1）
- 可选：`cloc` 或 `tokei`（若无则降级为 `wc -l`）
- 可选：`jsonschema` Python 包（validate.py 用；若无则降级为基本类型检查）

## 不可逾越的红线

- skill 只读，不修改 ae-sdd state、evidence、代码、EVALUATION.md、metrics.schema.json
- skill 不替 ae-sdd 走流程，只在流程跑完后采集评分
- 红线检查（A1/A6/A8/A9/A11）必须基于真实文件证据，不能凭操作员口述
- 若 evidence 伪造（如 manifest.json 存在但 snapshot sha256 不匹配），skill 应在 A11 标 `forged: true` 并把 A 强制 0
