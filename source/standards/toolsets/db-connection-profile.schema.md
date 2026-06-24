# ae-sdd DB Connection Profile Schema

## 1. Local Path

DB connection profiles are local-only:

```text
<project>/.ae-sdd/secrets/db-connections.local.json
```

This file must not be committed. The target project must ignore
`.ae-sdd/secrets/`.

## 2. JSON Shape

```json
{
  "profiles": [
    {
      "name": "local-sqlite",
      "driver": "sqlite",
      "database": "D:/path/to/local.db",
      "readonly": true,
      "note": "local-only"
    },
    {
      "name": "dev-mysql",
      "driver": "mysql",
      "host": "${AE_DB_HOST}",
      "port": 3306,
      "database": "${AE_DB_NAME}",
      "username": "${AE_DB_USER}",
      "passwordEnv": "AE_DB_PASSWORD",
      "readonly": true
    }
  ]
}
```

## 3. Policy

- `ae-sdd db profiles --init` may create a local template.
- `ae-sdd db query` defaults to read-only.
- Write SQL requires `--write`.
- Non-sqlite drivers may be configured before a driver adapter exists, but the
  command must return `blocked` instead of pretending execution happened.
- Reports may include `name`, `driver`, `host`, `database`, and schema, but must
  redact all secrets.

## 4. Required Evidence

For RA and CodingPlan:

- table/field existence evidence
- key SQL or EXPLAIN evidence when SQL is non-trivial
- missing DB access marked as unverified

For Coding:

- actual integration test, transaction rollback check, or DB query evidence
- write SQL audit when data can change
