"""Unit tests for scripts/ra_depth_scan.py（v3.5.9 机械派生深度扫描器）。

复刻 test_gates.py TestGRA4 模式：直接 subprocess 调 scripts/ra_depth_scan.py，
构造「空转 RA / 完整 RA / 单维度 BLOCKER」三种样本验证 D1-D5 行为。

与 G-RA-4 测试区别：本测试不通过 gates.check_ra_depth() 间接调，而是直接调扫描器，
因为 gates 层的 check_ra_depth 还要到步骤 B 才实装。先验证扫描器自身行为，
再在步骤 B 的 TestGRA5 中补 gates 层集成测试。
"""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCANNER = REPO_ROOT / "scripts" / "ra_depth_scan.py"


def _run_scan(root: Path) -> dict:
    """调扫描器跑一份 RA 样本，返回 JSON dict。"""
    result = subprocess.run(
        [sys.executable, str(SCANNER), "--root", str(root), "--format", "json"],
        capture_output=True, text=True, timeout=60, check=False,
    )
    return json.loads(result.stdout)


def _setup_project(structure: dict) -> Path:
    """构造临时项目目录。structure 是 {relpath: content} 字典"""
    tmp = Path(tempfile.mkdtemp())
    for rel, content in structure.items():
        p = tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    return tmp


# ─── 测试样本（复刻 smoke 阶段的两份样本）───────────────────────────────

EMPTY_RA = """# RA-空转样本

## §6.5 衍生规则登记表（状态变更类需求必填）

| 规则 # | 主规则 R | 衍生规则 R' | 衍生模式命中 | R' 优先级 | R' 验证方式 |
|--------|----------|-------------|--------------|-----------|-------------|
| R1 | 5 次登录失败锁账号 |  |  | P0 | 用户确认 |

## §8.5 衍生 AC 登记表

| AC # | 主场景 | 衍生场景 | 衍生动作 | 时效要求 | Given-When-Then | 对应规则 R' | 状态 |
|------|--------|----------|----------|----------|-----------------|-------------|------|

## §8.6 衍生覆盖率

| 维度 | 数量 | 覆盖率 |
|------|------|--------|
| 已配套衍生 AC 数 | 0 | 0/1 |

## §9-bis 业务模式匹配表

| 套用的模式 | 模式 # | 命中的衍生影响编号 | 备注 |
|------------|--------|--------------------|------|
| 账号状态变更 | 1 |  |  |

## §9-ter 跨域级联效应表（跨域需求）

本需求涉及微服务聚合根与 MQ topic、WebSocket、Redis、CQRS。

| 触发动作 | 受影响域 | 受影响状态机/事件/缓存/MQ | 触发方式 | 时效要求 | 反向影响 |
|----------|----------|---------------------------|----------|----------|----------|
| 账号锁定 | User | 状态变更 | 本域事务内 | 尽快 | — |
"""


FULL_RA = """# RA-完整样本

本需求涉及状态变更、状态机、触发、联动、禁用、启用、锁定、解锁、注销、角色变更、退款、取消、登录、登出、失败、超时、过期、状态流转。

## §6.5 衍生规则登记表（状态变更类需求必填）

| 规则 # | 主规则 R | 衍生规则 R' | 衍生模式命中 | R' 优先级 | R' 验证方式 |
|--------|----------|-------------|--------------|-----------|-------------|
| R1 | 5 次登录失败锁账号 | R1.1, R1.2 | H.5 模式 1.① | P0 | 用户确认 + 项目资产反查 |
| R1 | 5 次登录失败锁账号 | R1.3, R1.4 | H.5.1 模式 1.④ | P0 | 用户确认 + IM 域 RA 关联 |

## §8.5 衍生 AC 登记表

| AC # | 主场景 | 衍生场景 | 衍生动作 | 时效要求 | Given-When-Then | 对应规则 R' | 状态 |
|------|--------|----------|----------|----------|-----------------|-------------|------|
| AC-004 | 账号锁定 | 进行中会话处理 | 强制 T 下线 | 5 秒内 | Given 客服账号 A / When 锁定 / Then 5 秒内 3 个会话断开 | R1.1, R1.2 | ✅ |
| AC-005 | 账号锁定 | 缓存失效 | Redis DEL key | 1 秒内 | Given Redis 有 user:profile / When 锁定 / Then 1 秒内失效 | R1.3 | ✅ |
| AC-006 | 账号锁定 | IM 状态联动 | WebSocket 推送 | 5 秒内 | Given IM Online / When 锁定 / Then IM Offline | R1.4 | ✅ |

## §8.6 衍生覆盖率

| 维度 | 数量 | 覆盖率 |
|------|------|--------|
| 衍生规则 R' 总数 | 4 | — |
| 已配套衍生 AC 数 | 3 | 3/4 = 75% |
| 衍生 AC 时效要求明确率 | 3 | 100% |

## §9-bis 业务模式匹配表

| 套用的模式 | 模式 # | 命中的衍生影响编号 | 转化为业务规则 R# | 转化为 AC # | 备注 |
|------------|--------|--------------------|--------------------|-------------|------|
| 账号状态变更 | 1 | ①②③④⑤⑥⑦⑧⑨⑩ | R1 | AC-004, AC-005, AC-006 | — |
| 订单状态变更 | 2 | 无 | — | — | 本需求不涉及订单业务 |
| 支付状态变更 | 3 | 无 | — | — | 本需求不涉及支付 |
| 登录态变更 | 4 | ①②③④⑤⑥⑦ | R1 | AC-004, AC-005 | — |
| 权限变更 | 5 | 无 | — | — | 本需求不涉及权限 |
| 定时任务状态 | 6 | 无 | — | — | 本需求不涉及定时任务 |

## §9-ter 跨域级联效应表

本需求涉及微服务聚合根与 MQ topic、WebSocket、Redis、CQRS、跨域事件广播。

| 触发动作 | 受影响域 | 受影响状态机/事件/缓存/MQ | 触发方式 | 时效要求 | 反向影响 |
|----------|----------|---------------------------|----------|----------|----------|
| 账号锁定 | IM | Presence 状态机变更：Online→Offline，事件广播 t_user_locked，Redis key user:presence:{id} DEL，MQ topic im.presence.update | MQ 事件广播 | 5 秒内 | — |
| 账号锁定 | CS | Agent 状态机变更：Available→Away，事件广播 t_user_locked，Redis key cs:agent:{id} DEL，MQ topic cs.away.trigger | MQ 事件广播 | 5 秒内 | — |
"""


# ─── 测试类 ──────────────────────────────────────────────────────────────

class TestRaDepthScan(unittest.TestCase):
    """机械派生深度扫描器 D1-D5 行为测试。"""

    def test_state_machine_clean_passes(self):
        """FULL 样本：状态机/跨域类需求，§6.5/§8.5/§8.6/§9-bis/§9-ter 完整 → PASS。"""
        tmp = _setup_project({"design/RA-full-v1.0.md": FULL_RA})
        report = _run_scan(tmp)
        self.assertEqual(report["status"], "PASS",
                         f"完整样本应 PASS，但 status={report['status']}, findings={report['findings']}")
        self.assertEqual(report["blockers"], 0,
                         f"完整样本应有 0 BLOCKER，但有 {report['finders'] if 'finders' in report else report['findings']}")
        self.assertEqual(report["raFiles"], 1)
        self.assertEqual(report["ruleStats"]["D1"], 0)
        self.assertEqual(report["ruleStats"]["D2"], 0)
        self.assertEqual(report["ruleStats"]["D3"], 0)
        self.assertEqual(report["ruleStats"]["D4"], 0)
        self.assertEqual(report["ruleStats"]["D5"], 0)

    def test_empty_table_blocks_d1_d4_d5(self):
        """EMPTY 样本：§6.5/§8.5 空 + §9-ter 时效模糊 + §9-bis 缺模式 → 多 BLOCKER。"""
        tmp = _setup_project({"design/RA-empty-v1.0.md": EMPTY_RA})
        report = _run_scan(tmp)
        self.assertEqual(report["status"], "FAIL")
        self.assertGreater(report["blockers"], 0)
        rules_hit = {f["rule"] for f in report["findings"]}
        # D1 必命中（§6.5 空）
        self.assertIn("D1", rules_hit, f"D1 应命中：findings={report['findings']}")
        # D4 必命中（§9-ter 时效含「尽快」）
        self.assertIn("D4", rules_hit)
        # D5 必命中（§9-bis 6 模式不全）
        self.assertIn("D5", rules_hit)

    def test_missing_pattern_ref_blocks_d1(self):
        """§6.5 衍生 R′ 行「衍生模式命中」列含「无」/空 → D1 BLOCKER。"""
        ra = """# RA-模式缺失样本

## §6.5 衍生规则登记表

| 规则 # | 主规则 R | 衍生规则 R' | 衍生模式命中 | R' 优先级 |
|--------|----------|-------------|--------------|-----------|
| R1 | 锁定 | R1.1 | 无 | P0 |

## §8.5 衍生 AC 登记表

| AC # | 主场景 | 衍生动作 | 时效要求 | 对应规则 R' |
|------|--------|----------|----------|-------------|
| AC-001 | 锁定 | 缓存失效 | 5 秒内 | R1.1 |
"""
        tmp = _setup_project({"design/RA-missing-pattern-v1.0.md": ra})
        report = _run_scan(tmp)
        rules_hit = {f["rule"]: f["message"] for f in report["findings"]}
        self.assertIn("D1", rules_hit, "无模式编号的 R′ 应被 D1 拦截")
        self.assertIn("模式", rules_hit["D1"], "D1 消息应明确指向模式编号问题")

    def test_unlinked_rprime_blocks_d2(self):
        """§6.5 有 R1.1 但 §8.5 无对应 AC 行 → D2 BLOCKER。"""
        ra = """# RA-未链接样本

## §6.5 衍生规则登记表

| 规则 # | 主规则 R | 衍生规则 R' | 衍生模式命中 |
|--------|----------|-------------|--------------|
| R1 | 锁定 | R1.1, R1.2 | H.5 模式 1.① |
| R1 | 锁定 | R1.3 | H.5.1 模式 1.④ |

## §8.5 衍生 AC 登记表

| AC # | 主场景 | 衍生动作 | 时效要求 | 对应规则 R' |
|------|--------|----------|----------|-------------|
| AC-001 | 锁定 | 缓存 | 5 秒内 | R1.3 |

## §8.6 衍生覆盖率

| 维度 | 数量 | 覆盖率 |
|------|------|--------|
| R' 总数 | 3 | — |
| 已配套衍生 AC 数 | 1 | 1/3 |
"""
        tmp = _setup_project({"design/RA-unlinked-v1.0.md": ra})
        report = _run_scan(tmp)
        rules_hit = {f["rule"]: f["message"] for f in report["findings"]}
        self.assertIn("D2", rules_hit, f"R1.1 / R1.2 未在 §8.5 链接应被 D2 拦截：findings={report['findings']}")
        # 至少拦 R1.1 或 R1.2
        self.assertTrue("R1.1" in rules_hit["D2"] or "R1.2" in rules_hit["D2"])

    def test_coverage_mismatch_blocks_d3(self):
        """§8.6 声明 K/M=100% 但实际 K/M=50% → D3 BLOCKER（声明≠实际）。"""
        ra = """# RA-覆盖率不一致样本

## §6.5 衍生规则登记表

| 规则 # | 主规则 R | 衍生规则 R' | 衍生模式命中 |
|--------|----------|-------------|--------------|
| R1 | 锁定 | R1.1 | H.5 模式 1.① |
| R1 | 锁定 | R1.2 | H.5 模式 1.② |

## §8.5 衍生 AC 登记表

| AC # | 主场景 | 衍生动作 | 时效要求 | 对应规则 R' |
|------|--------|----------|----------|-------------|
| AC-001 | 锁定 | 缓存 | 5 秒内 | R1.1 |

## §8.6 衍生覆盖率

| 维度 | 数量 | 覆盖率 |
|------|------|--------|
| R' 总数 | 2 | — |
| 已配套衍生 AC 数 | 2 | 100% |
"""
        tmp = _setup_project({"design/RA-coverage-mismatch-v1.0.md": ra})
        report = _run_scan(tmp)
        rules_hit = {f["rule"]: f["message"] for f in report["findings"]}
        self.assertIn("D3", rules_hit, f"声明 100% 实际 1/2=50% 应被 D3 拦截：findings={report['findings']}")
        self.assertIn("声明", rules_hit["D3"])

    def test_h6_missing_dimension_blocks_d4(self):
        """§9-ter 触发动作只填缓存不填事件/MQ → D4 BLOCKER。"""
        ra = """# RA-五问缺失样本

本需求涉及微服务聚合根与 MQ topic、WebSocket、Redis、跨域事件广播。

## §9-ter 跨域级联效应表

| 触发动作 | 受影响域 | 受影响状态机/事件/缓存/MQ | 触发方式 | 时效要求 | 反向影响 |
|----------|----------|---------------------------|----------|----------|----------|
| 账号锁定 | User | Redis key 失效 | 本域事务内 | 5 秒内 | — |
"""
        tmp = _setup_project({"design/RA-h6-incomplete-v1.0.md": ra})
        report = _run_scan(tmp)
        rules_hit = {f["rule"]: f["message"] for f in report["findings"]}
        self.assertIn("D4", rules_hit, "§9-ter 只填缓存漏事件/MQ 应被 D4 拦截")
        # 必须命中「事件」和「MQ」缺失
        self.assertTrue("事件" in rules_hit["D4"] or "MQ" in rules_hit["D4"])

    def test_non_state_machine_skips_d1(self):
        """非状态机类需求（无 STATE_MACHINE_KEYWORDS 且无 §6.5）→ D1 不触发。"""
        # 注意：非状态机类需求不应有 §6.5（仅状态机类需求必填）。本测试模拟正确做法：
        # 没有 §6.5 章节，D1 因缺少 sec_65 上下文而跳过校验。
        ra = """# RA-纯查询样本

本需求是简单的用户信息查询接口。

## §9-bis 业务模式匹配表

| 套用的模式 | 模式 # | 命中的衍生影响编号 | 备注 |
|------------|--------|--------------------|------|
| 账号状态变更 | 1 | 无 | 本需求不涉及账号状态变更 |
| 订单状态变更 | 2 | 无 | 本需求不涉及订单 |
| 支付状态变更 | 3 | 无 | 本需求不涉及支付 |
| 登录态变更 | 4 | 无 | 本需求不涉及登录态 |
| 权限变更 | 5 | 无 | 本需求不涉及权限 |
| 定时任务状态 | 6 | 无 | 本需求不涉及定时任务 |
"""
        tmp = _setup_project({"design/RA-readonly-v1.0.md": ra})
        report = _run_scan(tmp)
        # 非状态机类需求且无 §6.5 → D1 不应触发
        rules_hit = {f["rule"] for f in report["findings"]}
        self.assertNotIn("D1", rules_hit,
                         f"非状态机类需求且无 §6.5 不应触发 D1：findings={report['findings']}")

    def test_cli_help(self):
        """CLI 帮助可访问（防 argparse 退化）。"""
        result = subprocess.run(
            [sys.executable, str(SCANNER), "--help"],
            capture_output=True, text=True, timeout=10, check=False,
        )
        self.assertEqual(result.returncode, 0)
        self.assertIn("ra_depth_scan", result.stdout.lower() or "depth" in result.stdout.lower()
                      or "--root" in result.stdout)


if __name__ == "__main__":
    unittest.main()