"""Static contracts for delivery priority and stop-loss policy."""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAIN = (ROOT / "source" / "skill-fallbacks" / "SKILL.full.md").read_text(encoding="utf-8")
AGENT = (ROOT / "source" / "skill-fallbacks" / "skills" / "cross-cutting"
         / "agent-orchestration-skill.full.md").read_text(encoding="utf-8")
ENTRY = (ROOT / "source" / "SKILL.md").read_text(encoding="utf-8")
AGENT_ENTRY = (ROOT / "source" / "skills" / "cross-cutting"
               / "agent-orchestration-skill.md").read_text(encoding="utf-8")


def test_main_flow_policy_is_canonical_and_complete():
    required = (
        "Story-first",
        "15 minutes",
        "two repair/review rounds",
        "VerificationPlan/evidence",
        "file-hash change",
        "focused tests first",
        "within two minutes",
        "at most once",
        "two rounds",
        "explicit user rule",
        "absolute paths",
        "business code or POMs",
        "baseline",
    )
    assert "Delivery priority and stop-loss policy (v3.10.9)" in MAIN
    assert all(item in MAIN for item in required)


def test_agent_policy_has_delegation_limits_and_pointer():
    required = (
        "Story-first",
        "15 minutes",
        "two repair/review rounds",
        "VerificationPlan/evidence",
        "hash changes",
        "two minutes",
        "at most once",
        "two rounds",
        "P0/P1",
        "user rules",
        "business code/POMs",
        "baseline",
    )
    assert "Delivery priority and stop-loss policy (v3.10.9)" in AGENT
    assert all(item in AGENT for item in required)
    assert "skill-fallbacks/SKILL.full.md" in ENTRY
    assert "agent-orchestration-skill.full.md" in AGENT_ENTRY


def test_version_and_previous_pom_namespace_fix_are_preserved():
    version_line = next(line for line in ENTRY.splitlines()[:5] if line.startswith("version: "))
    version = version_line.removeprefix("version: ").strip()
    assert tuple(int(part) for part in version.split(".")) >= (3, 11, 1)
    assert f"v{version}" in (ROOT / "README.md").read_text(encoding="utf-8")
    paths = (ROOT / "tools" / "lib" / "paths.py").read_text(encoding="utf-8")
    assert f'MASTER_VERSION = "{version}"' in paths
    pom_changelog = (ROOT / "source" / "CHANGELOG"
                     / "2026-07-14-v3.10.8-gcode1-fail-closed-attestation.md")
    assert "Maven" in pom_changelog.read_text(encoding="utf-8")
