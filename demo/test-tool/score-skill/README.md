# test-tool-score Skill

> ae-sdd 能力测试自动评分 skill。当用户跑完一次 ae-sdd 流程后，调用本 skill 即可自动采集证据、
> 计算分数、落地符合 schema 的 metrics JSON。

## 文件结构

```
score-skill/
├── SKILL.md              # 主入口（ZCode skill 合同）
├── README.md             # 本文件
├── requirements.txt      # 可选依赖（jsonschema）
└── scripts/
    ├── collect.py        # 采集 cargo/state/evidence/runtime-stats/code stats
    ├── score.py          # 按 EVALUATION.md 四维度计算分数 + 派生指标
    └── validate.py       # 校验 metrics JSON 是否符合 schema
```

## 安装

本 skill 写在仓库内，需要手动安装到用户目录才能被 ZCode 自动识别。

### Windows（符号链接，推荐）

```bash
# 以管理员身份运行
cmd //c "mklink /D \"C:\\Users\\EDY\\.zcode\\skills\\test-tool-score\" \"D:\\Item\\ae-sdd\\demo\\test-tool\\score-skill\""
```

### Windows（拷贝）

```bash
cp -r "D:/Item/ae-sdd/demo/test-tool/score-skill" "C:/Users/EDY/.zcode/skills/test-tool-score"
```

### macOS / Linux

```bash
ln -s "D:/Item/ae-sdd/demo/test-tool/score-skill" "$HOME/.zcode/skills/test-tool-score"
```

安装后重启 ZCode，输入 `/test-tool-score` 或自然语言「评分 test-tool」即可触发。

## 依赖

| 依赖 | 必需 | 用途 | 安装 |
| ---- | ---- | ---- | ---- |
| Python 3.10+ | ✅ | 跑三个脚本 | 仓库已假定有 |
| cargo / rustfmt / clippy | ✅ | collect.py 跑测试 | 仓库 toolchain 1.97.1 已固定 |
| jsonschema | 可选 | validate.py 严格校验 | `pip install jsonschema` |
| cloc / tokei | 可选 | collect.py LOC 统计 | 不装则降级为 wc -l |

## 使用流程

详见 [`SKILL.md`](./SKILL.md)。简述：

### 1. 跑完 ae-sdd 流程后，调用 skill

在 ZCode 新对话里说「评分 test-tool」或 `/test-tool-score`。

### 2. skill 会问你（强制前置数据）

| 数据 | 示例 |
| ---- | ---- |
| Story ID | `STORY-DEMO-TEST-TOOL-001` |
| run index | `1` |
| 总耗时 | `75` 分钟 |
| 8 个 phase 耗时 | route=2 ra=8 dr=10 ... |
| token input/output | 1200000 / 80000 |
| 对话轮次 | 45 |
| 跑前 state.revision | 74 |
| 模型名 / 单价（可选） | glm-4.5 / $0.5 / $1.5 |

### 3. skill 自动跑

```bash
# 采集
python scripts/collect.py --story-id STORY-DEMO-TEST-TOOL-001 ... --output .collected.json

# 评分
python scripts/score.py --collected .collected.json --output metrics-STORY-DEMO-TEST-TOOL-001-run1.json

# 校验
python scripts/validate.py --metrics metrics-STORY-DEMO-TEST-TOOL-001-run1.json --schema ../metrics.schema.json
```

### 4. 输出

- `demo/test-tool/metrics-<STORY-ID>-run<N>.json`（符合 schema 的最终指标）
- 终端打印总分、等级、能力上限、红线警告

### 5. 横向对比（多次跑后）

skill 会自动扫描 `demo/test-tool/metrics-*.json`，生成对比表（总分 + 派生指标）。

## 评分逻辑

严格对齐 [`../EVALUATION.md`](../EVALUATION.md)：

| 维度 | 权重 | 评分来源 |
| ---- | ---- | -------- |
| A 流程合规性 | 35% | state.json routeDecision/history/executionPlan + evidence 三 gate |
| B 能力项覆盖度 | 25% | RA/DR/Story 文档关键字 + executionPlan 字段 + cargo/gate |
| C 完成度与质量 | 25% | cargo test AC 通过 + 边界覆盖 + 契约对齐 + clippy/fmt + 选修组 |
| D 效率指标 | 15% | 时长分档 + 12 项派生比率 |

**红线**：A1/A6/A8/A9/A11 任一违反 → A=0，红线标记置位。

## 故障排查

| 症状 | 原因 | 解决 |
| ---- | ---- | ---- |
| collect.py 找不到 demo/test-tool | 不在仓库根运行 | 加 `--repo D:/Item/ae-sdd` |
| collect.py cargo test 超时 | 测试跑太久 | 加大 timeout 或先 `cargo build` |
| score.py 报 story state not found | Story ID 写错或 state 目录没生成 | 检查 `.auto-engineering/` 下目录名 |
| validate.py 报 jsonschema 未安装 | 没装可选依赖 | `pip install jsonschema` 或去掉 `--strict` |
| AC 通过数解析为 0 | 测试名不符合 `ac_1`/`ac1` 约定 | 让 ae-sdd 测试名含 `ac_<N>` 关键字 |
