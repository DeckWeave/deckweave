use crate::{Error, Result, model::Configuration};
use rusqlite::{Connection, params};
use std::{path::Path, time::Duration};

pub struct Store {
    connection: Connection,
    raw_json: serde_json::Value,
    typed_json: serde_json::Value,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::initialize(Connection::open(path)?, Some(path))
    }
    pub fn memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?, None)
    }
    fn initialize(mut connection: Connection, path: Option<&Path>) -> Result<Self> {
        connection.busy_timeout(Duration::from_secs(2))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                let tables: i64 = connection.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                    [],
                    |r| r.get(0),
                )?;
                if tables != 0 {
                    return Err(Error::new(
                        "schema",
                        "Database is not an empty DeckWeave database",
                    ));
                }
                let json = serde_json::to_string(&Configuration::default())?;
                let tx = connection.transaction()?;
                tx.execute_batch("CREATE TABLE configuration (id INTEGER PRIMARY KEY CHECK (id=1), revision INTEGER NOT NULL, json TEXT NOT NULL); PRAGMA user_version=3;")?;
                tx.execute(
                    "INSERT INTO configuration(id,revision,json) VALUES(1,0,?1)",
                    [json],
                )?;
                install_guard(&tx)?;
                tx.commit()?;
            }
            1 | 2 => {
                let (revision, json): (i64, String) = connection.query_row(
                    "SELECT revision,json FROM configuration WHERE id=1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                let mut value: serde_json::Value = serde_json::from_str(&json)?;
                let object = value
                    .as_object_mut()
                    .ok_or_else(|| Error::new("schema", "Configuration must be an object"))?;
                let missing = object.get("format_version").is_none();
                if !missing && object.get("format_version").and_then(|v| v.as_u64()) != Some(2) {
                    return Err(Error::new(
                        "schema",
                        "Unsupported configuration format; file was not reset",
                    ));
                }
                let mut config: Configuration = serde_json::from_value(value.clone())?;
                if i64::try_from(config.revision).ok() != Some(revision) {
                    return Err(Error::new("storage", "Stored revisions disagree"));
                }
                if let Some(path) = path {
                    backup(&connection, path, revision, "pre-v3")?;
                }
                let tx = connection
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let current: (i64, String) = tx.query_row(
                    "SELECT revision,json FROM configuration WHERE id=1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                if current != (revision, json) {
                    return Err(Error::new(
                        "conflict",
                        "Configuration changed during migration backup; retry opening",
                    ));
                }
                let typed_before = serde_json::to_value(&config)?;
                config.format_version = 3;
                crate::keyboard_aliases::migrate_legacy(&mut config);
                value = preserve_unknown(&value, &typed_before, serde_json::to_value(config)?);
                // Drop the previous gate only inside the atomic migration transaction.
                tx.execute_batch("DROP TRIGGER IF EXISTS configuration_v2_guard;")?;
                tx.execute(
                    "UPDATE configuration SET json=?1 WHERE id=1",
                    [serde_json::to_string(&value)?],
                )?;
                install_guard(&tx)?;
                tx.pragma_update(None, "user_version", 3)?;
                tx.commit()?;
            }
            3 => {
                let guarded: i64 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name='configuration_v3_guard'", [], |r| r.get(0))?;
                if guarded != 1 {
                    return Err(Error::new(
                        "schema",
                        "Format 3 writer guard is missing; file was not reset",
                    ));
                }
            }
            _ => {
                return Err(Error::new(
                    "schema",
                    format!("Unsupported database version {version}; file was not reset"),
                ));
            }
        }
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        let raw: String =
            connection.query_row("SELECT json FROM configuration WHERE id=1", [], |r| {
                r.get(0)
            })?;
        let raw_json: serde_json::Value = serde_json::from_str(&raw)?;
        let config: Configuration = serde_json::from_value(raw_json.clone())?;
        if config.format_version != 3 {
            return Err(Error::new("schema", "Unsupported configuration format"));
        }
        let typed_json = serde_json::to_value(config)?;
        Ok(Self {
            connection,
            raw_json,
            typed_json,
        })
    }
    pub fn load(&self) -> Result<Configuration> {
        let (revision, json): (i64, String) = self.connection.query_row(
            "SELECT revision,json FROM configuration WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let config: Configuration = serde_json::from_str(&json)?;
        if config.format_version != 3 {
            return Err(Error::new("schema", "Unsupported configuration format"));
        }
        if i64::try_from(config.revision).ok() != Some(revision) {
            return Err(Error::new("storage", "Stored revisions disagree"));
        }
        Ok(config)
    }
    pub fn save(&mut self, config: &Configuration, expected_revision: u64) -> Result<()> {
        if config.format_version != 3 {
            return Err(Error::new("schema", "Unsupported configuration format"));
        }
        let typed = serde_json::to_value(config)?;
        let merged = preserve_unknown(&self.raw_json, &self.typed_json, typed.clone());
        let json = serde_json::to_string(&merged)?;
        let revision =
            i64::try_from(config.revision).map_err(|_| Error::new("limit", "Revision overflow"))?;
        let expected = i64::try_from(expected_revision)
            .map_err(|_| Error::new("limit", "Revision overflow"))?;
        let tx = self.connection.transaction()?;
        let count = tx.execute(
            "UPDATE configuration SET revision=?1,json=?2 WHERE id=1 AND revision=?3",
            params![revision, json, expected],
        )?;
        if count != 1 {
            return Err(Error::new(
                "conflict",
                "Configuration changed in another writer",
            ));
        }
        tx.commit()?;
        self.raw_json = merged;
        self.typed_json = typed;
        Ok(())
    }
}
fn install_guard(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute_batch("CREATE TRIGGER IF NOT EXISTS configuration_v3_guard BEFORE UPDATE ON configuration WHEN json_valid(NEW.json)=0 OR json_extract(NEW.json,'$.format_version') IS NOT 3 BEGIN SELECT RAISE(ABORT,'DeckWeave format 3 requires a compatible writer'); END;")?;
    Ok(())
}

/// Retain unrecognized fields without resurrecting deleted typed records. Arrays of entities
/// match by stable ID, never by their position after a profile/device reorder.
fn preserve_unknown(
    raw: &serde_json::Value,
    before: &serde_json::Value,
    mut after: serde_json::Value,
) -> serde_json::Value {
    match (raw, before, &mut after) {
        (
            serde_json::Value::Object(raw),
            serde_json::Value::Object(before),
            serde_json::Value::Object(after),
        ) => {
            for (key, value) in raw {
                if let Some(next) = after.get_mut(key) {
                    if let Some(previous) = before.get(key) {
                        *next = preserve_unknown(value, previous, next.take());
                    }
                } else if !before.contains_key(key) {
                    after.insert(key.clone(), value.clone());
                }
            }
        }
        (
            serde_json::Value::Array(raw),
            serde_json::Value::Array(before),
            serde_json::Value::Array(after),
        ) => {
            for next in after {
                let Some(id) = next
                    .get("id")
                    .or_else(|| next.get("installation_id"))
                    .or_else(|| next.get("operation_id"))
                else {
                    continue;
                };
                let key = if next.get("id").is_some() {
                    "id"
                } else if next.get("installation_id").is_some() {
                    "installation_id"
                } else {
                    "operation_id"
                };
                if let (Some(previous), Some(original)) = (
                    before.iter().find(|v| v.get(key) == Some(id)),
                    raw.iter().find(|v| v.get(key) == Some(id)),
                ) {
                    *next = preserve_unknown(original, previous, next.take());
                }
            }
        }
        _ => {}
    }
    after
}
fn backup(connection: &Connection, path: &Path, revision: i64, label: &str) -> Result<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| Error::new("storage", e.to_string()))?
        .as_nanos();
    let backup = path.with_extension(format!("{label}-r{revision}-{stamp}.sqlite3"));
    connection.execute("VACUUM main INTO ?1", [backup.to_string_lossy().as_ref()])?;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(backup)
        .and_then(|f| f.sync_all())
        .map_err(|e| {
            Error::new(
                "storage",
                format!("Could not make migration backup durable: {e}"),
            )
        })?;
    Ok(())
}
