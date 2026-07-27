#!/usr/bin/env python3
"""test-tool 评分 skill — JSON Schema 校验脚本。

校验 score.py 产出的 metrics JSON 是否符合 metrics.schema.json。

优先用 jsonschema 包（若装了）；否则降级为基本结构检查（required 字段、类型）。
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def validate_with_jsonschema(metrics: dict, schema: dict) -> tuple[bool, list[str]]:
    """用 jsonschema 包校验。返回 (ok, errors)。"""
    try:
        import jsonschema
    except ImportError:
        return False, ["jsonschema 包未安装"]

    errors: list[str] = []
    validator = jsonschema.Draft202012Validator(schema)
    for err in validator.iter_errors(metrics):
        path = ".".join(str(p) for p in err.absolute_path) or "<root>"
        errors.append(f"{path}: {err.message}")
    return len(errors) == 0, errors


def validate_basic(metrics: dict, schema: dict) -> tuple[bool, list[str]]:
    """降级基本校验：检查 required 字段存在 + 顶层类型。"""
    errors: list[str] = []
    required = schema.get("required", [])
    for field in required:
        if field not in metrics:
            errors.append(f"<root>: 缺少必需字段 '{field}'")

    # 检查 schemaVersion
    if metrics.get("schemaVersion") != "ae-sdd.capabilityMetrics.test-tool.v1":
        errors.append(f"schemaVersion: 期望 'ae-sdd.capabilityMetrics.test-tool.v1'，实际 {metrics.get('schemaVersion')!r}")

    # 检查 totalScore 范围
    total = metrics.get("totalScore")
    if not isinstance(total, (int, float)) or not (0 <= total <= 100):
        errors.append(f"totalScore: 应为 0-100 的数字，实际 {total!r}")

    # 检查 runMeta.storyId 模式
    story_id = metrics.get("runMeta", {}).get("storyId", "")
    if not story_id.startswith("STORY-DEMO-TEST-TOOL-"):
        errors.append(f"runMeta.storyId: 应以 'STORY-DEMO-TEST-TOOL-' 开头，实际 {story_id!r}")

    # 检查维度分数对象存在
    for dim in ["complianceScore", "capabilityCoverage", "qualityScore", "efficiency"]:
        if dim not in metrics:
            errors.append(f"<root>: 缺少维度 '{dim}'")

    return len(errors) == 0, errors


def main() -> int:
    p = argparse.ArgumentParser(description="test-tool metrics JSON schema 校验")
    p.add_argument("--metrics", required=True, help="待校验的 metrics JSON 文件")
    p.add_argument("--schema", required=True, help="metrics.schema.json")
    p.add_argument("--strict", action="store_true", help="严格要求 jsonschema 包（无则失败）")
    args = p.parse_args()

    metrics_path = Path(args.metrics)
    schema_path = Path(args.schema)

    if not metrics_path.exists():
        print(f"ERROR: metrics file not found: {metrics_path}", file=sys.stderr)
        return 1
    if not schema_path.exists():
        print(f"ERROR: schema file not found: {schema_path}", file=sys.stderr)
        return 1

    try:
        metrics = json.loads(metrics_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        print(f"ERROR: metrics JSON 解析失败: {exc}", file=sys.stderr)
        return 2

    try:
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        print(f"ERROR: schema JSON 解析失败: {exc}", file=sys.stderr)
        return 2

    # 优先 jsonschema
    try:
        import jsonschema  # noqa: F401
        ok, errors = validate_with_jsonschema(metrics, schema)
        backend = "jsonschema"
    except ImportError:
        if args.strict:
            print("ERROR: --strict 模式要求安装 jsonschema 包：pip install jsonschema", file=sys.stderr)
            return 3
        ok, errors = validate_basic(metrics, schema)
        backend = "basic (jsonschema 未安装)"

    print(f"[validate] backend={backend}", file=sys.stderr)
    if ok:
        print(f"[validate] ✅ PASS: {metrics_path}", file=sys.stderr)
        # 简要摘要
        print(f"           totalScore={metrics.get('totalScore')} grade={metrics.get('grade')}", file=sys.stderr)
        print(f"           A={metrics.get('complianceScore', {}).get('total')} "
              f"B={metrics.get('capabilityCoverage', {}).get('total')} "
              f"C={metrics.get('qualityScore', {}).get('total')} "
              f"D={metrics.get('efficiency', {}).get('score')}", file=sys.stderr)
        print(f"           capabilityCeiling={metrics.get('qualityScore', {}).get('capabilityCeiling')}/30 "
              f"tier={metrics.get('qualityScore', {}).get('capabilityTier')}", file=sys.stderr)
        return 0
    else:
        print(f"[validate] ❌ FAIL: {metrics_path}", file=sys.stderr)
        for err in errors:
            print(f"           - {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
