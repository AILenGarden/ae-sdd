from __future__ import annotations

import sys
from pathlib import Path


TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib.http_scenario import evaluate_observation  # noqa: E402


def test_seeded_defect_mapping_omission_is_reported_at_field_level():
    findings = evaluate_observation(
        {"id": "o1", "tenantId": "t1", "amount": 10},
        {"id": "o1", "tenantId": "t1", "amount": 10},
        {"changedDimensions": {"amount": 20}, "invariants": ["id", "tenantId"]},
    )
    assert findings == [{"code": "changed-dimension-mismatch", "field": "amount",
                         "expected": 20, "actual": 10}]


def test_seeded_defect_invariant_corruption_is_reported():
    findings = evaluate_observation(
        {"id": "o1", "tenantId": "t1"},
        {"id": "o1", "tenantId": "t2"},
        {"changedDimensions": {}, "invariants": ["tenantId"]},
    )
    assert findings[0]["code"] == "invariant-violated"
    assert findings[0]["field"] == "tenantId"


def test_seeded_defect_illegal_partial_state_is_reported():
    findings = evaluate_observation(
        {"status": "NEW"},
        {"status": "PARTIAL", "amount": 100},
        {"changedDimensions": {}, "invariants": [],
         "forbiddenStates": [{"status": "PARTIAL"}]},
    )
    assert findings == [{"code": "forbidden-state-reached", "state": {"status": "PARTIAL"}}]


def test_valid_transition_has_no_findings():
    findings = evaluate_observation(
        {"id": "o1", "tenantId": "t1", "status": "NEW"},
        {"id": "o1", "tenantId": "t1", "status": "PAID"},
        {"changedDimensions": {"status": "PAID"}, "invariants": ["id", "tenantId"],
         "forbiddenStates": [{"status": "PARTIAL"}]},
    )
    assert findings == []
