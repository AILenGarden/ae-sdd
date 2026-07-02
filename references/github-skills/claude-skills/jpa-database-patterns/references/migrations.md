# Flyway Migrations: Rules, Expand/Contract Recipes, Lock Safety

Zero-downtime schema change for PostgreSQL services deployed with rolling updates
(old and new code run simultaneously during every deploy).

## Flyway ruleset

- Location `src/main/resources/db/migration`; naming `V<version>__<description>.sql`
  (two underscores). Use timestamp versions — `V20260610_01__add_orders_status_index.sql` —
  so parallel branches don't collide on `V42`.
- **Applied migrations are immutable.** Editing one changes its checksum →
  `FlywayValidateException: Migration checksum mismatch` in every environment that already
  ran it. Recovery: restore the original file, write a new forward migration. `flyway repair`
  is for genuinely corrupted history only — never routine.
- One logical change per migration file; a failed multi-statement file leaves the schema
  half-applied in the middle of a deploy.
- Idempotent guards (`IF NOT EXISTS` / `IF EXISTS`) make re-runs after partial failure safe.
- No data-destroying statements in the same release as the code change. `DROP` belongs to
  the contract phase, releases later.
- Migrations must not depend on application code (no Java migrations calling services).
  Large backfills don't belong in Flyway at all — see Backfills below.
- Environments: `spring.jpa.hibernate.ddl-auto: validate` everywhere deployed. Flyway owns
  the schema; Hibernate only verifies the mapping matches.
- Test every migration in CI by running the full chain against Testcontainers Postgres
  (Flyway enabled in `@DataJpaTest`/`@SpringBootTest` integration tests) — empty-to-current
  must always succeed.

## Postgres DDL lock safety

`ACCESS EXCLUSIVE` blocks all reads and writes. The lock *queue* is the killer: your DDL
waits behind one long transaction, and every subsequent query waits behind your DDL.

| Operation | Lock | Safe on hot table? |
|---|---|---|
| `ADD COLUMN` (nullable, no default) | brief ACCESS EXCLUSIVE | ✅ instant |
| `ADD COLUMN ... DEFAULT <constant>` | brief ACCESS EXCLUSIVE | ✅ PG 11+: metadata-only |
| `ADD COLUMN ... DEFAULT <volatile fn>` | ACCESS EXCLUSIVE + rewrite | ❌ rewrites table |
| `CREATE INDEX` | SHARE (blocks writes) | ❌ use CONCURRENTLY |
| `CREATE INDEX CONCURRENTLY` | no write block | ✅ slower; can leave INVALID index on failure — drop & retry |
| `ADD CONSTRAINT ... NOT VALID` then `VALIDATE CONSTRAINT` | brief / SHARE UPDATE EXCLUSIVE | ✅ two-step |
| `ADD CONSTRAINT` (FK/CHECK, direct) | blocks writes during full scan | ❌ use NOT VALID two-step |
| `SET NOT NULL` (direct) | ACCESS EXCLUSIVE + full scan | ❌ PG12+: add `CHECK (col IS NOT NULL) NOT VALID` → `VALIDATE` → `SET NOT NULL` (then drop the check) |
| `ALTER COLUMN TYPE` (e.g. int→bigint) | ACCESS EXCLUSIVE + rewrite | ❌ expand/contract with new column (exception: `varchar(n)`→larger n / `text` is metadata-only) |
| `DROP COLUMN` | brief ACCESS EXCLUSIVE | ✅ lock-wise — but breaks old code; contract phase only |
| `RENAME COLUMN/TABLE` | brief ACCESS EXCLUSIVE | ❌ semantically: old pods break instantly |

Always cap lock waits so a blocked migration aborts instead of queueing the whole app:

```sql
SET lock_timeout = '5s';
SET statement_timeout = '60s';   -- not for CONCURRENTLY index builds or VALIDATE
```

`CREATE INDEX CONCURRENTLY` cannot run inside a transaction. In Flyway, first line of the
file:

```sql
-- flyway:executeInTransaction=false
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_orders_customer_id ON orders (customer_id);
```

## Expand/contract recipes

Old code and new code overlap during rollout; every step must keep both working.

### Add a NOT NULL column

```
Release N   (expand):   ADD COLUMN channel varchar(20) NULL;
                        new code writes channel on every insert/update
Backfill:               batched UPDATE for historical rows (see Backfills)
Release N+1:            CHECK (channel IS NOT NULL) NOT VALID → VALIDATE → SET NOT NULL
                        (optionally add DEFAULT for future inserts)
```

### Rename a column (`placed_at` → `created_at`)

```
Release N   (expand):   ADD COLUMN created_at timestamptz NULL;
                        code dual-writes both columns, reads placed_at
Backfill:               copy placed_at → created_at in batches
Release N+1:            code reads created_at (still dual-writes)
Release N+2 (contract): code stops touching placed_at; migration DROP COLUMN placed_at
```

### Change a column type (int → bigint id, the classic)

Same shape as rename: add `new_id bigint`, dual-write (trigger or app code), backfill,
switch reads, swap constraints/sequence in one short transaction, drop old. Never
`ALTER COLUMN TYPE` in place on a big table — full rewrite under ACCESS EXCLUSIVE.

### Add an FK constraint to existing tables

```sql
ALTER TABLE order_line ADD CONSTRAINT fk_order_line_order
    FOREIGN KEY (order_id) REFERENCES orders (id) NOT VALID;  -- brief lock
ALTER TABLE order_line VALIDATE CONSTRAINT fk_order_line_order; -- scan, no write block
```

Index the FK column first (`CONCURRENTLY`).

### Drop a column/table

Only after a release in which **no deployed code** references it (search the codebase, then
check one full release cycle). Drop in its own migration. If risk is high, soft-decouple
first: `ALTER TABLE orders RENAME COLUMN legacy_flag TO legacy_flag_unused;` for one
release — anything still using it fails loudly in staging, and the rename is trivially
reversible.

## Backfills

Not in Flyway when the table is large — a single `UPDATE orders SET ...` rewrites the
table, bloats WAL, and can lock for minutes. Instead run a controlled job (separate
runbook/admin task):

```sql
UPDATE orders SET created_at = placed_at
WHERE id IN (SELECT id FROM orders
             WHERE created_at IS NULL ORDER BY id LIMIT 10000);
-- loop until 0 rows; sleep between batches; monitor replication lag and autovacuum
```

Keyset-batched (`WHERE id > :last ORDER BY id LIMIT n`) is preferable to repeated
subselects on very large tables. The dual-write code must already be live, so the backfill
only handles historical rows and can be paused/resumed safely.

## Review checklist for any migration PR

- [ ] New `V` file only; no edits to applied migrations
- [ ] Runs green from empty schema in CI (Testcontainers + Flyway chain)
- [ ] Backward-compatible with currently deployed code (expand/contract phase identified in the PR description)
- [ ] Hot-table DDL checked against the lock table above; `lock_timeout` set
- [ ] Indexes `CONCURRENTLY` + `executeInTransaction=false`
- [ ] No large in-migration backfills
- [ ] Destructive statements only in a contract-phase migration, with the release that stopped using the object linked
