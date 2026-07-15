from __future__ import annotations

import json
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest

from lib.state_store import LeaseOwner, StateStore, StateStoreError


class FakeClock:
    def __init__(self) -> None:
        self.current = datetime(2026, 7, 14, tzinfo=timezone.utc)

    def __call__(self) -> datetime:
        return self.current

    def advance(self, seconds: int) -> None:
        self.current += timedelta(seconds=seconds)


@pytest.fixture
def clock() -> FakeClock:
    return FakeClock()


@pytest.fixture
def state_path(tmp_path: Path) -> Path:
    path = tmp_path / "work-item" / "state.json"
    path.parent.mkdir(parents=True)
    path.write_text(
        json.dumps(
            {
                "version": "2",
                "projectKey": "ae-sdd",
                "phase": "initialized",
                "history": [],
            }
        ),
        encoding="utf-8",
    )
    return path


def owner(name: str) -> LeaseOwner:
    return LeaseOwner(
        agent_id=name,
        session_id=f"session-{name}",
        host="test-host",
        pid=100 if name == "A" else 200,
    )


def make_store(state_path: Path, clock: FakeClock) -> StateStore:
    ids = iter(["lease-a", "lease-b", "lease-c"])
    return StateStore(state_path, clock=clock, uuid_factory=lambda: next(ids))


def assert_error(exc: pytest.ExceptionInfo[StateStoreError], code: str) -> None:
    assert exc.value.code == code


def test_read_legacy_state_exposes_revision_zero_without_rewriting(
    state_path: Path, clock: FakeClock
) -> None:
    before = state_path.read_bytes()

    snapshot = make_store(state_path, clock).read()

    assert snapshot.revision == 0
    assert snapshot.state["phase"] == "initialized"
    assert state_path.read_bytes() == before


def test_mutate_increments_revision_and_preserves_existing_state(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)
    lease = store.acquire_lease(owner("A"), ttl_seconds=300, idempotency_key="acquire-a")

    response = store.mutate(
        expected_revision=0,
        lease_id=lease.lease_id,
        fencing_token=lease.fencing_token,
        idempotency_key="mutation-1",
        operation="state.transition",
        payload={"targetPhase": "dr-generated"},
        mutate=lambda state: state.update({"phase": "dr-generated"}),
    )

    assert response.changed is True
    assert response.revision_before == 0
    assert response.revision_after == 1
    persisted = json.loads(state_path.read_text(encoding="utf-8"))
    assert persisted["version"] == "2"
    assert persisted["projectKey"] == "ae-sdd"
    assert persisted["phase"] == "dr-generated"
    assert persisted["revision"] == 1
    assert persisted["lastFencingToken"] == lease.fencing_token
    assert persisted["lastMutation"]["idempotencyKey"] == "mutation-1"


def test_revision_conflict_rejects_lost_update_without_touching_state(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)
    lease = store.acquire_lease(owner("A"), ttl_seconds=300, idempotency_key="acquire-a")
    store.mutate(
        expected_revision=0,
        lease_id=lease.lease_id,
        fencing_token=lease.fencing_token,
        idempotency_key="mutation-1",
        operation="state.transition",
        payload={"targetPhase": "dr-generated"},
        mutate=lambda state: state.update({"phase": "dr-generated"}),
    )
    before = state_path.read_bytes()

    with pytest.raises(StateStoreError) as exc:
        store.mutate(
            expected_revision=0,
            lease_id=lease.lease_id,
            fencing_token=lease.fencing_token,
            idempotency_key="mutation-2",
            operation="state.transition",
            payload={"targetPhase": "story-generated"},
            mutate=lambda state: state.update({"phase": "story-generated"}),
        )

    assert_error(exc, "REVISION_CONFLICT")
    assert state_path.read_bytes() == before


def test_active_lease_conflicts_with_another_owner(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)
    lease = store.acquire_lease(owner("A"), ttl_seconds=300, idempotency_key="acquire-a")

    with pytest.raises(StateStoreError) as exc:
        store.acquire_lease(owner("B"), ttl_seconds=300, idempotency_key="acquire-b")

    assert_error(exc, "LEASE_CONFLICT")
    assert exc.value.details["holder"]["agentId"] == "A"
    assert store.lease_status()["leaseId"] == lease.lease_id


def test_acquire_retry_with_same_idempotency_key_returns_same_lease(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)

    first = store.acquire_lease(owner("A"), 300, "acquire-a")
    retried = store.acquire_lease(owner("A"), 300, "acquire-a")

    assert retried == first
    assert retried.fencing_token == 1


def test_renew_extends_expiry_without_changing_identity(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)
    lease = store.acquire_lease(owner("A"), 300, "acquire-a")
    clock.advance(290)

    renewed = store.renew_lease(
        owner("A"), lease.lease_id, lease.fencing_token, ttl_seconds=300
    )

    assert renewed.lease_id == lease.lease_id
    assert renewed.fencing_token == lease.fencing_token
    assert renewed.heartbeat_at > lease.heartbeat_at
    assert renewed.expires_at > lease.expires_at


def test_expired_lease_can_be_taken_over_and_stale_writer_is_fenced(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)
    old = store.acquire_lease(owner("A"), 30, "acquire-a")
    clock.advance(30)
    current = store.acquire_lease(owner("B"), 300, "acquire-b")
    before = state_path.read_bytes()

    assert current.fencing_token > old.fencing_token
    with pytest.raises(StateStoreError) as exc:
        store.mutate(
            expected_revision=0,
            lease_id=old.lease_id,
            fencing_token=old.fencing_token,
            idempotency_key="stale-write",
            operation="state.transition",
            payload={"targetPhase": "dr-generated"},
            mutate=lambda state: state.update({"phase": "dr-generated"}),
        )

    assert_error(exc, "STALE_FENCING_TOKEN")
    assert state_path.read_bytes() == before


def test_non_owner_cannot_release_but_owner_can_and_token_does_not_reset(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)
    lease = store.acquire_lease(owner("A"), 300, "acquire-a")

    with pytest.raises(StateStoreError) as exc:
        store.release_lease(owner("B"), lease.lease_id, lease.fencing_token)
    assert_error(exc, "LEASE_NOT_OWNED")

    released = store.release_lease(owner("A"), lease.lease_id, lease.fencing_token)
    assert released["status"] == "absent"
    replacement = store.acquire_lease(owner("B"), 300, "acquire-b")
    assert replacement.fencing_token == lease.fencing_token + 1


def test_break_requires_reason_and_records_audit(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)
    store.acquire_lease(owner("A"), 300, "acquire-a")

    with pytest.raises(StateStoreError) as exc:
        store.break_lease(owner("B"), "")
    assert_error(exc, "BREAK_REASON_REQUIRED")

    result = store.break_lease(owner("B"), "owner process terminated")
    assert result["status"] == "absent"
    persisted = json.loads((state_path.parent / "state.lease.json").read_text(encoding="utf-8"))
    assert persisted["history"][-1]["event"] == "broken"
    assert persisted["history"][-1]["reason"] == "owner process terminated"
    assert persisted["history"][-1]["actor"]["agentId"] == "B"


@pytest.mark.parametrize("ttl", [29, 3601])
def test_ttl_outside_supported_range_is_rejected(
    state_path: Path, clock: FakeClock, ttl: int
) -> None:
    store = make_store(state_path, clock)

    with pytest.raises(StateStoreError) as exc:
        store.acquire_lease(owner("A"), ttl, "acquire-a")

    assert_error(exc, "INVALID_LEASE_TTL")


@pytest.mark.parametrize("ttl", [30, 3600])
def test_ttl_boundaries_are_valid(state_path: Path, clock: FakeClock, ttl: int) -> None:
    store = make_store(state_path, clock)
    lease = store.acquire_lease(owner("A"), ttl, "acquire-a")
    assert lease.ttl_seconds == ttl


def test_corrupt_state_fails_closed_without_overwrite(
    state_path: Path, clock: FakeClock
) -> None:
    state_path.write_text("{broken", encoding="utf-8")
    before = state_path.read_bytes()

    with pytest.raises(StateStoreError) as exc:
        make_store(state_path, clock).read()

    assert_error(exc, "STATE_INVALID_JSON")
    assert str(state_path.resolve()) in exc.value.details["path"]
    assert state_path.read_bytes() == before


def test_corrupt_lease_fails_closed_without_overwrite(
    state_path: Path, clock: FakeClock
) -> None:
    lease_path = state_path.parent / "state.lease.json"
    lease_path.write_text("{broken", encoding="utf-8")
    before = lease_path.read_bytes()

    with pytest.raises(StateStoreError) as exc:
        make_store(state_path, clock).acquire_lease(owner("A"), 300, "acquire-a")

    assert_error(exc, "LEASE_INVALID_JSON")
    assert lease_path.read_bytes() == before


def test_allowed_root_rejects_symlink_escape_before_sidecar_write(tmp_path: Path) -> None:
    allowed_root = tmp_path / ".auto-engineering"
    outside = tmp_path / "outside"
    outside.mkdir()
    link = allowed_root / "Story-006"
    allowed_root.mkdir()
    try:
        link.symlink_to(outside, target_is_directory=True)
    except OSError as exc:
        pytest.skip(f"directory symlink unavailable: {exc}")

    with pytest.raises(StateStoreError) as caught:
        StateStore(link / "state.json", allowed_root=allowed_root)

    assert_error(caught, "STATE_PATH_OUTSIDE_ALLOWED_ROOT")
    assert list(outside.iterdir()) == []


def test_allowed_root_rejects_direct_outside_path_before_sidecar_write(tmp_path: Path) -> None:
    allowed_root = tmp_path / ".auto-engineering"
    outside_state = tmp_path / "outside" / "state.json"

    with pytest.raises(StateStoreError) as caught:
        StateStore(outside_state, allowed_root=allowed_root)

    assert_error(caught, "STATE_PATH_OUTSIDE_ALLOWED_ROOT")
    assert not outside_state.parent.exists()


def test_create_is_exclusive_and_never_overwrites_existing_state(tmp_path: Path) -> None:
    path = tmp_path / ".auto-engineering" / "Story-006" / "state.json"
    store = StateStore(path, allowed_root=tmp_path / ".auto-engineering")
    initial = {"version": "2", "phase": "initialized", "history": []}

    created = store.create(initial)
    before = path.read_bytes()
    with pytest.raises(StateStoreError) as caught:
        store.create({"version": "2", "phase": "coding", "history": []})

    assert created.revision == 0
    assert_error(caught, "STATE_ALREADY_EXISTS")
    assert path.read_bytes() == before


def test_mutation_callback_failure_leaves_story_add_and_binding_unwritten(
    state_path: Path, clock: FakeClock
) -> None:
    store = make_store(state_path, clock)
    lease = store.acquire_lease(owner("A"), 300, "acquire-a")
    before = state_path.read_bytes()

    def fail_after_partial_update(value: dict) -> None:
        value.setdefault("storyStates", {})["STORY-006-BE"] = {"phase": "initialized"}
        raise ValueError("binding failed")

    with pytest.raises(ValueError, match="binding failed"):
        store.mutate(
            expected_revision=0,
            lease_id=lease.lease_id,
            fencing_token=lease.fencing_token,
            idempotency_key="story-add-bind",
            operation="state.new-story-binding",
            payload={"storyId": "STORY-006-BE"},
            mutate=fail_after_partial_update,
        )

    assert state_path.read_bytes() == before
