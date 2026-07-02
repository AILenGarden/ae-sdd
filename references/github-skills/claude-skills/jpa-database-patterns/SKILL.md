---
name: jpa-database-patterns
description: >
  Use when working on JPA/Hibernate persistence, PostgreSQL performance, or Flyway
  migrations: N+1 query storms, LazyInitializationException ("could not initialize proxy
  — no Session"), slow endpoints traced to the database, deciding fetch strategy (JOIN
  FETCH, @EntityGraph, @BatchSize, projections), @Transactional placement and readOnly,
  optimistic locking with @Version and OptimisticLockException, pagination of large
  tables, missing indexes, EXPLAIN ANALYZE, HikariCP pool sizing, or writing/reviewing
  Flyway migrations (V__ naming, checksum mismatch, expand/contract for zero-downtime,
  CREATE INDEX CONCURRENTLY). Triggers include "hundreds of SELECTs in the log", "query
  is slow in prod", "FlywayValidateException", "lock wait on deploy". Not for REST/DTO/
  controller conventions — use spring-boot-standards. Not for Kafka or the outbox
  relay — use kafka-event-patterns. Not for cache/timeout tuning beyond the DB — use
  resilience-performance.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---

# JPA & Database Patterns

Hibernate, PostgreSQL, and Flyway done right: fetch strategies, transactions, locking,
indexing, and zero-downtime migrations for Spring Boot 3.x services.

## When to use

- Log shows one query followed by N identical queries with different IDs (N+1)
- `LazyInitializationException: could not initialize proxy ... no Session`
- An endpoint is slow and the flame graph points at JDBC / the database
- `ObjectOptimisticLockingFailureException` or lost updates under concurrency
- A list endpoint loads an entire table into memory
- Flyway fails with checksum/validation errors, or a migration locked a table in prod
- Designing entities/relationships, or reviewing a PR that touches `model/`, `repository/`, or `db/migration/`

## Quick reference

| Problem | Symptom | Solution |
|---|---|---|
| N+1 queries | 1 + N SELECTs in SQL log for one request | `JOIN FETCH` / `@EntityGraph` for the use case; `@BatchSize` as mitigation; projection if you don't need entities |
| LazyInitializationException | "no Session" outside service layer | Fetch what the use case needs inside the transaction; never fix with EAGER or Open Session in View |
| Eager loading everywhere | Huge joins, slow simple lookups | `FetchType.LAZY` on every association (incl. `@ManyToOne`, which defaults to EAGER) |
| Lost updates | Two writers, last one silently wins | `@Version` optimistic locking; map conflict to HTTP 409; retry idempotent flows |
| Unbounded query | `findAll()` on a growing table | `Pageable` + indexed sort; keyset pagination for deep pages |
| Slow query | p99 spike on one statement | `EXPLAIN (ANALYZE, BUFFERS)` → fix plan (index, rewrite) — see references/query-optimization.md |
| Missing FK index | Seq scan on child table joins/deletes | Postgres does NOT auto-index FKs — create one per FK column |
| Pool exhaustion | "Connection is not available, request timed out" | Size HikariCP deliberately (~10/pod), shorten transactions, no remote calls inside them |
| Migration edited after apply | `FlywayValidateException: Migration checksum mismatch` | Never edit applied migrations — add a new `V` script that corrects forward |
| Blocking DDL | Deploy stalls, lock queue on hot table | Expand/contract; `CREATE INDEX CONCURRENTLY`; see references/migrations.md |

## MUST

- Declare every association `FetchType.LAZY` — `@ManyToOne`/`@OneToOne` default to EAGER, override them
- Put `@Transactional` on service methods only; `@Transactional(readOnly = true)` on every query path
- Add `@Version` to every entity that can be concurrently updated
- Paginate every repository method that can return an unbounded result
- Create an index for every FK column and every frequently-filtered/sorted column (justified by EXPLAIN)
- Set `spring.jpa.open-in-view: false` explicitly
- Treat applied Flyway migrations as immutable; every schema change is a new `V<n>__description.sql`
- Keep migrations backward-compatible with the currently deployed code (expand/contract)

## MUST NOT

- No `FetchType.EAGER` as a fix for LazyInitializationException
- No `findAll()` without `Pageable` on tables that grow with usage
- No `@Transactional` on controllers, and no self-invocation of `@Transactional` methods (proxy is bypassed — silently no transaction)
- No HTTP/Kafka/remote calls inside a transaction — they hold a pooled connection hostage
- No `SELECT *` / full-entity fetch when a projection serves the read
- No destructive DDL (`DROP COLUMN/TABLE`, `ALTER TYPE` rewrite) in the same release as the code change — contract phase only, after code stops using it
- No plain `CREATE INDEX` on large hot tables — `CONCURRENTLY`, outside a transaction

## N+1: detect, then fix

Detect — enable SQL logging in `application-local.yml` (never prod):

```yaml
spring:
  jpa:
    properties:
      hibernate.format_sql: true
logging:
  level:
    org.hibernate.SQL: DEBUG
```

One request printing `select ... from orders` then 50× `select ... from order_line where
order_id=?` is the signature. `hibernate.generate_statistics: true` + the session metrics
log line gives a query count per request.

❌ BAD — lazy loop, one query per order:

```java
@Transactional(readOnly = true)
public List<OrderSummary> summaries() {
    return orderRepository.findAll().stream()                 // 1 query, also unbounded
            .map(o -> new OrderSummary(o.getId(), o.getLines().size()))  // +1 query each
            .toList();
}
```

✅ GOOD — fetch decided per use case:

```java
public interface OrderRepository extends JpaRepository<Order, UUID> {

    // (a) JOIN FETCH: detail view needs the lines
    @Query("select o from Order o join fetch o.lines where o.id = :id")
    Optional<Order> findWithLinesById(UUID id);

    // (b) @EntityGraph: same intent, derived query, works with Pageable
    @EntityGraph(attributePaths = "lines")
    Page<Order> findByStatus(OrderStatus status, Pageable pageable);

    // (c) Projection: list view needs two columns, not entities
    @Query("""
           select new com.example.orders.dto.OrderSummary(o.id, size(o.lines))
           from Order o where o.customerId = :customerId
           """)
    Page<OrderSummary> findSummaries(UUID customerId, Pageable pageable);
}
```

`@BatchSize(size = 50)` on the collection turns N+1 into N/50+1 — a global mitigation, not
a per-use-case fix. Don't combine `join fetch` on a collection with `Pageable` (Hibernate
paginates in memory — `HHH90003004 firstResult/maxResults specified with collection fetch`).

## LazyInitializationException

The exception means a lazy association was touched after the session closed — usually in a
DTO mapper or Jackson serialization in the controller.

❌ BAD reflexes: `FetchType.EAGER` (penalizes every other query), `spring.jpa.open-in-view:
true` (holds the DB connection through view rendering and hides the design error).

✅ GOOD: fetch the association inside the transactional service method using (a)/(b)/(c)
above, and map to a DTO **before** returning. The entity never crosses the service
boundary unhydrated.

## Transactions and locking

```java
@Service
public class OrderService {

    private final OrderRepository orderRepository;

    public OrderService(OrderRepository orderRepository) {
        this.orderRepository = orderRepository;
    }

    @Transactional(readOnly = true)            // flush off, read-only hint to driver/pool
    public OrderResponse getById(UUID id) { ... }

    @Transactional                             // one atomic boundary, no remote calls inside
    public OrderResponse approve(UUID id, long expectedVersion) { ... }
}
```

Optimistic locking — the default for typical web concurrency:

```java
@Entity
public class Order {
    @Id private UUID id;
    @Version private long version;             // Hibernate adds "where version = ?" to UPDATE
    ...
}
```

A concurrent modification throws `ObjectOptimisticLockingFailureException` → map to 409 in
the `@RestControllerAdvice`. Clients re-read and retry. Pessimistic locks
(`@Lock(PESSIMISTIC_WRITE)`, `SELECT ... FOR UPDATE`) are for genuine hot-row contention
only — propagation matrix, isolation levels, and a retry-on-conflict pattern are in
`references/transactions-locking.md`.

## PostgreSQL essentials

- **Index every FK** — Postgres creates indexes for PKs and unique constraints, *not* FKs.
  Unindexed FKs cause seq scans on joins and full child-table scans on parent deletes.
- **Index frequent predicates** — columns in hot `WHERE`/`ORDER BY`; composite indexes in
  predicate-then-sort order; partial indexes for skewed flags
  (`WHERE status = 'OPEN'`-style queries).
- **Prove it with EXPLAIN** — `EXPLAIN (ANALYZE, BUFFERS) <query>`; look for `Seq Scan` on
  large tables, row-estimate vs actual gaps, and `Rows Removed by Filter`. Full triage
  workflow: `references/query-optimization.md`.
- **Avoid SELECT \*** — interface/record projections cut I/O and let index-only scans work.
- **HikariCP** — start at `maximum-pool-size: 10` per pod; bigger pools usually move the
  queue into Postgres. `connections × pods` must stay well under `max_connections`.
  Set `connection-timeout` (e.g. 3s) so exhaustion fails fast and visibly.

## Flyway discipline

- Naming: `V<version>__<snake_case_description>.sql` in `db/migration` —
  `V20260610_01__add_orders_status_index.sql` (timestamp-style versions avoid number
  collisions between branches).
- **Applied migrations are immutable.** Checksum mismatch in an environment means someone
  edited history: revert the edit, add a new forward migration.
- **Expand/contract for zero downtime** (multiple pods run old+new code during rollout):
  1. *Expand* — release N: add nullable column / new table / index; code writes both, reads old.
  2. *Migrate* — backfill in batches; switch reads to new.
  3. *Contract* — release N+2 (or later): drop the old column once nothing deployed references it.
- Renaming a column is expand/contract too (add new, dual-write, backfill, drop old) —
  `ALTER TABLE ... RENAME` breaks the still-running old pods instantly.
- `CREATE INDEX CONCURRENTLY` on hot tables; it can't run in a transaction, so the
  migration file needs `-- flyway:executeInTransaction=false`. Full lock-safety table and
  recipes: `references/migrations.md`.
- `ddl-auto: validate` in every deployed environment — Flyway owns the schema, Hibernate
  only checks it.

## Verification

```bash
mvn verify                 # Maven (primary): unit + Testcontainers integration tests
./gradlew check            # Gradle equivalent
```

- Run repository tests against real Postgres via Testcontainers (`@DataJpaTest` +
  `@ServiceConnection`), with Flyway enabled — H2 hides Postgres-specific failures and
  validates nothing about your migrations.
- Flyway failure `Validate failed: Migration checksum mismatch for version X` → someone
  edited an applied script; restore it and write a new migration.
- Suspected N+1 fix: assert query count in the test
  (`hibernate.generate_statistics` + `SessionFactory` statistics, or a datasource proxy
  assertion library) — "looks fetched" is not evidence.
- Slow-query claim: paste the `EXPLAIN (ANALYZE, BUFFERS)` before/after into the PR. No
  plan, no merge.

## References

| File | Contents | When to load |
|---|---|---|
| `references/query-optimization.md` | EXPLAIN ANALYZE triage workflow, index strategy (composite/partial/covering), common slow-query patterns and rewrites | A specific query is slow, or designing indexes |
| `references/migrations.md` | Full Flyway ruleset, expand/contract recipes per change type, Postgres DDL lock-safety table, backfill patterns | Writing or reviewing any migration |
| `references/transactions-locking.md` | Propagation/isolation matrix, optimistic vs pessimistic decision guide, retry-on-conflict implementation, self-invocation pitfalls | Transaction boundary design or concurrency bugs |

## Related skills

- **spring-boot-standards** — controller/DTO/error-contract conventions; route there for anything above the service layer.
- **kafka-event-patterns** — the transactional outbox relay and event publishing that piggyback on these transactions.
- **resilience-performance** — timeouts, retries, caching, and metrics around the database, rather than queries themselves.
- **tdd-java** — how to drive these fixes test-first; this skill only defines what the tests must prove.
- **designing-systems** — data modeling and service-boundary decisions before any entity exists.
- **reviewing-java-code** — structured review checklists that reference these rules.
