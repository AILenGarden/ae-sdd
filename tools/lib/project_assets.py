"""Project asset generation and validation helpers for ae-sdd.

This module provides the executable baseline for the documented
``ae-sdd assets generate/check`` contract. The generated file is intentionally
conservative: it records discovered project structure and creates all required
G-00 sections, while leaving detailed business semantics for later asset
updates.
"""
from __future__ import annotations

import os
import shutil
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Optional

from lib import paths


REQUIRED_SECTIONS: tuple[str, ...] = ("§A", "§B", "§C", "§D", "§E", "§F", "§G")

SKIP_DIRS: set[str] = {
    ".git",
    ".ae-sdd",
    ".auto-engineering",
    ".claude",
    ".codex",
    ".zcode",
    ".idea",
    ".vscode",
    "__pycache__",
    ".pytest_cache",
    "node_modules",
    "target",
    "build",
    "dist",
    "out",
}

BUILD_FILES: tuple[str, ...] = (
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "package.json",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "Cargo.toml",
)

SOURCE_DIR_NAMES: tuple[str, ...] = (
    "src/main/java",
    "src/main/kotlin",
    "src/main/resources",
    "src/test/java",
    "src",
    "app",
    "apps",
    "lib",
)

CONFIG_FILE_NAMES: tuple[str, ...] = (
    "application.yml",
    "application.yaml",
    "application.properties",
    "application-dev.yml",
    "application-dev.yaml",
    "application-test.yml",
    "application-test.yaml",
    ".env.example",
    "Dockerfile",
    "docker-compose.yml",
    "docker-compose.yaml",
)


@dataclass
class AssetGenerationResult:
    asset_file: Path
    project_key: str
    created: bool
    changed: bool
    backup_file: Optional[Path]
    missing_before: list[str]
    missing_after: list[str]

    @property
    def pass_(self) -> bool:
        return not self.missing_after

    def to_dict(self) -> dict:
        return {
            "assetFile": str(self.asset_file),
            "projectKey": self.project_key,
            "created": self.created,
            "changed": self.changed,
            "backupFile": str(self.backup_file) if self.backup_file else None,
            "missingBefore": self.missing_before,
            "missingAfter": self.missing_after,
            "pass": self.pass_,
        }


def missing_required_sections(text: str) -> list[str]:
    """Return the missing G-00 asset section markers."""
    return [section for section in REQUIRED_SECTIONS if section not in text]


def has_required_sections(text: str) -> bool:
    return not missing_required_sections(text)


def _strip_quotes(value: str) -> str:
    return value.strip().strip('"').strip("'")


def asset_path_for(ade_sdd: Path, project_key: str) -> Path:
    """Resolve the configured project asset path.

    ``paths.find_asset_file`` only returns existing files. Generation needs the
    target path before the file exists, so this function consumes
    ``assetPath`` from config.yaml and falls back to the historical default.
    """
    cfg = paths.read_config(ade_sdd)
    raw_asset_path = _strip_quotes(str(cfg.get("assetPath") or ""))
    if raw_asset_path:
        configured = Path(raw_asset_path)
        if configured.is_absolute():
            return configured
        return ade_sdd / configured
    return paths.assets_dir(ade_sdd) / f"{project_key}.assets.md"


def check_asset(ade_sdd: Path, project_key: str) -> dict:
    asset_file = paths.find_asset_file(ade_sdd, project_key)
    exists = bool(asset_file and asset_file.is_file())
    text = ""
    read_error: Optional[str] = None
    if exists and asset_file is not None:
        try:
            text = asset_file.read_text(encoding="utf-8")
        except OSError as exc:
            read_error = str(exc)
    missing = missing_required_sections(text) if exists and read_error is None else list(REQUIRED_SECTIONS)
    return {
        "assetFile": str(asset_file) if asset_file else str(asset_path_for(ade_sdd, project_key)),
        "projectKey": project_key,
        "exists": exists,
        "readError": read_error,
        "missingSections": missing,
        "pass": exists and read_error is None and not missing,
    }


def _relative(path: Path, root: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()


def _iter_project_dirs(project_root: Path, max_depth: int = 4) -> Iterable[Path]:
    root_parts = len(project_root.parts)
    for current, dir_names, _ in os.walk(project_root):
        current_path = Path(current)
        dir_names[:] = [name for name in dir_names if name not in SKIP_DIRS]
        if len(current_path.parts) - root_parts > max_depth:
            dir_names[:] = []
            continue
        yield current_path


def discover_modules(project_root: Path) -> list[dict]:
    modules: list[dict] = []
    for directory in _iter_project_dirs(project_root):
        present_build_files = [name for name in BUILD_FILES if (directory / name).is_file()]
        if not present_build_files:
            continue
        present_source_dirs = [
            name for name in SOURCE_DIR_NAMES
            if (directory / Path(name)).is_dir()
        ]
        modules.append({
            "name": directory.name if directory != project_root else project_root.name,
            "path": _relative(directory, project_root),
            "buildFiles": present_build_files,
            "sourceDirs": present_source_dirs,
        })
    if not modules:
        modules.append({
            "name": project_root.name,
            "path": ".",
            "buildFiles": [],
            "sourceDirs": [
                name for name in SOURCE_DIR_NAMES
                if (project_root / Path(name)).is_dir()
            ],
        })
    return modules


def discover_named_files(project_root: Path, file_names: Iterable[str], limit: int = 40) -> list[str]:
    wanted = set(file_names)
    found: list[str] = []
    for current, dir_names, file_names_in_dir in os.walk(project_root):
        dir_names[:] = [name for name in dir_names if name not in SKIP_DIRS]
        for name in sorted(file_names_in_dir):
            if name in wanted:
                found.append(_relative(Path(current) / name, project_root))
                if len(found) >= limit:
                    return found
    return found


def _markdown_table(headers: tuple[str, ...], rows: Iterable[Iterable[str]]) -> str:
    header = "| " + " | ".join(headers) + " |"
    separator = "| " + " | ".join("---" for _ in headers) + " |"
    body = ["| " + " | ".join(str(cell) for cell in row) + " |" for row in rows]
    return "\n".join([header, separator, *body])


def _render_list(items: list[str], empty: str) -> str:
    if not items:
        return f"- {empty}"
    return "\n".join(f"- `{item}`" for item in items)


def render_project_assets(
    project_root: Path,
    project_key: str,
    *,
    generated_at: Optional[datetime] = None,
    source: str = "auto-baseline",
) -> str:
    generated_at = generated_at or datetime.now(timezone.utc)
    generated_date = generated_at.strftime("%Y-%m-%d")
    generated_ts = generated_at.strftime("%Y-%m-%dT%H:%M:%SZ")
    modules = discover_modules(project_root)
    config_files = discover_named_files(project_root, CONFIG_FILE_NAMES)

    metadata_rows = [
        ("projectKey", f"`{project_key}`"),
        ("gitPath", f"`{project_root}`"),
        ("docWorkspacePath", f"`{project_root}`"),
        ("lastAuditedAt", f"`{generated_date}`"),
        ("generatedAt", f"`{generated_ts}`"),
        ("generatedBy", "`ae-sdd assets generate`"),
        ("source", f"`{source}`"),
        ("confidence", "`auto-baseline`"),
    ]

    module_rows = [
        (
            module["name"],
            f"`{module['path']}`",
            ", ".join(f"`{item}`" for item in module["buildFiles"]) or "待确认",
            ", ".join(f"`{item}`" for item in module["sourceDirs"]) or "待确认",
        )
        for module in modules
    ]

    keyword_rows = [
        ("project", project_key, "§1 / §A"),
        ("module", ", ".join(module["name"] for module in modules[:8]) or project_root.name, "§B"),
        ("source", "src, app, lib", "§B / §D"),
        ("config", "application, docker, env", "§C"),
        ("api", "controller, route, endpoint", "§E"),
        ("test", "test, spec, junit, pytest", "§B"),
    ]

    return "\n".join([
        f"# {project_key} Project Assets",
        "",
        "> Auto-generated baseline asset. It is sufficient for G-00 structure checks; enrich domain details through project-assets-update when business context is known.",
        "",
        "## §1 Metadata",
        "",
        _markdown_table(("field", "value"), metadata_rows),
        "",
        "## §A Asset Outline",
        "",
        _markdown_table(("section", "purpose", "status"), [
            ("§A", "Asset outline and navigation", "generated"),
            ("§B", "Module and source layout index", "generated from filesystem"),
            ("§C", "Configuration and data field index", "baseline"),
            ("§D", "Component and code location index", "baseline"),
            ("§E", "API and integration index", "baseline"),
            ("§F", "Reverse keyword index", "baseline"),
            ("§G", "Read API contract", "generated"),
        ]),
        "",
        "## §B Module Index",
        "",
        _markdown_table(("module", "path", "build files", "source dirs"), module_rows),
        "",
        "## §C Field And Config Index",
        "",
        _render_list(config_files, "No common config files discovered yet. Add details during asset update."),
        "",
        "## §D Component Index",
        "",
        "- Baseline component discovery: use module paths in §B, then refine packages/classes during the first story or asset update.",
        "- Known source roots: " + (", ".join(
            f"`{module['path']}/{src}`" for module in modules for src in module["sourceDirs"]
        ) or "待确认"),
        "",
        "## §E API Index",
        "",
        "- API surface is not inferred from baseline generation.",
        "- Run targeted asset update when controllers, routes, RPC clients, or external contracts are needed.",
        "",
        "## §F Reverse Keyword Index",
        "",
        _markdown_table(("keyword", "meaning", "sections"), keyword_rows),
        "",
        "## §G Read API",
        "",
        _markdown_table(("intent", "command", "returns"), [
            ("outline", f"`ae-sdd assets outline --project {project_key}`", "§A + index stats"),
            ("search", f"`ae-sdd assets query <keyword> --project {project_key}`", "BM25 ranked snippets"),
            ("stage read", f"`ae-sdd assets read <stage> --project {project_key}`", "stage-specific baseline hits"),
            ("check", f"`ae-sdd assets check --project {project_key}`", "G-00 section readiness"),
        ]),
        "",
    ]) + "\n"


def _backup_existing(asset_file: Path) -> Optional[Path]:
    if not asset_file.is_file():
        return None
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")
    backup = asset_file.with_name(f"{asset_file.name}.bak-{stamp}")
    shutil.copy2(asset_file, backup)
    return backup


def generate_project_assets(
    ade_sdd: Path,
    project_key: str,
    *,
    project_root: Optional[Path] = None,
    force: bool = False,
    source_asset: Optional[Path] = None,
) -> AssetGenerationResult:
    """Create or repair a project asset file.

    Existing complete assets are kept unless ``force`` or ``source_asset`` is
    provided. Existing incomplete assets are backed up before repair.
    """
    asset_file = asset_path_for(ade_sdd, project_key)
    project_root = project_root or paths.project_root(ade_sdd)

    existing_text = ""
    if asset_file.is_file():
        existing_text = asset_file.read_text(encoding="utf-8")
    missing_before = missing_required_sections(existing_text) if asset_file.is_file() else list(REQUIRED_SECTIONS)

    if asset_file.is_file() and not missing_before and not force and source_asset is None:
        return AssetGenerationResult(
            asset_file=asset_file,
            project_key=project_key,
            created=False,
            changed=False,
            backup_file=None,
            missing_before=[],
            missing_after=[],
        )

    if source_asset is not None:
        text = source_asset.read_text(encoding="utf-8")
        source_label = str(source_asset)
    else:
        text = render_project_assets(project_root, project_key)
        source_label = "auto-baseline"

    if missing_required_sections(text):
        generated = render_project_assets(project_root, project_key, source=source_label)
        text = generated

    asset_file.parent.mkdir(parents=True, exist_ok=True)
    backup = _backup_existing(asset_file)
    created = not asset_file.exists()
    asset_file.write_text(text, encoding="utf-8")
    missing_after = missing_required_sections(text)

    return AssetGenerationResult(
        asset_file=asset_file,
        project_key=project_key,
        created=created,
        changed=True,
        backup_file=backup,
        missing_before=missing_before,
        missing_after=missing_after,
    )
