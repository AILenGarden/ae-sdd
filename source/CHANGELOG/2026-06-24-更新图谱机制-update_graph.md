# 2026-06-24 | ae-sdd 更新依赖图谱机制 — update_graph 检查器 + 图谱表（杜绝漏更新）

> **版本号**：3.2.1（v3.2.0 增量，工具层）
> **性质**：🟡 机制建设（不破坏兼容，新增防漏更新能力）
> **影响范围**：1 新建检查器（update_graph.py）+ 1 新建测试（test_update_graph.py）+ 1 改 CLI（ae-sdd update-check 子命令）+ 1 改 ae-sdd-update-skill（图谱章节 + 健康度清单）+ 1 更新 README + 1 新增 CHANGELOG + dev-sync

---

## 摘要

用户反馈：强化需求分析能力后做了很多新脚本，**有没有连带调整被漏掉？**（如实例化脚本、docToInstance 等）。这暴露了一个系统性问题——现有 ae-sdd-update-skill 的同步规则是线性的"改母版 → 跑 dev-sync"，**改了 A 之后要同步哪些 B/C/D 无表可查**，靠人记忆必然漏。

**本次建立"更新依赖图谱"机制**：① 图谱表（人工遵照，改动前查表列连带项）+ ② `ae-sdd update-check` 检查器（自动兜底，改动后跑 UC-01~UC-05 验证无漏更新）。双保险杜绝漏更新。

**本次强化实际暴露的漏更新（update-check 检出并已修复）：**
- 版本号漂移（SKILL.md / paths.py / README.md 四处不一致）→ UC-01 检出，已对齐 3.2.1
- 健康度清单缺 update_graph 项 → UC-05 检出，已补
- 命令契约缺口（SKILL.md 声明 `gate ra-required` 但 CLI 未实现）→ UC-03 检出，已补（前序工作）

---

## 改动 1：tools/lib/update_graph.py 检查器（核心）

对标 gates.py 的 GateResult 模式，5 项检查：

| 检查 ID | 检查内容 | 严重度 |
|---------|---------|--------|
| **UC-01** | 版本号一致性：SKILL.md / paths.py MASTER_VERSION / README.md:5 三处 | error |
| **UC-02** | 门禁注册一致性：GATE_REGISTRY 每个 id 在 CHECK_FUNCS 或 check_all 特判 | error |
| **UC-03** | 命令契约闭环：SKILL.md 引用的 `ae-sdd <cmd>` 在 CLI 实现（历史遗留 warn）| error/warn |
| **UC-04** | 扫描器分发一致性：scripts/*_scan.py 在 build_dist.py runtime_scripts 白名单 | error |
| **UC-05** | 健康度清单覆盖：ae-sdd-update-skill 含关键组件（G-RA / ra_authenticity_scan / RAModel / update_graph 等）| warn |

输出 `{total, passed, failed, warnings, checks}`，返回码 1 if failed。

## 改动 2：CLI `ae-sdd update-check` 子命令

`update-check`（全量）/ `update-check --only UC-01`（单项），对标 `gates check`。修复前能检出版本号漂移，修复后全绿。

## 改动 3：ae-sdd-update-skill 新增"更新依赖图谱"章节

含图谱表（8 行触发源→连带项）+ 使用 SOP（改动前查表/改动后跑检查）+ 5 项检查说明 + 图谱维护规则。健康度清单追加 5 项 v3.2 自检（update_graph.py / update-check 子命令 / test_update_graph / 图谱章节 / G-CODE-1）。

---

## 验证方式

```bash
# 1. 单元测试
python -m pytest tools/tests/test_update_graph.py -q   # 16 passed

# 2. 全量测试
python -m pytest tools/tests/ -q                        # 含新增 test_update_graph

# 3. update-check 冒烟（修复后应全绿）
ae-sdd update-check
# 预期：5 项 ✅，0 failed，2 warn（UC-03 历史遗留 + UC-05 warn 级）

# 4. update-check --json
ae-sdd update-check --json
```

---

## 不承诺声明

- ✅ 能检出：版本号漂移 / 门禁未注册 / 命令未实现 / 扫描器漏白名单 / 健康度清单缺项
- ❌ 不能自动修复：只检测+提示，修复由人执行
- ❌ 不实现历史遗留命令（assets/fork/init/run/skill/sync-tools）：UC-03 标 warn，待后续迭代

---

## Reviewer

- **改动设计**：Harness（harness root agent）
- **用户决策**：用户确认"图谱表+检查脚本"形态
- **Reviewer**：待指派
