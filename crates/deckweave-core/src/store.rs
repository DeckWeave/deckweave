use crate::{Error, Result, model::Configuration};
use rusqlite::{Connection, params};
use std::{path::Path, time::Duration};

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        Self::initialize(connection)
    }

    pub fn memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(mut connection: Connection) -> Result<Self> {
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
                let initial = Configuration::default();
                let json = serde_json::to_string(&initial)?;
                let tx = connection.transaction()?;
                tx.execute_batch("CREATE TABLE configuration (id INTEGER PRIMARY KEY CHECK (id=1), revision INTEGER NOT NULL, json TEXT NOT NULL); PRAGMA user_version=1;")?;
                tx.execute(
                    "INSERT INTO configuration (id, revision, json) VALUES (1, 0, ?1)",
                    [json],
                )?;
                tx.commit()?;
            }
            1 => (),
            _ => {
                return Err(Error::new(
                    "schema",
                    format!("Unsupported database version {version}; file was not reset"),
                ));
            }
        }
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        Ok(Self { connection })
    }

    pub fn load(&self) -> Result<Configuration> {
        let (revision, json): (i64, String) = self.connection.query_row(
            "SELECT revision, json FROM configuration WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let config: Configuration = serde_json::from_str(&json)?;
        if i64::try_from(config.revision).ok() != Some(revision) {
            return Err(Error::new("storage", "Stored revisions disagree"));
        }
        Ok(config)
    }

    pub fn save(&mut self, config: &Configuration, expected_revision: u64) -> Result<()> {
        let json = serde_json::to_string(config)?;
        let revision =
            i64::try_from(config.revision).map_err(|_| Error::new("limit", "Revision overflow"))?;
        let expected = i64::try_from(expected_revision)
            .map_err(|_| Error::new("limit", "Revision overflow"))?;
        let tx = self.connection.transaction()?;
        let count = tx.execute(
            "UPDATE configuration SET revision=?1, json=?2 WHERE id=1 AND revision=?3",
            params![revision, json, expected],
        )?;
        if count != 1 {
            return Err(Error::new(
                "conflict",
                "Configuration changed in another writer",
            ));
        }
        tx.commit()?;
        Ok(())
    }
}
