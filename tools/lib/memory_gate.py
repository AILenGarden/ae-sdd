"""
Mandatory memory checks for ae-sdd phase transitions.

This module is shared by the CLI and the PreToolUse gate so phase changes do
not rely on the Agent remembering to call memory exit manually.
"""
from __future__ import annotations

from pathlib import Path
from typing import Optional

from lib import memory_store, paths, state as state_mod


STATE_PHASE_TO_MEMORY_PHASE: dict[str, str] = {
    "ra-generated": "ra",          # 🆕 v3.4.0 修复 B3-6：ra 阶段 memory 覆盖
    "dr-generated": "design",
    "story-generated": "design",
    "story-reviewed": "design",
    "task-generated": "coding-plan",
    "task-reviewed": "coding-plan",
    "coding": "coding",
    "test-running": "coding",
    "code-reviewed": "review",
}


def memory_phase_for_state_phase(phase: str) -> Optional[str]:
    return STATE_PHASE_TO_MEMORY_PHASE.get(phase)


def is_forward_transition(current_phase: str, target_phase: str) -> bool:
    try:
        current_idx = state_mod.PHASE_FLOW.index(current_phase)
        target_idx = state_mod.PHASE_FLOW.index(target_phase)
    except ValueError:
        return False
    return target_idx > current_idx


def check_state_transition(
    *,
    ade_sdd: Optional[Path],
    state_data: dict,
    target_phase: str,
    allow_empty: bool = False,
) -> dict:
    """Return whether a phase transition satisfies mandatory memory rules."""
    current_phase = state_data.get("phase", "initialized")
    if ade_sdd is None:
        return {
            "pass": True,
            "blocked": False,
            "skipped": True,
            "reason": "no .ae-sdd project context",
        }
    if not is_forward_transition(current_phase, target_phase):
        return {
            "pass": True,
            "blocked": False,
            "skipped": True,
            "reason": "not a forward transition",
        }

    memory_phase = memory_phase_for_state_phase(current_phase)
    if not memory_phase:
        return {
            "pass": True,
            "blocked": False,
            "skipped": True,
            "reason": f"phase {current_phase} has no mandatory memory scope",
        }

    project_root = paths.project_root(ade_sdd)
    story = state_data.get("currentStory") or None
    scope = memory_store.locate_scope(
        project=str(project_root),
        phase=memory_phase,
        story=story,
        task=None,
    )
    check = memory_store.check_exit_ready(scope, allow_empty=allow_empty)
    check.update({
        "current_phase": current_phase,
        "target_phase": target_phase,
        "memory_phase": memory_phase,
        "story": story,
        "task": None,
        "scope_key": scope.scope_key,
        "project_root": str(project_root),
    })
    return check


def format_transition_block(check: dict) -> str:
    story = check.get("story")
    story_label = story or "<project>"
    story_arg = f" --story {story}" if story else ""
    return (
        "Mandatory memory gate failed before phase transition.\n"
        f"transition: {check.get('current_phase')} -> {check.get('target_phase')}\n"
        f"memory phase: {check.get('memory_phase')}, story: {story_label}, scope: {check.get('scope_key')}\n"
        f"reason: {check.get('reason')}\n"
        "Required sequence:\n"
        f"  ae-sdd memory enter --phase {check.get('memory_phase')}{story_arg}\n"
        f"  ae-sdd memory write --phase {check.get('memory_phase')}{story_arg} --summary \"...\" --evidence <file:line>\n"
        f"  ae-sdd memory exit --phase {check.get('memory_phase')}{story_arg}\n"
    )
