//! Common onboard assignments and device-specific host routes. Hardware writes
//! are explicit operations; the runtime only configures the provider's input.
use crate::{
    Application,
    device_providers::{Host, SessionStatus},
};
use deckweave_core::{
    Binding, Command, Core, Device, Error, KeyboardAliasPhase, KeyboardAliasProvenance,
    KeyboardAliasRoute, KeyboardAssignmentOperation, KeyboardAssignmentReadback,
    KeyboardAssignmentRequest, KeyboardLayer, KeyboardObservation, KeyboardRestoreRequest,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tauri::Manager;

#[derive(Clone, Default)]
pub(crate) struct InputConfiguration {
    pub generation: u64,
    pub sequence: u64,
    pub routes: Vec<KeyboardAliasRoute>,
    pub ready: bool,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct Status {
    pub state: String,
    pub trigger: u8,
    pub error: Option<String>,
    pub source: &'static str,
}
pub struct Runtime {
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}
impl Runtime {
    pub fn start(app: tauri::AppHandle) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            let mut previous: BTreeMap<String, Vec<KeyboardAliasRoute>> = BTreeMap::new();
            let mut sequence = 0u64;
            while !stopping.load(Ordering::Relaxed) {
                if let Some(state) = app.try_state::<Application>() {
                    let Application::Ready {
                        core,
                        device_providers,
                        ..
                    } = state.inner()
                    else {
                        break;
                    };
                    let routes = core
                        .lock()
                        .map(|c| c.keyboard_alias_routes())
                        .unwrap_or_default();
                    let mut current: BTreeMap<String, Vec<KeyboardAliasRoute>> = BTreeMap::new();
                    for route in routes {
                        current
                            .entry(route.device_id.clone())
                            .or_default()
                            .push(route);
                    }
                    let ids: BTreeSet<_> = previous.keys().chain(current.keys()).cloned().collect();
                    for id in ids {
                        if device_providers.keyboard_editing(&id) {
                            continue;
                        }
                        let wanted = current.get(&id).cloned().unwrap_or_default();
                        let eligible = wanted.is_empty()
                            || device_providers.keyboard_routes_ready(&id, &wanted);
                        let wanted = if eligible { wanted } else { vec![] };
                        if previous.get(&id) == Some(&wanted)
                            && device_providers
                                .keyboard_input(&id)
                                .is_some_and(|config| config.sequence > 0)
                        {
                            continue;
                        }
                        let Some(session) = device_providers
                            .session(&id)
                            .filter(|s| s.state == "connected")
                        else {
                            previous.remove(&id);
                            device_providers.set_keyboard_input(&id, InputConfiguration::default());
                            continue;
                        };
                        sequence += 1;
                        let configuration = InputConfiguration {
                            generation: session.generation,
                            sequence,
                            routes: wanted.clone(),
                            ready: false,
                            error: None,
                        };
                        // Retire old captured routes before the request, not after its ACK.
                        device_providers.set_keyboard_input(&id, configuration.clone());
                        let aliases: Vec<_> = wanted.iter().map(|route| json!({"controlId":route.control_id,"layer":route.layer.as_str(),"trigger":route.alias,"installedRecord":[2,0,route.alias+91,0],"installOperationId":route.installation_id})).collect();
                        if let Err(error) = device_providers.keyboard_call(&id, session.generation, "configureKeyboardInput", json!({"sequence":sequence,"aliases":aliases,"confirmedInstanceId":wanted.first().map(|route|route.identity.as_str())})) {
                            // ACK readiness is published by the pipe reader before the
                            // next alias event. Do not overwrite a later source-loss event.
                            if let Some(mut failed)=device_providers.keyboard_input(&id) && failed.sequence==sequence {
                                failed.ready=false; failed.error=Some(error);
                                device_providers.set_keyboard_input(&id,failed);
                            }
                        }
                        // No retry loop after uncertainty. A route/reconciliation change
                        // creates another explicit configuration generation.
                        previous.insert(id, wanted);
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        Self {
            stop,
            worker: Mutex::new(Some(worker)),
        }
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut worker) = self.worker.lock()
            && let Some(worker) = worker.take()
        {
            let _ = worker.join();
        }
    }
}

pub fn statuses(core: &Core, host: &Host, device_id: &str) -> BTreeMap<String, Status> {
    let snapshot = core.snapshot();
    let Some(device) = snapshot
        .configuration
        .devices
        .iter()
        .find(|d| d.id == device_id)
    else {
        return BTreeMap::new();
    };
    let session = host.session(device_id);
    let input = host.keyboard_input(device_id);
    device
        .keyboard_aliases
        .iter()
        .filter(|a| a.phase != KeyboardAliasPhase::Released)
        .map(|owner| {
            let active = input.as_ref().is_some_and(|config| {
                config.ready
                    && session.as_ref().is_some_and(|s| {
                        s.generation == config.generation && s.state == "connected"
                    })
                    && config
                        .routes
                        .iter()
                        .any(|r| r.operation_id == owner.operation_id)
            });
            let state = if !session.as_ref().is_some_and(|s| s.state == "connected") {
                "disconnected"
            } else if active {
                "active"
            } else {
                match owner.phase {
                    KeyboardAliasPhase::Verified => "unpaired",
                    KeyboardAliasPhase::Pending | KeyboardAliasPhase::Restoring => "pending",
                    KeyboardAliasPhase::Conflict => "conflict",
                    _ => "mapping_required",
                }
            };
            (
                format!("{}:{}", owner.layer.as_str(), owner.control_id),
                Status {
                    state: state.into(),
                    trigger: owner.alias,
                    error: input.as_ref().and_then(|i| i.error.clone()),
                    source: "device_raw_input",
                },
            )
        })
        .collect()
}
fn current_device(core: &Core, id: &str, expected: u64) -> Result<Device, Error> {
    if core.revision() != expected {
        return Err(Error::new(
            "conflict",
            "Configuration changed; read the current assignment again",
        ));
    }
    core.snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| Error::new("not_found", "Keyboard not found"))
}
fn observation(device: &Device, session: &SessionStatus) -> Result<KeyboardObservation, Error> {
    if session.state != "connected" {
        return Err(Error::new(
            "keyboard_connection",
            "Read the connected keyboard before editing",
        ));
    }
    let metadata = &session.observed["keyboardAliases"];
    if metadata["version"] != 1 {
        return Err(Error::new(
            "keyboard_capability",
            "Update the keyboard support package before assigning DeckWeave actions",
        ));
    }
    let mut control_indices = BTreeMap::new();
    for c in &device
        .provider_descriptor
        .as_ref()
        .ok_or_else(|| Error::new("keyboard_capability", "Keyboard descriptor missing"))?
        .geometry
        .controls
    {
        if let Some(index) =
            c.id.strip_prefix("key-")
                .and_then(|s| s.parse::<usize>().ok())
        {
            control_indices.insert(c.id.clone(), index);
        }
    }
    Ok(KeyboardObservation {
        main: serde_json::from_value(metadata["records"]["main"].clone())?,
        fn_layer: serde_json::from_value(metadata["records"]["fn"].clone())?,
        layer_revisions: serde_json::from_value(metadata["layerRevisions"].clone())?,
        control_indices,
    })
}
fn native_record(
    observed: &KeyboardObservation,
    control: &str,
    layer: KeyboardLayer,
) -> Result<[u8; 4], Error> {
    let index = observed
        .control_indices
        .get(control)
        .ok_or_else(|| Error::new("keyboard_control", "Unknown physical keyboard key"))?;
    match layer {
        KeyboardLayer::Main => &observed.main,
        KeyboardLayer::Fn => &observed.fn_layer,
    }
    .get(*index)
    .copied()
    .ok_or_else(|| Error::new("keyboard_control", "Keyboard key was not read"))
}
fn current_session(host: &Host, device: &str) -> Result<SessionStatus, Error> {
    host.session(device)
        .filter(|s| s.state == "connected")
        .ok_or_else(|| {
            Error::new(
                "keyboard_connection",
                "Connect and explicitly select the keyboard before editing",
            )
        })
}
fn payload(operation: &KeyboardAssignmentOperation) -> Value {
    json!({"operationId":operation.operation_id,"installOperationId":operation.installation_id,"controlId":operation.control_id,"layer":operation.layer.as_str(),"trigger":operation.alias,"originalRecord":operation.original,"installedRecord":operation.installed,"expectedRevision":operation.expected_layer_revision,"confirmedInstanceId":operation.identity})
}
fn finish(
    core: &Mutex<Core>,
    host: &Host,
    operation: &KeyboardAssignmentOperation,
    kind: &'static str,
) -> Result<(), Error> {
    let reply = host.keyboard_call(
        &operation.device_id,
        operation.generation,
        kind,
        payload(operation),
    );
    let result = reply.map_err(|e|Error::new("keyboard_operation",e)).and_then(|reply| {
        let expected_outcome=if kind=="installKeyboardAlias" {"installed"} else {"restored"};
        if reply["operationId"].as_str() != Some(&operation.operation_id) || reply["outcome"]!=expected_outcome {
            return Err(Error::new("keyboard_operation","Keyboard did not confirm the complete intended operation; retained recovery must be reconciled"));
        }
        if current_session(host,&operation.device_id)?.generation != operation.generation { return Err(Error::new("keyboard_connection","Keyboard connection changed during verification")); }
        let after_session = current_session(host,&operation.device_id)?;
        let mut c = core.lock().map_err(|_|Error::new("runtime","Workspace unavailable"))?;
        let device = current_device(&c,&operation.device_id,c.revision())?;
        c.validate_keyboard_alias_observation(&operation.device_id,&observation(&device,&after_session)?)?;
        let record = serde_json::from_value(reply["currentRecord"].clone())?;
        c.complete_keyboard_assignment(KeyboardAssignmentReadback {operation_id:operation.operation_id.clone(),generation:operation.generation,identity:operation.identity.clone(),record})?;
        Ok(())
    });
    if let Err(error) = &result
        && let Ok(mut c) = core.lock()
    {
        c.fail_keyboard_assignment(&operation.operation_id, &error.message)?;
    }
    result
}

pub struct AssignmentEdit {
    pub device_id: String,
    pub control_id: String,
    pub layer: KeyboardLayer,
    pub binding: Option<Binding>,
    pub expected_revision: u64,
    pub replace_native: bool,
    pub expected_native: Option<[u8; 4]>,
    pub expected_generation: Option<u64>,
}
pub fn assign(core: &Mutex<Core>, host: &Host, edit: AssignmentEdit) -> Result<(), Error> {
    let _lease = host
        .begin_keyboard_edit(&edit.device_id)
        .map_err(|e| Error::new("keyboard_operation", e))?;
    let device = current_device(
        &*core
            .lock()
            .map_err(|_| Error::new("runtime", "Workspace unavailable"))?,
        &edit.device_id,
        edit.expected_revision,
    )?;
    let Some(binding) = edit.binding else {
        let command = match edit.layer {
            KeyboardLayer::Fn => Command::SetFnBinding {
                device_id: edit.device_id.clone(),
                control_id: edit.control_id,
                trigger: 13,
                configure_hardware: false,
                binding: None,
                expected_revision: edit.expected_revision,
            },
            KeyboardLayer::Main => Command::ConfigureControl {
                device_id: edit.device_id.clone(),
                page_id: device.profile.active_page_id,
                control_id: edit.control_id,
                kind: None,
                label: String::new(),
                expected_revision: edit.expected_revision,
            },
        };
        core.lock()
            .map_err(|_| Error::new("runtime", "Workspace unavailable"))?
            .dispatch(command, &format!("device-{}", edit.device_id))?;
        return Ok(());
    };
    let session = current_session(host, &edit.device_id)?;
    let observed = observation(&device, &session)?;
    let native = native_record(&observed, &edit.control_id, edit.layer)?;
    let owner = device.keyboard_aliases.iter().find(|a| {
        a.provenance == KeyboardAliasProvenance::Deckweave
            && a.phase != KeyboardAliasPhase::Released
            && a.layer == edit.layer
            && a.control_id == edit.control_id
    });
    if owner.is_none()
        && (!edit.replace_native
            || edit.expected_native != Some(native)
            || edit.expected_generation != Some(session.generation))
    {
        return Err(Error::new(
            "keyboard_preview",
            "The keyboard assignment changed; review its replacement before writing",
        ));
    }
    let identity = device.hardware_key.clone().ok_or_else(|| {
        Error::new(
            "keyboard_identity",
            "Explicitly select the connected keyboard",
        )
    })?;
    let operation = core
        .lock()
        .map_err(|_| Error::new("runtime", "Workspace unavailable"))?
        .prepare_keyboard_assignment(KeyboardAssignmentRequest {
            device_id: edit.device_id,
            profile_id: device.profile.id,
            page_id: Some(device.profile.active_page_id),
            control_id: edit.control_id,
            layer: edit.layer,
            binding,
            observation: observed,
            generation: session.generation,
            identity,
            expected_revision: edit.expected_revision,
        })?;
    if operation.requires_write {
        finish(core, host, &operation, "installKeyboardAlias")?;
    }
    Ok(())
}
pub fn restore(
    core: &Mutex<Core>,
    host: &Host,
    device_id: String,
    control_id: String,
    layer: KeyboardLayer,
    expected_revision: u64,
) -> Result<(), Error> {
    let _lease = host
        .begin_keyboard_edit(&device_id)
        .map_err(|e| Error::new("keyboard_operation", e))?;
    let device = current_device(
        &*core
            .lock()
            .map_err(|_| Error::new("runtime", "Workspace unavailable"))?,
        &device_id,
        expected_revision,
    )?;
    let session = current_session(host, &device_id)?;
    let operation = core
        .lock()
        .map_err(|_| Error::new("runtime", "Workspace unavailable"))?
        .prepare_keyboard_restore(KeyboardRestoreRequest {
            device_id,
            control_id,
            layer,
            observation: observation(&device, &session)?,
            generation: session.generation,
            identity: device.hardware_key.ok_or_else(|| {
                Error::new(
                    "keyboard_identity",
                    "Explicitly select the connected keyboard",
                )
            })?,
            expected_revision,
        })?;
    if operation.requires_write {
        finish(core, host, &operation, "restoreKeyboardAlias")?;
    }
    Ok(())
}
pub fn reconcile(
    core: &Mutex<Core>,
    host: &Host,
    device_id: String,
    control_id: String,
    layer: KeyboardLayer,
    expected_revision: u64,
) -> Result<(), Error> {
    let _lease = host
        .begin_keyboard_edit(&device_id)
        .map_err(|e| Error::new("keyboard_operation", e))?;
    let device = current_device(
        &*core
            .lock()
            .map_err(|_| Error::new("runtime", "Workspace unavailable"))?,
        &device_id,
        expected_revision,
    )?;
    let session = host
        .session(&device_id)
        .filter(|s| matches!(s.state.as_str(), "connected" | "error" | "recovery_pending"))
        .ok_or_else(|| {
            Error::new(
                "keyboard_connection",
                "Select the currently connected keyboard before reconciliation",
            )
        })?;
    let mut observed_session = session.clone();
    observed_session.state = "connected".into(); // Reconciliation reads through an unresolved session; it never authorizes SET.
    let observed = observation(&device, &observed_session)?;
    let owner = device
        .keyboard_aliases
        .iter()
        .find(|a| {
            a.control_id == control_id
                && a.layer == layer
                && a.provenance == KeyboardAliasProvenance::Deckweave
                && a.phase != KeyboardAliasPhase::Released
        })
        .ok_or_else(|| {
            Error::new(
                "keyboard_ownership",
                "Complete the keyboard assignment setup first",
            )
        })?;
    let identity = device.hardware_key.clone().ok_or_else(|| {
        Error::new(
            "keyboard_identity",
            "Explicitly select the connected keyboard",
        )
    })?;
    let operation = KeyboardAssignmentOperation {
        operation_id: owner.operation_id.clone(),
        installation_id: owner.installation_id.clone(),
        device_id: device_id.clone(),
        control_id,
        layer,
        alias: owner.alias,
        original: owner
            .original
            .ok_or_else(|| Error::new("keyboard_ownership", "Original assignment is unknown"))?,
        installed: owner.installed,
        generation: session.generation,
        identity: identity.clone(),
        profile_id: owner.profile_id.clone(),
        page_id: owner.page_id.clone(),
        expected_layer_revision: observed.layer_revisions[&layer].clone(),
        requires_write: false,
    };
    let reply = host
        .keyboard_call(
            &device_id,
            session.generation,
            "reconcileKeyboardAlias",
            payload(&operation),
        )
        .map_err(|e| Error::new("keyboard_operation", e))?;
    if reply["operationId"].as_str() != Some(&operation.operation_id)
        || !matches!(
            reply["outcome"].as_str(),
            Some("installed" | "restored" | "unchanged")
        )
        || reply["readOnly"] != true
    {
        core.lock()
            .map_err(|_| Error::new("runtime", "Workspace unavailable"))?
            .fail_keyboard_assignment(
                &operation.operation_id,
                "Native full-layer reconciliation remains conflicted; no SET replayed",
            )?;
        return Err(Error::new(
            "keyboard_operation",
            "Keyboard operation remains conflicted; no settings were replayed",
        ));
    }
    if current_session(host, &device_id)?.generation != operation.generation {
        return Err(Error::new(
            "keyboard_operation",
            "Reconciliation connection changed",
        ));
    }
    let after_session = current_session(host, &device_id)?;
    let mut c = core
        .lock()
        .map_err(|_| Error::new("runtime", "Workspace unavailable"))?;
    c.validate_keyboard_alias_observation(&device_id, &observation(&device, &after_session)?)?;
    let phase = c.reconcile_keyboard_alias_confirmed(KeyboardAssignmentReadback {
        operation_id: operation.operation_id,
        generation: session.generation,
        identity,
        record: serde_json::from_value(reply["currentRecord"].clone())?,
    })?;
    if matches!(
        phase,
        KeyboardAliasPhase::Conflict | KeyboardAliasPhase::Uncertain
    ) {
        return Err(Error::new(
            "keyboard_reconcile",
            "Current mapping differs; the saved operation remains visible and was not replayed",
        ));
    }
    host.set_keyboard_input(&device_id, InputConfiguration::default()); // Explicit recheck permits one fresh source configuration, never a SET replay.
    Ok(())
}
