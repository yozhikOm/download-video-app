use std::ffi::OsString;
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

#[test]
fn a_finished_stopper_no_longer_stops() {
    let stopper = Stopper::default();

    assert!(!stopper.finish());
    assert!(!stopper.stop());
}
