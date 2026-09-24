use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use futures_util::TryStreamExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlRow, MySqlSslMode};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgRow, PgSslMode};
use sqlx::types::Json;
use sqlx::{Column, Executor, MySqlPool, PgPool, Row, TypeInfo, ValueRef};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::cache::SchemaObject;
use crate::registry::WriteLevel;
use crate::registry::{ConnectionProfile, Engine, config_dir};
use crate::sql_policy::validate_for_level;
use crate::vault::CredentialVault;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub connection: String,
    pub engine: String,
    pub database: Option<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
    pub truncated: bool,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub enum PoolHandle {
    Mysql(MySqlPool),
    Postgresql(PgPool),
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("credential lookup failed: {0}")]
    Credential(String),
    #[error("connection failed: {0}")]
    Connection(String),
    #[error("session setup failed: {0}")]
    Session(String),
    #[error("query failed: {0}")]
    Query(String),
    #[error("profile fingerprint failed: {0}")]
    Fingerprint(#[from] serde_json::Error),
    #[error("SQL policy rejected query: {0}")]
    Policy(String),
}

pub fn profile_fingerprint(profile: &ConnectionProfile) -> Result<String, DatabaseError> {
    let serialized = serde_json::to_vec(profile)?;
    let digest = Sha256::digest(serialized);
    Ok(format!("{digest:x}"))
}

pub fn read_password(alias: &str) -> Result<Zeroizing<String>, DatabaseError> {
    read_password_from(&config_dir(), alias)
}

pub fn read_password_from(
    private_dir: &Path,
    alias: &str,
) -> Result<Zeroizing<String>, DatabaseError> {
    CredentialVault::open(private_dir)
        .map_err(|error| DatabaseError::Credential(error.to_string()))?
        .read(alias)
        .map_err(|error| DatabaseError::Credential(error.to_string()))
}

pub fn store_password(alias: &str, password: &str) -> Result<(), DatabaseError> {
    store_password_in(&config_dir(), alias, password)
}

pub fn store_password_in(
    private_dir: &Path,
    alias: &str,
    password: &str,
) -> Result<(), DatabaseError> {
    CredentialVault::open(private_dir)
        .map_err(|error| DatabaseError::Credential(error.to_string()))?
        .store(alias, password)
        .map_err(|error| DatabaseError::Credential(error.to_string()))
}

pub fn delete_password(alias: &str) -> Result<(), DatabaseError> {
    delete_password_from(&config_dir(), alias)
}

pub fn delete_password_from(private_dir: &Path, alias: &str) -> Result<(), DatabaseError> {
    CredentialVault::open(private_dir)
        .map_err(|error| DatabaseError::Credential(error.to_string()))?
        .delete(alias)
        .map_err(|error| DatabaseError::Credential(error.to_string()))
}

impl PoolHandle {
    pub async fn connect(
        profile: &ConnectionProfile,
        password: &str,
    ) -> Result<Self, DatabaseError> {
        match profile.engine {
            Engine::Mysql => connect_mysql(profile, password).await.map(Self::Mysql),
            Engine::Postgresql => connect_postgres(profile, password)
                .await
                .map(Self::Postgresql),
        }
    }

    pub async fn close(&self) {
        match self {
            Self::Mysql(pool) => pool.close().await,
            Self::Postgresql(pool) => pool.close().await,
        }
    }

    pub async fn execute(
        &self,
        alias: &str,
        profile: &ConnectionProfile,
        sql: &str,
        password: &str,
        write_level: WriteLevel,
    ) -> Result<QueryResult, DatabaseError> {
        let safe_sql = validate_for_level(sql, profile.engine, write_level)
            .map_err(|error| DatabaseError::Policy(error.to_string()))?;
        match self {
            Self::Mysql(pool) => {
                execute_mysql(pool, alias, profile, &safe_sql, password, write_level).await
            }
            Self::Postgresql(pool) => {
                execute_postgres(pool, alias, profile, &safe_sql, password, write_level).await
            }
        }
    }

    pub async fn schema_snapshot(
        &self,
        profile: &ConnectionProfile,
        password: &str,
    ) -> Result<Vec<SchemaObject>, DatabaseError> {
        match self {
            Self::Mysql(pool) => mysql_schema_snapshot(pool, profile, password).await,
            Self::Postgresql(pool) => postgres_schema_snapshot(pool, profile, password).await,
        }
    }
}

async fn mysql_schema_snapshot(
    pool: &MySqlPool,
    profile: &ConnectionProfile,
    password: &str,
) -> Result<Vec<SchemaObject>, DatabaseError> {
    let rows = sqlx::query(
        "SELECT table_schema, table_name, column_name, ordinal_position,
                column_type, is_nullable, column_default, extra
         FROM information_schema.columns
         WHERE (? IS NULL OR table_schema = ?)
           AND table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')
         ORDER BY table_schema, table_name, ordinal_position",
    )
    .bind(profile.database.as_deref())
    .bind(profile.database.as_deref())
    .fetch_all(pool)
    .await
    .map_err(|error| {
        DatabaseError::Query(redact_connection_error(
            error.to_string(),
            profile,
            password,
        ))
    })?;
    let refreshed_at = Utc::now().timestamp();
    rows.into_iter()
        .map(|row| {
            let schema = named_text(&row, "table_schema").ok_or_else(|| {
                DatabaseError::Query("schema snapshot lost the table_schema column".to_owned())
            })?;
            let table = named_text(&row, "table_name").ok_or_else(|| {
                DatabaseError::Query("schema snapshot lost the table_name column".to_owned())
            })?;
            let column = named_text(&row, "column_name").ok_or_else(|| {
                DatabaseError::Query("schema snapshot lost the column_name column".to_owned())
            })?;
            Ok(SchemaObject {
                database: schema.clone(),
                schema,
                object_type: "column".to_owned(),
                object_name: format!("{table}.{column}"),
                metadata: serde_json::json!({
                    "table": table,
                    "column": column,
                    "ordinal_position": mysql_named_unsigned(&row, "ordinal_position"),
                    "data_type": named_text(&row, "column_type"),
                    "nullable": named_text(&row, "is_nullable").as_deref() == Some("YES"),
                    "default": named_text(&row, "column_default"),
                    "extra": named_text(&row, "extra"),
                }),
                refreshed_at,
            })
        })
        .collect()
}

async fn postgres_schema_snapshot(
    pool: &PgPool,
    profile: &ConnectionProfile,
    password: &str,
) -> Result<Vec<SchemaObject>, DatabaseError> {
    let rows = sqlx::query(
        "SELECT table_catalog, table_schema, table_name, column_name,
                ordinal_position, data_type, is_nullable, column_default
         FROM information_schema.columns
         WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
         ORDER BY table_schema, table_name, ordinal_position",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| {
        DatabaseError::Query(redact_connection_error(
            error.to_string(),
            profile,
            password,
        ))
    })?;
    let refreshed_at = Utc::now().timestamp();
    rows.into_iter()
        .map(|row| {
            let database = named_text(&row, "table_catalog")
                .unwrap_or_else(|| profile.database.clone().unwrap_or_default());
            let schema = named_text(&row, "table_schema").ok_or_else(|| {
                DatabaseError::Query("schema snapshot lost the table_schema column".to_owned())
            })?;
            let table = named_text(&row, "table_name").ok_or_else(|| {
                DatabaseError::Query("schema snapshot lost the table_name column".to_owned())
            })?;
            let column = named_text(&row, "column_name").ok_or_else(|| {
                DatabaseError::Query("schema snapshot lost the column_name column".to_owned())
            })?;
            Ok(SchemaObject {
                database,
                schema,
                object_type: "column".to_owned(),
                object_name: format!("{table}.{column}"),
                metadata: serde_json::json!({
                    "table": table,
                    "column": column,
                    "ordinal_position": named_signed(&row, "ordinal_position"),
                    "data_type": named_text(&row, "data_type"),
                    "nullable": named_text(&row, "is_nullable").as_deref() == Some("YES"),
                    "default": named_text(&row, "column_default"),
                }),
                refreshed_at,
            })
        })
        .collect()
}

pub async fn execute_direct(
    alias: &str,
    profile: &ConnectionProfile,
    sql: &str,
) -> Result<QueryResult, DatabaseError> {
    let password = read_password(alias)?;
    let pool = PoolHandle::connect(profile, &password).await?;
    let result = pool
        .execute(alias, profile, sql, &password, WriteLevel::None)
        .await;
    pool.close().await;
    result
}

async fn connect_mysql(
    profile: &ConnectionProfile,
    password: &str,
) -> Result<MySqlPool, DatabaseError> {
    let mut options = MySqlConnectOptions::new()
        .host(&profile.endpoint.host)
        .port(profile.endpoint.port)
        .username(&profile.username)
        .password(password)
        .ssl_mode(mysql_ssl_mode(&profile.ssl.mode));
    if let Some(database) = profile.database.as_deref() {
        options = options.database(database);
    }
    if let Some(path) = profile.ssl.ca_file.as_deref() {
        options = options.ssl_ca(Path::new(path));
    }
    if let Some(path) = profile.ssl.client_cert.as_deref() {
        options = options.ssl_client_cert(Path::new(path));
    }
    if let Some(path) = profile.ssl.client_key.as_deref() {
        options = options.ssl_client_key(Path::new(path));
    }
    MySqlPoolOptions::new()
        .min_connections(0)
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(profile.limits.connect_timeout_seconds))
        .connect_with(options)
        .await
        .map_err(|error| {
            DatabaseError::Connection(redact_connection_error(
                error.to_string(),
                profile,
                password,
            ))
        })
}

async fn connect_postgres(
    profile: &ConnectionProfile,
    password: &str,
) -> Result<PgPool, DatabaseError> {
    let mut options = PgConnectOptions::new()
        .host(&profile.endpoint.host)
        .port(profile.endpoint.port)
        .username(&profile.username)
        .password(password)
        .database(profile.database.as_deref().unwrap_or_default())
        .ssl_mode(postgres_ssl_mode(&profile.ssl.mode));
    if let Some(path) = profile.ssl.ca_file.as_deref() {
        options = options.ssl_root_cert(Path::new(path));
    }
    if let Some(path) = profile.ssl.client_cert.as_deref() {
        options = options.ssl_client_cert(Path::new(path));
    }
    if let Some(path) = profile.ssl.client_key.as_deref() {
        options = options.ssl_client_key(Path::new(path));
    }
    PgPoolOptions::new()
        .min_connections(0)
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(profile.limits.connect_timeout_seconds))
        .connect_with(options)
        .await
        .map_err(|error| {
            DatabaseError::Connection(redact_connection_error(
                error.to_string(),
                profile,
                password,
            ))
        })
}

fn mysql_ssl_mode(mode: &str) -> MySqlSslMode {
    match mode {
        "disable" => MySqlSslMode::Disabled,
        "require" => MySqlSslMode::Required,
        "verify-ca" => MySqlSslMode::VerifyCa,
        "verify-full" => MySqlSslMode::VerifyIdentity,
        _ => MySqlSslMode::Preferred,
    }
}

fn postgres_ssl_mode(mode: &str) -> PgSslMode {
    match mode {
        "disable" => PgSslMode::Disable,
        "require" => PgSslMode::Require,
        "verify-ca" => PgSslMode::VerifyCa,
        "verify-full" => PgSslMode::VerifyFull,
        _ => PgSslMode::Prefer,
    }
}

async fn execute_mysql(
    pool: &MySqlPool,
    alias: &str,
    profile: &ConnectionProfile,
    sql: &str,
    password: &str,
    write_level: WriteLevel,
) -> Result<QueryResult, DatabaseError> {
    let mut connection = pool.acquire().await.map_err(|error| {
        DatabaseError::Connection(redact_connection_error(
            error.to_string(),
            profile,
            password,
        ))
    })?;
    let timeout_ms = profile.limits.statement_timeout_seconds * 1000;
    let timeout_statement = format!("SET SESSION MAX_EXECUTION_TIME = {timeout_ms}");
    (&mut *connection)
        .execute(timeout_statement.as_str())
        .await
        .map_err(|error| {
            DatabaseError::Session(redact_connection_error(
                error.to_string(),
                profile,
                password,
            ))
        })?;
    (&mut *connection)
        .execute(if write_level == WriteLevel::None {
            "START TRANSACTION READ ONLY"
        } else {
            "START TRANSACTION"
        })
        .await
        .map_err(|error| {
            DatabaseError::Session(redact_connection_error(
                error.to_string(),
                profile,
                password,
            ))
        })?;

    let result = fetch_mysql(&mut connection, alias, profile, sql)
        .await
        .map_err(|error| {
            DatabaseError::Query(redact_connection_error(
                error.to_string(),
                profile,
                password,
            ))
        })
        .map(|mut result| {
            result.read_only = write_level == WriteLevel::None;
            result
        });
    let end_transaction = if write_level == WriteLevel::None || result.is_err() {
        "ROLLBACK"
    } else {
        "COMMIT"
    };
    if let Err(error) = (&mut *connection).execute(end_transaction).await {
        return Err(DatabaseError::Session(redact_connection_error(
            error.to_string(),
            profile,
            password,
        )));
    }
    result
}

async fn execute_postgres(
    pool: &PgPool,
    alias: &str,
    profile: &ConnectionProfile,
    sql: &str,
    password: &str,
    write_level: WriteLevel,
) -> Result<QueryResult, DatabaseError> {
    let mut connection = pool.acquire().await.map_err(|error| {
        DatabaseError::Connection(redact_connection_error(
            error.to_string(),
            profile,
            password,
        ))
    })?;
    (&mut *connection)
        .execute(if write_level == WriteLevel::None {
            "BEGIN READ ONLY"
        } else {
            "BEGIN"
        })
        .await
        .map_err(|error| {
            DatabaseError::Session(redact_connection_error(
                error.to_string(),
                profile,
                password,
            ))
        })?;
    let timeout_ms = profile.limits.statement_timeout_seconds * 1000;
    let timeout_statement = format!("SET LOCAL statement_timeout = {timeout_ms}");
    (&mut *connection)
        .execute(timeout_statement.as_str())
        .await
        .map_err(|error| {
            DatabaseError::Session(redact_connection_error(
                error.to_string(),
                profile,
                password,
            ))
        })?;

    let result = fetch_postgres(&mut connection, alias, profile, sql)
        .await
        .map_err(|error| {
            DatabaseError::Query(redact_connection_error(
                error.to_string(),
                profile,
                password,
            ))
        })
        .map(|mut result| {
            result.read_only = write_level == WriteLevel::None;
            result
        });
    let end_transaction = if write_level == WriteLevel::None || result.is_err() {
        "ROLLBACK"
    } else {
        "COMMIT"
    };
    if let Err(error) = (&mut *connection).execute(end_transaction).await {
        return Err(DatabaseError::Session(redact_connection_error(
            error.to_string(),
            profile,
            password,
        )));
    }
    result
}

async fn fetch_mysql(
    connection: &mut sqlx::pool::PoolConnection<sqlx::MySql>,
    alias: &str,
    profile: &ConnectionProfile,
    sql: &str,
) -> Result<QueryResult, sqlx::Error> {
    let description = (&mut **connection).describe(sql).await?;
    let columns = description
        .columns()
        .iter()
        .map(|column| column.name().to_owned())
        .collect();
    let mut stream = sqlx::query(sql).fetch(&mut **connection);
    let mut rows = Vec::new();
    let mut truncated = false;
    while let Some(row) = stream.try_next().await? {
        if rows.len() == profile.limits.max_rows {
            truncated = true;
            break;
        }
        rows.push(mysql_row_to_json(&row));
    }
    Ok(query_result(alias, profile, columns, rows, truncated))
}

async fn fetch_postgres(
    connection: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    alias: &str,
    profile: &ConnectionProfile,
    sql: &str,
) -> Result<QueryResult, sqlx::Error> {
    let description = (&mut **connection).describe(sql).await?;
    let columns = description
        .columns()
        .iter()
        .map(|column| column.name().to_owned())
        .collect();
    let mut stream = sqlx::query(sql).fetch(&mut **connection);
    let mut rows = Vec::new();
    let mut truncated = false;
    while let Some(row) = stream.try_next().await? {
        if rows.len() == profile.limits.max_rows {
            truncated = true;
            break;
        }
        rows.push(postgres_row_to_json(&row));
    }
    Ok(query_result(alias, profile, columns, rows, truncated))
}

fn query_result(
    alias: &str,
    profile: &ConnectionProfile,
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
    truncated: bool,
) -> QueryResult {
    QueryResult {
        connection: alias.to_owned(),
        engine: profile.engine.as_str().to_owned(),
        database: profile.database.clone(),
        row_count: rows.len(),
        columns,
        rows,
        truncated,
        read_only: true,
    }
}

/// Look a column up by name without depending on the server's reported case:
/// MySQL labels information_schema columns in upper case, and sqlx matches names
/// exactly, so a lower-case lookup misses them.
pub fn position_by_column_name<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    name: &str,
) -> Option<usize> {
    candidates
        .into_iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(name))
}

fn column_index<R: Row>(row: &R, name: &str) -> Option<usize> {
    position_by_column_name(row.columns().iter().map(|column| column.name()), name)
}

/// Read a text column by name through its raw bytes. MySQL reports information_schema
/// text columns as BLOB-family types, which do not decode as String through sqlx, so
/// the bytes are read directly and decoded as UTF-8 here.
fn named_text<R>(row: &R, name: &str) -> Option<String>
where
    R: Row,
    for<'r> &'r [u8]: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    usize: sqlx::ColumnIndex<R>,
{
    let bytes = row
        .try_get::<Option<&[u8]>, _>(column_index(row, name)?)
        .ok()
        .flatten()?;
    String::from_utf8(bytes.to_vec()).ok()
}

fn named_signed<R>(row: &R, name: &str) -> Option<i64>
where
    R: Row,
    usize: sqlx::ColumnIndex<R>,
    for<'r> i64: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    row.try_get::<i64, _>(column_index(row, name)?).ok()
}

/// MySQL information_schema counters are declared UNSIGNED, which does not decode as
/// i64; PostgreSQL has no unsigned integer types, so this stays MySQL-only.
fn mysql_named_unsigned(row: &MySqlRow, name: &str) -> Option<i64> {
    let index = column_index(row, name)?;
    row.try_get::<u64, _>(index)
        .ok()
        .map(|value| value as i64)
        .or_else(|| row.try_get::<i64, _>(index).ok())
}

fn mysql_row_to_json(row: &MySqlRow) -> Vec<Value> {
    (0..row.len())
        .map(|index| mysql_value(row, index))
        .collect()
}

fn postgres_row_to_json(row: &PgRow) -> Vec<Value> {
    (0..row.len())
        .map(|index| postgres_value(row, index))
        .collect()
}

fn mysql_value(row: &MySqlRow, index: usize) -> Value {
    if row.try_get_raw(index).is_ok_and(|value| value.is_null()) {
        return Value::Null;
    }
    let type_name = row.columns()[index].type_info().name().to_ascii_uppercase();
    match type_name.as_str() {
        "BOOL" | "BOOLEAN" => json_try::<bool, _>(row, index),
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => row
            .try_get::<i64, _>(index)
            .map(Value::from)
            .or_else(|_| row.try_get::<u64, _>(index).map(Value::from))
            .unwrap_or_else(|_| unsupported(&type_name)),
        "FLOAT" | "DOUBLE" => json_try::<f64, _>(row, index),
        "DECIMAL" | "NUMERIC" => string_try::<Decimal, _>(row, index),
        "DATE" => string_try::<NaiveDate, _>(row, index),
        "TIME" => string_try::<NaiveTime, _>(row, index),
        "DATETIME" | "TIMESTAMP" => string_try::<NaiveDateTime, _>(row, index),
        "JSON" => row
            .try_get::<Json<Value>, _>(index)
            .map(|value| value.0)
            .unwrap_or_else(|_| unsupported(&type_name)),
        name if name.contains("BLOB") || name.contains("BINARY") || name == "BIT" => row
            .try_get::<Vec<u8>, _>(index)
            .map(binary_value)
            .unwrap_or_else(|_| unsupported(&type_name)),
        _ => row
            .try_get::<String, _>(index)
            .map(Value::String)
            .or_else(|_| row.try_get::<Vec<u8>, _>(index).map(binary_value))
            .unwrap_or_else(|_| unsupported(&type_name)),
    }
}

fn postgres_value(row: &PgRow, index: usize) -> Value {
    if row.try_get_raw(index).is_ok_and(|value| value.is_null()) {
        return Value::Null;
    }
    let type_name = row.columns()[index].type_info().name().to_ascii_uppercase();
    match type_name.as_str() {
        "BOOL" => json_try::<bool, _>(row, index),
        "INT2" => row
            .try_get::<i16, _>(index)
            .map(Value::from)
            .unwrap_or_else(|_| unsupported(&type_name)),
        "INT4" => row
            .try_get::<i32, _>(index)
            .map(Value::from)
            .unwrap_or_else(|_| unsupported(&type_name)),
        "INT8" => json_try::<i64, _>(row, index),
        "FLOAT4" => row
            .try_get::<f32, _>(index)
            .map(|value| Value::from(value as f64))
            .unwrap_or_else(|_| unsupported(&type_name)),
        "FLOAT8" => json_try::<f64, _>(row, index),
        "NUMERIC" => string_try::<Decimal, _>(row, index),
        "DATE" => string_try::<NaiveDate, _>(row, index),
        "TIME" => string_try::<NaiveTime, _>(row, index),
        "TIMESTAMP" => string_try::<NaiveDateTime, _>(row, index),
        "TIMESTAMPTZ" => string_try::<DateTime<Utc>, _>(row, index),
        "JSON" | "JSONB" => row
            .try_get::<Json<Value>, _>(index)
            .map(|value| value.0)
            .unwrap_or_else(|_| unsupported(&type_name)),
        "UUID" => string_try::<Uuid, _>(row, index),
        "BYTEA" => row
            .try_get::<Vec<u8>, _>(index)
            .map(binary_value)
            .unwrap_or_else(|_| unsupported(&type_name)),
        _ => row
            .try_get::<String, _>(index)
            .map(Value::String)
            .unwrap_or_else(|_| unsupported(&type_name)),
    }
}

fn json_try<T, R>(row: &R, index: usize) -> Value
where
    R: Row,
    usize: sqlx::ColumnIndex<R>,
    for<'r> T: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database> + Into<Value>,
{
    row.try_get::<T, _>(index)
        .map(Into::into)
        .unwrap_or_else(|_| unsupported(row.columns()[index].type_info().name()))
}

fn string_try<T, R>(row: &R, index: usize) -> Value
where
    R: Row,
    usize: sqlx::ColumnIndex<R>,
    for<'r> T: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database> + ToString,
{
    row.try_get::<T, _>(index)
        .map(|value| Value::String(value.to_string()))
        .unwrap_or_else(|_| unsupported(row.columns()[index].type_info().name()))
}

fn binary_value(bytes: Vec<u8>) -> Value {
    match String::from_utf8(bytes.clone()) {
        Ok(text) => Value::String(text),
        Err(_) => Value::String(format!(
            "base64:{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )),
    }
}

fn unsupported(type_name: &str) -> Value {
    Value::String(format!("<unsupported:{type_name}>"))
}

const REDACTED_PLACEHOLDER: &str = "<redacted>";

/// Replace `value` only where it appears as a whole word, so short values such as
/// a one-letter user name or a port number never corrupt unrelated error text
/// (for example turning "timed out" into "timed o<redacted>t").
fn replace_whole_word(message: &str, value: &str) -> String {
    if value.is_empty() {
        return message.to_owned();
    }
    let is_word_byte =
        |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80;
    let bytes = message.as_bytes();
    let mut output = String::with_capacity(message.len());
    let mut copied = 0usize;
    let mut searched = 0usize;
    while let Some(found) = message[searched..].find(value) {
        let start = searched + found;
        let end = start + value.len();
        let bounded_before = start == 0 || !is_word_byte(bytes[start - 1]);
        let bounded_after = end == bytes.len() || !is_word_byte(bytes[end]);
        if bounded_before && bounded_after {
            output.push_str(&message[copied..start]);
            output.push_str(REDACTED_PLACEHOLDER);
            copied = end;
        }
        searched = end;
    }
    output.push_str(&message[copied..]);
    output
}

pub fn redact_connection_error(
    message: impl Into<String>,
    profile: &ConnectionProfile,
    password: &str,
) -> String {
    let mut redacted = message.into();
    // Secrets, vault references, and private key paths are highly specific
    // strings: remove every occurrence unconditionally.
    for value in [
        password,
        profile.secret_ref.as_str(),
        profile.ssl.ca_file.as_deref().unwrap_or_default(),
        profile.ssl.client_cert.as_deref().unwrap_or_default(),
        profile.ssl.client_key.as_deref().unwrap_or_default(),
    ] {
        if !value.is_empty() {
            redacted = redacted.replace(value, REDACTED_PLACEHOLDER);
        }
    }
    // Domain-style hosts and the host:port pair stay specific enough for
    // unconditional replacement; bare host names, user names, and ports are
    // common words and must only be redacted as whole words.
    let endpoint = format!("{}:{}", profile.endpoint.host, profile.endpoint.port);
    if !endpoint.is_empty() {
        redacted = redacted.replace(&endpoint, REDACTED_PLACEHOLDER);
    }
    let host = profile.endpoint.host.as_str();
    if host.contains('.') {
        redacted = redacted.replace(host, REDACTED_PLACEHOLDER);
    } else if !host.is_empty() {
        redacted = replace_whole_word(&redacted, host);
    }
    if !profile.username.is_empty() {
        redacted = replace_whole_word(&redacted, profile.username.as_str());
    }
    let port = profile.endpoint.port.to_string();
    if !port.is_empty() {
        redacted = replace_whole_word(&redacted, &port);
    }
    redacted
}
