use deckweave_platform::{foreground, hotkey};
// Monotonic scheduler and OS effects. All external work runs after releasing Core.
use deckweave_core::{Core, Effect, Plan};
use std::{
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tauri::Emitter;
pub struct Engine {
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl Engine {
    pub fn start(core: Arc<Mutex<Core>>, app: tauri::AppHandle, epoch: Arc<AtomicU64>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = thread::spawn(move || {
            let start = Instant::now();
            let mut foreground_at = Instant::now() - Duration::from_secs(1);
            while !stopping.load(Ordering::Relaxed) {
                let app_name = if foreground_at.elapsed() >= Duration::from_millis(300) {
                    foreground_at = Instant::now();
                    Some(foreground())
                } else {
                    None
                };
                let Ok((changed, effects)) = core.lock().map(|mut c| {
                    let mut changed =
                        c.advance(start.elapsed().as_millis().min(u64::MAX as u128) as u64);
                    if let Some(name) = app_name {
                        changed |= c.foreground_application(name.as_deref()).unwrap_or(false);
                    }
                    (changed, c.take_effects())
                }) else {
                    break;
                };
                let mut failed = false;
                let mut failed_owners = std::collections::BTreeSet::new();
                for effect in effects {
                    if failed_owners
                        .contains(&(effect.device_id.clone(), effect.binding_id.clone()))
                    {
                        continue;
                    }
                    if let Err(error) = execute(&effect)
                        && let Ok(mut c) = core.lock()
                    {
                        c.action_error(&effect.device_id, &effect.binding_id, &error);
                        failed_owners.insert((effect.device_id.clone(), effect.binding_id.clone()));
                        failed = true;
                    }
                }
                if changed || failed {
                    epoch.fetch_add(1, Ordering::Relaxed);
                    let _ = app.emit("deckweave:changed", ());
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        Self {
            stop,
            thread: Mutex::new(Some(thread)),
        }
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut thread) = self.thread.lock()
            && let Some(thread) = thread.take()
        {
            let _ = thread.join();
        }
    }
}
fn execute(effect: &Effect) -> Result<(), String> {
    match &effect.plan {
        Plan::Launch { program, args } => {
            Command::new(program)
                .args(args)
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        Plan::Hotkey { keys } => hotkey(keys),
        _ => Err("Unsupported system action".into()),
    }
}
