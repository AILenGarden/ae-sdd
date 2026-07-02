#!/usr/bin/env python3
"""Detect the layout of a Maven project so the loop can locate the right pom,
test source folder, and report whether jacoco-maven-plugin is configured.

Usage:
    detect_project.py <project-root> [--class com.example.Foo]

Output (stdout, JSON):
    {
      "project_root": "/abs/path",
      "is_maven": true,
      "modules": [
        {
          "rel_path": ".",
          "pom": "/abs/path/pom.xml",
          "main_src": "/abs/path/src/main/java",
          "test_src": "/abs/path/src/test/java",
          "has_jacoco_plugin": true,
          "has_surefire": true
        },
        ...
      ],
      "target_class": {                       # only when --class given and located
        "fqcn": "com.example.Foo",
        "main_file": "/abs/path/.../Foo.java",
        "module_rel_path": ".",
        "expected_test_file": "/abs/path/.../FooTest.java",
        "expected_test_exists": false
      }
    }

Exit 0 on success, 2 on usage error.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


JACOCO_PAT = re.compile(r"<artifactId>\s*jacoco-maven-plugin\s*</artifactId>", re.I)
SUREFIRE_PAT = re.compile(r"<artifactId>\s*maven-surefire-plugin\s*</artifactId>", re.I)


def discover_modules(root: Path) -> list[dict]:
    modules: list[dict] = []
    # Walk only down to depth 4; deeper trees are unusual for module roots.
    for pom in sorted(root.rglob("pom.xml")):
        # ignore build outputs
        parts = set(pom.parts)
        if "target" in parts or "node_modules" in parts:
            continue
        rel = pom.parent.relative_to(root).as_posix() or "."
        text = pom.read_text(encoding="utf-8", errors="replace")
        modules.append({
            "rel_path": rel,
            "pom": str(pom),
            "main_src": _existing(pom.parent / "src" / "main" / "java"),
            "test_src": _existing(pom.parent / "src" / "test" / "java"),
            "has_jacoco_plugin": bool(JACOCO_PAT.search(text)),
            "has_surefire": bool(SUREFIRE_PAT.search(text)),
        })
    return modules


def _existing(p: Path) -> str | None:
    return str(p) if p.is_dir() else None


def find_class(root: Path, fqcn: str, modules: list[dict]) -> dict | None:
    rel_java = fqcn.replace(".", "/") + ".java"
    for mod in modules:
        if not mod["main_src"]:
            continue
        cand = Path(mod["main_src"]) / rel_java
        if cand.is_file():
            test_src = mod["test_src"] or str(Path(mod["pom"]).parent / "src" / "test" / "java")
            test_path_rel = fqcn.replace(".", "/") + "Test.java"
            test_file = Path(test_src) / test_path_rel
            return {
                "fqcn": fqcn,
                "main_file": str(cand),
                "module_rel_path": mod["rel_path"],
                "expected_test_file": str(test_file),
                "expected_test_exists": test_file.is_file(),
            }
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description="Detect Maven project layout for the UT coverage loop.")
    ap.add_argument("project_root")
    ap.add_argument("--class", dest="fqcn", default=None,
                    help="Optional FQCN to locate; output includes its main/test paths.")
    args = ap.parse_args()

    root = Path(args.project_root).resolve()
    if not root.is_dir():
        print(json.dumps({"error": f"not a directory: {root}"}), file=sys.stderr)
        return 2

    is_maven = (root / "pom.xml").is_file()
    modules = discover_modules(root) if is_maven else []
    out: dict = {
        "project_root": str(root),
        "is_maven": is_maven,
        "modules": modules,
    }
    if args.fqcn:
        out["target_class"] = find_class(root, args.fqcn, modules)

    json.dump(out, sys.stdout, indent=2, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
