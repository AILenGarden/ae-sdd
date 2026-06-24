"""
Database toolset skeleton for ae-sdd.

The DB tool deliberately starts conservative:
- connection profiles are local-only under .ae-sdd/secrets/
- raw passwords are never expected in repo-tracked files
- query execution defaults to read-only
- only sqlite is executable without extra drivers; other engines are reported
  as configured but unsupported by this dependency-free skeleton.
"""
from __future__ import annotations

import json
import re
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from lib import paths


READONLY_SQL = re.compile(r"^\s*(select|with|pragma|explain)\b", re.IGNORECASE | re.DOTALL)
WRITE_SQL = re.compile(
    r"\b(insert|update|delete|merge|drop|alter|create|truncate|replace|grant|revoke)\b",
    re.IGNORECASE,
)


@dataclass
class DbProfile:
    name: str
    driver: str
    database: Optional[str] = None
    host: Optional[str] = None
    port: Optional[str] = None
    schema: Optional[str] = None
    source: Optional[Path] = None

    def as_safe_dict(self) -> dict:
        return {
            "name": self.name,
            "driver": self.driver,
            "database": self.database,
            "host": self.host,
            "port": self.port,
            "schema": self.schema,
            "source": str(self.source) if self.source else None,
            "secrets": "redacted",
        }


def secrets_dir(project: Optional[str] = None) -> Path:
    project_dir = Path(project).resolve() if project else Path.cwd()
    ade_sdd = paths.locate_project_ae_sdd(project_dir)
    if ade_sdd is None:
        ade_sdd = project_dir / ".ae-sdd"
    return ade_sdd / "secrets"


def profile_path(project: Optional[str] = None) -> Path:
    return secrets_dir(project) / "db-connections.local.json"


def template() -> dict:
    return {
        "profiles": [
            {
                "name": "local-sqlite",
                "driver": "sqlite",
                "database": "D:/path/to/local.db",
                "readonly": True,
                "note": "Local-only file. Do not commit .ae-sdd/secrets/.",
            }
        ]
    }


def ensure_template(project: Optional[str] = None) -> dict:
    p = profile_path(project)
    if p.exists():
        return {"created": False, "path": str(p)}
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(template(), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return {"created": True, "path": str(p)}


def _load_raw(project: Optional[str] = None) -> dict:
    p = profile_path(project)
    if not p.is_file():
        return {"profiles": []}
    return json.loads(p.read_text(encoding="utf-8"))


def list_profiles(project: Optional[str] = None) -> list[dict]:
    raw = _load_raw(project)
    out = []
    for item in raw.get("profiles", []):
        profile = DbProfile(
            name=str(item.get("name", "")),
            driver=str(item.get("driver", "")).lower(),
            database=item.get("database"),
            host=item.get("host"),
            port=str(item.get("port")) if item.get("port") is not None else None,
            schema=item.get("schema"),
            source=profile_path(project),
        )
        out.append(profile.as_safe_dict())
    return out


def get_profile(name: str, project: Optional[str] = None) -> DbProfile:
    raw = _load_raw(project)
    for item in raw.get("profiles", []):
        if item.get("name") == name:
            return DbProfile(
                name=str(item.get("name", "")),
                driver=str(item.get("driver", "")).lower(),
                database=item.get("database"),
                host=item.get("host"),
                port=str(item.get("port")) if item.get("port") is not None else None,
                schema=item.get("schema"),
                source=profile_path(project),
            )
    raise ValueError(f"unknown db profile: {name}")


def read_sql(sql: Optional[str] = None, sql_file: Optional[str] = None) -> str:
    if sql_file:
        return Path(sql_file).read_text(encoding="utf-8")
    if sql:
        return sql
    raise ValueError("SQL is required (--sql or --sql-file)")


def classify_sql(sql: str) -> dict:
    readonly = bool(READONLY_SQL.search(sql)) and not bool(WRITE_SQL.search(sql))
    has_write = bool(WRITE_SQL.search(sql))
    return {"readonly": readonly, "has_write": has_write}


def query(
    *,
    profile_name: str,
    sql: str,
    project: Optional[str] = None,
    write: bool = False,
    limit: int = 100,
) -> dict:
    profile = get_profile(profile_name, project)
    sql_class = classify_sql(sql)
    if sql_class["has_write"] and not write:
        return {
            "ok": False,
            "blocked": True,
            "reason": "write SQL requires explicit --write",
            "sql_class": sql_class,
            "profile": profile.as_safe_dict(),
        }
    if profile.driver != "sqlite":
        return {
            "ok": False,
            "blocked": True,
            "reason": f"driver '{profile.driver}' is configured but not executable by the dependency-free skeleton",
            "profile": profile.as_safe_dict(),
        }
    if not profile.database:
        raise ValueError(f"profile {profile.name} has no database path")

    db_path = Path(profile.database).expanduser()
    if not db_path.is_file():
        return {
            "ok": False,
            "blocked": True,
            "reason": f"sqlite database does not exist: {db_path}",
            "profile": profile.as_safe_dict(),
        }

    uri = f"file:{db_path.as_posix()}?mode={'rw' if write else 'ro'}"
    rows: list[dict] = []
    with sqlite3.connect(uri, uri=True) as conn:
        conn.row_factory = sqlite3.Row
        cur = conn.execute(sql)
        if cur.description:
            fetched = cur.fetchmany(max(limit, 1))
            rows = [dict(r) for r in fetched]
        if write:
            conn.commit()
    return {
        "ok": True,
        "blocked": False,
        "profile": profile.as_safe_dict(),
        "sql_class": sql_class,
        "row_count": len(rows),
        "limit": limit,
        "rows": rows,
    }


def explain(*, profile_name: str, sql: str, project: Optional[str] = None, limit: int = 100) -> dict:
    profile = get_profile(profile_name, project)
    if profile.driver != "sqlite":
        return {
            "ok": False,
            "blocked": True,
            "reason": f"EXPLAIN is not implemented for driver '{profile.driver}' in this skeleton",
            "profile": profile.as_safe_dict(),
        }
    explain_sql = sql if sql.lstrip().lower().startswith("explain") else f"EXPLAIN QUERY PLAN {sql}"
    return query(profile_name=profile_name, sql=explain_sql, project=project, write=False, limit=limit)


def audit(project: Optional[str] = None) -> dict:
    p = profile_path(project)
    exists = p.is_file()
    profiles = list_profiles(project) if exists else []
    return {
        "profile_path": str(p),
        "exists": exists,
        "profiles": profiles,
        "policy": {
            "repo_safe": ".ae-sdd/secrets/ must stay local and ignored",
            "default_mode": "read-only",
            "write_policy": "requires --write and should be paired with transaction/dry-run evidence",
        },
    }
