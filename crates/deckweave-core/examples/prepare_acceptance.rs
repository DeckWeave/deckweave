//! Explicit, idempotent local QA setup. Run only while the desktop app is stopped.
use deckweave_core::*;
use serde_json::json;
use std::path::PathBuf;

fn send(core: &mut Core, command: impl FnOnce(u64) -> Command) -> Result<Snapshot> {
    core.dispatch(command(core.revision()), "acceptance-setup")
}
fn program(plan: Plan, label: &str) -> Binding {
    let mut b = Binding::new(ActionKind::Program, label);
    b.plan = Some(plan);
    b
}
fn counter(label: &str) -> Binding {
    Binding::new(ActionKind::Counter, label)
}
fn configure(
    core: &mut Core,
    device: &str,
    page: &str,
    control: &str,
    b: Binding,
) -> Result<Snapshot> {
    send(core, |r| {
        if let Some(plan) = b.plan {
            Command::ConfigureProgram {
                device_id: device.into(),
                page_id: page.into(),
                control_id: control.into(),
                plan,
                label: b.label,
                expected_revision: r,
            }
        } else if let Some(plugin) = b.plugin {
            Command::ConfigurePlugin {
                device_id: device.into(),
                page_id: page.into(),
                control_id: control.into(),
                plugin,
                label: b.label,
                expected_revision: r,
            }
        } else {
            Command::ConfigureControl {
                device_id: device.into(),
                page_id: page.into(),
                control_id: control.into(),
                kind: Some(b.kind),
                label: b.label,
                expected_revision: r,
            }
        }
    })
}
fn plugin(package: &str, action: &str, controller: Controller, label: &str) -> Binding {
    let mut b = Binding::new(ActionKind::Plugin, label);
    b.plugin = Some(PluginAction {
        package: package.into(),
        action: action.into(),
        controller: Some(controller),
        settings: json!({}),
    });
    b
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let path = PathBuf::from(
        args.first()
            .ok_or("Usage: prepare_acceptance <existing sqlite3 path> [--apply]")?,
    );
    if !path.is_file() {
        return Err("Existing database required".into());
    }
    let mut core = Core::open(&path)?;
    let snapshot = core.snapshot();
    let candidates: Vec<_> = snapshot
        .configuration
        .devices
        .iter()
        .filter(|d| d.provider_id == "deckweave.n4")
        .collect();
    if candidates.len() != 1 {
        return Err("Expected exactly one configured N4 instance".into());
    }
    let d = candidates[0];
    let name = "Acceptance 2026-11-01";
    if let Some(existing) = std::iter::once(&d.profile)
        .chain(&d.profiles)
        .find(|p| p.name == name)
    {
        if args.iter().any(|a| a == "--remove-system-demo") && args.iter().any(|a| a == "--apply") {
            let device = d.id.clone();
            let profile = existing.id.clone();
            let first = existing.pages[0].clone();
            send(&mut core, |r| Command::SelectProfile {
                device_id: device.clone(),
                profile_id: profile,
                expected_revision: r,
            })?;
            for (control, label) in [("key-7", "Open Notepad"), ("key-8", "F5 (Notepad)")] {
                if first
                    .bindings
                    .get(control)
                    .is_some_and(|b| b.label == label)
                {
                    send(&mut core, |r| Command::ConfigureControl {
                        device_id: device.clone(),
                        page_id: first.id.clone(),
                        control_id: control.into(),
                        kind: None,
                        label: String::new(),
                        expected_revision: r,
                    })?;
                }
            }
            send(&mut core, |r| Command::SelectPage {
                device_id: device.clone(),
                page_id: first.id,
                expected_revision: r,
            })?;
            println!(
                "Removed the two original OS demo assignments; safe QA actions and other profiles preserved"
            );
            return Ok(());
        }
        println!("Acceptance profile already exists; no changes made");
        return Ok(());
    }
    if !args.iter().any(|a| a == "--apply") {
        println!(
            "Would add independent profile {name} to the existing N4; no changes made. Close the desktop app and pass --apply to prepare it."
        );
        return Ok(());
    }
    // Open/close above may checkpoint SQLite; the ordinary app must not be running.
    let device = d.id.clone();
    drop(core);
    let backup = path.with_extension("sqlite3.before-acceptance.bak");
    if backup.exists() {
        return Err("Backup already exists; refuse to overwrite it".into());
    }
    std::fs::copy(&path, &backup)?;
    let mut core = Core::open(&path)?;
    send(&mut core, |r| Command::AddProfile {
        device_id: device.clone(),
        name: name.into(),
        expected_revision: r,
    })?;
    let first = core
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == device)
        .unwrap()
        .profile
        .active_page_id
        .clone();
    send(&mut core, |r| Command::SetPanelLayout {
        device_id: device.clone(),
        page_id: first.clone(),
        layout: Some(PanelLayout {
            regions: vec![PanelRegion {
                id: new_id(),
                name: "Panel Lab".into(),
                rect: [0., 0., 1., 1.],
                dials: (1..=4).map(|n| format!("dial-{n}")).collect(),
            }],
            swipe_pages: true,
        }),
        expected_revision: r,
    })?;
    let region = core.snapshot().geometries[&device]
        .controls
        .iter()
        .find(|c| c.kind == ControlKind::Touch)
        .unwrap()
        .id
        .clone();
    configure(
        &mut core,
        &device,
        &first,
        &region,
        plugin(
            "dev.deckweave.panel",
            "dev.deckweave.panel.slider",
            Controller::Panel,
            "Panel Lab",
        ),
    )?;
    configure(
        &mut core,
        &device,
        &first,
        "key-1",
        program(
            Plan::Switch {
                states: vec![counter("A"), counter("B")],
            },
            "Switch A/B",
        ),
    )?;
    configure(
        &mut core,
        &device,
        &first,
        "key-2",
        program(
            Plan::KeyLogic {
                single: Some(Box::new(counter("Single"))),
                double: Some(Box::new(counter("Double"))),
                hold: Some(Box::new(counter("Hold"))),
                double_ms: 350,
                hold_ms: 650,
            },
            "Key Logic",
        ),
    )?;
    configure(
        &mut core,
        &device,
        &first,
        "key-3",
        program(
            Plan::Sequence {
                steps: vec![
                    Step {
                        delay_ms: 0,
                        action: counter("First"),
                    },
                    Step {
                        delay_ms: 800,
                        action: counter("Second"),
                    },
                    Step {
                        delay_ms: 800,
                        action: counter("Third"),
                    },
                ],
            },
            "Multi Action",
        ),
    )?;
    send(&mut core, |r| Command::AddFolder {
        device_id: device.clone(),
        control_id: "key-4".into(),
        name: "Folder".into(),
        expected_revision: r,
    })?;
    configure(
        &mut core,
        &device,
        &first,
        "key-5",
        plugin(
            "fail.marc.onairclock",
            "fail.marc.onairclock.action",
            Controller::Keypad,
            "OnAirClock",
        ),
    )?;
    send(&mut core, |r| Command::AddPage {
        device_id: device.clone(),
        expected_revision: r,
    })?;
    let second = core
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == device)
        .unwrap()
        .profile
        .active_page_id
        .clone();
    configure(
        &mut core,
        &device,
        &first,
        "key-6",
        program(
            Plan::Page {
                page_id: second.clone(),
            },
            "Dials page",
        ),
    )?;
    configure(
        &mut core,
        &device,
        &second,
        "key-1",
        program(
            Plan::Page {
                page_id: first.clone(),
            },
            "Panel Lab page",
        ),
    )?;
    configure(
        &mut core,
        &device,
        &second,
        "dial-1",
        plugin(
            "fail.marc.onairclock",
            "fail.marc.onairclock.action",
            Controller::Encoder,
            "Clock Encoder",
        ),
    )?;
    configure(
        &mut core,
        &device,
        &second,
        "dial-2",
        program(
            Plan::DialStack {
                actions: vec![counter("Stack A"), counter("Stack B")],
            },
            "Dial Stack",
        ),
    )?;
    configure(
        &mut core,
        &device,
        &second,
        "dial-3",
        program(
            Plan::ActionWheel {
                actions: vec![counter("Wheel A"), counter("Wheel B")],
            },
            "Action Wheel",
        ),
    )?;
    configure(
        &mut core,
        &device,
        &second,
        "dial-4",
        program(
            Plan::DialBindings {
                left: Some(Box::new(counter("Left"))),
                right: Some(Box::new(counter("Right"))),
                press: Some(Box::new(Binding::new(ActionKind::Toggle, "Push"))),
                tap: Some(Box::new(Binding::new(ActionKind::Toggle, "Tap"))),
            },
            "Dial actions",
        ),
    )?;
    send(&mut core, |r| Command::SelectPage {
        device_id: device.clone(),
        page_id: first,
        expected_revision: r,
    })?;
    println!(
        "Prepared {name}; previous profiles preserved and database backed up. No actions executed."
    );
    Ok(())
}
