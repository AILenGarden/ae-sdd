#!/usr/bin/env python3
"""
compile_all_skills.py - compile ae-sdd plus repository SKILL packages.

The ae-sdd source tree uses two SKILL shapes:

1. source/SKILL.md plus source/skills/**/*.md, compiled by build_dist.py into
   dist/ae-sdd.
2. Directory SKILL packages containing SKILL.md, compiled by the standalone
   skill-runtime-compiler into dist/compiled-skills by default.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")


SUMMARY_SCHEMA = "ae-sdd-compile-all-skills/v1"


def _load_skill_compiler(repo_root: Path):
    script_path = repo_root / "standalone-skills" / "skill-runtime-compiler" / "scripts" / "compile_skill_package.py"
    spec = importlib.util.spec_from_file_location("ae_sdd_skill_runtime_compiler", script_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load compiler script: {script_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _load_source_slimmer(repo_root: Path):
    script_path = repo_root / "scripts" / "slim_source_skills.py"
    spec = importlib.util.spec_from_file_location("ae_sdd_source_slimmer", script_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load source slimmer script: {script_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _run_build_dist(repo_root: Path) -> None:
    cmd = [sys.executable, str(repo_root / "scripts" / "build_dist.py")]
    result = subprocess.run(cmd, cwd=repo_root)
    if result.returncode != 0:
        raise RuntimeError(f"build_dist.py failed with exit code {result.returncode}")


def compile_all_skills(
    repo_root: Path,
    *,
    output_root: Path | None = None,
    include_references: bool = False,
    skip_source_slim: bool = False,
    skip_ae_sdd_dist: bool = False,
    skip_up_to_date: bool = True,
    dry_run: bool = False,
    force: bool = False,
) -> dict[str, Any]:
    repo_root = repo_root.resolve()
    if not repo_root.is_dir():
        raise RuntimeError(f"repo root is not a directory: {repo_root}")

    compiler = _load_skill_compiler(repo_root)
    output_root = output_root.resolve() if output_root else repo_root / "dist" / "compiled-skills"

    if skip_source_slim:
        slim_summary: dict[str, Any] = {"status": "skipped", "counts": {"slimmed": 0, "planned": 0, "skipped": 0}}
    else:
        slimmer = _load_source_slimmer(repo_root)
        slim_summary = slimmer.slim_source_skills(repo_root / "source", dry_run=dry_run)
        slim_summary["status"] = "planned" if dry_run else "completed"
        if slim_summary["counts"].get("failed", 0):
            raise RuntimeError(
                "source SKILL slimming validation failed; run "
                "`python scripts/slim_source_skills.py --validate --json` for details"
            )

    ae_sdd_status = "skipped"
    if not skip_ae_sdd_dist:
        if dry_run:
            ae_sdd_status = "planned"
        else:
            _run_build_dist(repo_root)
            ae_sdd_status = "compiled"

    batch_summary = compiler.compile_skill_packages_under(
        repo_root,
        output_root=output_root,
        include_references=include_references,
        include_compiled=False,
        exclude_prefixes=("source",),
        skip_up_to_date=skip_up_to_date,
        dry_run=dry_run,
        force=force,
    )

    summary: dict[str, Any] = {
        "schema": SUMMARY_SCHEMA,
        "deterministic": True,
        "repo_root": ".",
        "source_slim": slim_summary,
        "ae_sdd": {
            "status": ae_sdd_status,
            "output": "dist/ae-sdd",
            "covers": ["source/SKILL.md", "source/skills/**/*.md"],
        },
        "directory_skills": batch_summary,
    }
    if batch_summary["counts"]["failed"]:
        return summary
    if not dry_run:
        output_root.mkdir(parents=True, exist_ok=True)
        (output_root / "ae-sdd-compile-all.json").write_text(
            json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
            newline="\n",
        )
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description="Compile all ae-sdd SKILL runtime artifacts.")
    parser.add_argument("--repo-root", type=Path, default=None, help="Repository root; default = script parent parent")
    parser.add_argument("--output-root", type=Path, default=None, help="Directory SKILL output root; default = dist/compiled-skills")
    parser.add_argument("--include-references", action="store_true", help="Also compile SKILL packages under references/")
    parser.add_argument("--skip-source-slim", action="store_true", help="Do not slim source/SKILL.md and source/skills/**/*.md before compiling")
    parser.add_argument("--skip-ae-sdd-dist", action="store_true", help="Do not rebuild dist/ae-sdd")
    parser.add_argument("--no-skip-up-to-date", action="store_true", help="Rebuild generated directory SKILL outputs even if current")
    parser.add_argument("--dry-run", action="store_true", help="Report planned work without writing generated outputs")
    parser.add_argument("--force", action="store_true", help="Allow replacing unrelated existing directory SKILL outputs")
    parser.add_argument("--json", action="store_true", help="Print machine-readable summary")
    args = parser.parse_args()

    repo_root = args.repo_root.resolve() if args.repo_root else Path(__file__).resolve().parent.parent
    try:
        summary = compile_all_skills(
            repo_root,
            output_root=args.output_root,
            include_references=args.include_references,
            skip_source_slim=args.skip_source_slim,
            skip_ae_sdd_dist=args.skip_ae_sdd_dist,
            skip_up_to_date=not args.no_skip_up_to_date,
            dry_run=args.dry_run,
            force=args.force,
        )
    except Exception as exc:
        print(f"[compile-all-skills] ERROR: {exc}", file=sys.stderr)
        return 1

    counts = summary["directory_skills"]["counts"]
    if args.json:
        print(json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        print(
            "[compile-all-skills] ok "
            f"source_slimmed={summary['source_slim']['counts']['slimmed']} "
            f"source_upgraded={summary['source_slim']['counts'].get('upgraded', 0)} "
            f"source_planned={summary['source_slim']['counts']['planned']} "
            f"source_skipped={summary['source_slim']['counts']['skipped']} "
            f"source_validated={summary['source_slim']['counts'].get('validated', 0)} "
            f"source_failed={summary['source_slim']['counts'].get('failed', 0)} "
            f"ae_sdd={summary['ae_sdd']['status']} "
            f"directory_compiled={counts['compiled']} "
            f"up_to_date={counts['up_to_date']} "
            f"planned={counts['planned']} "
            f"skipped={counts['skipped']} "
            f"failed={counts['failed']} "
            f"output_root={summary['directory_skills']['output_root']}"
        )
    return 1 if counts["failed"] else 0


if __name__ == "__main__":
    sys.exit(main())
