//! Durable host-owned panels. No hardware, renderer, plugin socket or UI dependency.
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};
use uuid::Uuid;

pub const MAX_OPEN: usize = 8;
pub const MAX_SAVED: usize = 128;
pub mod focus;
pub mod timers;
pub type Result<T> = std::result::Result<T, ServiceError>;
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct ServiceError {
    pub code: String,
    pub message: String,
}
impl ServiceError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl From<rusqlite::Error> for ServiceError {
    fn from(e: rusqlite::Error) -> Self {
        Self::new("storage", e.to_string())
    }
}
impl From<serde_json::Error> for ServiceError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("record", e.to_string())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Content {
    Note {
        text: String,
    },
    Metric {
        value: String,
        detail: String,
    },
    Timer {
        seconds: u32,
        #[serde(default)]
        sound: Option<String>,
        #[serde(default = "default_volume")]
        volume: u8,
    },
}
fn default_volume() -> u8 {
    35
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Placement {
    pub anchor: String,
    pub monitor: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub background: String,
    pub foreground: String,
    pub opacity: u8,
    pub click_through: bool,
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            anchor: "top_right".into(),
            monitor: None,
            x: 16.,
            y: 16.,
            width: 320.,
            height: 160.,
            background: "#252525".into(),
            foreground: "#ededed".into(),
            opacity: 100,
            click_through: false,
        }
    }
}
impl Placement {
    pub fn validate(&self) -> Result<()> {
        if ![
            "top_left",
            "top_right",
            "bottom_left",
            "bottom_right",
            "free",
        ]
        .contains(&self.anchor.as_str())
            || ![self.x, self.y, self.width, self.height]
                .iter()
                .all(|v| v.is_finite())
            || self.x.abs() > 100_000.
            || self.y.abs() > 100_000.
            || !(160. ..=1280.).contains(&self.width)
            || !(80. ..=960.).contains(&self.height)
            || self.opacity > 100
            || self.monitor.as_ref().is_some_and(|s| s.len() > 256)
            || !color(&self.background)
            || !color(&self.foreground)
        {
            return Err(ServiceError::new(
                "placement",
                "Invalid panel position, size or color",
            ));
        }
        Ok(())
    }
}
fn color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|c| c.is_ascii_hexdigit())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PanelSpec {
    pub title: String,
    pub content: Content,
    #[serde(default)]
    pub placement: Placement,
}
impl PanelSpec {
    pub fn validate(&self) -> Result<()> {
        self.placement.validate()?;
        if self.title.chars().count() > 120 || self.title.contains('\0') {
            return Err(ServiceError::new("content", "Panel title is too long"));
        }
        let valid = match &self.content {
            Content::Note { text } => text.len() <= 8192 && !text.contains('\0'),
            Content::Metric { value, detail } => value.len() <= 256 && detail.len() <= 1024,
            Content::Timer {
                seconds,
                sound,
                volume,
            } => {
                (1..=86400).contains(seconds)
                    && *volume <= 100
                    && sound
                        .as_ref()
                        .is_none_or(|s| ["complete", "notice", "warning"].contains(&s.as_str()))
            }
        };
        if !valid {
            return Err(ServiceError::new(
                "content",
                "Panel content exceeds its limit",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    pub version: u32,
    pub id: String,
    pub service_id: String,
    pub owner: String,
    pub origin: String,
    pub local_key: String,
    pub revision: i64,
    pub open: bool,
    pub spec: PanelSpec,
}
impl Panel {
    fn validate(&self) -> Result<()> {
        if self.version != 1
            || Uuid::parse_str(&self.id).is_err()
            || Uuid::parse_str(&self.service_id).is_err()
            || self.revision <= 0
        {
            return Err(ServiceError::new("record", "Unsupported panel record"));
        }
        identity(&self.owner)?;
        identity(&self.origin)?;
        identity(&self.local_key)?;
        self.spec.validate()
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct InvalidRecord {
    pub id: String,
    pub error: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Inventory {
    pub panels: Vec<Panel>,
    pub unavailable: Vec<InvalidRecord>,
}
pub fn identity(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 160
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(ServiceError::new("identity", "Invalid service identity"));
    }
    Ok(())
}
pub struct Store {
    connection: Connection,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 1 {
            return Err(ServiceError::new(
                "schema",
                "Host service database requires a newer application",
            ));
        }
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE IF NOT EXISTS panels(id TEXT PRIMARY KEY,owner TEXT NOT NULL,origin TEXT NOT NULL,local_key TEXT NOT NULL,revision INTEGER NOT NULL,json TEXT NOT NULL,UNIQUE(owner,origin,local_key)); CREATE TABLE IF NOT EXISTS timer_jobs(panel TEXT PRIMARY KEY,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS focus_jobs(owner TEXT NOT NULL,workspace TEXT NOT NULL,revision INTEGER NOT NULL,json TEXT NOT NULL,PRIMARY KEY(owner,workspace)); PRAGMA user_version=1; COMMIT;")?;
        Ok(Self { connection })
    }
    pub fn inventory(&self) -> Result<Inventory> {
        let mut inventory = Inventory::default();
        let mut statement = self
            .connection
            .prepare("SELECT id,revision,json FROM panels ORDER BY rowid LIMIT 129")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, revision, json) = row?;
            let parsed = (|| {
                if json.len() > 32768 {
                    return Err(ServiceError::new("record", "Record too large"));
                }
                let panel: Panel = serde_json::from_str(&json)?;
                panel.validate()?;
                if panel.id != id || panel.revision != revision {
                    return Err(ServiceError::new(
                        "record",
                        "Record identity or revision differs",
                    ));
                }
                Ok(panel)
            })();
            match parsed {
                Ok(panel) => inventory.panels.push(panel),
                Err(e) => inventory.unavailable.push(InvalidRecord {
                    id,
                    error: e.message,
                }),
            }
        }
        Ok(inventory)
    }
    pub fn get(&self, id: &str) -> Result<Panel> {
        let (revision, json): (i64, String) = self
            .connection
            .query_row("SELECT revision,json FROM panels WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?
            .ok_or_else(|| ServiceError::new("not_found", "Panel not found"))?;
        let panel: Panel = serde_json::from_str(&json)?;
        panel.validate()?;
        if panel.id != id || panel.revision != revision {
            return Err(ServiceError::new(
                "record",
                "Record identity or revision differs",
            ));
        }
        Ok(panel)
    }
    fn save(&self, panel: &Panel, previous: Option<i64>) -> Result<()> {
        panel.validate()?;
        let json = serde_json::to_string(panel)?;
        if let Some(revision) = previous {
            if self.connection.execute(
                "UPDATE panels SET revision=?1,json=?2 WHERE id=?3 AND revision=?4",
                params![panel.revision, json, panel.id, revision],
            )? != 1
            {
                return Err(ServiceError::new(
                    "conflict",
                    "Panel changed; reload before editing",
                ));
            }
        } else {
            self.connection.execute("INSERT INTO panels(id,owner,origin,local_key,revision,json) VALUES(?1,?2,?3,?4,?5,?6)",params![panel.id,panel.owner,panel.origin,panel.local_key,panel.revision,json])?;
        }
        Ok(())
    }
    /// Read this origin's saved panel without opening or changing it.
    pub fn find_panel(&self, owner: &str, origin: &str, key: &str) -> Result<Option<Panel>> {
        identity(owner)?;
        identity(origin)?;
        identity(key)?;
        let id: Option<String> = self
            .connection
            .query_row(
                "SELECT id FROM panels WHERE owner=?1 AND origin=?2 AND local_key=?3",
                params![owner, origin, key],
                |r| r.get(0),
            )
            .optional()?;
        id.map(|id| self.get(&id)).transpose()
    }
    /// Explicit opening never overwrites an existing panel's content or user placement.
    pub fn open_panel(
        &self,
        owner: &str,
        origin: &str,
        key: &str,
        spec: PanelSpec,
    ) -> Result<Panel> {
        identity(owner)?;
        identity(origin)?;
        identity(key)?;
        spec.validate()?;
        let existing: Option<String> = self
            .connection
            .query_row(
                "SELECT id FROM panels WHERE owner=?1 AND origin=?2 AND local_key=?3",
                params![owner, origin, key],
                |r| r.get(0),
            )
            .optional()?;
        let inventory = self.inventory()?;
        if let Some(id) = existing {
            let mut panel = self.get(&id)?;
            if panel.open {
                return Ok(panel);
            }
            if inventory.panels.iter().filter(|p| p.open).count() >= MAX_OPEN {
                return Err(ServiceError::new(
                    "quota",
                    "At most eight panels may be open",
                ));
            }
            let before = panel.revision;
            panel.revision += 1;
            panel.open = true;
            self.save(&panel, Some(before))?;
            return Ok(panel);
        }
        if inventory.panels.len() + inventory.unavailable.len() >= MAX_SAVED
            || inventory.panels.iter().filter(|p| p.open).count() >= MAX_OPEN
        {
            return Err(ServiceError::new(
                "quota",
                "Panel limit reached; close or remove an existing panel",
            ));
        }
        let panel = Panel {
            version: 1,
            id: Uuid::new_v4().to_string(),
            service_id: Uuid::new_v4().to_string(),
            owner: owner.into(),
            origin: origin.into(),
            local_key: key.into(),
            revision: 1,
            open: true,
            spec,
        };
        self.save(&panel, None)?;
        Ok(panel)
    }
    pub fn configure(
        &self,
        owner: &str,
        id: &str,
        revision: i64,
        spec: PanelSpec,
    ) -> Result<Panel> {
        spec.validate()?;
        let mut panel = self.owned(owner, id, revision)?;
        if std::mem::discriminant(&panel.spec.content) != std::mem::discriminant(&spec.content) {
            return Err(ServiceError::new(
                "content",
                "Panel type cannot be changed; create a separate panel",
            ));
        }
        panel.spec = spec;
        panel.revision += 1;
        self.save(&panel, Some(revision))?;
        Ok(panel)
    }
    pub fn set_open(&self, owner: &str, id: &str, revision: i64, open: bool) -> Result<Panel> {
        let mut panel = self.owned(owner, id, revision)?;
        if open
            && !panel.open
            && self.inventory()?.panels.iter().filter(|p| p.open).count() >= MAX_OPEN
        {
            return Err(ServiceError::new(
                "quota",
                "At most eight panels may be open",
            ));
        }
        panel.open = open;
        panel.revision += 1;
        self.save(&panel, Some(revision))?;
        Ok(panel)
    }
    fn owned(&self, owner: &str, id: &str, revision: i64) -> Result<Panel> {
        let panel = self.get(id)?;
        if panel.owner != owner {
            return Err(ServiceError::new(
                "owner",
                "Panel belongs to another plugin",
            ));
        }
        if panel.revision != revision {
            return Err(ServiceError::new(
                "conflict",
                "Panel changed; reload before editing",
            ));
        }
        Ok(panel)
    }
    pub fn remove(&self, owner: &str, id: &str, revision: i64) -> Result<()> {
        self.owned(owner, id, revision)?;
        let transaction = self.connection.unchecked_transaction()?;
        if transaction.execute(
            "DELETE FROM panels WHERE id=?1 AND owner=?2 AND revision=?3",
            params![id, owner, revision],
        )? != 1
        {
            return Err(ServiceError::new("conflict", "Panel changed"));
        }
        transaction.execute("DELETE FROM timer_jobs WHERE panel=?1", [id])?;
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> PanelSpec {
        PanelSpec {
            title: "Note".into(),
            content: Content::Note {
                text: "Saved text".into(),
            },
            placement: Placement::default(),
        }
    }
    #[test]
    fn restart_close_and_copy_keep_independent_identity_and_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("services.sqlite3");
        let store = Store::open(&path).unwrap();
        let a = store
            .open_panel("plugin.a", "binding1", "main", spec())
            .unwrap();
        assert_eq!(
            store
                .open_panel("plugin.a", "binding1", "main", spec())
                .unwrap(),
            a
        );
        let b = store
            .open_panel("plugin.a", "binding2", "main", spec())
            .unwrap();
        assert_ne!(a.service_id, b.service_id);
        store
            .set_open("plugin.a", &a.id, a.revision, false)
            .unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert!(!store.get(&a.id).unwrap().open);
        assert!(
            !store
                .find_panel("plugin.a", "binding1", "main")
                .unwrap()
                .unwrap()
                .open
        );
        assert!(
            store
                .find_panel("plugin.b", "binding1", "main")
                .unwrap()
                .is_none()
        );
        assert!(store.get(&b.id).unwrap().open);
        assert!(store.configure("plugin.b", &a.id, 2, spec()).is_err());
        assert!(store.configure("plugin.a", &a.id, 1, spec()).is_err());
        let changed = PanelSpec {
            title: "Changed creation default".into(),
            ..spec()
        };
        let reopened = store
            .open_panel("plugin.a", "binding1", "main", changed)
            .unwrap();
        assert_eq!(reopened.spec, a.spec);
    }
    #[test]
    fn malformed_records_and_newer_schema_do_not_erase_other_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("services.sqlite3");
        let store = Store::open(&path).unwrap();
        let a = store
            .open_panel("plugin.a", "binding1", "main", spec())
            .unwrap();
        store
            .connection
            .execute(
                "INSERT INTO panels VALUES('broken','a','b','c',1,'broken')",
                [],
            )
            .unwrap();
        let inv = store.inventory().unwrap();
        assert_eq!(inv.panels.len(), 1);
        assert_eq!(inv.unavailable.len(), 1);
        assert_eq!(inv.panels[0].id, a.id);
        store
            .connection
            .pragma_update(None, "user_version", 99)
            .unwrap();
        drop(store);
        assert!(Store::open(&path).is_err());
    }
    #[test]
    fn failed_commit_preserves_open_intent_and_limits_reject_before_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(&tmp.path().join("services.sqlite3")).unwrap();
        let a = store.open_panel("plugin.a", "a", "main", spec()).unwrap();
        store.connection.execute_batch("CREATE TRIGGER fail BEFORE UPDATE ON panels BEGIN SELECT RAISE(FAIL,'disk failure fixture'); END;").unwrap();
        assert!(store.set_open("plugin.a", &a.id, 1, false).is_err());
        assert!(store.get(&a.id).unwrap().open);
        let mut invalid = spec();
        invalid.placement.width = f64::NAN;
        assert!(store.open_panel("plugin.a", "b", "main", invalid).is_err());
        for n in 1..MAX_OPEN {
            store
                .open_panel("plugin.a", &format!("b{n}"), "main", spec())
                .unwrap();
        }
        assert_eq!(
            store
                .open_panel("plugin.a", "overflow", "main", spec())
                .unwrap_err()
                .code,
            "quota"
        );
    }
}
