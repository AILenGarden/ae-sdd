use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaObject {
    pub database: String,
    pub schema: String,
    pub object_type: String,
    pub object_name: String,
    pub metadata: Value,
    pub refreshed_at: i64,
}

#[derive(Debug, Clone)]
pub struct SchemaCache {
    path: PathBuf,
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cannot create schema cache directory: {0}")]
    CreateDirectory(#[from] std::io::Error),
    #[error("schema cache error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("schema cache metadata is invalid: {0}")]
    Json(#[from] serde_json::Error),
}

impl SchemaCache {
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let cache = Self {
            path: path.to_owned(),
        };
        cache.initialize()?;
        Ok(cache)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn remove_connection(&self, alias: &str) -> Result<(), CacheError> {
        let connection = self.connect()?;
        connection.execute_batch("PRAGMA secure_delete = ON;")?;
        connection.execute("DELETE FROM schema_objects WHERE alias = ?1", [alias])?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    pub fn replace_snapshot(
        &self,
        alias: &str,
        fingerprint: &str,
        objects: &[SchemaObject],
    ) -> Result<(), CacheError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM schema_objects WHERE alias = ?1", [alias])?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO schema_objects (
                    alias, fingerprint, database_name, schema_name,
                    object_type, object_name, metadata_json, refreshed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for object in objects {
                statement.execute(params![
                    alias,
                    fingerprint,
                    object.database,
                    object.schema,
                    object.object_type,
                    object.object_name,
                    serde_json::to_string(&object.metadata)?,
                    object.refreshed_at,
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn get_snapshot(
        &self,
        alias: &str,
        fingerprint: &str,
    ) -> Result<Vec<SchemaObject>, CacheError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT database_name, schema_name, object_type, object_name,
                    metadata_json, refreshed_at
             FROM schema_objects
             WHERE alias = ?1 AND fingerprint = ?2
             ORDER BY database_name, schema_name, object_type, object_name",
        )?;
        let rows = statement.query_map(params![alias, fingerprint], |row| {
            let metadata_json: String = row.get(4)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                metadata_json,
                row.get::<_, i64>(5)?,
            ))
        })?;
        let mut objects = Vec::new();
        for row in rows {
            let (database, schema, object_type, object_name, metadata_json, refreshed_at) = row?;
            objects.push(SchemaObject {
                database,
                schema,
                object_type,
                object_name,
                metadata: serde_json::from_str(&metadata_json)?,
                refreshed_at,
            });
        }
        Ok(objects)
    }

    fn initialize(&self) -> Result<(), CacheError> {
        let connection = self.connect()?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS schema_objects (
                 alias TEXT NOT NULL,
                 fingerprint TEXT NOT NULL,
                 database_name TEXT NOT NULL,
                 schema_name TEXT NOT NULL,
                 object_type TEXT NOT NULL,
                 object_name TEXT NOT NULL,
                 metadata_json TEXT NOT NULL,
                 refreshed_at INTEGER NOT NULL,
                 PRIMARY KEY (
                     alias, fingerprint, database_name, schema_name,
                     object_type, object_name
                 )
             );",
        )?;
        Ok(())
    }

    fn connect(&self) -> Result<Connection, rusqlite::Error> {
        Connection::open(&self.path)
    }
}
