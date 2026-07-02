# SKILL Runtime Compiler 说明书

> 目标版本：v3.8 起引入  
> 适用对象：ae-sdd 维护者、分发脚本维护者、Agent 运行时接入方  
> 核心结论：`source/` 是未编译母版，`dist/ae-sdd/` 是编译后的实例化运行包；正式发版给各 Agent 的只能是编译后版本。

---

## 1. 背景与目标

ae-sdd 的 SKILL 文档既承担人类维护说明，又承担 LLM 运行时指令。随着流程、门禁、状态机、工具链增加，单一 Markdown 会带来三个问题：

- 运行时上下文过长：Agent 每次加载大量解释性内容、历史背景和低频规则。
- 执行重点不够集中：门禁、路由、状态机、宏动作混在长篇说明中。
- 分发语义不清：`source/` 母版和 `dist/` 实例化包目前主要是复制关系，不是编译关系。

本设计把 ae-sdd 分成两版：

| 版本 | 位置 | 面向对象 | 允许手工编辑 | 说明 |
| --- | --- | --- | --- | --- |
| 未编译母版 | `source/` | 维护者 | 是 | 人类可读、可审查、可 diff 的完整 SSOT |
| 编译实例版 | `dist/ae-sdd/` | Agent 运行时 | 否 | 由编译工具生成，正式发给 Claude/Codex/ZCode/Hermes/Mavis |

编译不是把 SKILL 变成不可读密文，而是生成一组短、结构化、稳定的 runtime compact slices，让 LLM 优先读取运行时契约；完整 Markdown 保留为 fallback。

---

## 2. 设计原则

1. **人类维护源不压缩**  
   `source/` 继续保存完整设计、解释、模板、标准和子 SKILL，是唯一人工编辑点。

2. **运行时入口必须短**  
   编译后的 `dist/ae-sdd/SKILL.md` 是 bootloader，只负责加载顺序、冲突优先级和最小执行约束。

3. **结构化优先，不做密文**  
   runtime compact 使用 Markdown 表格、列表、JSON manifest，不使用私有短码、难读缩写或不可审查符号。

4. **代码事实优先**  
   能从代码抽取的内容优先从代码抽取，例如 `GATE_REGISTRY`、`PHASE_FLOWS`。避免在编译器中手写第二份状态机或门禁注册表。

5. **CLI gate 高于 prompt**  
   compact runtime 是 LLM 执行契约；若 compact 文档与 CLI 门禁结果冲突，以 CLI 结果为准。

6. **compiled package 才能发版**  
   install/distribute 链路只能安装 `dist/ae-sdd/`，不得直接安装 `source/`。

---

## 3. 产物结构

编译后实例包结构：

```text
dist/ae-sdd/
├── SKILL.md                         # 编译后 bootloader，Agent 主入口
├── runtime/
│   ├── manifest.json                # 机器可校验 manifest：版本、source hash、runtime_fingerprint、load_order
│   ├── boot.compact.md              # 运行时总契约
│   ├── route.compact.md             # 路由压缩表
│   ├── gates.compact.md             # 门禁压缩表
│   ├── flow.compact.md              # 状态机压缩表
│   ├── macros.compact.md            # 公共动作宏
│   └── fallback/
│       └── SKILL.full.md            # 原始 source/SKILL.md 备份，只做 fallback
├── skills/                          # 子 SKILL fallback
├── standards/                       # 标准 fallback
├── templates/                       # 模板 fallback
├── tools/                           # CLI 与 lib
├── scripts/                         # 运行时扫描器
├── VERSION
└── .claude-plugin/plugin.json
```

运行时读取顺序：

1. `SKILL.md`
2. `runtime/boot.compact.md`
3. `runtime/route.compact.md`
4. `runtime/gates.compact.md`
5. `runtime/flow.compact.md`
6. `runtime/macros.compact.md`
7. 仅在 compact 不足时读取 `runtime/fallback/SKILL.full.md` 或对应子 SKILL

---

## 4. 编译流程

```text
source/                           # 未编译母版
  ↓ scripts/build_dist.py
dist/ae-sdd/                      # 先复制母版必要文件
  ↓ scripts/compile_skill_runtime.py
dist/ae-sdd/SKILL.md              # 替换为 bootloader
dist/ae-sdd/runtime/*.compact.md  # 生成 compact runtime
  ↓ scripts/distribute.py
~/.claude|~/.codex|.../skills/ae-sdd/
```

`build_dist.py` 是 source → dist 的唯一通用构建入口，因此编译器必须接入它，而不是让各 Agent 分发器各自编译一份。

分发器仍可以有 Agent 专属 compile 阶段，例如 Mavis harness 转译，但它们接收的输入必须已经是通用编译实例包。

---

## 5. 幂等性契约

runtime 编译器必须满足字节级幂等：

```text
同一份 source/ + 同一版编译器 + 同一份 GATE_REGISTRY + 同一份 PHASE_FLOWS
  ↓
重复运行 scripts/compile_skill_runtime.py
  ↓
dist/ae-sdd/SKILL.md 与 dist/ae-sdd/runtime/** 字节内容完全一致
```

硬性约束：

- 墙钟时间、当前日期、执行机器、临时目录等外部状态不得进入 runtime 编译结果。
- `runtime/manifest.json` 不记录 `compiled_at`；用 `runtime_fingerprint` 表示确定性输入集合的摘要。
- `--build-date` 仅作为旧脚本兼容参数保留，runtime 编译器必须忽略它。
- 已存在的 `runtime/fallback/SKILL.full.md` 是 fallback 原文锚点；二次编译不得把它覆盖成上一次生成的 bootloader。
- `source_checksums`、fallback 原文哈希、`GATE_REGISTRY`、`PHASE_FLOWS`、路由表、宏表、编译器版本共同参与 `runtime_fingerprint`。
- `dist/ae-sdd/VERSION`、`.claude-plugin/plugin.json` 等分发元数据若继续保留构建时间，属于外层打包元数据，不得污染 runtime 编译产物。

验收方式：单元测试必须用不同 `--build-date` 连续编译同一目录，并比较 `SKILL.md` 与 `runtime/**` 的字节快照完全一致。

---

## 6. Runtime IR 范围

第一阶段只编译高收益、低歧义内容：

| Runtime slice | 来源 | 内容 |
| --- | --- | --- |
| `boot.compact.md` | 固定契约 + manifest | 加载顺序、冲突优先级、fallback 规则 |
| `route.compact.md` | `source/SKILL.md` 规则摘要 | 自更新、大/中/小/微任务、继续流程、文档定位等路由 |
| `gates.compact.md` | `tools/lib/gates.py:GATE_REGISTRY` | Gate ID、名称、强度、运行命令、关键触发点 |
| `flow.compact.md` | `tools/lib/state.py:PHASE_FLOWS` | 大/中/小/微四条状态链 |
| `macros.compact.md` | 固定宏表 | BLOCK、WARN、ASK_USER、LOOP3、EVIDENCE、STATE_WRITE 等 |

暂不编译的内容：

- 具体子 SKILL 的长流程细节
- 模板正文
- 设计背景、历史变更、FAQ
- 项目资产内容
- 需要复杂语义理解才能安全抽取的规则

这些内容继续作为 fallback 延迟加载。

---

## 7. 冲突优先级

当不同来源出现冲突时，按以下优先级裁决：

1. 用户最新明确指令，但不能绕过安全、权限和流程硬门禁。
2. CLI / gate / state 工具真实输出。
3. runtime compact 规则。
4. 子 SKILL fallback。
5. `runtime/fallback/SKILL.full.md`。
6. 历史文档、CHANGELOG、说明性背景。

如果 compact 规则与 fallback 文档冲突，且 `runtime/manifest.json` 的 source hash 与当前包一致，优先 compact；否则降级读取 fallback 并报告可能的编译漂移。

---

## 8. 编译器职责

`scripts/compile_skill_runtime.py` 负责：

- 读取 `source/SKILL.md` 版本号。
- 计算关键源文件 sha256。
- 从 `tools/lib/gates.py` 抽取 `GATE_REGISTRY`。
- 从 `tools/lib/state.py` 抽取 `PHASE_FLOWS`。
- 生成 `runtime/manifest.json`。
- 生成 `runtime/*.compact.md`。
- 把原 `dist/SKILL.md` 备份到 `runtime/fallback/SKILL.full.md`。
- 用 bootloader 替换 `dist/SKILL.md`。

它不负责：

- 修改 `source/`。
- 安装到任何 Agent。
- 解释或执行业务流程。
- 替代 `ae-sdd gates check`。
- 替代 Mavis harness 等 Agent 专属适配器。

---

## 9. 编译器输入输出契约

命令：

```bash
python scripts/compile_skill_runtime.py --source source --dist dist/ae-sdd
```

可选参数：

```bash
python scripts/compile_skill_runtime.py \
  --repo-root . \
  --source source \
  --dist dist/ae-sdd
```

兼容参数：`--build-date` 可被旧脚本传入，但 runtime 编译器必须忽略它，不能把时间写入任何 runtime 文件。

成功条件：

- `dist/ae-sdd/SKILL.md` 存在且为 compiled bootloader。
- `dist/ae-sdd/runtime/manifest.json` 存在且 `compiled=true`。
- `manifest.load_order` 指向的文件全部存在。
- `runtime/fallback/SKILL.full.md` 存在。
- `runtime/gates.compact.md` 中 gate 数量等于 `GATE_REGISTRY` 数量。
- `runtime/flow.compact.md` 中包含大/中/小/微四条子链。

失败条件：

- `source/SKILL.md` 缺失。
- `dist/ae-sdd/` 尚未由 build_dist 生成。
- runtime 文件写入失败。
- 无法生成 bootloader。

代码抽取失败时允许降级为静态提示，但必须在 manifest 中记录 warning。当前第一期实现优先失败显式化，避免静默生成错误 runtime。

---

## 10. 发版规则

正式发版链路：

```text
维护者修改 source/
  ↓
python tools/tests/run.py
  ↓
python scripts/build_dist.py
  ↓
python scripts/distribute.py
```

`scripts/distribute.py` 必须只安装 `dist/ae-sdd/` 或基于它生成的 Agent 专属编译产物。

禁止：

- 直接把 `source/` 复制到 `~/.claude/skills/ae-sdd/`。
- 手工编辑 `dist/ae-sdd/SKILL.md`。
- 手工编辑 `runtime/*.compact.md`。
- 各分发器绕过 `build_dist.py` 自己复制 source。

---

## 11. 与现有实例化体系的关系

原 4 层体系保持不变，但 Layer 2 的语义升级：

| Layer | 旧语义 | 新语义 |
| --- | --- | --- |
| Layer 1 `source/` | 母版 SSOT | 未编译母版 SSOT |
| Layer 2 `dist/ae-sdd/` | 实例化分发包 | 编译后实例化运行包 |
| Layer 3 `~/.agent/skills/ae-sdd/` | 本地安装 | 编译包安装结果 |
| Layer 4 `<project>/.ae-sdd/` | 项目实例 | 项目状态、资产、override |

也就是说，实例化工具仍然存在，但实例化动作从“复制母版”升级为“复制母版必要内容 + 编译 runtime + 注入版本 + 分发”。

---

## 12. 验收标准

第一期验收：

- `python scripts/compile_skill_runtime.py --source source --dist <tmp>` 可独立运行。
- `python scripts/build_dist.py` 自动生成 runtime compact。
- `dist/ae-sdd/SKILL.md` 是 bootloader，不再是完整母版主入口。
- `dist/ae-sdd/runtime/manifest.json` 包含版本、source hash、runtime_fingerprint、load_order，不包含 `compiled_at`。
- `runtime/gates.compact.md` 从 `GATE_REGISTRY` 生成。
- `runtime/flow.compact.md` 从 `PHASE_FLOWS` 生成。
- 单元测试覆盖编译器最小行为和字节级幂等：不同 `--build-date` 重复编译同一输入，runtime 输出完全一致。

已落地扩展：

- `ae-sdd runtime verify` 校验 installed package 是否为 compiled。
- `update-check` 增加 UC-15 runtime 编译一致性检查。
- Agent 分发入口安装前拒绝未编译或不完整 runtime package。

后续可扩展：

- 从子 SKILL 自动抽取更多局部 compact slices。
- Agent 专属分发器可选择二次编译，但不能跳过通用 runtime。

---

## 13. 维护注意事项

- 修改 `GATE_REGISTRY` 后无需手工改 `gates.compact.md`，重新 build 即可。
- 修改 `PHASE_FLOWS` 后无需手工改 `flow.compact.md`，重新 build 即可。
- 修改路由大原则后，需要同步 `compile_skill_runtime.py` 中的 route compact 生成逻辑。
- 修改 bootloader 契约后，需要同步本说明书和对应测试。
- compact 产物是生成物，不进 git；源规则和编译器进 git。
- 禁止把时间戳、随机数、绝对临时路径写进 runtime 编译产物；需要记录构建时间时放在外层分发元数据，并明确不参与 runtime fingerprint。

---

## 14. 当前实现状态与实现准备

第一期已落地为可运行实现，不再是纯设计：

| 项 | 状态 | 文件 / 命令 |
| --- | --- | --- |
| runtime 编译器 | 已实现 | `scripts/compile_skill_runtime.py` |
| 构建链路接入 | 已实现 | `scripts/build_dist.py` 调用 runtime 编译器 |
| compiled bootloader | 已实现 | `dist/ae-sdd/SKILL.md` 由编译器生成 |
| runtime manifest | 已实现 | `dist/ae-sdd/runtime/manifest.json` |
| compact slices | 已实现 | `boot/route/gates/flow/macros.compact.md` |
| fallback 原文锚点 | 已实现 | `runtime/fallback/SKILL.full.md` |
| 字节级幂等测试 | 已实现 | `tools/tests/test_skill_runtime_compiler.py` |
| runtime package 校验器 | 已实现 | `tools/lib/runtime_verify.py` + `ae-sdd runtime verify` |
| update-check 编译一致性 | 已实现 | `tools/lib/update_graph.py` UC-15 |
| 分发器 compiled-only 校验 | 已实现 | `scripts/distribute.py` + `scripts/distributors/_base.py` |
| 设计文档 | 已实现 | `source/docs/skill-runtime-compiler.md` + `source/docs/ae-sdd-design.md` |
| 变更记录 | 已实现 | `source/CHANGELOG/2026-07-02-skill-runtime-compiler.md` |

实现入口：

```bash
python scripts/build_dist.py
```

独立编译器入口：

```bash
python scripts/compile_skill_runtime.py --source source --dist dist/ae-sdd
```

最小验证命令：

```bash
python -m py_compile scripts/compile_skill_runtime.py scripts/build_dist.py
python tools/tests/run.py skill_runtime_compiler -v
python tools/tests/run.py runtime_verify -v
python tools/bin/ae-sdd runtime verify --path dist/ae-sdd
python tools/bin/ae-sdd update-check --only UC-15
python scripts/build_dist.py
```

实现约束已经进入测试：同一输入重复编译时，`dist/ae-sdd/SKILL.md` 与 `dist/ae-sdd/runtime/**` 必须字节级一致；不同 `--build-date` 不得改变 runtime 输出。

后续实现清单：

| 状态 | 事项 | 目标 |
| --- | --- | --- |
| 已实现 | `ae-sdd runtime verify` | 校验安装包是否为 compiled runtime，manifest/load_order/fingerprint 是否完整 |
| 已实现 | `update-check` 接入 runtime 编译一致性 | 修改编译器、门禁、状态机后自动提示需要重编译或更新测试 |
| P2 | dist 外层可复现构建 | 让 `VERSION`、`plugin.json` 的构建时间可配置或可复现，扩展到整个分发包幂等 |
| 已实现 | 分发器 compiled-only 校验 | `scripts/distribute.py` 安装前拒绝未编译 `source/` 包 |
| P3 | 子 SKILL 局部 compact | 从高频子 SKILL 抽取更细粒度 runtime slices，但保持 fallback 可审查 |
| P3 | Agent 专属二次编译约束 | 允许 Hermes/Mavis 等做二次适配，但输入必须是通用 compiled runtime |

---

## 15. 通用编译器 SKILL

ae-sdd 专用编译器和通用编译器 SKILL 分开维护：

| 编译器 | 位置 | 适用对象 | 输出 |
| --- | --- | --- | --- |
| ae-sdd 专用 runtime compiler | `scripts/compile_skill_runtime.py` | `source/` -> `dist/ae-sdd/` | ae-sdd boot/route/gates/flow/macros compact |
| 通用 compiler SKILL | `standalone-skills/skill-runtime-compiler/` | 任意包含 `SKILL.md` 的 SKILL 包 | 同级 `<skill-name>-compiled/` compact runtime package |

通用 compiler SKILL 的目标是给其它 SKILL 复用，不依赖 ae-sdd 的 `GATE_REGISTRY`、`PHASE_FLOWS`、分发器或项目状态。它只使用 Python 标准库，核心入口是：

```bash
python standalone-skills/skill-runtime-compiler/scripts/compile_skill_package.py <source-skill-dir>
```

默认输出：

```text
<source-skill-dir-parent>/<source-skill-dir-name>-compiled/
```

输出结构：

```text
<skill>-compiled/
├── SKILL.md
└── runtime/
    ├── manifest.json
    ├── boot.compact.md
    ├── outline.compact.md
    └── fallback/
        └── SKILL.full.md
```

硬约束：

- 源 SKILL 包保持不变。
- 编译输出目录不得等于源目录，也不得位于源目录内部。
- 未加 `--force` 时，不覆盖非本编译器生成的既有目录。
- `runtime_fingerprint` 不包含墙钟时间、临时路径、主机名、随机数或文件 mtime。
- 同一输入重复编译时，compiled package 的 `SKILL.md` 与 `runtime/**` 必须字节级一致。

验收命令：

```bash
python -m py_compile standalone-skills/skill-runtime-compiler/scripts/compile_skill_package.py
python tools/tests/run.py standalone_skill_runtime_compiler -v
python tools/bin/ae-sdd update-check --only UC-15
```

UC-15 已同时检查 ae-sdd 专用编译器和 standalone compiler SKILL 的幂等性；修改任一编译器后都必须重跑。
