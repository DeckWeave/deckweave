//! Headless keyboard protocol regressions. Only an isolated Node fixture runs;
//! these tests never discover HID, register Raw Input or open the user workspace.
use super::*;
use deckweave_core::{
    ActionKind, Binding, KeyboardAssignmentReadback, KeyboardAssignmentRequest, KeyboardLayer,
    KeyboardObservation,
};

struct Fixture {
    directory: tempfile::TempDir,
    package: Package,
    transport: Option<Transport>,
    host: Host,
    state: ProcessState,
    device: String,
    binding: String,
    operation: deckweave_core::KeyboardAssignmentOperation,
    native_state: Value,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("provider.mjs"),r#"
import {createInterface} from 'node:readline';
import fs from 'node:fs';
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})) {
 const q=JSON.parse(line);
 fs.appendFileSync(process.env.DECKWEAVE_PROVIDER_STATE_DIR+'/requests.log',q.type+'\n');
 process.stdout.write(JSON.stringify({type:'result',requestId:q.requestId,session:q.session,generation:q.generation,ok:true,value:q.fixtureReply??{}})+'\n');
}
"#).unwrap();
        std::fs::write(directory.path().join("deckweave.device.json"),serde_json::to_vec(&json!({"kind":"device","protocol":1,"id":"dev.deckweave.keyboard-fixture","name":"Keyboard fixture","version":"1.0.0","runtime":"node","entry":"provider.mjs","models":["dev.deckweave.keyboard.model"],"platforms":["windows","macos","linux"]})).unwrap()).unwrap();
        let package = load(directory.path()).unwrap();
        let definition = json!({"instanceId":"confirmed-a","modelId":"dev.deckweave.keyboard.model","name":"Keyboard fixture","geometry":{"width":1,"height":1},"controls":[{"id":"key-49","kind":"key","label":"T","x":0,"y":0,"width":1,"height":1,"events":[]}],"settings":[],"keyboard":{"aliases":{"version":1,"semantics":"oneShot"},"onboardMappings":{"settingId":"keyMappings","layers":["main","fn"],"bindingFormat":"keyboard-usage"}}});
        let mut core = Core::memory().unwrap();
        let device = core
            .register_provider_device(
                &package.manifest.id,
                "confirmed-a",
                "Keyboard fixture",
                descriptor_geometry(&definition).unwrap(),
            )
            .unwrap();
        core.connection_changed(&device, 7, ConnectionState::Connected, None);
        let d = core
            .snapshot()
            .configuration
            .devices
            .into_iter()
            .find(|d| d.id == device)
            .unwrap();
        let observed = KeyboardObservation {
            main: vec![[2, 0, 23, 0]; 128],
            fn_layer: vec![[2, 0, 23, 0]; 128],
            control_indices: BTreeMap::from([("key-49".into(), 49)]),
            layer_revisions: BTreeMap::from([
                (KeyboardLayer::Main, "a".repeat(64)),
                (KeyboardLayer::Fn, "b".repeat(64)),
            ]),
        };
        let binding = Binding::new(ActionKind::Counter, "Fixture counter");
        let binding_id = binding.id.clone();
        let operation = core
            .prepare_keyboard_assignment(KeyboardAssignmentRequest {
                device_id: device.clone(),
                profile_id: d.profile.id,
                page_id: Some(d.profile.active_page_id),
                control_id: "key-49".into(),
                layer: KeyboardLayer::Fn,
                binding,
                observation: observed.clone(),
                generation: 7,
                identity: "confirmed-a".into(),
                expected_revision: core.revision(),
            })
            .unwrap();
        core.complete_keyboard_assignment(KeyboardAssignmentReadback {
            operation_id: operation.operation_id.clone(),
            generation: 7,
            identity: "confirmed-a".into(),
            record: operation.installed,
        })
        .unwrap();
        let mut records = observed.fn_layer;
        records[49] = operation.installed;
        let native_state = json!({"observed":{"keyMappings":{"key-49":{"fn":{"usage":operation.alias+91}}},"keyboardAliases":{"version":1,"layerRevisions":{"main":"a".repeat(64),"fn":"b".repeat(64)},"records":{"main":observed.main,"fn":records}}},"evidence":{"transport":"accepted","readback":"verified","physical":"unverified"},"recovery":{"state":"not_needed","pending":[]}});
        let shared = Arc::new(Shared {
            core: Arc::new(Mutex::new(core)),
            app: None,
            epoch: Arc::new(AtomicU64::new(0)),
            stop: AtomicBool::new(false),
            inventory: Mutex::new(ProviderSnapshot::default()),
            refresh: Mutex::new(BTreeSet::new()),
            edits: Mutex::new(BTreeMap::new()),
            paused: Mutex::new(BTreeSet::new()),
            active: Mutex::new(BTreeSet::new()),
            quiesced: Mutex::new(BTreeMap::new()),
            reload: Mutex::new(BTreeSet::new()),
            lifecycle: Arc::new(crate::plugin_lifecycle::Store::open(
                directory.path().join("lifecycle.json"),
            )),
            recovery_packages: Mutex::new(BTreeSet::new()),
            controls: Mutex::new(VecDeque::new()),
            observations: Mutex::new(BTreeMap::new()),
            key_lighting: Mutex::new(BTreeMap::new()),
            keyboard_input: Mutex::new(BTreeMap::new()),
            keyboard_edits: Mutex::new(BTreeSet::new()),
            next_generation: AtomicU64::new(7),
        });
        let lifecycle_epoch = shared.core.lock().unwrap().device_lifecycle_epoch(&device);
        shared
            .inventory
            .lock()
            .unwrap()
            .discovered
            .push(DiscoveredDevice {
                provider_id: package.manifest.id.clone(),
                instance_id: "confirmed-a".into(),
                name: "Keyboard fixture".into(),
                descriptor: definition.clone(),
                device_id: Some(device.clone()),
                state: "connected".into(),
                error: None,
            });
        let session = Session {
            device: device.clone(),
            generation: 7,
            lifecycle_epoch,
            sequence: 0,
            definition,
            opened: true,
            failed: false,
            edit: 0,
            recovery: json!({"state":"not_needed","pending":[]}),
            released: false,
            observing: false,
            observation_needs_stop: false,
            last_observe: Instant::now(),
            depth_sequence: 0,
            keyboard_sequence: 0,
            read_sequence: 0,
            recovery_preview: None,
        };
        status(&shared, &session, &native_state, None, None);
        let state = ProcessState {
            sessions: BTreeMap::from([("session".into(), session)]),
            pending: BTreeMap::new(),
            next_request: 0,
            hello: true,
        };
        let host = Host {
            shared,
            workers: Mutex::new(vec![]),
        };
        let transport = Some(Transport::start(&package, directory.path(), false).unwrap());
        Self {
            directory,
            package,
            transport,
            host,
            state,
            device,
            binding: binding_id,
            operation,
            native_state,
        }
    }
    fn ready(&self, sequence: u64) {
        let routes = self
            .host
            .shared
            .core
            .lock()
            .unwrap()
            .keyboard_alias_routes();
        assert_eq!(routes.len(), 1);
        self.host.set_keyboard_input(
            &self.device,
            crate::keyboard_assignments::InputConfiguration {
                generation: 7,
                sequence,
                routes,
                ready: true,
                error: None,
            },
        );
    }
    fn event(&mut self, value: Value) {
        handle(
            &self.host.shared,
            &self.package,
            self.transport.as_ref().unwrap(),
            &mut self.state,
            value,
        )
        .unwrap();
    }
    fn alias(&mut self, sequence: u64, configuration: u64, generation: u64) {
        self.event(json!({"type":"keyboardAlias","session":"session","generation":generation,"sequence":sequence,"configurationSequence":configuration,"control":"key-49","layer":"fn","trigger":self.operation.alias}));
    }
    fn count(&self) -> i64 {
        self.host
            .shared
            .core
            .lock()
            .unwrap()
            .snapshot()
            .feedback
            .iter()
            .find(|f| f.binding_id == self.binding)
            .map(|f| f.value)
            .unwrap_or(0)
    }
    fn queue(
        &self,
        generation: u64,
        kind: &'static str,
        payload: Value,
    ) -> Receiver<Result<Value, String>> {
        let (reply, result) = mpsc::channel();
        self.host
            .shared
            .controls
            .lock()
            .unwrap()
            .push_back(RecoveryRequest {
                device: self.device.clone(),
                kind,
                generation: Some(generation),
                payload,
                reply,
            });
        result
    }
    fn reconcile(&mut self) {
        reconcile(
            &self.host.shared,
            &self.package,
            self.transport.as_ref().unwrap(),
            &mut self.state,
        )
        .unwrap();
    }
    fn receive(&mut self) {
        match self
            .transport
            .as_ref()
            .unwrap()
            .read
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
        {
            PipeEvent::Message(value) => self.event(value),
            _ => panic!("Fixture provider failed"),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(transport) = self.transport.take() {
            transport.stop().unwrap();
        }
    }
}

#[test]
fn explicit_alias_rpc_is_generation_scoped_and_never_becomes_settings_replay() {
    let mut f = Fixture::new();
    let initial_settings = f
        .host
        .shared
        .core
        .lock()
        .unwrap()
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == f.device)
        .unwrap()
        .provider_settings;
    assert!(
        f.host
            .keyboard_call(&f.device, 6, "installKeyboardAlias", json!({}))
            .unwrap_err()
            .contains("connection changed")
    );
    assert!(f.host.shared.controls.lock().unwrap().is_empty());
    let stale = f.queue(6, "installKeyboardAlias", json!({}));
    f.reconcile();
    assert!(stale.recv_timeout(Duration::from_secs(1)).unwrap().is_err());
    assert!(f.state.pending.is_empty());
    let reply=f.queue(7,"installKeyboardAlias",json!({"operationId":f.operation.operation_id,"fixtureReply":{"operationId":f.operation.operation_id,"outcome":"installed","currentRecord":f.operation.installed,"state":f.native_state}}));
    f.reconcile();
    assert_eq!(f.state.pending.len(), 1);
    f.receive();
    assert_eq!(
        reply.recv_timeout(Duration::from_secs(1)).unwrap().unwrap()["outcome"],
        "installed"
    );
    f.reconcile();
    f.reconcile();
    let mut core = f.host.shared.core.lock().unwrap();
    let d = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == f.device)
        .unwrap();
    assert_eq!(
        d.provider_settings, initial_settings,
        "Explicit alias transport must not change persistent ordinary settings"
    );
    assert!(f.host.shared.edits.lock().unwrap().is_empty());
    assert!(
        core.take_provider_outputs().is_empty(),
        "Explicit alias RPC must not leave an ordinary settings output for later replay"
    );
    drop(core);
    assert_eq!(
        std::fs::read_to_string(f.directory.path().join("requests.log")).unwrap(),
        "installKeyboardAlias\n"
    );
    assert_eq!(
        f.host.snapshot().sessions[&f.device].observed["keyboardAliases"]["records"]["fn"]
            .as_array()
            .unwrap()
            .len(),
        128
    );
}

#[test]
fn stale_configuration_and_connection_events_cannot_activate_a_current_binding() {
    let mut f = Fixture::new();
    f.ready(41);
    f.alias(1, 41, 6);
    assert_eq!(f.count(), 0);
    assert_eq!(f.state.sessions["session"].keyboard_sequence, 0);
    f.alias(1, 40, 7);
    assert_eq!(f.count(), 0);
    f.alias(2, 41, 7);
    assert_eq!(f.count(), 1);
    f.ready(42);
    f.alias(3, 41, 7);
    assert_eq!(f.count(), 1);
    f.alias(4, 42, 7);
    assert_eq!(f.count(), 2);
    assert!(!f.state.sessions["session"].failed);
    assert!(
        f.host
            .shared
            .core
            .lock()
            .unwrap()
            .snapshot()
            .pressed
            .is_empty()
    );
}

#[test]
fn keyboard_input_state_retires_only_its_current_input_epoch() {
    let mut f = Fixture::new();
    f.ready(10);
    f.event(json!({"type":"keyboardInputState","session":"session","generation":7,"configurationSequence":9,"sourceState":"unpaired"}));
    assert!(f.host.keyboard_input(&f.device).unwrap().ready);
    f.event(json!({"type":"keyboardInputState","session":"session","generation":6,"configurationSequence":10,"sourceState":"unpaired"}));
    assert!(f.host.keyboard_input(&f.device).unwrap().ready);
    f.event(json!({"type":"keyboardInputState","session":"session","generation":7,"configurationSequence":10,"sourceState":"unpaired"}));
    assert!(!f.host.keyboard_input(&f.device).unwrap().ready);
    f.alias(1, 10, 7);
    assert_eq!(f.count(), 0);
    assert!(!f.state.sessions["session"].failed);
    assert_eq!(f.host.snapshot().sessions[&f.device].state, "connected");
    let reply=f.queue(7,"configureKeyboardInput",json!({"sequence":11,"aliases":[],"fixtureReply":{"sourceState":"disabled","configurationSequence":11}}));
    f.reconcile();
    f.receive();
    assert!(reply.recv_timeout(Duration::from_secs(1)).unwrap().is_ok());
    assert!(!f.state.sessions["session"].failed);
}

#[test]
fn alias_sequence_gap_executes_nothing_and_retires_the_input_route() {
    let mut f = Fixture::new();
    f.ready(10);
    f.alias(2, 10, 7);
    assert_eq!(f.count(), 0);
    assert!(
        f.host
            .keyboard_input(&f.device)
            .is_none_or(|config| !config.ready)
    );
    f.alias(3, 10, 7);
    assert_eq!(f.count(), 0);
    assert!(!f.state.sessions["session"].failed);
    assert_eq!(f.host.snapshot().sessions[&f.device].state, "connected");
}

#[test]
fn native_conflict_ack_cannot_implicitly_mark_an_input_route_ready() {
    let mut f = Fixture::new();
    f.ready(20);
    let edit = f.host.begin_keyboard_edit(&f.device).unwrap();
    assert!(!f.host.keyboard_input(&f.device).unwrap().ready);
    drop(edit);
    let reply=f.queue(7,"reconcileKeyboardAlias",json!({"fixtureReply":{"operationId":f.operation.operation_id,"outcome":"conflict","currentRecord":[2,0,24,0],"state":f.native_state}}));
    f.reconcile();
    f.receive();
    assert_eq!(
        reply.recv_timeout(Duration::from_secs(1)).unwrap().unwrap()["outcome"],
        "conflict"
    );
    assert!(!f.host.keyboard_input(&f.device).unwrap().ready);
    f.alias(1, 20, 7);
    assert_eq!(f.count(), 0);
}

#[test]
fn source_ack_publishes_readiness_before_the_immediately_following_alias() {
    let mut f = Fixture::new();
    f.ready(12);
    {
        let mut states = f.host.shared.keyboard_input.lock().unwrap();
        states.get_mut(&f.device).unwrap().ready = false;
    }
    let result=f.queue(7,"configureKeyboardInput",json!({"sequence":12,"aliases":[{}],"fixtureReply":{"sourceState":"available","configurationSequence":12,"aliases":1}}));
    f.reconcile();
    f.receive();
    // Intentionally process the next event before the caller receives its ACK.
    f.alias(1, 12, 7);
    assert_eq!(f.count(), 1);
    assert!(f.host.keyboard_input(&f.device).unwrap().ready);
    assert!(result.recv_timeout(Duration::from_secs(1)).unwrap().is_ok());
}

#[test]
fn rejected_or_malformed_source_ack_retains_a_nonready_configuration_marker() {
    for response in [
        json!({"sourceState":"unpaired","configurationSequence":12,"aliases":0}),
        json!({"sourceState":"available","configurationSequence":12,"aliases":2}),
        json!({"sourceState":"available","configurationSequence":11,"aliases":1}),
    ] {
        let mut f = Fixture::new();
        f.ready(12);
        f.host
            .shared
            .keyboard_input
            .lock()
            .unwrap()
            .get_mut(&f.device)
            .unwrap()
            .ready = false;
        let result = f.queue(
            7,
            "configureKeyboardInput",
            json!({"sequence":12,"aliases":[{}],"fixtureReply":response}),
        );
        f.reconcile();
        f.receive();
        result
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();
        let configuration = f.host.keyboard_input(&f.device).unwrap();
        assert!(!configuration.ready);
        assert_eq!(configuration.sequence, 12);
        assert!(configuration.error.is_some());
        assert!(!f.state.sessions["session"].failed);
        f.alias(1, 12, 7);
        assert_eq!(f.count(), 0);
    }
}
