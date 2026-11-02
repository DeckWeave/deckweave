//! One native countdown authority per panel; rendering never creates a completion.
use super::*;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TimerStatus {
    Idle,
    Running,
    Paused,
    Done,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerRun {
    pub panel: String,
    pub run_id: String,
    pub revision: i64,
    pub status: TimerStatus,
    pub remaining_ms: u64,
    pub due_ms: Option<u64>,
    pub sound: Option<String>,
    pub volume: u8,
}
impl TimerRun {
    pub fn remaining(&self, now: u64) -> u64 {
        if self.status == TimerStatus::Running {
            self.due_ms.unwrap_or(now).saturating_sub(now)
        } else {
            self.remaining_ms
        }
    }
    fn validate(&self) -> Result<()> {
        if Uuid::parse_str(&self.panel).is_err()
            || Uuid::parse_str(&self.run_id).is_err()
            || self.revision <= 0
            || self.remaining_ms > 86_400_000
            || self.volume > 100
            || (self.status == TimerStatus::Running) != self.due_ms.is_some()
            || self
                .sound
                .as_ref()
                .is_some_and(|s| !["complete", "notice", "warning"].contains(&s.as_str()))
        {
            return Err(ServiceError::new("timer", "Invalid saved timer"));
        }
        Ok(())
    }
}
impl Store {
    pub fn timer(&self, panel: &str) -> Result<Option<TimerRun>> {
        let raw: Option<String> = self
            .connection
            .query_row("SELECT json FROM timer_jobs WHERE panel=?1", [panel], |r| {
                r.get(0)
            })
            .optional()?;
        raw.map(|s| {
            let r: TimerRun = serde_json::from_str(&s)?;
            r.validate()?;
            if r.panel != panel {
                return Err(ServiceError::new("timer", "Timer identity differs"));
            }
            Ok(r)
        })
        .transpose()
    }
    fn save_timer(&self, run: &TimerRun) -> Result<()> {
        run.validate()?;
        self.connection.execute("INSERT INTO timer_jobs(panel,json) VALUES(?1,?2) ON CONFLICT(panel) DO UPDATE SET json=excluded.json",params![run.panel,serde_json::to_string(run)?])?;
        Ok(())
    }
    pub fn timer_control(
        &self,
        owner: &str,
        panel: &str,
        command: &str,
        now: u64,
        sound: Option<String>,
        volume: u8,
    ) -> Result<TimerRun> {
        let p = self.get(panel)?;
        if p.owner != owner {
            return Err(ServiceError::new(
                "owner",
                "Timer belongs to another plugin",
            ));
        }
        let Content::Timer { seconds, .. } = p.spec.content else {
            return Err(ServiceError::new("timer", "This panel is not a timer"));
        };
        let mut run = self.timer(panel)?.unwrap_or(TimerRun {
            panel: panel.into(),
            run_id: Uuid::new_v4().to_string(),
            revision: 1,
            status: TimerStatus::Idle,
            remaining_ms: seconds as u64 * 1000,
            due_ms: None,
            sound: None,
            volume: 35,
        });
        run.remaining_ms = run.remaining(now);
        match command {
            "start" => {
                if run.status != TimerStatus::Running {
                    run.sound = sound;
                    run.volume = volume;
                    if run.status != TimerStatus::Paused {
                        run.run_id = Uuid::new_v4().to_string();
                        run.remaining_ms = seconds as u64 * 1000;
                    }
                    run.status = TimerStatus::Running;
                    run.due_ms = Some(now.saturating_add(run.remaining_ms));
                }
            }
            "pause" => {
                if run.status == TimerStatus::Running {
                    run.status = TimerStatus::Paused;
                    run.due_ms = None;
                }
            }
            "reset" | "stop" => {
                run.status = TimerStatus::Idle;
                run.remaining_ms = seconds as u64 * 1000;
                run.due_ms = None;
            }
            _ => return Err(ServiceError::new("timer", "Unknown timer command")),
        }
        run.revision += 1;
        self.save_timer(&run)?;
        Ok(run)
    }
    /// Durable terminal transition happens before output is attempted. Loading elapsed
    /// jobs calls this with announce=false, so an uncertain old output is never replayed.
    pub fn finish_due(&self, now: u64, announce: bool) -> Result<Vec<TimerRun>> {
        let mut statement = self
            .connection
            .prepare("SELECT json FROM timer_jobs LIMIT 128")?;
        let values = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut due = vec![];
        for raw in values {
            let Ok(mut run) = serde_json::from_str::<TimerRun>(&raw) else {
                continue;
            };
            if run.validate().is_err() {
                continue;
            }
            if run.status == TimerStatus::Running && run.due_ms.is_some_and(|d| d <= now) {
                run.status = TimerStatus::Done;
                run.remaining_ms = 0;
                run.due_ms = None;
                run.revision += 1;
                self.save_timer(&run)?;
                if announce {
                    due.push(run);
                }
            }
        }
        Ok(due)
    }
    pub fn cancel_owner_timers(&self, owner: &str, now: u64) -> Result<()> {
        for panel in self
            .inventory()?
            .panels
            .into_iter()
            .filter(|p| p.owner == owner && matches!(p.spec.content, Content::Timer { .. }))
        {
            if let Some(mut run) = self.timer(&panel.id)?
                && run.status == TimerStatus::Running
            {
                run.remaining_ms = run.remaining(now);
                run.status = TimerStatus::Paused;
                run.due_ms = None;
                run.revision += 1;
                self.save_timer(&run)?;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_timer_survives_closed_panel_and_announces_once_without_historical_replay() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("jobs.sqlite3");
        let store = Store::open(&path).unwrap();
        let p = store
            .open_panel(
                "plugin.a",
                "origin",
                "timer",
                PanelSpec {
                    title: "Timer".into(),
                    content: Content::Timer {
                        seconds: 5,
                        sound: None,
                        volume: 35,
                    },
                    placement: Placement::default(),
                },
            )
            .unwrap();
        store
            .timer_control(
                "plugin.a",
                &p.id,
                "start",
                1000,
                Some("complete".into()),
                35,
            )
            .unwrap();
        store.set_open("plugin.a", &p.id, 1, false).unwrap();
        assert!(store.finish_due(5999, true).unwrap().is_empty());
        assert_eq!(store.finish_due(6000, true).unwrap().len(), 1);
        assert!(store.finish_due(6001, true).unwrap().is_empty());
        store
            .timer_control("plugin.a", &p.id, "start", 7000, None, 35)
            .unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert!(store.finish_due(20000, false).unwrap().is_empty());
        assert_eq!(
            store.timer(&p.id).unwrap().unwrap().status,
            TimerStatus::Done
        );
        assert!(
            store
                .timer_control("plugin.b", &p.id, "start", 0, None, 35)
                .is_err()
        );
    }
    #[test]
    fn stale_remove_cannot_cancel_and_failed_delete_rolls_back_timer() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(&tmp.path().join("jobs.sqlite3")).unwrap();
        let spec: PanelSpec = serde_json::from_value(serde_json::json!({"title":"Timer","content":{"type":"timer","seconds":5,"sound":"complete","volume":21}})).unwrap();
        let p = store
            .open_panel("plugin.a", "origin", "timer", spec)
            .unwrap();
        let run = store
            .timer_control(
                "plugin.a",
                &p.id,
                "start",
                1000,
                Some("complete".into()),
                21,
            )
            .unwrap();
        assert!(store.remove("plugin.a", &p.id, 0).is_err());
        let mut changed = p.spec.clone();
        changed.content = Content::Note {
            text: "Cannot orphan an active timer".into(),
        };
        assert!(store.configure("plugin.a", &p.id, 1, changed).is_err());
        assert_eq!(store.timer(&p.id).unwrap().unwrap().due_ms, run.due_ms);
        store.connection.execute_batch("CREATE TRIGGER fail_timer_delete BEFORE DELETE ON timer_jobs BEGIN SELECT RAISE(FAIL,'failure'); END;").unwrap();
        assert!(store.remove("plugin.a", &p.id, 1).is_err());
        assert!(store.get(&p.id).is_ok());
        store
            .connection
            .execute_batch("DROP TRIGGER fail_timer_delete;")
            .unwrap();
        store.remove("plugin.a", &p.id, 1).unwrap();
        assert!(store.timer(&p.id).unwrap().is_none());
        assert!(store.finish_due(9000, true).unwrap().is_empty());
    }
}
