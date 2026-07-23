# 2026-06-24 | ae-sdd v3.1.2 — ae-sdd-install SKILL + install.py 智能引导

> **版本号**：3.1.2（v3.1.1 增量）
> **性质**：🟢 产品矩阵补齐 + 🟡 用户体验提升（Agent 启动 + 人启动两条路径全覆盖）
> **影响范围**：1 新建 SKILL + 1 改母版主入口 + 1 改 install.py + 1 CHANGELOG + 1 README 更新

---

## 摘要

本次新增 **ae-sdd-install-skill** —— 专门给 Agent 看的"安装引导 SKILL"，解决两个真实 gap：

1. **Agent 启动路径**：当 Harness / Claude Code / Codex 受用户委托装 ae-sdd 时，没有专门的引导文档
2. **人启动路径**：当人跑 `install.sh` / `install.ps1` 后，没有智能引导（不知道下一步怎么用）

**核心设计哲学：**
> install.sh 的核心职责是"把 SKILL 装上 + 写 hooks"，**不是**"启动 SKILL"。SKILL 由 Agent 在后续对话中通过触发词自动加载。

---

## 改动 1：新建 ae-sdd-install-skill.md（核心）

### 1.1 位置

`source/skills/orchestration/ae-sdd-install-skill.md`（与 ae-sdd-skill.md / ae-sdd-update-skill.md 同级）

### 1.2 规模

约 280 行（含完整 SOP）

### 1.3 触发词

- "安装 ae-sdd" / "装 ae-sdd"
- "重装 ae-sdd" / "升级 ae-sdd"
- "卸载 ae-sdd"
- "给 <项目> 接 ae-sdd"

### 1.4 职责

**10 节**：
1. 触发场景与产物对照
2. 平台检测（必跑，硬前置）
3. 选择安装模式（4 选 1）
4. 执行 install（按平台分支）
5. 验证（必跑，4 项）
6. 常见失败 FAQ（5 组：Python / Hook / 安装 / macOS / Windows）
7. 与其他 SKILL 的边界
8. 触发后输出模板
9. 执行清单
10. 禁止事项

---

## 改动 2：ae-sdd SKILL.md 主入口（3 处追加）

### 2.1 YAML frontmatter description 加触发词分流说明

```yaml
description: |
  ...原描述...
  🆕 v3.1.2：安装 ae-sdd 触发词分流到 `ae-sdd-install-skill.md`（"安装 ae-sdd"/"装 ae-sdd"/"重装"）。
```

### 2.2 §🎯 智能路由表新增一行

```markdown
| 🆕 **"安装 ae-sdd" / "装 ae-sdd" / "重装 ae-sdd" / "升级 ae-sdd" / "卸载 ae-sdd" / "给 <项目> 接 ae-sdd"** | **`ae-sdd-install-skill.md`** | **横向（安装引导 🆕）** |
```

### 2.3 §子 SKILL 索引新增一行

```markdown
| **🆕 ae-sdd Install** | [ae-sdd-install-skill.md](../orchestration/ae-sdd-install-skill.md) | **安装引导 SKILL — 平台检测 → 选模式 → 执行 install → 写 hooks → 验证** |
```

### 2.4 版本号升级

v3.1.1 → v3.1.2

---

## 改动 3：scripts/install.py print_usage() 智能引导

### 3.1 原行为

```python
def print_usage() -> None:
    print()
    success("ae-sdd SKILL 安装成功！")
    print()
    print(f"  安装路径：{DST}")
    ...
    print("  在 Claude Code 中使用：")
    print("    输入  /ae-sdd  启动自动化工程助手")
    print()
    print(f"  更多信息：{REPO_URL}")
    print()
```

### 3.2 新行为

新增 `_detect_agents()` 函数，检测 Claude Code / Codex / Harness CLI：

```python
def _detect_agents() -> dict:
    """检测可用的 Agent CLI"""
    agents = {}
    if shutil.which("claude") or shutil.which("claude.exe"):
        agents["claude"] = "Claude Code"
    if shutil.which("codex") or shutil.which("codex.exe"):
        agents["codex"] = "Codex CLI"
    if shutil.which("harness") or shutil.which("harness.exe"):
        agents["harness"] = "Harness daemon"
    return agents
```

`print_usage()` 根据检测结果输出智能引导：
- **检测到 Agent CLI**：列出可用的 Agent + 启动命令 + 触发词
- **未检测到**：推荐 Claude Code 安装链接 + 触发词

### 3.3 install.sh / install.ps1 自动受益

由于 install.sh / install.ps1 都是 install.py 的薄壳（仅 exec install.py），**print_usage() 改动自动生效**。

---

## 设计哲学：为什么 install.py 不"自动启动 Claude Code"？

考虑过的 4 个方案：

| 方案 | 优 | 劣 | 决策 |
|------|---|---|------|
| A. install.py 跑完输出引导 + 推荐启动 | 零入侵 | 用户手动 2 步 | ✅ 采用 |
| B. install.py 检测 CLI 存在 + 输出启动命令 | 零入侵 + 智能 | 用户手动 1 步 | ✅ 采用 |
| C. install.py 检测 CLI 是否运行中 + 注入 prompt | 半自动 | 检测不一定可靠 | ❌ |
| D. install.py 自动启动 claude + 注入 prompt | 全自动 | **入侵用户环境** | ❌ |

**理由：** install.py 跑完即退出，强行启动 Claude Code 是不合理的副作用。用户可能想自己启动 / 可能想先去喝杯茶 / 可能想先看文档。智能引导 = 给用户**正确的信息 + 选择权**。

---

## 同步执行

本次改动通过 `bash scripts/dev-sync.sh` 同步到：
- `dist/ae-sdd/skills/orchestration/ae-sdd-install-skill.md`（新增）
- `dist/ae-sdd/SKILL.md`（更新主入口）
- `dist/ae-sdd/scripts/install.py`（更新 print_usage）
- `~/.claude/skills/ae-sdd/...`（本地安装）

---

## 验证方式

### 验证 1：触发词路由测试

```bash
# Claude Code 中输入 "装 ae-sdd"
# 预期：ae-sdd-install-skill 自动触发 + 显示 10 节 SOP
```

### 验证 2：install.py 智能引导

```bash
python scripts/install.py --from-build
# 预期：检测到 claude CLI，输出：
#   ✅ 检测到以下 Agent CLI 可用：
#      • Claude Code（命令: claude）
#   → 下一步（任选其一）：
#     1. 启动 Claude Code：claude
#     2. 启动后输入 /ae-sdd 启动自动化工程助手
#     3. 或输入"装 ae-sdd"让 ae-sdd-install-skill 引导后续配置
```

### 验证 3：卸载路径

```bash
python ~/.claude/skills/ae-sdd/tools/bin/ae-sdd init-hooks --uninstall
# 预期：卸载 + 备份到 .uninstalled.<时间戳>
```

---

## Reviewer

- **改动设计**：Harness（harness root agent）
- **用户决策**：icec-cloud-boss User 域 owner
- **Reviewer**：待指派

---

## 维护

- **触发条件**：用户反馈"Agent 不知道怎么装 ae-sdd" / "装完不知道下一步" 类问题时
- **后续迭代**：根据用户场景补充 FAQ（如 Linux 发行版特定问题 / 企业代理网络等）
- **同步要求**：任何修改必须同步执行 `bash scripts/dev-sync.sh`
