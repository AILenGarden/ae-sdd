# Transactions and Locking

`@Transactional` semantics, propagation, isolation, optimistic vs pessimistic locking, and
retry-on-conflict for Spring Boot 3.x + Hibernate + PostgreSQL.

## Where the boundary lives

Service methods. One use case = one transaction. Controllers translate HTTP; repositories
execute statements; the service method defines atomicity.

```java
@Service
public class TransferService {

    private final AccountRepository accountRepository;
    private final TransferRepository transferRepository;

    public TransferService(AccountRepository accountRepository,
                           TransferRepository transferRepository) {
        this.accountRepository = accountRepository;
        this.transferRepository = transferRepository;
    }

    @Transactional
    public TransferResponse transfer(TransferRequest request) {
        // debit + credit + transfer record: all or nothing
    }

    @Transactional(readOnly = true)
    public Page<TransferResponse> history(UUID accountId, Pageable pageable) { ... }
}
```

`readOnly = true` on every query path: skips dirty-checking flush, hints the JDBC driver,
and lets routing datasources target replicas. Forgetting it is the most common silent
perf bug in service code.

## Proxy pitfalls (transactions that silently don't exist)

- **Self-invocation**: `this.otherTransactionalMethod()` bypasses the Spring proxy — no
  transaction, no exception, no warning. Fix: move the method to another bean, or restructure
  so the entry point owns the annotation.
- **Non-public methods**: `@Transactional` on private/package methods is ignored by
  proxy-based AOP.
- **Rollback rules**: rollback happens for unchecked exceptions only. Checked exceptions
  commit unless `rollbackFor` says otherwise. Prefer unchecked domain exceptions.
- **Catch-and-continue inside a tx**: once a data-access exception marks the tx
  rollback-only, committing throws `UnexpectedRollbackException`. Don't swallow and proceed.
- **No remote calls inside a tx**: an HTTP/Kafka call inside `@Transactional` holds a
  pooled connection for the call's full latency (pool exhaustion under load) and creates a
  dual-write (DB committed, message not, or vice versa — see kafka-event-patterns outbox).

## Propagation

| Propagation | Behavior | Use |
|---|---|---|
| `REQUIRED` (default) | Join existing tx or start one | Almost everything |
| `REQUIRES_NEW` | Suspend caller's tx, run independent one | Audit/outbox-relay bookkeeping that must commit even if business tx rolls back. Costs a **second connection** — deadlock-prone if the outer tx holds locks the inner needs |
| `SUPPORTS` | Join if present, else non-transactional | Rarely worth specifying |
| `MANDATORY` | Throw if no tx active | Guard for must-be-called-in-tx internals |
| `NOT_SUPPORTED` | Suspend tx, run outside | Long ops mid-flow that mustn't hold a connection |
| `NEVER` | Throw if tx active | Guard against accidental tx context |
| `NESTED` | JDBC savepoint within outer tx | Partial rollback of a sub-step; JPA support is patchy — prefer restructuring |

Default to `REQUIRED`; every other value in a PR deserves a comment explaining why.

## Isolation (PostgreSQL)

| Level | Postgres behavior | When |
|---|---|---|
| `READ_COMMITTED` (PG default) | Each statement sees latest committed data | Default — correct for almost all service work when paired with optimistic locking |
| `REPEATABLE_READ` | Snapshot per tx; concurrent write conflicts throw serialization errors | Multi-read consistency within one tx (reports) |
| `SERIALIZABLE` | Full SSI; aborts anomalous tx with SQLSTATE 40001 | Invariants spanning rows/tables that locks can't express; must pair with retry |

Postgres has no dirty reads, so `READ_UNCOMMITTED` = `READ_COMMITTED`. Don't reach for
isolation to fix lost updates — `@Version` is cheaper and clearer.

## Optimistic vs pessimistic

**Optimistic (`@Version`) is the default.** Web traffic conflicts are rare; pay only on
actual conflict.

```java
@Entity
public class Account {
    @Id private UUID id;
    @Version private long version;   // UPDATE ... WHERE id=? AND version=?  → 0 rows = conflict
    private long balanceCents;
}
```

Conflict surfaces as `ObjectOptimisticLockingFailureException` (wrapping
`OptimisticLockException`). Map to HTTP 409; client re-reads and retries. For REST
update flows, expose the version (e.g. `ETag` / `If-Match` or a `version` field in the DTO)
so clients can't overwrite each other blindly.

**Pessimistic** for genuine hot rows where conflicts are the norm, in short transactions:

```java
public interface AccountRepository extends JpaRepository<Account, UUID> {

    @Lock(LockModeType.PESSIMISTIC_WRITE)               // SELECT ... FOR UPDATE
    @QueryHints(@QueryHint(name = "jakarta.persistence.lock.timeout", value = "3000"))
    @Query("select a from Account a where a.id = :id")
    Optional<Account> findForUpdate(UUID id);
}
```

- Lock rows in a **consistent global order** (e.g. by id) when locking more than one —
  classic deadlock prevention for transfers.
- `FOR UPDATE SKIP LOCKED` is the work-queue pattern (each worker grabs unclaimed rows) —
  used by outbox relays; see kafka-event-patterns.
- A pessimistic lock held across anything slow is a system-wide stall. If the tx isn't
  tens of milliseconds, redesign.

Decision guide: conflicts rare → optimistic. Conflicts constant on few rows (counters,
inventory) → pessimistic or a set-based atomic
`UPDATE ... SET balance = balance - ? WHERE id = ? AND balance >= ?`. Cross-aggregate
invariants → SERIALIZABLE + retry, or redesign the aggregate.

## Retry on conflict

Optimistic conflicts and serialization failures (SQLSTATE 40001/40P01) are transient —
retry the **whole transaction** from a fresh read, never inside it (the entity is stale and
the tx is rollback-only).

With Spring Retry (`@EnableRetry`):

```java
@Service
public class BalanceService {

    private final AccountRepository accountRepository;

    public BalanceService(AccountRepository accountRepository) {
        this.accountRepository = accountRepository;
    }

    @Retryable(
            retryFor = {ObjectOptimisticLockingFailureException.class,
                        CannotAcquireLockException.class},
            maxAttempts = 3,
            backoff = @Backoff(delay = 50, multiplier = 2, random = true))
    @Transactional
    public void applyDebit(UUID accountId, long amountCents) {
        Account account = accountRepository.findById(accountId)
                .orElseThrow(() -> new AccountNotFoundException(accountId));
        account.debit(amountCents);                    // re-read fresh each attempt
    }

    @Recover
    void exhausted(ObjectOptimisticLockingFailureException ex, UUID accountId, long amount) {
        throw new ConcurrentModificationConflictException(accountId);  // → HTTP 409
    }
}
```

Ordering matters: `@Retryable` must wrap `@Transactional` (retry interceptor outside the
tx interceptor) so each attempt gets a fresh transaction — Spring Retry's default order
does this; verify if you customize advisor order. Only retry operations that are
idempotent at business level; cap attempts and surface 409 after exhaustion rather than
retrying forever.

## Timeouts

- `@Transactional(timeout = 5)` — seconds; rolls back runaway transactions.
- HikariCP `connection-timeout` bounds waiting for a connection (see query-optimization.md).
- Postgres-side safety nets: `idle_in_transaction_session_timeout` kills transactions
  abandoned mid-flight (the silent pool killer), `lock_timeout` for DDL (see migrations.md).
