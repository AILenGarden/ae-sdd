---
name: ae-sdd-install
description: ae-sdd 安装引导 SKILL。当 Agent 需要安装/重装/升级 ae-sdd 时触发，引导完成：平台检测 → 选择安装模式 → 执行 install → 写 harness hooks → 验证。当用户说"安装 ae-sdd"/"装 ae-sdd 到 <项目>"/"给 <项目> 接 ae-sdd"/"重装 ae-sdd"/"升级 ae-sdd"/"卸载 ae-sdd"时触发。
version: 1.0.0
allowed_tools:
  - "ae-sdd"   # CLI（init-hooks 子命令）
  - "Bash"     # 执行 cargo build / ae-sdd-build
---

# ae-sdd Install — 安装引导 SKILL

> **🆕 2026-06-24 新建：** 这是 ae-sdd 体系的"安装引导入口"，**面向 Agent**（不是给人 curl | bash 用的脚本）。
> 适用于：Agent 受用户委托安装 ae-sdd / 重装升级 / 给新项目接 ae-sdd / 卸载 ae-sdd。
> 不适用于：人直接手工构建并拷贝二进制。

---

## 0. 触发场景与产物对照

| 用户说 | 目标产物 |
|--------|---------|
| "安装 ae-sdd" / "装 ae-sdd" / "给 <项目> 接 ae-sdd" | ae-sdd 已装到本地 Agent skills（Claude：`~/.claude/skills/ae-sdd/`；Codex：`~/.codex/skills/ae-sdd/`；Hermes：`~/.hermes/skills/ae-sdd/`），hooks 已配置 |
| "重装 ae-sdd" / "升级 ae-sdd" / "二进制路径变了" | 重新 build + install + 重写 hooks |
| "卸载 ae-sdd" | 删除本地 Agent skills 中的 ae-sdd 安装 + 清理 hooks |
| "把 ae-sdd 装到 <项目路径>" | 仅写 hooks（前提：ae-sdd 已全局装好）|

---

## 1. 平台检测（必跑，硬前置）

执行以下检查，把结果写进上下文：

```bash
# 1. OS
uname -s        # Linux / Darwin → Unix-like
# Windows 用 $env:OS 或 [System.Environment]::OSVersion

# 2. Shell
echo $SHELL     # bash / zsh / fish / powershell

# 3. Rust 工具链（从源码构建时必须）
cargo --version

# 4. Git
git --version

# 5. Claude Code / Codex 是否已装
which claude 2>&1 || where.exe claude 2>&1
which codex 2>&1 || where.exe codex 2>&1

# 6. ae-sdd 是否已装
ls ~/.claude/skills/ae-sdd/SKILL.md 2>&1
ls ~/.codex/skills/ae-sdd/SKILL.md 2>&1

# 7. 项目根（如果是给项目接 ae-sdd）
test -d <项目路径> && echo "项目存在" || echo "项目不存在"
```

**🔴 硬前置：从源码安装时 `cargo` 找不到 → 阻断，告诉用户先装 Rust 工具链。**

---

## 2. 选择安装模式（4 选 1）

| 场景 | 命令 | 何时用 |
|------|------|-------|
| **从源码构建 + 分发**（推荐开发者）| `cargo build --workspace --release` 然后提交触发 `.githooks/post-commit`（`ae-sdd-build harness` + `ae-sdd-build post-commit`）| 已有 clone 仓库，要本地开发 |
| **显式分步** | `cargo run -p ae-sdd-build --release -- harness ...` 再 `... -- post-commit ...` | 想看每步输出 |
| **仅写 hooks**（已装过 ae-sdd，要给项目接）| `ae-sdd init-hooks <项目路径>` | 仅扩展 hooks 范围 |

分发目标由 `~/.ae-sdd/distributors.json` 声明，注册一个 host 就是一次编辑。

---

## 3. 执行 install（按平台分支）

### 3.1 模式 A：构建原生二进制

```bash
cd <ae-sdd 仓库根>
cargo build --workspace --release
```

产出 `ae-sddd`（daemon）、`ae-sdd`（CLI/Hook）、`ae-sdd-build`（构建与审计工具）三个原生二进制。

**预期输出：**
```
[ae-sdd] 开始安装 ae-sdd SKILL...
[ae-sdd] ✅ ae-sdd SKILL 安装成功！
  安装路径：
    - ~/.claude/skills/ae-sdd/
    - ~/.codex/skills/ae-sdd/（当目录已存在或检测到 codex CLI 时自动同步）
  安装版本：3.1.1
  在 Claude Code 中使用：
    输入  /ae-sdd  启动自动化工程助手
```

### 3.2 模式 B：编译并分发到各 host（开发者）

```bash
cd <ae-sdd 仓库根>
# 提交后由 .githooks/post-commit 自动执行；也可手工跑：
cargo run -p ae-sdd-build --release -- post-commit \
    --repository-root . --source ./source --package ./dist/package \
    --commit <commit> --allowed-root . --allowed-root "$HOME" \
    --distributor-registry "$HOME/.ae-sdd/distributors.json" --registry-home "$HOME"
```

### 3.3 模式 C：仅写 hooks（已装过 ae-sdd）

```bash
# 项目级 hook（写到 <项目>/.claude/settings.json）
ae-sdd init-hooks <项目路径>

# 全局级 hook（写到 ~/.claude/settings.json）
ae-sdd init-hooks --global
```

---

## 4. 验证（必跑，不通过不交付）

执行以下 4 个验证：

```bash
# 1. 主入口存在
ls ~/.claude/skills/ae-sdd/SKILL.md

# 2. CLI 可执行
ae-sdd version
# 预期：{"name": "ae-sdd", "version": "3.1.1", ...}

# 3. hooks 配置
cat ~/.claude/settings.json | grep -A2 "hooks"
# 或 项目级：cat <项目>/.claude/settings.json

# 4. （如项目接 ae-sdd）检查项目是否有 .ae-sdd/ 目录
ls <项目>/.ae-sdd/ 2>&1
# ⚠️ 注意：没有 .ae-sdd/ 时 gate-intercept 默认放行（见 source/HARNESS.md）
```

**🔴 任意一项不通过 → 不交付，告知用户修复。**

---

## 4.5 自动化模式提示（🆕 v3.8.0，可选）

安装/初始化完成后，向用户提示自动化开关选项（**不主动开启**，仅告知）：

```
ℹ️  ae-sdd v3.8.0 支持自动化模式（默认关闭）：
   - 开启后 6 个人工审核点改走 Tier 3 多 reviewer 联审共识，实现输入→结果全自动化
   - 开启方式：ae-sdd automation enable
   - 开工前会自动收集所有必需信息（第三方凭证/复用选择/环境配置等）
   - 详见 source/SKILL.md §🚀 自动化模式
```

仅在用户明确要求"开启自动化/全自动/跳过人工审核"时执行 `ae-sdd automation enable`；否则只提示不操作。

---

## 5. 常见失败（FAQ，按 OS 分组）

### 5.1 二进制与工具链相关

| 症状 | 原因 | 处置 |
|------|------|------|
| `ae-sdd: command not found` | 二进制未安装 / PATH 没设 | 构建 release 后把 `target/release` 加进 PATH |
| `cargo: command not found` | 没装 Rust 工具链 | 装 Rust（rustup）+ 重启 shell |
| hook 不触发 / 二进制路径变了 | ae-sdd 重装后路径变了 | `ae-sdd init-hooks --force` 重写 |

### 5.2 Hook 相关

| 症状 | 原因 | 处置 |
|------|------|------|
| Claude Code 完全不调 hook | `.claude/settings.json` 格式错 | 用 JSON 校验工具确认该文件可解析 |
| `permissionDecision: deny` 总出现 | 当前 phase 不允许 | `ae-sdd state read` 看 phase，必要时 `state write --phase coding` |
| hook 调用慢 | daemon 未预热 | `ae-sdd runtime ensure --quiet`；改全局 hook 影响面大时谨慎 |
| 已有 hook 与 ae-sdd 冲突 | 其他 SKILL 也写了 hook | `--force` 覆盖 |

### 5.3 安装相关

| 症状 | 原因 | 处置 |
|------|------|------|
| `git: command not found` | 没装 Git | 先装 Git 再 clone 仓库 |
| 下载失败 / 网络超时 | GFW / 公司网络 | 手动 git clone 后本地构建 |
| dist 包不存在 | 没跑编译 | 跑 `ae-sdd-build post-commit`（或让 post-commit hook 触发）|

### 5.4 macOS 特殊

- Gatekeeper 拦截未签名二进制 → 系统设置 → 隐私与安全性 → 仍要打开
- zsh 默认 PATH 不含 `~/.cargo/bin` → rustup 安装后重启 shell

### 5.5 Windows 特殊

- PowerShell 执行策略拦截 `.ps1` → `Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned`
- CRLF 换行问题 → `git config --global core.autocrlf false`（ae-sdd SKILL.md 是 LF）
- 中文路径乱码 → PowerShell 终端用 UTF-8：`[Console]::OutputEncoding = [System.Text.Encoding]::UTF8`

---

## 6. 卸载

```bash
# 1. 卸载 SKILL（含备份）
ae-sdd init-hooks --uninstall

# 2. 手动清理项目级 hooks（如果之前 init-hooks 写过）
# 项目级 .claude/settings.json 的 hooks 字段需手动删除
# 或重写：ae-sdd init-hooks <项目路径>（只覆盖 hooks，不卸载 SKILL）

# 3. 验证卸载
ls ~/.claude/skills/ae-sdd/ 2>&1   # 应该不存在
```

**注意：** `--uninstall` 会把 `~/.claude/skills/ae-sdd/` 备份到 `~/.claude/skills/ae-sdd.uninstalled.<时间戳>`（可恢复）。

---

## 7. 与其他 SKILL 的边界

| SKILL | 职责 | install-skill 是否处理 |
|-------|------|----------------------|
| `ae-sdd.md`（主入口）| 业务使用 ae-sdd（跑流程 / 写代码）| ❌ 不管（用户用 `/ae-sdd` 触发）|
| `ae-sdd-update-skill.md` | 维护 ae-sdd SKILL 自身（修改母版 / 同步）| ❌ 不管（用户用"修改 SKILL"触发）|
| **`ae-sdd-install-skill.md`（本文件）** | **安装 / 重装 / 卸载 ae-sdd** | ✅ **本职** |
| `ae-sdd-harness-adapter`（如有）| 转译为 Harness 格式 | ❌ 不管 |
| 项目级初始化 | 项目内 `.ae-sdd/` 初始化 | ❌ 不管（init-skill 是另一个职责）|

**判断原则：** 用户的目标是**"让 ae-sdd 跑起来"还是"维护 ae-sdd"**？
- 跑起来（装/重装/卸载）→ **本 SKILL**
- 维护（改 SKILL 母版/同步/扩 SKILL）→ `ae-sdd-update-skill`

---

## 8. 触发后输出模板

执行完本 SKILL 后，**对话内**直接给用户以下内容（不要让用户打开文件看）：

```text
✅ ae-sdd 安装完成
  - 安装路径：
    - ~/.claude/skills/ae-sdd/
    - ~/.codex/skills/ae-sdd/（如当前环境使用 Codex）
  - 安装版本：3.1.1
  - Hooks 配置：项目级（<项目路径>/.claude/settings.json）/ 全局（~/.claude/settings.json）/ 未配置
  - CLI 可执行：✅

下一步：
  1. 启动 Claude Code：`claude`
  2. 在 Claude Code 中输入：`/ae-sdd` 启动自动化工程助手
  3. 或输入："装 ae-sdd" 再次调用本 SKILL（如有更多配置需求）

如需卸载：`ae-sdd init-hooks --uninstall`
```

---

## 9. 执行清单（按 TodoWrite 拆活）

| # | 步骤 | 验证 |
|---|------|------|
| 1 | §1 平台检测 | cargo / git / claude 或 codex 至少一项 |
| 2 | §2 选安装模式 | 用户场景 vs 模式匹配 |
| 3 | §3 执行 install | 构建与分发退出码 = 0 |
| 4 | §3.3 写 hooks（如需要）| init-hooks 退出码 = 0 + settings.json 有 hooks 字段 |
| 5 | §4 验证 4 项 | 全部通过 |
| 6 | §8 输出模板 | 对话内直接给用户 |

---

## 10. 禁止

| 禁止 | 原因 |
|------|------|
| ❌ 跳过 §1 平台检测 | 不同 OS / shell 命令分支大，跳过会装错 |
| ❌ 跳过 §4 验证 | 装完不验证 = 假装装好 |
| ❌ 自动启动 Claude Code 并注入 prompt | 入侵用户环境（用户可能不想要） |
| ❌ 用 init-skill 走 ae-sdd 内部初始化 | 与本 SKILL 不同职责 |
| ❌ 改 source/ 母版来"修安装问题" | 安装问题应该在构建与分发链路（`ae-sdd-build`）修，不要污染母版 |
