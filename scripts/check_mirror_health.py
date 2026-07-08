#!/usr/bin/env python3
"""
check_mirror_health.py — ae-sdd 镜像健康检测脚本（🆕 v3.9.11）

根因（v3.9.10 life 项目事故复盘）：
  .ae-sdd/state.json 镜像作为「反模式缓存」长期存在，但缺少主动健康检测。
  镜像可能在以下场景下冻结/误指：
    1. work-item 已完成（phase=completed）但镜像未刷新
    2. work-item 已迁移到 .auto-engineering/ 但镜像仍指旧路径
    3. 镜像 activeStory 与 .auto-engineering/ 下最近活跃 work-item 不一致
    4. 镜像存在但 activeStatePath 指死（目标文件不存在）
    5. work-item 源 state 缺 phase 字段（hook 链路断开）

  这些场景叠加会导致 hook 读到错误 phase、G-00 通过但实际不通、
  AI 卡住无法推进等疑难问题。

用途：
  - life 项目维护者定期跑：python scripts/check_mirror_health.py /d/Item/life
  - ae-sdd doctor 子命令未来可包装此脚本
  - CI/CD 集成：检测到任何 fail 项即阻断部署

退出码：
  0 = 全部健康
  1 = 检测到问题（fail 项 ≥ 1）
  2 = 脚本自身异常（参数错误等）
"""
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Optional


def _read_json(path: Path) -> Optional[dict]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def check_mirror_health(project_dir: Path) -> list[dict]:
    """检测项目镜像健康度，返回检查项列表。

    Args:
        project_dir: 项目根目录（含 .ae-sdd/ 和 .auto-engineering/）

    Returns:
        list[dict]，每项 {name, pass, message, severity}
        severity: "blocker" / "warn"
    """
    results: list[dict] = []
    ade_sdd = project_dir / ".ae-sdd"
    auto_root = project_dir / ".auto-engineering"
    mirror_path = ade_sdd / "state.json"

    # ─── 检查 1：.ae-sdd/ 目录存在 ──────────────────────────────────────────
    if not ade_sdd.is_dir():
        results.append({
            "name": ".ae-sdd/ 目录",
            "pass": False,
            "severity": "blocker",
            "message": f"未找到 .ae-sdd/ 目录（{ade_sdd}），项目未 init",
        })
        return results
    results.append({
        "name": ".ae-sdd/ 目录",
        "pass": True,
        "severity": "blocker",
        "message": f"{ade_sdd} 存在",
    })

    # ─── 检查 2：镜像存在性 + 反模式提示 ─────────────────────────────────────
    mirror_exists = mirror_path.is_file()
    if not mirror_exists:
        results.append({
            "name": ".ae-sdd/state.json 镜像",
            "pass": True,
            "severity": "warn",
            "message": "镜像不存在（v3.9.8 mirror-fallback 模式，CLI 走 work-item 源）",
        })
    else:
        results.append({
            "name": ".ae-sdd/state.json 镜像",
            "pass": True,
            "severity": "warn",
            "message": f"镜像存在（{mirror_path.stat().st_size} bytes）",
        })

    # ─── 检查 3：.auto-engineering/ 下 work-item 源清单 ──────────────────────
    if not auto_root.is_dir():
        results.append({
            "name": ".auto-engineering/ work-item 源",
            "pass": False,
            "severity": "blocker",
            "message": f"未找到 .auto-engineering/ 目录（{auto_root}）",
        })
        return results

    work_item_states: list[tuple[Path, float, dict]] = []
    for child in sorted(auto_root.iterdir()):
        if not child.is_dir():
            continue
        sp = child / "state.json"
        if not sp.is_file():
            continue
        data = _read_json(sp)
        if data is None:
            results.append({
                "name": f"work-item {child.name} state.json 解析",
                "pass": False,
                "severity": "blocker",
                "message": f"{sp} JSON 解析失败",
            })
            continue
        try:
            mtime = sp.stat().st_mtime
        except OSError:
            mtime = 0.0
        work_item_states.append((sp, mtime, data))

    if not work_item_states:
        results.append({
            "name": ".auto-engineering/ work-item 源",
            "pass": False,
            "severity": "blocker",
            "message": ".auto-engineering/ 下无任何 work-item state.json",
        })
        return results

    results.append({
        "name": ".auto-engineering/ work-item 源",
        "pass": True,
        "severity": "blocker",
        "message": f"发现 {len(work_item_states)} 个 work-item state",
    })

    # ─── 检查 4：每个 work-item state 必须有 phase 字段 ──────────────────────
    # 注意：nested state（stateModel=nested）的 phase 在 storyStates[*].phase，
    #   不在顶层。flat state 的 phase 在顶层。
    for sp, _, data in work_item_states:
        wi_name = sp.parent.name
        is_nested = data.get("stateModel") == "nested" or bool(data.get("storyStates"))
        if is_nested:
            story_states = data.get("storyStates") or {}
            if not story_states:
                results.append({
                    "name": f"work-item {wi_name} phase 字段",
                    "pass": False,
                    "severity": "blocker",
                    "message": f"{wi_name} 是 nested state 但 storyStates 为空",
                })
                continue
            missing_phases = [
                sid for sid, ss in story_states.items()
                if not (ss or {}).get("phase")
            ]
            if missing_phases:
                results.append({
                    "name": f"work-item {wi_name} phase 字段",
                    "pass": False,
                    "severity": "blocker",
                    "message": (
                        f"{wi_name} nested state 的 storyStates 子状态缺 phase: {missing_phases}"
                    ),
                })
            else:
                phases_summary = ", ".join(
                    f"{sid}={ss.get('phase')}" for sid, ss in story_states.items()
                )
                results.append({
                    "name": f"work-item {wi_name} phase 字段",
                    "pass": True,
                    "severity": "blocker",
                    "message": f"nested state storyStates: {phases_summary}",
                })
        else:
            phase = data.get("phase")
            if not phase:
                results.append({
                    "name": f"work-item {wi_name} phase 字段",
                    "pass": False,
                    "severity": "blocker",
                    "message": (
                        f"{wi_name} state.json 缺 phase 字段（currentStep={data.get('currentStep', 'N/A')}）"
                        f"，hook 链路会断开。请补 phase 字段（参考 ae-sdd PHASE_FLOWS）"
                    ),
                })
            else:
                results.append({
                    "name": f"work-item {wi_name} phase 字段",
                    "pass": True,
                    "severity": "blocker",
                    "message": f"phase={phase}",
                })

    # ─── 检查 5：镜像与最近活跃 work-item 一致性 ─────────────────────────────
    if mirror_exists:
        mirror_data = _read_json(mirror_path) or {}
        mirror_wi = (
            mirror_data.get("activeWorkItem")
            or mirror_data.get("currentWorkItem")
            or mirror_data.get("activeStory")
            or mirror_data.get("currentStory")
            or ""
        ).strip()
        # 按 mtime 选最近活跃 work-item
        work_item_states.sort(key=lambda x: x[1], reverse=True)
        recent_path = work_item_states[0][0]
        recent_wi = recent_path.parent.name

        if not mirror_wi:
            results.append({
                "name": "镜像 activeStory 锚点",
                "pass": False,
                "severity": "blocker",
                "message": "镜像存在但缺 activeStory/currentStory/activeWorkItem 锚点字段",
            })
        elif mirror_wi != recent_wi:
            results.append({
                "name": "镜像与最近活跃 work-item 一致性",
                "pass": False,
                "severity": "warn",
                "message": (
                    f"镜像 activeStory={mirror_wi}，"
                    f"但 .auto-engineering/ 下最近活跃 work-item={recent_wi}（按 mtime）。"
                    f"建议跑 `ae-sdd state relocate --story {recent_wi}` 重定位，或删除镜像走 fallback"
                ),
            })
        else:
            results.append({
                "name": "镜像与最近活跃 work-item 一致性",
                "pass": True,
                "severity": "warn",
                "message": f"镜像 activeStory={mirror_wi} 与最近活跃 work-item 一致",
            })

        # ─── 检查 6：镜像 activeStatePath 指死检测 ───────────────────────────
        active_path = (mirror_data.get("activeStatePath") or "").strip()
        if active_path:
            active_p = Path(active_path)
            if not active_p.is_file():
                results.append({
                    "name": "镜像 activeStatePath 指死检测",
                    "pass": False,
                    "severity": "blocker",
                    "message": (
                        f"镜像 activeStatePath={active_path} 但文件不存在（镜像指死）。"
                        f"建议删除镜像或 `ae-sdd state relocate --story {mirror_wi}`"
                    ),
                })
            else:
                results.append({
                    "name": "镜像 activeStatePath 指死检测",
                    "pass": True,
                    "severity": "blocker",
                    "message": f"activeStatePath 指向的文件存在",
                })

    # ─── 检查 7：检测 work-item currentStep 非标命名（step-X-） ──────────────
    import re
    step_pattern = re.compile(r"\bstep-\d+-[a-z][a-z0-9-]*", re.IGNORECASE)
    for sp, _, data in work_item_states:
        wi_name = sp.parent.name
        current_step = data.get("currentStep", "")
        if current_step and step_pattern.search(current_step):
            results.append({
                "name": f"work-item {wi_name} currentStep 命名规范",
                "pass": False,
                "severity": "warn",
                "message": (
                    f"{wi_name} state.json currentStep={current_step!r} 含非标 step-X- 命名。"
                    f"ae-sdd PHASE_FLOWS 不识别此命名，建议改用标准 phase 名（参考 prompt-inject 反模式提示）"
                ),
            })

    return results


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("用法: python check_mirror_health.py <project-dir> [--json]", file=sys.stderr)
        return 2
    project_dir = Path(argv[1]).resolve()
    as_json = "--json" in argv

    if not project_dir.is_dir():
        print(f"项目目录不存在: {project_dir}", file=sys.stderr)
        return 2

    results = check_mirror_health(project_dir)

    fail_count = sum(1 for r in results if not r["pass"])
    blocker_fail = sum(1 for r in results if not r["pass"] and r["severity"] == "blocker")

    if as_json:
        print(json.dumps({
            "project_dir": str(project_dir),
            "total": len(results),
            "passed": len(results) - fail_count,
            "failed": fail_count,
            "blocker_failed": blocker_fail,
            "items": results,
        }, ensure_ascii=False, indent=2))
    else:
        print(f"ae-sdd 镜像健康检测 @ {project_dir}")
        print(f"{'='*60}")
        for r in results:
            flag = "✅" if r["pass"] else ("🔴" if r["severity"] == "blocker" else "⚠️")
            print(f"  {flag} [{r['severity']}] {r['name']}: {r['message']}")
        print(f"{'='*60}")
        print(f"通过: {len(results) - fail_count} / {len(results)}"
              f"（blocker fail: {blocker_fail}）")

    return 1 if blocker_fail > 0 else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
