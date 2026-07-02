-- Harden cross-tenant database isolation at the connection layer.
--
-- WHY: PostgreSQL grants CONNECT on every database to PUBLIC by default. Because
-- roles are cluster-global, that default lets ANY tenant's db_user open a session
-- on ANOTHER tenant's database and -- via the shared service_role/authenticated/anon
-- roles it belongs to -- read that tenant's data. New databases created by
-- DatabaseInitService now revoke this at creation time; this script retrofits any
-- databases that were created before that fix.
--
-- WHAT: For every non-template database (except the bootstrap `postgres` db) it
-- revokes CONNECT from PUBLIC and re-grants CONNECT only to the database's owner
-- (the tenant's own db_user). The metadata database is included in the revoke: it
-- is connected to by a superuser, which bypasses CONNECT checks, so this is safe
-- and additionally stops tenant roles from reaching the tenant credential table.
--
-- RUN AS a superuser (e.g. postgres), once:
--   psql -h <host> -U postgres -d postgres -v ON_ERROR_STOP=1 -f script/harden-tenant-db-connect.sql
--
-- Idempotent: re-running it is safe.
--
-- NOTE: if your metadata DB connection user (METADATA_DB_USER) is NOT a superuser,
-- grant it CONNECT explicitly after running this, e.g.:
--   GRANT CONNECT ON DATABASE postgrest_metadata TO <that_user>;

\set ON_ERROR_STOP on

DO $$
DECLARE
    r record;
    -- Adjust if your metadata database uses a different name (METADATA_DB_URL).
    meta_db text := 'postgrest_metadata';
BEGIN
    FOR r IN
        SELECT d.datname,
               pg_catalog.pg_get_userbyid(d.datdba) AS owner
        FROM pg_database d
        WHERE d.datistemplate = false
          AND d.datname <> 'postgres'
    LOOP
        EXECUTE format('REVOKE CONNECT ON DATABASE %I FROM PUBLIC', r.datname);

        -- Re-grant CONNECT to the owning tenant user. Skip the metadata db and any
        -- database owned by the bootstrap superuser (it connects as a superuser).
        IF r.datname <> meta_db AND r.owner <> 'postgres' THEN
            EXECUTE format('GRANT CONNECT ON DATABASE %I TO %I', r.datname, r.owner);
        END IF;

        RAISE NOTICE 'Hardened CONNECT on database % (owner %)', r.datname, r.owner;
    END LOOP;
END $$;
