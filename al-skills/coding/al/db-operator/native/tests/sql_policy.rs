use db_operator_native::registry::Engine;
use db_operator_native::registry::WriteLevel;
use db_operator_native::sql_policy::{SqlPolicyError, validate_for_level, validate_read_only};

#[test]
fn policy_allows_supported_read_only_statements() {
    for sql in [
        "SELECT * FROM users LIMIT 5",
        "SHOW TABLES",
        "DESCRIBE users",
        "EXPLAIN SELECT * FROM users",
        "WITH x AS (SELECT 1 AS id) SELECT * FROM x",
    ] {
        assert_eq!(validate_read_only(sql, Engine::Mysql).unwrap(), sql);
    }

    for sql in [
        "SELECT current_date",
        "WITH x AS (SELECT 1 AS id) SELECT * FROM x",
        "EXPLAIN SELECT * FROM users",
        "EXPLAIN (VERBOSE, COSTS TRUE, FORMAT JSON) SELECT * FROM users",
    ] {
        assert_eq!(validate_read_only(sql, Engine::Postgresql).unwrap(), sql);
    }
}

#[test]
fn policy_rejects_writes_admin_sql_and_multiple_statements() {
    for sql in [
        "INSERT INTO users(id) VALUES (1)",
        "UPDATE users SET name = 'x'",
        "DELETE FROM users",
        "DROP TABLE users",
        "ALTER TABLE users ADD COLUMN x INT",
        "CREATE TABLE x(id INT)",
        "TRUNCATE TABLE users",
        "GRANT SELECT ON users TO reader",
        "SET sql_safe_updates = 0",
        "SELECT 1; DELETE FROM users",
        "WITH x AS (DELETE FROM users RETURNING *) SELECT * FROM x",
    ] {
        assert!(
            validate_read_only(sql, Engine::Postgresql).is_err(),
            "must reject {sql}"
        );
    }
}

#[test]
fn dml_level_allows_only_dml_and_ddl_level_allows_selected_ddl() {
    for sql in [
        "INSERT INTO users (name) VALUES ('a')",
        "UPDATE users SET name = 'b'",
        "DELETE FROM users",
    ] {
        assert!(validate_for_level(sql, Engine::Mysql, WriteLevel::Dml).is_ok());
        assert!(validate_for_level(sql, Engine::Mysql, WriteLevel::Ddl).is_ok());
    }
    // A write level extends reads: verifying a write needs a follow-up SELECT.
    for sql in [
        "SELECT COUNT(*) AS c FROM users",
        "SHOW TABLES",
        "SELECT id FROM users WHERE name = 'a'",
    ] {
        assert!(
            validate_for_level(sql, Engine::Mysql, WriteLevel::Dml).is_ok(),
            "dml level must still allow {sql}"
        );
        assert!(
            validate_for_level(sql, Engine::Mysql, WriteLevel::Ddl).is_ok(),
            "ddl level must still allow {sql}"
        );
    }
    // Reads stay subject to the read-only policy even at a write level, and the
    // rejection names the real reason instead of blaming the statement type.
    for sql in [
        "SELECT * FROM users FOR UPDATE",
        "SELECT @@version",
        "SELECT 1; SELECT 2",
        "SELECT GET_LOCK('db-operator', 5)",
    ] {
        let rejected = validate_for_level(sql, Engine::Mysql, WriteLevel::Dml)
            .expect_err(&format!("dml level must still reject {sql}"));
        assert!(
            !matches!(&rejected, SqlPolicyError::StatementType(_) if rejected.to_string().contains("select")),
            "{sql} must be rejected for its construct, not for being a select: {rejected}"
        );
    }
    assert!(
        validate_for_level(
            "CREATE TABLE users (id INT)",
            Engine::Mysql,
            WriteLevel::Dml
        )
        .is_err()
    );
    assert!(
        validate_for_level(
            "CREATE TABLE users (id INT)",
            Engine::Mysql,
            WriteLevel::Ddl
        )
        .is_ok()
    );
    assert!(validate_for_level("DROP TABLE users", Engine::Mysql, WriteLevel::Ddl).is_err());
    assert!(validate_for_level("TRUNCATE TABLE users", Engine::Mysql, WriteLevel::Ddl).is_err());
}

#[test]
fn policy_rejects_locking_output_and_side_effect_expressions() {
    let cases = [
        ("SELECT id INTO @user_id FROM users", Engine::Mysql),
        ("SELECT * FROM users FOR UPDATE", Engine::Mysql),
        ("SHOW TABLES INTO OUTFILE '/tmp/tables.txt'", Engine::Mysql),
        ("EXPLAIN ANALYZE SELECT * FROM users", Engine::Mysql),
        ("SELECT @session_value := 1", Engine::Mysql),
        ("SELECT GET_LOCK('db-operator', 5)", Engine::Mysql),
        ("SELECT LAST_INSERT_ID(42)", Engine::Mysql),
        ("SELECT * INTO temp_users FROM users", Engine::Postgresql),
        (
            "EXPLAIN (ANALYZE TRUE, FORMAT JSON) SELECT * FROM users",
            Engine::Postgresql,
        ),
        ("SELECT nextval('orders_id_seq')", Engine::Postgresql),
        ("SELECT pg_sleep(1)", Engine::Postgresql),
        ("SELECT pg_advisory_unlock(1)", Engine::Postgresql),
        (
            "SELECT dblink('remote', 'DELETE FROM users')",
            Engine::Postgresql,
        ),
        ("SELECT pg_terminate_backend(123)", Engine::Postgresql),
        ("SELECT lo_unlink(123)", Engine::Postgresql),
        (
            "SELECT set_config('search_path', 'public', false)",
            Engine::Postgresql,
        ),
    ];

    for (sql, engine) in cases {
        assert!(
            validate_read_only(sql, engine).is_err(),
            "must reject {sql}"
        );
    }
}

#[test]
fn policy_rejects_every_sql_comment_form() {
    let cases = [
        ("SELECT 1 -- hidden operation", Engine::Mysql),
        ("SELECT 1 # hidden operation", Engine::Mysql),
        ("SELECT 1 /* hidden operation */", Engine::Mysql),
        ("SELECT 1 /*!50000 + 1 */", Engine::Mysql),
        ("SELECT /*+ MAX_EXECUTION_TIME(1000) */ 1", Engine::Mysql),
        ("SELECT 1 -- hidden operation", Engine::Postgresql),
        ("SELECT 1 /* hidden operation */", Engine::Postgresql),
    ];

    for (sql, engine) in cases {
        assert!(
            validate_read_only(sql, engine).is_err(),
            "comments must be rejected before parsing: {sql}"
        );
    }
}

#[test]
fn policy_uses_a_safe_function_allowlist() {
    for (sql, engine) in [
        (
            "SELECT COUNT(*), LOWER(name), COALESCE(email, '') FROM users",
            Engine::Mysql,
        ),
        (
            "SELECT pg_catalog.date_trunc('day', created_at), md5(name) FROM users",
            Engine::Postgresql,
        ),
    ] {
        assert_eq!(validate_read_only(sql, engine).unwrap(), sql);
    }

    for (sql, engine) in [
        ("SELECT company_udf(id) FROM users", Engine::Mysql),
        (
            "SELECT public.company_udf(id) FROM users",
            Engine::Postgresql,
        ),
        ("SELECT generate_series(1, 1000000000)", Engine::Postgresql),
    ] {
        assert!(
            validate_read_only(sql, engine).is_err(),
            "unknown or user-defined functions must be rejected: {sql}"
        );
    }
}

#[test]
fn policy_rejects_connection_identity_and_server_detail_queries() {
    for (sql, engine) in [
        ("SELECT CURRENT_USER", Engine::Mysql),
        ("SELECT USER()", Engine::Mysql),
        ("SELECT DATABASE()", Engine::Mysql),
        ("SELECT @@hostname", Engine::Mysql),
        ("SHOW VARIABLES", Engine::Mysql),
        ("SELECT CURRENT_USER", Engine::Postgresql),
        ("SELECT SESSION_USER", Engine::Postgresql),
        ("SELECT inet_server_addr()", Engine::Postgresql),
        ("SHOW server_version", Engine::Postgresql),
    ] {
        assert!(
            validate_read_only(sql, engine).is_err(),
            "connection identity and server details must be hidden: {sql}"
        );
    }
}
