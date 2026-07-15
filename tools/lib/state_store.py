"""Concurrency-safe persistence for one AE-SDD Work Item state.

The OS lock is deliberately short-lived: it protects one read/validate/write
transaction. ``state.lease.json`` represents the longer-lived writer lease and
retains a tombstone so fencing tokens never reset after release or break.
"""

from __future__ import annotations

import copy
import hashlib
import json
import os
import socket
import tempfile
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Callable, Optional

from lib.state import read_state as read_legacy_state
from lib.state import validate_state_invariants


MIN_LEASE_TTL_SECONDS = 30
MAX_LEASE_TTL_SECONDS = 3600
DEFAULT_LEASE_TTL_SECONDS = 300
DEFAULT_LOCK_TIMEOUT_SECONDS = 10.0


class StateStoreError(RuntimeError):
    """Stable, JSON-friendly StateStore failure."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        details: Optional[dict[str, Any]] = None,
        remediation: str = "",
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.details = details or {}
        self.remediation = remediation

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "message": self.message,
            "remediation": self.remediation,
            "details": self.details,
        }


@dataclass(frozen=True)
class LeaseOwner:
    agent_id: str
    session_id: str
    host: str
    pid: int

    @classmethod
    def current(cls, agent_id: str, session_id: str) -> "LeaseOwner":
        return cls(agent_id, session_id, socket.gethostname(), os.getpid())

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "LeaseOwner":
        try:
            return cls(
                agent_id=str(value["agentId"]),
                session_id=str(value["sessionId"]),
                host=str(value["host"]),
                pid=int(value["pid"]),
            )
        except (KeyError, TypeError, ValueError) as exc:
            raise StateStoreError(
                "LEASE_INVALID_SCHEMA",
                "lease owner schema is invalid",
                details={"owner": value},
            ) from exc

    def validate(self) -> None:
        if not self.agent_id or not self.session_id or not self.host or self.pid < 0:
            raise StateStoreError(
                "INVALID_LEASE_OWNER",
                "lease owner requires agentId, sessionId, host and a non-negative pid",
                details={"owner": self.to_dict()},
            )

    def to_dict(self) -> dict[str, Any]:
        return {
            "agentId": self.agent_id,
            "sessionId": self.session_id,
            "host": self.host,
            "pid": self.pid,
        }


@dataclass(frozen=True)
class LeaseRecord:
    lease_id: str
    owner: LeaseOwner
    fencing_token: int
    acquired_at: datetime
    heartbeat_at: datetime
    expires_at: datetime
    ttl_seconds: int
    acquire_idempotency_key: str

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "LeaseRecord":
        try:
            return cls(
                lease_id=str(value["leaseId"]),
                owner=LeaseOwner.from_dict(value["owner"]),
                fencing_token=int(value["fencingToken"]),
                acquired_at=_parse_timestamp(value["acquiredAt"]),
                heartbeat_at=_parse_timestamp(value["heartbeatAt"]),
                expires_at=_parse_timestamp(value["expiresAt"]),
                ttl_seconds=int(value["ttlSeconds"]),
                acquire_idempotency_key=str(value["acquireIdempotencyKey"]),
            )
        except (KeyError, TypeError, ValueError, StateStoreError) as exc:
            if isinstance(exc, StateStoreError):
                raise
            raise StateStoreError(
                "LEASE_INVALID_SCHEMA",
                "lease record schema is invalid",
                details={"lease": value},
            ) from exc

    def is_expired(self, now: datetime) -> bool:
        return now >= self.expires_at

    def to_dict(self) -> dict[str, Any]:
        return {
            "leaseId": self.lease_id,
            "owner": self.owner.to_dict(),
            "fencingToken": self.fencing_token,
            "acquiredAt": _format_timestamp(self.acquired_at),
            "heartbeatAt": _format_timestamp(self.heartbeat_at),
            "expiresAt": _format_timestamp(self.expires_at),
            "ttlSeconds": self.ttl_seconds,
            "acquireIdempotencyKey": self.acquire_idempotency_key,
        }


@dataclass(frozen=True)
class StateSnapshot:
    state: dict[str, Any]
    revision: int


@dataclass(frozen=True)
class MutationResponse:
    state: dict[str, Any]
    revision_before: int
    revision_after: int
    changed: bool
    result: Any = None
    replayed: bool = False


class CrossPlatformFileLock:
    """Exclusive non-blocking file lock with a bounded polling timeout."""

    def __init__(self, path: Path, timeout_seconds: float) -> None:
        self.path = path
        self.timeout_seconds = timeout_seconds
        self._handle: Any = None

    def __enter__(self) -> "CrossPlatformFileLock":
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._handle = self.path.open("a+b")
        self._ensure_lock_byte()
        deadline = time.monotonic() + self.timeout_seconds
        while True:
            try:
                self._try_acquire()
                return self
            except (BlockingIOError, OSError) as exc:
                if time.monotonic() >= deadline:
                    self._handle.close()
                    self._handle = None
                    raise StateStoreError(
                        "STATE_LOCK_TIMEOUT",
                        "timed out acquiring the Work Item state transaction lock",
                        details={"path": str(self.path.resolve())},
                        remediation="retry the operation after the active transaction completes",
                    ) from exc
                time.sleep(0.01)

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        if self._handle is None:
            return
        try:
            self._release()
        finally:
            self._handle.close()
            self._handle = None

    def _ensure_lock_byte(self) -> None:
        self._handle.seek(0, os.SEEK_END)
        if self._handle.tell() == 0:
            self._handle.write(b"\0")
            self._handle.flush()
        self._handle.seek(0)

    def _try_acquire(self) -> None:
        self._handle.seek(0)
        if os.name == "nt":
            import msvcrt

            msvcrt.locking(self._handle.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl

            fcntl.flock(self._handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)

    def _release(self) -> None:
        self._handle.seek(0)
        if os.name == "nt":
            import msvcrt

            msvcrt.locking(self._handle.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl

            fcntl.flock(self._handle.fileno(), fcntl.LOCK_UN)


def _infer_allowed_root(state_path: Path) -> Optional[Path]:
    """Infer the lexical .auto-engineering boundary for legacy callers."""
    for parent in Path(state_path).parents:
        if parent.name == ".auto-engineering":
            return parent
    return None


class StateStore:
    """Canonical transaction and lease boundary for one state.json file."""

    def __init__(
        self,
        state_path: Path,
        *,
        allowed_root: Optional[Path] = None,
        clock: Optional[Callable[[], datetime]] = None,
        uuid_factory: Optional[Callable[[], str]] = None,
        lock_timeout_seconds: float = DEFAULT_LOCK_TIMEOUT_SECONDS,
    ) -> None:
        self.state_path = Path(state_path)
        self.work_item_dir = self.state_path.parent
        self.lease_path = self.work_item_dir / "state.lease.json"
        self.lock_path = self.work_item_dir / ".state.lock"
        self.allowed_root = Path(allowed_root) if allowed_root is not None else _infer_allowed_root(self.state_path)
        self._clock = clock or (lambda: datetime.now(timezone.utc))
        self._uuid_factory = uuid_factory or (lambda: str(uuid.uuid4()))
        self._lock_timeout_seconds = lock_timeout_seconds
        self._assert_allowed_path(self.state_path)

    def create(self, initial_state: dict[str, Any]) -> StateSnapshot:
        """Create a new state exactly once under the transaction lock."""
        self._assert_allowed_path(self.state_path)
        with self._transaction_lock():
            if self.state_path.exists():
                raise StateStoreError(
                    "STATE_ALREADY_EXISTS",
                    "state.json already exists and exclusive create will not overwrite it",
                    details={"path": str(self.state_path.resolve(strict=False))},
                )
            created = copy.deepcopy(initial_state)
            validate_state_invariants(created)
            revision = _state_revision(created)
            self._atomic_write_json(self.state_path, created)
            return StateSnapshot(copy.deepcopy(created), revision)

    def read(self) -> StateSnapshot:
        state = self._read_state()
        return StateSnapshot(copy.deepcopy(state), _state_revision(state))

    def lease_status(self) -> dict[str, Any]:
        document = self._read_lease_document()
        if document is None:
            return {"status": "absent", "fencingToken": 0}
        token = _lease_document_token(document)
        if document.get("status") != "active":
            return {"status": "absent", "fencingToken": token}
        record = LeaseRecord.from_dict(document)
        status = "expired" if record.is_expired(self._now()) else "active"
        return {"status": status, **record.to_dict()}

    def acquire_lease(
        self,
        owner: LeaseOwner,
        ttl_seconds: int = DEFAULT_LEASE_TTL_SECONDS,
        idempotency_key: str = "",
    ) -> LeaseRecord:
        owner.validate()
        _validate_ttl(ttl_seconds)
        _require_idempotency_key(idempotency_key)
        with self._transaction_lock():
            now = self._now()
            document = self._read_lease_document()
            history = list((document or {}).get("history") or [])
            previous_token = _lease_document_token(document)
            if document and document.get("status") == "active":
                current = LeaseRecord.from_dict(document)
                if not current.is_expired(now):
                    if (
                        current.owner == owner
                        and current.acquire_idempotency_key == idempotency_key
                    ):
                        return current
                    raise StateStoreError(
                        "LEASE_CONFLICT",
                        "the Work Item already has an active writer lease",
                        details={
                            "holder": current.owner.to_dict(),
                            "leaseId": current.lease_id,
                            "fencingToken": current.fencing_token,
                            "expiresAt": _format_timestamp(current.expires_at),
                            "retryAfterSeconds": max(
                                0, int((current.expires_at - now).total_seconds())
                            ),
                        },
                        remediation="wait for expiry or ask the holder to release the lease",
                    )
                history.append(
                    {
                        "event": "expired",
                        "leaseId": current.lease_id,
                        "owner": current.owner.to_dict(),
                        "fencingToken": current.fencing_token,
                        "at": _format_timestamp(now),
                    }
                )
            record = LeaseRecord(
                lease_id=str(self._uuid_factory()),
                owner=owner,
                fencing_token=previous_token + 1,
                acquired_at=now,
                heartbeat_at=now,
                expires_at=now + timedelta(seconds=ttl_seconds),
                ttl_seconds=ttl_seconds,
                acquire_idempotency_key=idempotency_key,
            )
            history.append(
                {
                    "event": "acquired",
                    "leaseId": record.lease_id,
                    "owner": owner.to_dict(),
                    "fencingToken": record.fencing_token,
                    "at": _format_timestamp(now),
                }
            )
            self._write_lease_record(record, history)
            return record

    def renew_lease(
        self,
        owner: LeaseOwner,
        lease_id: str,
        fencing_token: int,
        ttl_seconds: int = DEFAULT_LEASE_TTL_SECONDS,
    ) -> LeaseRecord:
        owner.validate()
        _validate_ttl(ttl_seconds)
        with self._transaction_lock():
            now = self._now()
            document = self._require_active_lease_document()
            current = LeaseRecord.from_dict(document)
            self._assert_lease(current, lease_id, fencing_token, now)
            if current.owner != owner:
                raise StateStoreError(
                    "LEASE_NOT_OWNED",
                    "only the current lease owner can renew it",
                    details={"holder": current.owner.to_dict()},
                )
            renewed = LeaseRecord(
                lease_id=current.lease_id,
                owner=current.owner,
                fencing_token=current.fencing_token,
                acquired_at=current.acquired_at,
                heartbeat_at=now,
                expires_at=now + timedelta(seconds=ttl_seconds),
                ttl_seconds=ttl_seconds,
                acquire_idempotency_key=current.acquire_idempotency_key,
            )
            history = list(document.get("history") or [])
            history.append(
                {
                    "event": "renewed",
                    "leaseId": renewed.lease_id,
                    "owner": owner.to_dict(),
                    "fencingToken": renewed.fencing_token,
                    "at": _format_timestamp(now),
                    "expiresAt": _format_timestamp(renewed.expires_at),
                }
            )
            self._write_lease_record(renewed, history)
            return renewed

    def release_lease(
        self, owner: LeaseOwner, lease_id: str, fencing_token: int
    ) -> dict[str, Any]:
        owner.validate()
        with self._transaction_lock():
            now = self._now()
            document = self._require_active_lease_document()
            current = LeaseRecord.from_dict(document)
            self._assert_lease(current, lease_id, fencing_token, now)
            if current.owner != owner:
                raise StateStoreError(
                    "LEASE_NOT_OWNED",
                    "only the current lease owner can release it",
                    details={"holder": current.owner.to_dict()},
                )
            history = list(document.get("history") or [])
            history.append(
                {
                    "event": "released",
                    "leaseId": current.lease_id,
                    "owner": owner.to_dict(),
                    "fencingToken": current.fencing_token,
                    "at": _format_timestamp(now),
                }
            )
            tombstone = dict(document)
            tombstone.update(
                {
                    "status": "released",
                    "releasedAt": _format_timestamp(now),
                    "history": history,
                }
            )
            self._atomic_write_json(self.lease_path, tombstone)
            return {"status": "absent", "fencingToken": current.fencing_token}

    def break_lease(self, actor: LeaseOwner, reason: str) -> dict[str, Any]:
        actor.validate()
        if not reason.strip():
            raise StateStoreError(
                "BREAK_REASON_REQUIRED", "breaking a lease requires an audit reason"
            )
        with self._transaction_lock():
            now = self._now()
            document = self._read_lease_document()
            if document is None or document.get("status") != "active":
                return {
                    "status": "absent",
                    "fencingToken": _lease_document_token(document),
                }
            current = LeaseRecord.from_dict(document)
            history = list(document.get("history") or [])
            history.append(
                {
                    "event": "broken",
                    "leaseId": current.lease_id,
                    "owner": current.owner.to_dict(),
                    "actor": actor.to_dict(),
                    "reason": reason.strip(),
                    "fencingToken": current.fencing_token,
                    "at": _format_timestamp(now),
                }
            )
            tombstone = dict(document)
            tombstone.update(
                {
                    "status": "broken",
                    "brokenAt": _format_timestamp(now),
                    "brokenBy": actor.to_dict(),
                    "breakReason": reason.strip(),
                    "history": history,
                }
            )
            self._atomic_write_json(self.lease_path, tombstone)
            return {"status": "absent", "fencingToken": current.fencing_token}

    def mutate(
        self,
        *,
        expected_revision: int,
        lease_id: str,
        fencing_token: int,
        idempotency_key: str,
        operation: str,
        payload: dict[str, Any],
        mutate: Callable[[dict[str, Any]], Any],
    ) -> MutationResponse:
        _require_idempotency_key(idempotency_key)
        if not operation:
            raise StateStoreError("OPERATION_REQUIRED", "operation name is required")
        payload_hash = _canonical_hash({"operation": operation, "payload": payload})
        with self._transaction_lock():
            now = self._now()
            state = self._read_state()
            current_revision = _state_revision(state)
            document = self._require_active_lease_document()
            current_lease = LeaseRecord.from_dict(document)
            self._assert_lease(
                current_lease, lease_id, fencing_token, now, allow_stale_detail=True
            )
            previous = state.get("lastMutation")
            if isinstance(previous, dict) and previous.get("idempotencyKey") == idempotency_key:
                if previous.get("payloadHash") != payload_hash:
                    raise StateStoreError(
                        "IDEMPOTENCY_KEY_REUSED",
                        "the idempotency key was already used with a different payload",
                        details={"idempotencyKey": idempotency_key},
                    )
                return MutationResponse(
                    state=copy.deepcopy(state),
                    revision_before=int(previous.get("revisionBefore", current_revision)),
                    revision_after=current_revision,
                    changed=False,
                    result=copy.deepcopy(previous.get("result")),
                    replayed=True,
                )
            if expected_revision != current_revision:
                raise StateStoreError(
                    "REVISION_CONFLICT",
                    "expected revision does not match the current Work Item revision",
                    details={
                        "expectedRevision": expected_revision,
                        "currentRevision": current_revision,
                    },
                    remediation="read the Work Item again and recompute the mutation",
                )
            updated = copy.deepcopy(state)
            result = mutate(updated)
            next_revision = current_revision + 1
            updated["revision"] = next_revision
            updated["lastFencingToken"] = current_lease.fencing_token
            updated["lastMutation"] = {
                "operation": operation,
                "idempotencyKey": idempotency_key,
                "payloadHash": payload_hash,
                "revisionBefore": current_revision,
                "revisionAfter": next_revision,
                "timestamp": _format_timestamp(now),
                "result": copy.deepcopy(result),
            }
            validate_state_invariants(updated)
            self._atomic_write_json(self.state_path, updated)
            return MutationResponse(
                state=copy.deepcopy(updated),
                revision_before=current_revision,
                revision_after=next_revision,
                changed=True,
                result=result,
            )

    def validate_mutation(
        self,
        *,
        expected_revision: int,
        lease_id: str,
        fencing_token: int,
    ) -> StateSnapshot:
        """Validate write ownership and revision without changing any artifact."""
        with self._transaction_lock():
            now = self._now()
            state = self._read_state()
            current_revision = _state_revision(state)
            document = self._require_active_lease_document()
            current_lease = LeaseRecord.from_dict(document)
            self._assert_lease(
                current_lease, lease_id, fencing_token, now, allow_stale_detail=True
            )
            if expected_revision != current_revision:
                raise StateStoreError(
                    "REVISION_CONFLICT",
                    "expected revision does not match the current Work Item revision",
                    details={
                        "expectedRevision": expected_revision,
                        "currentRevision": current_revision,
                    },
                    remediation="read the Work Item again and recompute the mutation",
                )
            return StateSnapshot(copy.deepcopy(state), current_revision)

    def _transaction_lock(self) -> CrossPlatformFileLock:
        self._assert_allowed_path(self.lock_path)
        return CrossPlatformFileLock(self.lock_path, self._lock_timeout_seconds)

    def _assert_allowed_path(self, path: Path) -> None:
        if self.allowed_root is None:
            return
        try:
            root = self.allowed_root.resolve(strict=False)
            resolved = path.resolve(strict=False)
            resolved.relative_to(root)
        except (OSError, RuntimeError, ValueError) as exc:
            raise StateStoreError(
                "STATE_PATH_OUTSIDE_ALLOWED_ROOT",
                "state persistence path escapes the allowed Work Item root",
                details={
                    "path": str(path),
                    "allowedRoot": str(self.allowed_root),
                },
            ) from exc

    def _read_state(self) -> dict[str, Any]:
        try:
            state = read_legacy_state(self.state_path)
        except json.JSONDecodeError as exc:
            raise StateStoreError(
                "STATE_INVALID_JSON",
                "state.json is not valid JSON",
                details={"path": str(self.state_path.resolve()), "error": str(exc)},
                remediation="restore a valid state.json before retrying",
            ) from exc
        if not isinstance(state, dict):
            raise StateStoreError(
                "STATE_INVALID_SCHEMA",
                "state.json root must be an object",
                details={"path": str(self.state_path.resolve())},
            )
        return state

    def _read_lease_document(self) -> Optional[dict[str, Any]]:
        if not self.lease_path.is_file():
            return None
        try:
            value = json.loads(self.lease_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            raise StateStoreError(
                "LEASE_INVALID_JSON",
                "state.lease.json is not valid JSON",
                details={"path": str(self.lease_path.resolve()), "error": str(exc)},
                remediation="restore or explicitly break a valid lease record",
            ) from exc
        if not isinstance(value, dict):
            raise StateStoreError(
                "LEASE_INVALID_SCHEMA",
                "state.lease.json root must be an object",
                details={"path": str(self.lease_path.resolve())},
            )
        return value

    def _require_active_lease_document(self) -> dict[str, Any]:
        document = self._read_lease_document()
        if document is None or document.get("status") != "active":
            raise StateStoreError(
                "LEASE_REQUIRED",
                "the Work Item has no active writer lease",
                remediation="acquire a Work Item lease before writing state",
            )
        return document

    def _assert_lease(
        self,
        current: LeaseRecord,
        lease_id: str,
        fencing_token: int,
        now: datetime,
        *,
        allow_stale_detail: bool = False,
    ) -> None:
        if fencing_token < current.fencing_token:
            raise StateStoreError(
                "STALE_FENCING_TOKEN",
                "the writer lease was superseded by a newer owner",
                details={
                    "providedFencingToken": fencing_token,
                    "currentFencingToken": current.fencing_token,
                    "currentLeaseId": current.lease_id if allow_stale_detail else None,
                },
                remediation="discard the stale request and acquire a new lease",
            )
        if current.is_expired(now):
            raise StateStoreError(
                "LEASE_EXPIRED",
                "the writer lease has expired",
                details={"expiresAt": _format_timestamp(current.expires_at)},
                remediation="acquire a new lease before writing",
            )
        if fencing_token != current.fencing_token or lease_id != current.lease_id:
            raise StateStoreError(
                "LEASE_NOT_OWNED",
                "the provided lease identity does not own the Work Item",
                details={"currentFencingToken": current.fencing_token},
            )

    def _write_lease_record(
        self, record: LeaseRecord, history: list[dict[str, Any]]
    ) -> None:
        document = {
            "schemaVersion": "1",
            "status": "active",
            **record.to_dict(),
            "history": history,
        }
        self._atomic_write_json(self.lease_path, document)

    def _atomic_write_json(self, path: Path, value: dict[str, Any]) -> None:
        self._assert_allowed_path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        fd, temp_name = tempfile.mkstemp(
            prefix=f".{path.name}.", suffix=".tmp", dir=str(path.parent)
        )
        temp_path = Path(temp_name)
        try:
            with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as handle:
                json.dump(value, handle, ensure_ascii=False, indent=2)
                handle.write("\n")
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temp_path, path)
            _fsync_directory(path.parent)
        finally:
            if temp_path.exists():
                temp_path.unlink()

    def _now(self) -> datetime:
        value = self._clock()
        if value.tzinfo is None:
            value = value.replace(tzinfo=timezone.utc)
        return value.astimezone(timezone.utc)


def _validate_ttl(ttl_seconds: int) -> None:
    if not MIN_LEASE_TTL_SECONDS <= ttl_seconds <= MAX_LEASE_TTL_SECONDS:
        raise StateStoreError(
            "INVALID_LEASE_TTL",
            f"lease TTL must be between {MIN_LEASE_TTL_SECONDS} and "
            f"{MAX_LEASE_TTL_SECONDS} seconds",
            details={"ttlSeconds": ttl_seconds},
        )


def _require_idempotency_key(value: str) -> None:
    if not value:
        raise StateStoreError(
            "IDEMPOTENCY_KEY_REQUIRED", "a non-empty idempotency key is required"
        )


def _state_revision(state: dict[str, Any]) -> int:
    value = state.get("revision", 0)
    if type(value) is not int or value < 0:
        raise StateStoreError(
            "STATE_INVALID_REVISION",
            "state revision must be a non-negative integer",
            details={"revision": value},
        )
    return value


def _lease_document_token(document: Optional[dict[str, Any]]) -> int:
    if not document:
        return 0
    value = document.get("fencingToken", 0)
    if type(value) is not int or value < 0:
        raise StateStoreError(
            "LEASE_INVALID_SCHEMA",
            "lease fencingToken must be a non-negative integer",
            details={"fencingToken": value},
        )
    return value


def _canonical_hash(value: Any) -> str:
    encoded = json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _parse_timestamp(value: Any) -> datetime:
    if not isinstance(value, str) or not value:
        raise ValueError("timestamp must be a non-empty string")
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def _format_timestamp(value: datetime) -> str:
    return value.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _fsync_directory(path: Path) -> None:
    if os.name == "nt":
        return
    flags = getattr(os, "O_DIRECTORY", 0) | os.O_RDONLY
    try:
        descriptor = os.open(str(path), flags)
    except OSError:
        return
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
