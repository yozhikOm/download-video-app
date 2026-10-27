use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use super::{Processes, Stopper, SystemProcesses};

/// `cmd` starting `ping`, which prints a line a second for half a minute
/// and holds `cmd`'s stdout open while it runs.
#[cfg(windows)]
fn ping_through_cmd() -> Vec<OsString> {
    ["/C", "ping", "-n", "30", "127.0.0.1"]
        .map(OsString::from)
        .into()
}

#[cfg(windows)]
#[test]
fn a_stopped_program_is_killed_with_the_programs_it_started() {
    let stopper = Stopper::default();
    let started = Instant::now();

    let output = SystemProcesses
        .run_streaming(
            Path::new("cmd"),
            &ping_through_cmd(),
            &mut |_| {
                stopper.stop();
            },
            &stopper,
        )
        .unwrap();

    assert!(!output.status.success());
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "ping ran on for {:?}",
        started.elapsed()
    );
}

#[cfg(windows)]
#[test]
fn a_program_started_once_stopped_is_killed_at_once() {
    let stopper = Stopper::default();
    assert!(stopper.stop());
    let started = Instant::now();

    let output = SystemProcesses
        .run_streaming(Path::new("cmd"), &ping_through_cmd(), &mut |_| {}, &stopper)
        .unwrap();

    assert!(!output.status.success());
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[cfg(windows)]
#[test]
fn a_stopped_program_has_let_go_of_its_files_once_it_returns() {
    let dir = tempfile::TempDir::new().unwrap();
    let file = dir.path().join("ping.txt");
    // `ping` writes the file, so that `cmd`'s pipes don't wait for it.
    let mut args: Vec<OsString> = ["/C", "ping", "-n", "30", "127.0.0.1", ">"]
        .map(OsString::from)
        .into();
    args.extend([file.clone().into_os_string(), "2>&1".into()]);
    let stopper = Stopper::default();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(500));
            stopper.stop();
        });
        SystemProcesses
            .run_streaming(Path::new("cmd"), &args, &mut |_| {}, &stopper)
            .unwrap();
        fs::remove_file(&file).unwrap();
    });
}

#[cfg(windows)]
#[test]
fn a_program_runs_in_the_job_that_dies_with_the_app() {
    let mut child =
        super::spawn(&mut super::command(Path::new("cmd"), &ping_through_cmd())).unwrap();

    let held = super::job::holds(&child);
    let _ = child.kill();
    let _ = child.wait();
    assert!(held.unwrap());
}

#[cfg(windows)]
#[test]
fn a_program_started_in_the_job_runs_to_its_end() {
    let output = SystemProcesses
        .run(Path::new("cmd"), &["/C".into(), "echo started".into()])
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "started");
}

#[test]
fn a_finished_stopper_no_longer_stops() {
    let stopper = Stopper::default();

    assert!(!stopper.finish());
    assert!(!stopper.stop());
}
