//! Host-owned removal intent. It survives package deletion and applies to every scan root.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf, sync::Mutex};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Record {
    #[serde(default = "action_kind")]
    pub kind: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub phase: String,
    pub error: Option<String>,
}
fn action_kind() -> String {
    "action".into()
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Event {
    pub id: String,
    pub phase: String,
    pub error: Option<String>,
}
#[derive(Clone, Default, Deserialize, Serialize)]
struct Data {
    removed: BTreeMap<String, Record>,
    history: Vec<Event>,
}
pub struct Store {
    file: PathBuf,
    data: Mutex<Result<Data, String>>,
}
impl Store {
    pub fn open(file: PathBuf) -> Self {
        let data = match fs::read(&file) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| format!("Removal state unreadable: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Data::default()),
            Err(e) => Err(format!("Removal state unavailable: {e}")),
        };
        Self {
            file,
            data: Mutex::new(data),
        }
    }
    pub fn allows(&self, id: &str) -> bool {
        self.data
            .lock()
            .is_ok_and(|data| data.as_ref().is_ok_and(|d| !d.removed.contains_key(id)))
    }
    pub fn records(&self) -> Result<Vec<Record>, String> {
        let data = self.data.lock().map_err(|_| "Removal state unavailable")?;
        Ok(data
            .as_ref()
            .map_err(Clone::clone)?
            .removed
            .values()
            .cloned()
            .collect())
    }
    pub fn history(&self) -> Result<Vec<Event>, String> {
        let data = self.data.lock().map_err(|_| "Removal state unavailable")?;
        Ok(data.as_ref().map_err(Clone::clone)?.history.clone())
    }
    pub fn block(&self, record: Record) -> Result<(), String> {
        self.change(|data| {
            data.history.push(Event {
                id: record.id.clone(),
                phase: record.phase.clone(),
                error: record.error.clone(),
            });
            let key = if record.kind == "device" {
                format!("device:{}", record.id)
            } else {
                record.id.clone()
            };
            data.removed.insert(key, record);
        })
    }
    pub fn reconnect(&self, id: &str) -> Result<(), String> {
        self.change(|data| {
            data.removed.remove(id);
            data.history.push(Event {
                id: id.into(),
                phase: "reconnected".into(),
                error: None,
            });
        })
    }
    fn change(&self, change: impl FnOnce(&mut Data)) -> Result<(), String> {
        let mut guard = self.data.lock().map_err(|_| "Removal state unavailable")?;
        let mut next = guard.as_ref().map_err(Clone::clone)?.clone();
        change(&mut next);
        if next.history.len() > 128 {
            next.history.drain(..next.history.len() - 128);
        }
        let parent = self.file.parent().ok_or("Invalid removal state path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec(&next).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(&self.file).map_err(|e| e.to_string())?;
        *guard = Ok(next);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> Record {
        Record {
            kind: "action".into(),
            id: "test.plugin".into(),
            name: "Test".into(),
            version: "1".into(),
            source: "managed".into(),
            phase: "stopping".into(),
            error: None,
        }
    }
    #[test]
    fn removal_survives_restart_and_only_explicit_reconnect_clears_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lifecycle.json");
        let store = Store::open(path.clone());
        store.block(record()).unwrap();
        drop(store);
        let store = Store::open(path.clone());
        assert!(!store.allows("test.plugin"));
        assert!(store.allows("test.other"));
        assert_eq!(store.records().unwrap()[0].phase, "stopping");
        store.reconnect("test.plugin").unwrap();
        assert!(Store::open(path).allows("test.plugin"));
    }
    #[test]
    fn action_and_device_removal_intent_are_independent_even_for_same_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("lifecycle.json"));
        let mut device = record();
        device.kind = "device".into();
        store.block(device).unwrap();
        assert!(store.allows("test.plugin"));
        assert!(!store.allows("device:test.plugin"));
        store.block(record()).unwrap();
        store.reconnect("test.plugin").unwrap();
        assert!(!store.allows("device:test.plugin"));
        store.reconnect("device:test.plugin").unwrap();
        assert!(store.allows("device:test.plugin"));
    }
    #[test]
    fn corrupt_state_fails_closed_without_overwriting_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lifecycle.json");
        fs::write(&path, b"invalid").unwrap();
        let store = Store::open(path.clone());
        assert!(!store.allows("any.plugin"));
        assert!(store.reconnect("any.plugin").is_err());
        assert_eq!(fs::read(path).unwrap(), b"invalid");
    }
}
