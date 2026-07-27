#!/usr/bin/env python3
"""test-tool 评分 skill — 数据采集脚本。

从仓库自动采集所有可机读的评分证据，输出 .collected.json 供 score.py 评分。

采集范围：
  - cargo fmt --check / cargo clippy / cargo test 输出
  - .auto-engineering/<story>/state.json
  - .auto-engineering/<story>/evidence/manifest.json
  - .ae-sdd/runtime-stats/<date>.jsonl
  - 代码静态统计（git ls-files / wc -l / grep unsafe|unwrap|todo）
  - 关键文件 sha256
  - 测试用例通过/失败解析

不采集（需操作员手动提供）：
  - 总耗时 / 各阶段耗时 / token / 轮次 / 模型名 / 单价
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


# ---------------------------------------------------------------------------
# 工具函数
# ---------------------------------------------------------------------------


def sha256_file(path: Path) -> str:
    if not path.exists():
        return "0" * 64
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def run_cmd(cmd: list[str], cwd: Path | None = None, timeout: int = 300) -> tuple[int, str, str]:
    """运行命令，返回 (returncode, stdout, stderr)。"""
    try:
        result = subprocess.run(
            cmd,
            cwd=str(cwd) if cwd else None,
            capture_output=True,
            text=True,
            timeout=timeout,
            shell=False,
        )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return 124, "", f"TIMEOUT after {timeout}s"
    except FileNotFoundError as exc:
        return 127, "", str(exc)


# ---------------------------------------------------------------------------
# 采集器
# ---------------------------------------------------------------------------


def collect_cargo(demo_dir: Path) -> dict[str, Any]:
    """跑 cargo fmt/clippy/test，采集 AC 通过情况。"""
    out: dict[str, Any] = {}

    # cargo fmt --check
    rc, stdout, stderr = run_cmd(
        ["cargo", "fmt", "--check"], cwd=demo_dir, timeout=60
    )
    out["fmtCheckPassed"] = rc == 0
    out["fmtDiff"] = stdout if rc != 0 else ""

    # cargo clippy --all-targets -- -D warnings
    rc, stdout, stderr = run_cmd(
        ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
        cwd=demo_dir,
        timeout=300,
    )
    out["clippyWarnings"] = _count_clippy_warnings(stderr + stdout) if rc != 0 else 0
    out["clippyOutput"] = (stderr + stdout)[-2000:] if rc != 0 else ""

    # cargo test --list（统计用例数）
    rc, stdout, stderr = run_cmd(
        ["cargo", "test", "--", "--list", "--format", "terse"],
        cwd=demo_dir,
        timeout=120,
    )
    test_list = stdout + stderr
    out["testCasesTotal"] = test_list.count(": test")

    # cargo test（实际跑，解析通过/失败）
    rc, stdout, stderr = run_cmd(
        ["cargo", "test", "--no-fail-fast"], cwd=demo_dir, timeout=600
    )
    full = stdout + stderr
    out["cargoTestExitCode"] = rc
    out["cargoTestTail"] = full[-3000:]

    passed = len(re.findall(r"^test .+ \.\.\. ok$", full, re.MULTILINE))
    failed = len(re.findall(r"^test .+ \.\.\. FAILED$", full, re.MULTILINE))
    out["testCasesPassed"] = passed
    out["testCasesFailed"] = failed

    # 按名字判定哪些 AC 通过（测试名约定包含 ac_1/ac_2/...）
    out["acResults"] = _parse_ac_results(full)
    return out


def _count_clippy_warnings(output: str) -> int:
    return len(re.findall(r"^warning:", output, re.MULTILINE))


def _parse_ac_results(test_output: str) -> dict[str, bool]:
    """从 cargo test 输出里按测试名解析 AC 通过情况。

    约定测试名含 ac_1/ac_2/.../ac_18 关键字（ae-sdd 实现的测试名应符合此约定）。
    """
    ac_map: dict[str, bool] = {}
    for ac_id_num in range(1, 19):
        ac_id = f"AC-{ac_id_num}"
        # 匹配 ac_1 / ac1 / ac_01 等变体
        pattern = rf"test .*ac_?0?{ac_id_num}.* \.\.\. (\w+)$"
        matches = re.findall(pattern, test_output, re.MULTILINE | re.IGNORECASE)
        if not matches:
            ac_map[ac_id] = False
            continue
        # 全部 ok 才算通过
        ac_map[ac_id] = all(m == "ok" for m in matches)
    return ac_map


def collect_state(repo: Path, story_id: str) -> dict[str, Any]:
    """读 .auto-engineering/<story>/state.json + evidence/manifest.json。"""
    out: dict[str, Any] = {"storyId": story_id, "found": False}

    # 找 state 目录（目录名包含 story_id）
    ae_root = repo / ".auto-engineering"
    if not ae_root.exists():
        return out

    state_dir = None
    for entry in ae_root.iterdir():
        if story_id in entry.name:
            state_dir = entry
            break
    if state_dir is None:
        return out

    out["found"] = True
    out["stateDir"] = str(state_dir)

    state_file = state_dir / "state.json"
    if state_file.exists():
        try:
            state = json.loads(state_file.read_text(encoding="utf-8"))
            out["state"] = {
                "phase": state.get("phase"),
                "currentPhase": state.get("currentPhase"),
                "scale": state.get("scale"),
                "revision": state.get("revision"),
                "lastFencingToken": state.get("lastFencingToken"),
                "routeDecision": state.get("routeDecision"),
                "executionPlan": state.get("executionPlan"),
                "reviewSession": state.get("reviewSession"),
                "reviewLoop": state.get("reviewLoop"),
                "historyCount": len(state.get("history", [])),
                "history": state.get("history", []),
                "completedSteps": state.get("completedSteps", []),
            }
        except Exception as exc:
            out["stateError"] = str(exc)
    else:
        out["stateExists"] = False

    # evidence/manifest.json
    evidence_dir = state_dir / "evidence"
    out["evidenceDir"] = str(evidence_dir) if evidence_dir.exists() else None

    manifest_file = evidence_dir / "manifest.json"
    if manifest_file.exists():
        try:
            manifest = json.loads(manifest_file.read_text(encoding="utf-8"))
            # manifest 可能是单个对象或 list
            if isinstance(manifest, dict):
                entries = [manifest]
            elif isinstance(manifest, list):
                entries = manifest
            else:
                entries = []
            out["evidence"] = {
                "entries": entries,
                "allExitZero": all(e.get("exitCode") == 0 for e in entries if isinstance(e, dict)),
                "hasTestSnapshot": any(
                    "cargo test" in str(e.get("summary", "")).lower()
                    or "cargo test" in str(e.get("command", "")).lower()
                    for e in entries
                    if isinstance(e, dict)
                ),
            }
        except Exception as exc:
            out["evidenceError"] = str(exc)

    # 列出 evidence artifacts（含 sha256 文件名前缀）
    artifacts_dir = evidence_dir / "artifacts"
    if artifacts_dir.exists():
        out["evidenceArtifacts"] = [
            f.name for f in artifacts_dir.iterdir() if f.is_file()
        ]

    # sha256 of state.json
    out["stateSha256"] = sha256_file(state_file)
    return out


def collect_runtime_stats(repo: Path, story_id: str) -> dict[str, Any]:
    """读 .ae-sdd/runtime-stats/<date>.jsonl，统计 CLI 调用次数与 gate BLOCK。"""
    stats_dir = repo / ".ae-sdd" / "runtime-stats"
    out: dict[str, Any] = {"found": stats_dir.exists(), "cliInvocations": 0, "gateBlocks": {}}

    if not stats_dir.exists():
        return out

    # 找最近 7 天的 jsonl 文件
    files = sorted(stats_dir.glob("*.jsonl"), reverse=True)[:7]
    gate_block_count = 0
    gate_ids: set[str] = set()
    cli_invocations = 0

    for f in files:
        try:
            for line in f.read_text(encoding="utf-8").splitlines():
                if not line.strip():
                    continue
                try:
                    entry = json.loads(line)
                except json.JSONDecodeError:
                    continue
                cli_invocations += 1
                for span in entry.get("spans", []):
                    attrs = span.get("attrs", {})
                    if attrs.get("allowed") is False:
                        gate_block_count += 1
                        tool = attrs.get("toolName", "unknown")
                        reason = attrs.get("reason", "")
                        gate_ids.add(f"{tool}:{reason[:50]}")
        except Exception:
            continue

    out["cliInvocations"] = cli_invocations
    out["gateBlocks"] = {
        "count": gate_block_count,
        "gateIds": sorted(gate_ids),
    }
    return out


def collect_code_stats(demo_dir: Path) -> dict[str, Any]:
    """统计代码行数、文件数、禁止模式计数。"""
    src_dir = demo_dir / "src"
    out: dict[str, Any] = {"srcDirExists": src_dir.exists()}

    if not src_dir.exists():
        return out

    # 文件分类计数
    rs_files = list(src_dir.rglob("*.rs"))
    out["filesTotal"] = len(rs_files)
    out["filesContracts"] = sum(
        1 for f in rs_files if "contract" in f.name.lower()
    )
    out["filesAlgorithm"] = sum(
        1 for f in rs_files if "algorithm" in str(f).lower().replace("\\", "/")
    )
    elective_names = {"pathfinder", "generic", "builder", "serde_impl"}
    out["filesElective"] = sum(
        1 for f in rs_files
        if any(name in f.stem.lower() for name in elective_names)
    )

    tests_dir = src_dir / "tests"
    out["filesTests"] = (
        sum(1 for f in tests_dir.rglob("*.rs")) if tests_dir.exists() else 0
    )

    # LOC 统计（无 cloc/tokei 时用 wc -l）
    out["locSrc"] = _count_loc(rs_files)
    test_files = list(tests_dir.rglob("*.rs")) if tests_dir.exists() else []
    out["locTests"] = _count_loc(test_files)
    out["locTotal"] = out["locSrc"] + out["locTests"]

    # 禁止模式
    out["forbiddenPatterns"] = _count_forbidden_patterns(src_dir, test_files)
    return out


def _count_loc(files: list[Path]) -> int:
    """简单 LOC 计数：非空非注释行。无 cloc/tokei 时降级用 wc -l。"""
    # 优先 cloc
    rc, stdout, _ = run_cmd(["cloc", "--json", "--by-file", "--include-lang=Rust"]
                            + [str(f) for f in files], timeout=60)
    if rc == 0:
        try:
            data = json.loads(stdout)
            sum_val = data.get("sum", {})
            code = sum_val.get("code", 0)
            if isinstance(code, int) and code > 0:
                return code
        except json.JSONDecodeError:
            pass

    # 降级 wc -l
    total = 0
    for f in files:
        try:
            with f.open("r", encoding="utf-8", errors="ignore") as fh:
                for line in fh:
                    stripped = line.strip()
                    if stripped and not stripped.startswith("//"):
                        total += 1
        except OSError:
            continue
    return total


def _count_forbidden_patterns(src_dir: Path, test_files: list[Path]) -> dict[str, int]:
    """grep 统计 unsafe/unwrap/expect/todo/unimplemented/panic。"""
    patterns = {
        "unsafeCount": r"\bunsafe\b",
        "unwrapCount": r"\.unwrap\(\)",
        "expectCount": r"\.expect\(",
        "todoCount": r"\b(todo!|unimplemented!)\b",
        "panicCount": r"\bpanic!?\b",
    }

    def _count_in_files(files: list[Path], pattern: str) -> int:
        count = 0
        regex = re.compile(pattern)
        for f in files:
            try:
                for line in f.read_text(encoding="utf-8", errors="ignore").splitlines():
                    if regex.search(line):
                        count += 1
            except OSError:
                continue
        return count

    # 只统计非 test 代码
    src_files = [f for f in src_dir.rglob("*.rs") if "tests" not in str(f).replace("\\", "/").lower()]
    return {k: _count_in_files(src_files, p) for k, p in patterns.items()}


def collect_artifact_hashes(repo: Path, story_id: str, demo_dir: Path) -> list[dict[str, Any]]:
    """采集关键文件 sha256 清单。"""
    docs_root = repo / "ae-sdd-doc"
    ae_root = repo / ".auto-engineering"

    candidates: list[tuple[str, Path]] = [
        ("ra", docs_root / "RA" / f"RA-AE-SDD-CAPABILITY-TEST-TEST-TOOL.md"),
    ]

    # 找 story 对应的 DR/Story/Coding/Test/CR 文件
    story_doc = docs_root / "Story" / f"{story_id}.md"
    candidates.append(("story", story_doc))

    # Coding/Test/CR 目录
    for role, dirname in [
        ("codingPlan", "Coding"),
        ("evidenceManifest", "Test"),
    ]:
        d = docs_root / dirname / story_id
        if d.exists():
            for f in d.glob("*.md"):
                candidates.append((role, f))
            break

    # state 目录下的文件
    for entry in ae_root.iterdir():
        if story_id in entry.name:
            candidates.append(("state", entry / "state.json"))
            manifest = entry / "evidence" / "manifest.json"
            candidates.append(("evidenceManifest", manifest))
            break

    # demo src 文件
    src_dir = demo_dir / "src"
    if src_dir.exists():
        candidates.append(("codeLib", src_dir / "lib.rs"))
        candidates.append(("codeContracts", src_dir / "contracts.rs"))
        for name in ["pathfinder", "generic", "builder", "serde_impl"]:
            candidates.append((f"code{name.capitalize()}", src_dir / f"{name}.rs"))

    result: list[dict[str, Any]] = []
    for role, path in candidates:
        exists = path.exists()
        result.append(
            {
                "role": role,
                "path": str(path.relative_to(repo)) if path.is_relative_to(repo) else str(path),
                "sha256": sha256_file(path) if exists else "0" * 64,
                "exists": exists,
            }
        )
    return result


# ---------------------------------------------------------------------------
# 主函数
# ---------------------------------------------------------------------------


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="test-tool 评分采集")
    p.add_argument("--repo", default="D:/Item/ae-sdd", help="ae-sdd 仓库根")
    p.add_argument("--story-id", required=True, help="如 STORY-DEMO-TEST-TOOL-001")
    p.add_argument("--operator", default="user:operator")
    p.add_argument("--ae-sdd-version", default="unknown")
    p.add_argument("--host-agent", default="ZCode")
    p.add_argument("--model-id", default="")
    p.add_argument("--total-minutes", type=float, required=True)
    # phase 耗时
    p.add_argument("--phase-route", type=float, default=0)
    p.add_argument("--phase-ra", type=float, default=0)
    p.add_argument("--phase-dr", type=float, default=0)
    p.add_argument("--phase-story", type=float, default=0)
    p.add_argument("--phase-coding-plan", dest="phase_coding_plan", type=float, default=0)
    p.add_argument("--phase-coding", type=float, default=0)
    p.add_argument("--phase-test", type=float, default=0)
    p.add_argument("--phase-review", type=float, default=0)
    # token
    p.add_argument("--tokens-input", type=int, required=True)
    p.add_argument("--tokens-output", type=int, required=True)
    p.add_argument("--tokens-cached", type=int, default=0)
    p.add_argument("--context-window-peak", type=int, default=0)
    # 其他
    p.add_argument("--turn-count", type=int, required=True)
    p.add_argument("--baseline-revision", type=int, required=True)
    p.add_argument("--input-price", type=float, default=0.0, help="USD per 1M input tokens")
    p.add_argument("--output-price", type=float, default=0.0, help="USD per 1M output tokens")
    p.add_argument("--started-at", default="")
    p.add_argument("--finished-at", default="")
    p.add_argument("--output", default=".collected.json", help="输出文件路径")
    return p.parse_args()


def main() -> int:
    args = parse_args()
    repo = Path(args.repo)
    demo_dir = repo / "demo" / "test-tool"

    if not demo_dir.exists():
        print(f"ERROR: demo dir not found: {demo_dir}", file=sys.stderr)
        return 1

    print(f"[collect] story_id={args.story_id}", file=sys.stderr)
    print(f"[collect] demo_dir={demo_dir}", file=sys.stderr)

    collected: dict[str, Any] = {
        "schemaVersion": "ae-sdd.capabilityMetrics.test-tool.v1",
        "runMeta": {
            "storyId": args.story_id,
            "operator": args.operator,
            "aeSddVersion": args.ae_sdd_version,
            "hostAgent": args.host_agent,
            "modelId": args.model_id,
            "startedAt": args.started_at,
            "finishedAt": args.finished_at,
            "baselineGitSha": _git_sha(repo),
            "baselineStateRevision": args.baseline_revision,
            "totalDurationMinutes": args.total_minutes,
            "phaseDurations": {
                "route": args.phase_route,
                "ra": args.phase_ra,
                "dr": args.phase_dr,
                "story": args.phase_story,
                "codingPlan": args.phase_coding_plan,
                "coding": args.phase_coding,
                "test": args.phase_test,
                "review": args.phase_review,
            },
            "tokens": {
                "input": args.tokens_input,
                "output": args.tokens_output,
                "total": args.tokens_input + args.tokens_output,
                "cached": args.tokens_cached,
                "contextWindowPeak": args.context_window_peak,
            },
            "cost": {
                "modelName": args.model_id,
                "inputPricePerMillion": args.input_price,
                "outputPricePerMillion": args.output_price,
                "inputUsd": args.tokens_input * args.input_price / 1_000_000,
                "outputUsd": args.tokens_output * args.output_price / 1_000_000,
                "totalUsd": (
                    args.tokens_input * args.input_price
                    + args.tokens_output * args.output_price
                )
                / 1_000_000,
            },
            "turnCount": args.turn_count,
            "baselineRevision": args.baseline_revision,
        },
    }

    print("[collect] running cargo fmt/clippy/test ...", file=sys.stderr)
    collected["cargo"] = collect_cargo(demo_dir)

    print("[collect] reading ae-sdd state ...", file=sys.stderr)
    collected["state"] = collect_state(repo, args.story_id)

    print("[collect] reading runtime-stats ...", file=sys.stderr)
    collected["runtimeStats"] = collect_runtime_stats(repo, args.story_id)

    print("[collect] counting code stats ...", file=sys.stderr)
    collected["codeStats"] = collect_code_stats(demo_dir)

    print("[collect] hashing artifacts ...", file=sys.stderr)
    collected["artifacts"] = collect_artifact_hashes(repo, args.story_id, demo_dir)

    # 采集时间戳
    collected["collectedAt"] = datetime.now(timezone.utc).isoformat()

    output_path = Path(args.output)
    output_path.write_text(
        json.dumps(collected, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    print(f"[collect] OK -> {output_path}", file=sys.stderr)
    return 0


def _git_sha(repo: Path) -> str:
    rc, stdout, _ = run_cmd(["git", "rev-parse", "HEAD"], cwd=repo, timeout=10)
    return stdout.strip() if rc == 0 else "unknown"


if __name__ == "__main__":
    sys.exit(main())
