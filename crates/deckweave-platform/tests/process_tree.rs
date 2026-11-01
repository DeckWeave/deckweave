#[cfg(windows)]
#[test]
fn dropping_owned_job_terminates_its_backend() {
    use std::{
        os::windows::process::CommandExt,
        process::Command,
        thread,
        time::{Duration, Instant},
    };
    let mut child = Command::new("node")
        .args(["-e", "setInterval(()=>{},1000)"])
        .creation_flags(0x08000000)
        .spawn()
        .unwrap();
    let tree = deckweave_platform::contain_process(&child).unwrap();
    assert!(child.try_wait().unwrap().is_none());
    drop(tree);
    let deadline = Instant::now() + Duration::from_secs(3);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Owned backend survived closing its job");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
