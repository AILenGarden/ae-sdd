use std::path::PathBuf;
use std::time::Duration;

use ae_sdd_runtime::RuntimeResult;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::{Map, Value, json};

use super::common::{
    JobContext, MAX_FILE_BYTES, bounded_u64, digest, read_bounded, read_json, required_string,
    schema_error,
};

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    match entrypoint {
        "db.profiles" => profiles(context, arguments),
        "db.audit" => audit(context),
        "db.query" => query(context, arguments, false),
        "db.explain" => query(context, arguments, true),
        _ => unreachable!("database entrypoint was classified by caller"),
    }
}

#[derive(Clone)]
struct Profile {
    name: String,
    driver: String,
    database: Option<String>,
    host: Option<String>,
    port: Option<String>,
    schema: Option<String>,
    readonly: bool,
}

impl Profile {
    fn safe_value(&self) -> Value {
        json!({
            "name":self.name,
            "driver":self.driver,
            "database":self.database,
            "host":self.host,
            "port":self.port,
            "schema":self.schema,
            "readonly":self.readonly,
            "secrets":"redacted",
        })
    }
}

fn profiles(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    if arguments.get("init").and_then(Value::as_bool) == Some(true) {
        return super::common::mutation_rejected(context, "db.profiles --init");
    }
    let (path, values) = load_profiles(context)?;
    Ok(json!({
        "outcome":"PASS",
        "profilePath":path,
        "profiles":values.iter().map(Profile::safe_value).collect::<Vec<_>>(),
    }))
}

fn audit(context: &JobContext<'_>) -> RuntimeResult<Value> {
    let (path, profiles) = load_profiles(context)?;
    let mut issues = Vec::new();
    for profile in &profiles {
        if profile.driver != "sqlite" {
            issues.push(json!({
                "profile":profile.name,
                "code":"DRIVER_UNSUPPORTED",
            }));
            continue;
        }
        if !profile.readonly {
            issues.push(json!({
                "profile":profile.name,
                "code":"READONLY_NOT_DECLARED",
            }));
        }
        match profile.database.as_deref() {
            Some(database) if context.existing_file(database).is_ok() => {}
            Some(_) => issues.push(json!({
                "profile":profile.name,
                "code":"DATABASE_OUTSIDE_WORKSPACE_OR_MISSING",
            })),
            None => issues.push(json!({
                "profile":profile.name,
                "code":"DATABASE_MISSING",
            })),
        }
    }
    Ok(json!({
        "outcome":if issues.is_empty() {"PASS"} else {"FAIL"},
        "profilePath":path,
        "exists":context.project_file(".ae-sdd/secrets/db-connections.local.json").is_ok(),
        "profiles":profiles.iter().map(Profile::safe_value).collect::<Vec<_>>(),
        "issues":issues,
        "policy":{
            "repoSafe":".ae-sdd/secrets must stay local and ignored",
            "defaultMode":"read-only",
            "writePolicy":"job.submit never permits write SQL",
            "pathPolicy":"SQLite files must remain inside the registered workspace",
        },
    }))
}

fn query(context: &JobContext<'_>, arguments: &Value, explain: bool) -> RuntimeResult<Value> {
    if arguments.get("write").and_then(Value::as_bool) == Some(true) {
        return super::common::mutation_rejected(context, "db.query --write");
    }
    let profile_name = required_string(arguments, "profile")?;
    let (_, profiles) = load_profiles(context)?;
    let profile = profiles
        .iter()
        .find(|profile| profile.name == profile_name)
        .ok_or_else(|| schema_error("database profile does not exist"))?;
    if profile.driver != "sqlite" {
        return Ok(blocked(profile, "configured driver is not supported by the Rust runtime"));
    }
    let database = profile
        .database
        .as_deref()
        .ok_or_else(|| schema_error("SQLite profile has no database path"))?;
    let database = context.existing_file(database)?;
    let mut sql = read_sql(context, arguments)?;
    validate_readonly_sql(&sql)?;
    if explain && !sql.trim_start().to_ascii_lowercase().starts_with("explain") {
        sql = format!("EXPLAIN QUERY PLAN {sql}");
    }
    let limit = bounded_u64(arguments, "limit", 100, 1_000)? as usize;
    let connection = Connection::open_with_flags(
        &database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|_| schema_error("SQLite database could not be opened read-only"))?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|_| schema_error("SQLite busy timeout could not be configured"))?;
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| schema_error("read-only SQL could not be prepared"))?;
    if statement.readonly() == Ok(false) {
        return Err(schema_error("SQLite classified the statement as mutating"));
    }
    let columns = statement
        .column_names()
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    let mut cursor = statement
        .query([])
        .map_err(|_| schema_error("read-only SQL execution failed"))?;
    let mut rows = Vec::new();
    let mut output_bytes = 0_usize;
    while rows.len() < limit {
        let Some(row) = cursor
            .next()
            .map_err(|_| schema_error("SQLite row iteration failed"))?
        else {
            break;
        };
        let mut value = Map::new();
        for (index, name) in columns.iter().enumerate() {
            let cell = sqlite_value(
                row.get_ref(index)
                    .map_err(|_| schema_error("SQLite column decoding failed"))?,
            )?;
            value.insert(name.clone(), cell);
        }
        let row = Value::Object(value);
        output_bytes = output_bytes.saturating_add(
            serde_json::to_vec(&row)
                .map_err(|_| schema_error("SQLite row serialization failed"))?
                .len(),
        );
        if output_bytes > MAX_FILE_BYTES as usize {
            return Err(schema_error("SQLite result exceeds the 1 MiB output bound"));
        }
        rows.push(row);
    }
    Ok(json!({
        "outcome":"PASS",
        "ok":true,
        "blocked":false,
        "profile":profile.safe_value(),
        "sqlClass":{"readonly":true,"hasWrite":false},
        "rowCount":rows.len(),
        "limit":limit,
        "rows":rows,
    }))
}

fn load_profiles(context: &JobContext<'_>) -> RuntimeResult<(String, Vec<Profile>)> {
    const RELATIVE: &str = ".ae-sdd/secrets/db-connections.local.json";
    let path = match context.project_file(RELATIVE) {
        Ok(path) => path,
        Err(_) => return Ok((RELATIVE.to_owned(), Vec::new())),
    };
    let payload = read_json(&path, MAX_FILE_BYTES)?;
    let values = payload
        .get("profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("database profiles file must contain a profiles array"))?;
    if values.len() > 128 {
        return Err(schema_error("database profile count exceeds its bound"));
    }
    let mut profiles = Vec::with_capacity(values.len());
    for value in values {
        let object = value
            .as_object()
            .ok_or_else(|| schema_error("database profile must be an object"))?;
        let name = object_string(object, "name")?;
        if profiles.iter().any(|profile: &Profile| profile.name == name) {
            return Err(schema_error("database profile names must be unique"));
        }
        profiles.push(Profile {
            name,
            driver: object_string(object, "driver")?.to_ascii_lowercase(),
            database: optional_string(object, "database")?,
            host: optional_string(object, "host")?,
            port: object.get("port").map(scalar_string).transpose()?,
            schema: optional_string(object, "schema")?,
            readonly: object.get("readonly").and_then(Value::as_bool).unwrap_or(true),
        });
    }
    Ok((RELATIVE.to_owned(), profiles))
}

fn read_sql(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<String> {
    if let Some(path) = arguments
        .get("sqlFile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let bytes = read_bounded(&context.existing_file(path)?, MAX_FILE_BYTES)?;
        return String::from_utf8(bytes).map_err(|_| schema_error("SQL file must be UTF-8"));
    }
    Ok(required_string(arguments, "sql")?.to_owned())
}

fn validate_readonly_sql(sql: &str) -> RuntimeResult<()> {
    let trimmed = sql.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_FILE_BYTES as usize {
        return Err(schema_error("SQL is empty or exceeds its length bound"));
    }
    let without_trailing = trimmed.strip_suffix(';').unwrap_or(trimmed);
    if without_trailing.contains(';') || without_trailing.contains('\0') {
        return Err(schema_error("only one read-only SQL statement is permitted"));
    }
    let folded = without_trailing.to_ascii_lowercase();
    let first = folded.split_whitespace().next().unwrap_or_default();
    if !matches!(first, "select" | "with" | "explain" | "pragma") {
        return Err(schema_error("only SELECT, WITH, EXPLAIN, or safe PRAGMA is permitted"));
    }
    let mut words = folded
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty());
    if words.any(|word| {
        matches!(
            word,
            "insert"
                | "update"
                | "delete"
                | "merge"
                | "drop"
                | "alter"
                | "create"
                | "truncate"
                | "replace"
                | "grant"
                | "revoke"
                | "attach"
                | "detach"
                | "vacuum"
        )
    }) {
        return Err(schema_error("write-capable SQL token is forbidden"));
    }
    if first == "pragma" {
        let pragma = folded
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .split(['(', '='])
            .next()
            .unwrap_or_default();
        if !matches!(
            pragma,
            "table_info"
                | "table_xinfo"
                | "index_list"
                | "index_info"
                | "index_xinfo"
                | "foreign_key_list"
                | "database_list"
                | "integrity_check"
                | "quick_check"
                | "compile_options"
        ) {
            return Err(schema_error("PRAGMA is not in the read-only allowlist"));
        }
    }
    Ok(())
}

fn sqlite_value(value: ValueRef<'_>) -> RuntimeResult<Value> {
    match value {
        ValueRef::Null => Ok(Value::Null),
        ValueRef::Integer(value) => Ok(json!(value)),
        ValueRef::Real(value) if value.is_finite() => Ok(json!(value)),
        ValueRef::Real(_) => Err(schema_error("SQLite returned a non-finite number")),
        ValueRef::Text(bytes) => std::str::from_utf8(bytes)
            .map(|value| Value::String(value.chars().take(65_536).collect()))
            .map_err(|_| schema_error("SQLite returned non-UTF-8 text")),
        ValueRef::Blob(bytes) => Ok(json!({
            "blobBytes":bytes.len(),
            "digest":digest(bytes),
        })),
    }
}

fn blocked(profile: &Profile, reason: &str) -> Value {
    json!({
        "outcome":"FAIL",
        "ok":false,
        "blocked":true,
        "reason":reason,
        "profile":profile.safe_value(),
    })
}

fn object_string(object: &Map<String, Value>, name: &str) -> RuntimeResult<String> {
    object
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 4_096)
        .map(str::to_owned)
        .ok_or_else(|| schema_error(&format!("database profile {name} is invalid")))
}

fn optional_string(object: &Map<String, Value>, name: &str) -> RuntimeResult<Option<String>> {
    object
        .get(name)
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| value.len() <= 4_096)
                .map(str::to_owned)
                .ok_or_else(|| schema_error(&format!("database profile {name} is invalid")))
        })
        .transpose()
}

fn scalar_string(value: &Value) -> RuntimeResult<String> {
    match value {
        Value::String(value) if value.len() <= 32 => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        _ => Err(schema_error("database profile port is invalid")),
    }
}
