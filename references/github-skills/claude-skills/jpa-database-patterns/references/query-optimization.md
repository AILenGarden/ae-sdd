# Query Optimization: EXPLAIN Triage, Indexing, Slow-Query Patterns

PostgreSQL-specific. Work from evidence (plans, statistics), never from intuition.

## EXPLAIN ANALYZE triage workflow

1. **Capture the real query.** From Hibernate SQL logs (with bind values via
   `org.hibernate.orm.jdbc.bind: TRACE` locally), `pg_stat_statements`
   (`ORDER BY total_exec_time DESC`), or APM. Optimize the query Postgres actually runs,
   not the JPQL.
2. **Get the plan with execution data:**

```sql
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)
SELECT o.id, o.status FROM orders o
WHERE o.customer_id = '7f3a…' AND o.status = 'OPEN'
ORDER BY o.created_at DESC LIMIT 20;
```

   On prod replicas, wrap mutating statements: `BEGIN; EXPLAIN ANALYZE <stmt>; ROLLBACK;`
3. **Read inside-out, find where time is spent.** Red flags:

| Plan signal | Meaning | Typical fix |
|---|---|---|
| `Seq Scan` on large table + small result | Missing/unusable index | Add index; check predicate isn't wrapped in a function |
| `Rows Removed by Filter: 1,500,000` | Index found candidates, filter discarded most | Composite or partial index covering the full predicate |
| Estimate vs actual off by 100x+ | Stale statistics | `ANALYZE <table>`; raise column statistics target if skewed |
| `Sort Method: external merge Disk` | Sort spills to disk | Index providing the order, or raise `work_mem` for that workload |
| Nested Loop with huge inner loop count | Bad join strategy from bad estimates | Fix stats; index the inner join key |
| `Heap Fetches` high on Index Only Scan | Visibility map stale | More frequent `VACUUM` (check autovacuum on hot tables) |

4. **Change one thing, re-run, compare** `Execution Time` and shared-buffer hits. Paste
   before/after plans in the PR.
5. **Confirm in prod telemetry** — p99 latency for the endpoint, `pg_stat_statements`
   `mean_exec_time` for the statement.

## Index strategy

```sql
-- FK index: Postgres does NOT create these automatically
CREATE INDEX CONCURRENTLY idx_order_line_order_id ON order_line (order_id);

-- Composite: equality columns first, then range/sort columns
-- serves: WHERE customer_id = ? AND status = ? ORDER BY created_at DESC
CREATE INDEX CONCURRENTLY idx_orders_customer_status_created
    ON orders (customer_id, status, created_at DESC);

-- Partial: hot subset of a skewed column (1% of rows are OPEN)
CREATE INDEX CONCURRENTLY idx_orders_open
    ON orders (customer_id) WHERE status = 'OPEN';

-- Covering: enables index-only scans for a specific hot read
CREATE INDEX CONCURRENTLY idx_orders_customer_inc
    ON orders (customer_id) INCLUDE (status, created_at);

-- Expression: when the query filters on an expression
CREATE INDEX CONCURRENTLY idx_customers_email_lower ON customers (lower(email));
```

Rules of thumb:

- A composite index `(a, b, c)` serves predicates on `(a)`, `(a,b)`, `(a,b,c)` — not `(b)`
  or `(c)` alone. Order by selectivity *within* the equality group; range column last.
- Every index taxes writes and autovacuum. Before adding, check for an existing index you
  can extend; after migrating, drop unused ones
  (`pg_stat_user_indexes.idx_scan = 0` over a representative window).
- Indexes on low-cardinality columns alone (`status`, booleans) are rarely used — make
  them partial or composite instead.
- Always `CONCURRENTLY` on live tables (see migrations.md for the Flyway mechanics).

## Common slow-query patterns

### Function on an indexed column

```sql
-- ❌ index on created_at is unusable
WHERE date(created_at) = '2026-06-10'
-- ✅ sargable range
WHERE created_at >= '2026-06-10' AND created_at < '2026-06-11'
```

Same trap: `lower(email) = ?` without the expression index; implicit casts
(`varchar_col = 123`); JPA passing a timestamp with the wrong type precision.

### Leading-wildcard LIKE

`WHERE name LIKE '%pump%'` can't use a btree. Options: trigram index
(`CREATE EXTENSION pg_trgm; CREATE INDEX ... USING gin (name gin_trgm_ops)`) or full-text
search. Don't ship substring search on a large table without one of these.

### OFFSET pagination on deep pages

`LIMIT 20 OFFSET 100000` reads and discards 100k rows. Keyset instead:

```sql
WHERE (created_at, id) < (:lastCreatedAt, :lastId)
ORDER BY created_at DESC, id DESC LIMIT 20;
```

Spring Data: `Slice` + explicit `@Query`, or Spring Data 3.1+ `ScrollPosition.keyset()`.
Requires an index matching the sort and a unique tiebreaker column.

### COUNT(*) on every page request

`Page<T>` issues a count query per call — often costlier than the page itself. Use
`Slice<T>` (no count, just "has next") when the UI doesn't need total pages; cache or
estimate counts (`reltuples`) for dashboards.

### IN-list explosion

JPA `WHERE id IN (:ids)` with thousands of ids → giant parse/plan cost and plan-cache
churn. Chunk to ≤1000, or join against `unnest(:ids::uuid[])`.

### SELECT * / full-entity hydration for read views

Fetching wide rows (incl. `text`/`jsonb` columns) to render three fields. Use interface or
record projections — also unlocks index-only scans:

```java
public record OrderRow(UUID id, OrderStatus status, Instant createdAt) {}

@Query("select new com.example.dto.OrderRow(o.id, o.status, o.createdAt) " +
       "from Order o where o.customerId = :customerId")
Page<OrderRow> rows(UUID customerId, Pageable pageable);
```

### Row-by-row writes

Saving a list in a loop = one round trip per row. Enable JDBC batching:

```yaml
spring.jpa.properties:
  hibernate.jdbc.batch_size: 50
  hibernate.order_inserts: true
  hibernate.order_updates: true
```

Note: `IDENTITY` id generation disables insert batching — prefer `SEQUENCE` (pooled
optimizer) or client-generated UUIDv7. For bulk maintenance updates, a single set-based
`UPDATE ... WHERE` beats any entity loop.

## Connection pool sizing (HikariCP)

```yaml
spring:
  datasource:
    hikari:
      maximum-pool-size: 10      # per pod — start here, tune on evidence
      minimum-idle: 10           # fixed-size pool avoids resize churn
      connection-timeout: 3000   # fail fast and visibly on exhaustion
      max-lifetime: 1800000      # < any infra-side idle timeout (LB, pgbouncer)
```

- Useful ceiling ≈ `cores × 2 + effective_spindles` on the *database* — a 16-core Postgres
  saturates around ~40–50 active connections total. Sum `maximum-pool-size × pods × services`
  against that and against `max_connections`; use pgbouncer when the multiplied total is
  unavoidable.
- Pool exhaustion is usually **transactions held too long** (remote calls inside
  `@Transactional`, missing `readOnly`, giant batch in one tx) — fix that before raising
  the pool, which typically just moves the queue into Postgres.
- Watch via Micrometer: `hikaricp_connections_active`, `hikaricp_connections_pending`,
  `hikaricp_connections_timeout_total` on the Grafana dashboard; pending > 0 sustained is
  the early-warning signal.
