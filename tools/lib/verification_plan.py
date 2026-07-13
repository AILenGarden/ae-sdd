"""Deterministic change classification for minimal verification planning."""
from __future__ import annotations

from pathlib import Path
from typing import Iterable

from .review_batch import canonical_fingerprint


def classify_path(path: str) -> str:
    value = path.replace("\\", "/").lower()
    if value.endswith((".md", ".rst", ".adoc")):
        return "documentation"
    if value.endswith(("test.java", "tests.py", "_test.py", ".test.ts", ".spec.ts")) or "/test/" in value:
        return "test-code"
    if value.endswith(("pom.xml", "build.gradle", "build.gradle.kts", ".yml", ".yaml", ".properties")):
        return "build-or-config"
    if value.endswith((".java", ".kt", ".py", ".js", ".ts", ".go", ".cs")):
        return "production-code"
    return "other"


def build_plan(project_dir: Path, story_id: str, changed_paths: Iterable[str], since_fingerprint: str = "") -> dict:
    paths = sorted({str(p).replace("\\", "/") for p in changed_paths if str(p).strip()})
    classes = sorted({classify_path(p) for p in paths})
    modules = sorted({Path(p).parts[0] for p in paths if Path(p).parts})
    required = []
    deferred = []
    not_required = []
    if "production-code" in classes:
        required.extend(["focused-test", "module-test", "G-09", "G-CODE-1-delta"])
        deferred.append("full-story-regression")
    if "test-code" in classes:
        required.append("affected-tests")
        deferred.append("final-story-test-suite")
    if "build-or-config" in classes:
        required.extend(["affected-module-package", "package-after-test"])
    if "documentation" in classes and not any(c in classes for c in ("production-code", "test-code", "build-or-config")):
        required.extend(["document-schema", "AC/TC-mapping"])
        not_required.append("Maven/full-story-regression")
    if not required:
        required.append("targeted-validation")
    return {
        "schemaVersion": 1,
        "storyId": story_id,
        "sinceFingerprint": since_fingerprint,
        "changeClass": classes,
        "affectedModules": modules,
        "required": list(dict.fromkeys(required)),
        "deferredUntilFinal": list(dict.fromkeys(deferred)),
        "notRequired": list(dict.fromkeys(not_required)),
        "planFingerprint": canonical_fingerprint({"storyId": story_id, "paths": paths, "classes": classes}),
        "changedPaths": paths,
    }
