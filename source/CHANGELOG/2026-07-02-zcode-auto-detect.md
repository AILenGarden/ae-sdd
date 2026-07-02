# 2026-07-02 ZCode auto 分发检测补全

## 背景

ZCode 运行时目录 `~/.zcode/skills/` 已存在，但 `~/.zcode/skills/ae-sdd/` 尚未安装时，`ZcodeDistributor.detect()` 只检查目标目录或 `zcode` CLI，导致 auto/post-commit 分发不会把 ae-sdd 首次实例化到 ZCode。

## 变更

- `scripts/distributors/zcode.py` 新增 `skills_root()`，`target_path()` 统一从该根目录派生。
- `detect()` 扩展为三条件：ZCode skills 根目录存在、ae-sdd 目标目录已存在、或 `zcode`/`zcode.exe` CLI 可用，任一满足即纳入 auto 分发。
- 新增 `tools/tests/test_zcode_distributor.py`，覆盖 skills 根目录、目标目录、CLI、无信号四种检测分支。

## 影响

- 已有 ZCode skills 环境会在后续 `scripts/distribute.py` auto/post-commit 中自动收到编译后的 ae-sdd 实例化版本。
- 未安装 ZCode、也没有 `~/.zcode/skills/` 的环境仍保持跳过，不会凭空创建 ZCode 运行时。

## 验证

- `python tools/tests/run.py zcode_distributor -v`
- `python scripts/distribute.py --target zcode`
