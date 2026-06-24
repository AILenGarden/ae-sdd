"""
Read-only Git insight helpers for ae-sdd.

This is not a replacement for git. It normalizes common read-only queries into
JSON so ae-sdd reports can cite the same evidence shape across phases.
"""
from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Optional


def _repo_root(project: Optional[str] = None) -> Path:
    cwd = Path(project).resolve() if project else Path.cwd()
    result = subprocess.run(
        ["git", "-C", str(cwd), "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "not a git repository")
    return Path(result.stdout.strip()).resolve()


def _run_git(args: list[str], project: Optional[str] = None) -> str:
    root = _repo_root(project)
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout


def status(project: Optional[str] = None) -> dict:
    root = _repo_root(project)
    porcelain = _run_git(["status", "--short"], str(root))
    branch = _run_git(["branch", "--show-current"], str(root)).strip()
    entries = []
    for line in porcelain.splitlines():
        if not line:
            continue
        entries.append({
            "status": line[:2],
            "path": line[3:] if len(line) > 3 else "",
            "raw": line,
        })
    return {"repo": str(root), "branch": branch, "dirty": bool(entries), "entries": entries}


def diff(project: Optional[str] = None, *, base: Optional[str] = None, head: Optional[str] = None, stat: bool = False) -> dict:
    root = _repo_root(project)
    args = ["diff"]
    if stat:
        args.append("--stat")
    if base and head:
        args.append(f"{base}..{head}")
    elif base:
        args.append(base)
    text = _run_git(args, str(root))
    return {"repo": str(root), "base": base, "head": head, "stat": stat, "diff": text}


def log(project: Optional[str] = None, *, path: Optional[str] = None, limit: int = 20) -> dict:
    root = _repo_root(project)
    args = ["log", f"--max-count={limit}", "--date=iso-strict", "--pretty=format:%H%x09%ad%x09%an%x09%s"]
    if path:
        args.extend(["--", path])
    text = _run_git(args, str(root))
    commits = []
    for line in text.splitlines():
        parts = line.split("\t", 3)
        if len(parts) == 4:
            commits.append({"hash": parts[0], "date": parts[1], "author": parts[2], "subject": parts[3]})
    return {"repo": str(root), "path": path, "limit": limit, "commits": commits}


def blame(project: Optional[str] = None, *, file: str, start: Optional[int] = None, end: Optional[int] = None) -> dict:
    root = _repo_root(project)
    args = ["blame", "--line-porcelain"]
    if start and end:
        args.extend([f"-L{start},{end}"])
    elif start:
        args.extend([f"-L{start},+1"])
    args.append(file)
    text = _run_git(args, str(root))
    commits = []
    current = None
    for line in text.splitlines():
        if not line:
            continue
        if len(line.split()) >= 3 and not line.startswith(("\t", "author ", "summary ", "filename ")):
            current = {"hash": line.split()[0]}
            commits.append(current)
        elif current is not None and line.startswith("author "):
            current["author"] = line[len("author "):]
        elif current is not None and line.startswith("summary "):
            current["summary"] = line[len("summary "):]
        elif current is not None and line.startswith("filename "):
            current["filename"] = line[len("filename "):]
    return {"repo": str(root), "file": file, "entries": commits}


def impact(project: Optional[str] = None, *, files: Optional[list[str]] = None, base: Optional[str] = None, head: Optional[str] = None) -> dict:
    root = _repo_root(project)
    changed = files or []
    if not changed:
        args = ["diff", "--name-only"]
        if base and head:
            args.append(f"{base}..{head}")
        elif base:
            args.append(base)
        changed = [line.strip() for line in _run_git(args, str(root)).splitlines() if line.strip()]
    modules = sorted({Path(f).parts[0] for f in changed if Path(f).parts})
    by_ext: dict[str, int] = {}
    for f in changed:
        ext = Path(f).suffix or "<none>"
        by_ext[ext] = by_ext.get(ext, 0) + 1
    return {
        "repo": str(root),
        "base": base,
        "head": head,
        "files": changed,
        "modules": modules,
        "by_extension": by_ext,
        "risk_hints": _risk_hints(changed),
    }


def _risk_hints(files: list[str]) -> list[str]:
    hints = []
    lowered = [f.lower() for f in files]
    if any("mapper" in f or f.endswith(".sql") for f in lowered):
        hints.append("database/sql path changed; require DB evidence or explain plan")
    if any("controller" in f or "api" in f for f in lowered):
        hints.append("API surface changed; require contract and compatibility review")
    if any("security" in f or "auth" in f for f in lowered):
        hints.append("security/auth path changed; require permission review")
    if any("test" in f for f in lowered):
        hints.append("test code changed; require test authenticity evidence")
    return hints
