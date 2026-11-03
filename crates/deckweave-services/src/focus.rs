//! Versioned workspace jobs, independent of panel visibility and legacy timers.
use super::*;

pub const MAX_JOBS: usize = 32;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FocusConfig {
    pub work_seconds: u32,
    pub short_seconds: u32,
    pub long_seconds: u32,
    pub rounds: u32,
    pub sound: Option<String>,
    pub volume: u8,
}
impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            work_seconds: 1500,
            short_seconds: 300,
            long_seconds: 900,
            rounds: 4,
            sound: Some("complete".into()),
            volume: 35,
        }
    }
}
impl FocusConfig {
    fn validate(&self) -> Result<()> {
        if [self.work_seconds, self.short_seconds, self.long_seconds]
            .iter()
            .any(|s| !(1..=86400).contains(s))
            || !(1..=16).contains(&self.rounds)
            || self.volume > 100
            || self
                .sound
                .as_ref()
                .is_some_and(|s| !["complete", "notice", "warning"].contains(&s.as_str()))
        {
            return Err(ServiceError::new(
                "focus",
                "Invalid Pomodoro duration, rounds, or sound",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FocusPhase {
    Work,
    ShortBreak,
    LongBreak,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FocusStatus {
    Idle,
    Running,
    Paused,
    Ready,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    id: String,
    fingerprint: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FocusJob {
    pub version: u32,
    pub owner: String,
    pub workspace: String,
    pub revision: i64,
    pub run_id: String,
    pub phase: FocusPhase,
    pub status: FocusStatus,
    pub round: u32,
    pub remaining_ms: u64,
    pub due_ms: Option<u64>,
    pub transition: u64,
    pub config: FocusConfig,
    receipts: Vec<Receipt>,
}
#[derive(Clone, Debug, Serialize)]
pub struct FocusView {
    #[serde(flatten)]
    pub job: FocusJob,
    pub live_remaining_ms: u64,
}
#[derive(Default)]
pub struct FocusInventory {
    pub jobs: Vec<FocusJob>,
    pub unavailable: Vec<InvalidRecord>,
}
impl FocusJob {
    pub fn remaining(&self, now: u64) -> u64 {
        if self.status == FocusStatus::Running {
            self.due_ms.unwrap_or(now).saturating_sub(now)
        } else {
            self.remaining_ms
        }
    }
    pub fn view(&self, now: u64) -> FocusView {
        let mut job = self.clone();
        job.receipts.clear();
        FocusView {
            job,
            live_remaining_ms: self.remaining(now),
        }
    }
    pub fn event_id(&self) -> String {
        format!("focus-{}-{}", self.run_id, self.transition)
    }
    fn duration(&self) -> u64 {
        u64::from(match self.phase {
            FocusPhase::Work => self.config.work_seconds,
            FocusPhase::ShortBreak => self.config.short_seconds,
            FocusPhase::LongBreak => self.config.long_seconds,
        }) * 1000
    }
    fn advance(&mut self) {
        self.phase = match self.phase {
            FocusPhase::Work => {
                if self.round == self.config.rounds {
                    FocusPhase::LongBreak
                } else {
                    FocusPhase::ShortBreak
                }
            }
            FocusPhase::ShortBreak => {
                self.round = if self.round >= self.config.rounds {
                    1
                } else {
                    self.round + 1
                };
                FocusPhase::Work
            }
            FocusPhase::LongBreak => {
                self.round = 1;
                FocusPhase::Work
            }
        };
        self.status = FocusStatus::Ready;
        self.remaining_ms = self.duration();
        self.due_ms = None;
        self.transition += 1;
    }
    fn validate(&self) -> Result<()> {
        identity(&self.owner)?;
        identity(&self.workspace)?;
        self.config.validate()?;
        if self.version != 1
            || self.revision <= 0
            || Uuid::parse_str(&self.run_id).is_err()
            || !(1..=self.config.rounds).contains(&self.round)
            || self.remaining_ms > 86_400_000
            || self.receipts.len() > 64
            || (self.status == FocusStatus::Running) != self.due_ms.is_some()
        {
            return Err(ServiceError::new(
                "focus",
                "Unsupported or invalid Pomodoro job",
            ));
        }
        Ok(())
    }
}
impl Store {
    pub fn focus_job(&self, owner: &str, workspace: &str) -> Result<Option<FocusJob>> {
        identity(owner)?;
        identity(workspace)?;
        let row: Option<(i64, String)> = self
            .connection
            .query_row(
                "SELECT revision,json FROM focus_jobs WHERE owner=?1 AND workspace=?2",
                params![owner, workspace],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        row.map(|(revision, json)| {
            let job: FocusJob = serde_json::from_str(&json)?;
            job.validate()?;
            if job.owner != owner || job.workspace != workspace || job.revision != revision {
                return Err(ServiceError::new(
                    "focus",
                    "Job identity or revision differs",
                ));
            }
            Ok(job)
        })
        .transpose()
    }
    pub fn focus_inventory(&self) -> Result<FocusInventory> {
        let mut statement = self
            .connection
            .prepare("SELECT owner,workspace FROM focus_jobs ORDER BY owner,workspace LIMIT 33")?;
        let keys = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut inventory = FocusInventory::default();
        for (owner, workspace) in keys {
            match self.focus_job(&owner, &workspace) {
                Ok(Some(job)) => inventory.jobs.push(job),
                Ok(None) => {}
                Err(error) => inventory.unavailable.push(InvalidRecord {
                    id: format!("focus:{owner}:{workspace}"),
                    error: error.message,
                }),
            }
        }
        Ok(inventory)
    }
    pub fn focus_jobs(&self) -> Result<Vec<FocusJob>> {
        Ok(self.focus_inventory()?.jobs)
    }
    fn save_focus(&self, job: &FocusJob, previous: i64) -> Result<()> {
        job.validate()?;
        if self.connection.execute("UPDATE focus_jobs SET revision=?1,json=?2 WHERE owner=?3 AND workspace=?4 AND revision=?5",params![job.revision,serde_json::to_string(job)?,job.owner,job.workspace,previous])? != 1 { return Err(ServiceError::new("conflict","Pomodoro changed; reload before editing")); }
        Ok(())
    }
    /// Only first creation uses the binding's defaults. Copies never overwrite a job.
    pub fn ensure_focus(
        &self,
        owner: &str,
        workspace: &str,
        config: FocusConfig,
    ) -> Result<FocusJob> {
        identity(owner)?;
        identity(workspace)?;
        config.validate()?;
        if let Some(job) = self.focus_job(owner, workspace)? {
            return Ok(job);
        }
        let count: u32 = self
            .connection
            .query_row("SELECT COUNT(*) FROM focus_jobs", [], |r| r.get(0))?;
        if count >= MAX_JOBS as u32 {
            return Err(ServiceError::new(
                "quota",
                "At most 32 workspace Pomodoros may be saved",
            ));
        }
        let job = FocusJob {
            version: 1,
            owner: owner.into(),
            workspace: workspace.into(),
            revision: 1,
            run_id: Uuid::new_v4().to_string(),
            phase: FocusPhase::Work,
            status: FocusStatus::Idle,
            round: 1,
            remaining_ms: u64::from(config.work_seconds) * 1000,
            due_ms: None,
            transition: 0,
            config,
            receipts: vec![],
        };
        self.connection.execute(
            "INSERT INTO focus_jobs(owner,workspace,revision,json) VALUES(?1,?2,?3,?4)",
            params![
                job.owner,
                job.workspace,
                job.revision,
                serde_json::to_string(&job)?
            ],
        )?;
        Ok(job)
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "Guarded durable command includes explicit identity, clock and optional configuration"
    )]
    pub fn focus_control(
        &self,
        owner: &str,
        workspace: &str,
        revision: i64,
        command_id: &str,
        command: &str,
        config: Option<FocusConfig>,
        now: u64,
    ) -> Result<FocusJob> {
        identity(command_id)?;
        let mut job = self
            .focus_job(owner, workspace)?
            .ok_or_else(|| ServiceError::new("not_found", "Create the workspace Pomodoro first"))?;
        let fingerprint = serde_json::to_string(&(command, &config))?;
        if let Some(receipt) = job.receipts.iter().find(|r| r.id == command_id) {
            if receipt.fingerprint != fingerprint {
                return Err(ServiceError::new(
                    "identity",
                    "Command identity was already used with different content",
                ));
            }
            return Ok(job);
        }
        if job.revision != revision {
            return Err(ServiceError::new(
                "conflict",
                "Pomodoro changed; reload before editing",
            ));
        }
        // Expiration is settled by the scheduler, never turned into a fresh run by a late command.
        if job.status == FocusStatus::Running && job.due_ms.is_some_and(|d| d <= now) {
            return Err(ServiceError::new(
                "conflict",
                "Phase just completed; reload before editing",
            ));
        }
        job.remaining_ms = job.remaining(now);
        match command {
            "toggle" | "start" | "pause" => {
                if job.status == FocusStatus::Running {
                    if command != "start" {
                        job.status = FocusStatus::Paused;
                        job.due_ms = None;
                    }
                } else if command != "pause" {
                    job.status = FocusStatus::Running;
                    job.due_ms = Some(now.saturating_add(job.remaining_ms));
                }
            }
            "reset" => {
                job.phase = FocusPhase::Work;
                job.round = 1;
                job.status = FocusStatus::Idle;
                job.due_ms = None;
                job.remaining_ms = job.duration();
                job.run_id = Uuid::new_v4().to_string();
                job.transition = 0;
            }
            "skip" => job.advance(),
            "configure" => {
                let config = config
                    .ok_or_else(|| ServiceError::new("focus", "Configuration is required"))?;
                config.validate()?;
                // A running phase keeps its deadline. New durations apply to subsequent phases.
                job.config = config;
                job.round = job.round.min(job.config.rounds);
                if job.status != FocusStatus::Running && job.status != FocusStatus::Paused {
                    job.remaining_ms = job.duration();
                }
            }
            _ => return Err(ServiceError::new("focus", "Unknown Pomodoro command")),
        }
        job.revision += 1;
        job.receipts.push(Receipt {
            id: command_id.into(),
            fingerprint,
        });
        if job.receipts.len() > 64 {
            job.receipts.remove(0);
        }
        self.save_focus(&job, revision)?;
        Ok(job)
    }
    /// One transition only. Startup calls announce=false: missed breaks never cascade.
    pub fn finish_focus_due(&self, now: u64, announce: bool) -> Result<Vec<FocusJob>> {
        let mut due = vec![];
        for mut job in self.focus_jobs()? {
            if job.status == FocusStatus::Running && job.due_ms.is_some_and(|d| d <= now) {
                let previous = job.revision;
                job.advance();
                job.revision += 1;
                self.save_focus(&job, previous)?;
                if announce {
                    due.push(job);
                }
            }
        }
        Ok(due)
    }
    pub fn pause_owner_focus(&self, owner: &str, now: u64) -> Result<()> {
        for mut job in self
            .focus_jobs()?
            .into_iter()
            .filter(|j| j.owner == owner && j.status == FocusStatus::Running)
        {
            let previous = job.revision;
            job.remaining_ms = job.remaining(now);
            job.status = FocusStatus::Paused;
            job.due_ms = None;
            job.revision += 1;
            self.save_focus(&job, previous)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn control(s: &Store, j: &FocusJob, id: &str, command: &str, now: u64) -> FocusJob {
        s.focus_control(&j.owner, &j.workspace, j.revision, id, command, None, now)
            .unwrap()
    }
    #[test]
    fn four_rounds_and_manual_phase_start_without_tick_writes() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let mut j = s
            .ensure_focus(
                "plugin.focus",
                "personal",
                FocusConfig {
                    work_seconds: 1,
                    short_seconds: 1,
                    long_seconds: 1,
                    ..Default::default()
                },
            )
            .unwrap();
        let mut now = 1000;
        for round in 1..=4 {
            assert_eq!(j.round, round);
            assert_eq!(j.phase, FocusPhase::Work);
            j = control(&s, &j, &format!("work{round}"), "start", now);
            let revision = j.revision;
            assert!(s.finish_focus_due(now + 999, true).unwrap().is_empty());
            assert_eq!(
                s.focus_job("plugin.focus", "personal")
                    .unwrap()
                    .unwrap()
                    .revision,
                revision
            );
            j = s.finish_focus_due(now + 1000, true).unwrap().remove(0);
            assert_eq!(j.status, FocusStatus::Ready);
            assert_eq!(
                j.phase,
                if round == 4 {
                    FocusPhase::LongBreak
                } else {
                    FocusPhase::ShortBreak
                }
            );
            assert!(s.finish_focus_due(now + 100000, true).unwrap().is_empty());
            now += 2000;
            j = control(&s, &j, &format!("break{round}"), "start", now);
            assert_eq!(j.status, FocusStatus::Running);
            j = s.finish_focus_due(now + 1000, true).unwrap().remove(0);
            now += 2000;
        }
        assert_eq!(j.round, 1);
        assert_eq!(j.phase, FocusPhase::Work);
        assert_eq!(j.status, FocusStatus::Ready);
    }
    #[test]
    fn pause_restart_silent_expiration_and_idempotent_guarded_commands() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("jobs.sqlite3");
        let s = Store::open(&path).unwrap();
        let j = s
            .ensure_focus("plugin.focus", "personal", FocusConfig::default())
            .unwrap();
        let running = control(&s, &j, "one", "toggle", 1000);
        let retry = s
            .focus_control(
                &j.owner,
                &j.workspace,
                j.revision,
                "one",
                "toggle",
                None,
                1500,
            )
            .unwrap();
        assert_eq!(retry.revision, running.revision);
        assert_eq!(retry.status, FocusStatus::Running);
        assert!(
            s.focus_control(
                &j.owner,
                &j.workspace,
                j.revision,
                "two",
                "toggle",
                None,
                1500
            )
            .is_err()
        );
        assert!(
            s.focus_control(
                &j.owner,
                &j.workspace,
                running.revision,
                "one",
                "reset",
                None,
                1500
            )
            .is_err()
        );
        let paused = control(&s, &running, "pause", "pause", 2000);
        assert_eq!(paused.remaining_ms, 1499000);
        drop(s);
        let s = Store::open(&path).unwrap();
        let paused = s.focus_job("plugin.focus", "personal").unwrap().unwrap();
        assert_eq!(paused.status, FocusStatus::Paused);
        let running = control(&s, &paused, "resume", "start", 5000);
        assert_eq!(running.due_ms, Some(1504000));
        drop(s);
        let s = Store::open(&path).unwrap();
        assert!(s.finish_focus_due(100000000, false).unwrap().is_empty());
        let ready = s.focus_job("plugin.focus", "personal").unwrap().unwrap();
        assert_eq!(ready.phase, FocusPhase::ShortBreak);
        assert_eq!(ready.status, FocusStatus::Ready);
        assert_eq!(ready.transition, 1);
        assert!(s.finish_focus_due(200000000, true).unwrap().is_empty());
        let skipped = control(&s, &ready, "skip", "skip", 200000000);
        assert_eq!(skipped.phase, FocusPhase::Work);
        assert_eq!(skipped.round, 2);
        let reset = control(&s, &skipped, "reset", "reset", 200000001);
        assert_eq!(reset.status, FocusStatus::Idle);
        assert_eq!(reset.round, 1);
        assert!(s.focus_job("other.plugin", "personal").unwrap().is_none());
    }
    #[test]
    fn old_panel_and_timer_schema_remain_compatible_and_copies_keep_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("old.sqlite3");
        let c = Connection::open(&path).unwrap();
        c.execute_batch("CREATE TABLE panels(id TEXT PRIMARY KEY,owner TEXT NOT NULL,origin TEXT NOT NULL,local_key TEXT NOT NULL,revision INTEGER NOT NULL,json TEXT NOT NULL,UNIQUE(owner,origin,local_key));CREATE TABLE timer_jobs(panel TEXT PRIMARY KEY,json TEXT NOT NULL);PRAGMA user_version=1;").unwrap();
        drop(c);
        let s = Store::open(&path).unwrap();
        let j = s
            .ensure_focus("plugin.focus", "personal", FocusConfig::default())
            .unwrap();
        let copy = s
            .ensure_focus(
                "plugin.focus",
                "personal",
                FocusConfig {
                    work_seconds: 10,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(copy.run_id, j.run_id);
        assert_eq!(copy.config.work_seconds, 1500);
        assert!(s.inventory().unwrap().panels.is_empty());
        assert!(s.finish_due(0, true).unwrap().is_empty());
        assert_eq!(
            s.connection
                .pragma_query_value::<u32, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn reconfiguration_preserves_running_deadline_and_paused_remaining() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let j = s
            .ensure_focus("plugin.focus", "personal", FocusConfig::default())
            .unwrap();
        let j = control(&s, &j, "start", "start", 1000);
        let j = s
            .focus_control(
                &j.owner,
                &j.workspace,
                j.revision,
                "configure",
                "configure",
                Some(FocusConfig {
                    work_seconds: 60,
                    rounds: 1,
                    ..Default::default()
                }),
                2000,
            )
            .unwrap();
        assert_eq!(j.due_ms, Some(1501000));
        let j = control(&s, &j, "pause", "pause", 3000);
        let j = s
            .focus_control(
                &j.owner,
                &j.workspace,
                j.revision,
                "configure-paused",
                "configure",
                Some(FocusConfig {
                    work_seconds: 120,
                    rounds: 1,
                    ..Default::default()
                }),
                4000,
            )
            .unwrap();
        assert_eq!(j.remaining_ms, 1498000);
        assert_eq!(j.status, FocusStatus::Paused);
        let j = control(&s, &j, "skip", "skip", 5000);
        assert_eq!(j.phase, FocusPhase::LongBreak);
        let j = control(&s, &j, "next", "skip", 6000);
        assert_eq!(j.phase, FocusPhase::Work);
        assert_eq!(j.remaining_ms, 120000);
        // Reducing rounds during a short break cannot create an invalid next round.
        let j = s
            .focus_control(
                &j.owner,
                &j.workspace,
                j.revision,
                "four",
                "configure",
                Some(FocusConfig::default()),
                7000,
            )
            .unwrap();
        let j = control(&s, &j, "short", "skip", 8000);
        assert_eq!(j.phase, FocusPhase::ShortBreak);
        let j = s
            .focus_control(
                &j.owner,
                &j.workspace,
                j.revision,
                "one",
                "configure",
                Some(FocusConfig {
                    rounds: 1,
                    ..Default::default()
                }),
                9000,
            )
            .unwrap();
        let j = control(&s, &j, "work", "skip", 10000);
        assert_eq!(j.round, 1);
        assert_eq!(j.phase, FocusPhase::Work);
    }
    #[test]
    fn invalid_jobs_are_reported_without_stopping_or_erasing_other_jobs() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let j = s
            .ensure_focus(
                "plugin.focus",
                "valid",
                FocusConfig {
                    work_seconds: 1,
                    ..Default::default()
                },
            )
            .unwrap();
        let j = control(&s, &j, "start", "start", 1000);
        s.connection
            .execute(
                "INSERT INTO focus_jobs VALUES('plugin.focus','broken',1,'bad json')",
                [],
            )
            .unwrap();
        let inventory = s.focus_inventory().unwrap();
        assert_eq!(inventory.jobs.len(), 1);
        assert_eq!(inventory.unavailable.len(), 1);
        let finished = s.finish_focus_due(2000, true).unwrap();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].run_id, j.run_id);
        let raw: String = s
            .connection
            .query_row(
                "SELECT json FROM focus_jobs WHERE workspace='broken'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(raw, "bad json");
        assert!(
            s.ensure_focus("plugin.focus", "broken", FocusConfig::default())
                .is_err()
        );
    }
}
