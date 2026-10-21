//! Scripted [`Processes`] for tests: records the programs started, answers
//! with scripted output and lets a test break a tool.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::Mutex;

use super::Processes;

#[derive(Default)]
pub struct FakeProcesses(Mutex<Scenario>);

#[derive(Default)]
struct Scenario {
    broken: HashSet<OsString>,
    answers: HashMap<OsString, Output>,
    runs: Vec<(PathBuf, Vec<OsString>)>,
}

impl FakeProcesses {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes any program with this file name fail to start.
    pub fn break_tool(&self, file_name: &str) {
        self.scenario().broken.insert(file_name.into());
    }

    /// Makes any program with this file name print `stdout` and exit with success.
    pub fn succeed_with(&self, file_name: &str, stdout: &[u8]) {
        self.answer(file_name, exit_status(0), stdout, b"");
    }

    /// Makes any program with this file name print `stderr` and exit with code 1.
    pub fn fail_with(&self, file_name: &str, stderr: &[u8]) {
        self.answer(file_name, exit_status(1), b"", stderr);
    }

    /// Programs started so far with their arguments, in order.
    pub fn runs(&self) -> Vec<(PathBuf, Vec<OsString>)> {
        self.scenario().runs.clone()
    }

    fn answer(&self, file_name: &str, status: ExitStatus, stdout: &[u8], stderr: &[u8]) {
        let output = Output {
            status,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        };
        self.scenario().answers.insert(file_name.into(), output);
    }

    fn scenario(&self) -> std::sync::MutexGuard<'_, Scenario> {
        self.0.lock().unwrap()
    }
}

impl Processes for FakeProcesses {
    fn run(&self, program: &Path, args: &[OsString]) -> io::Result<Output> {
        let mut scenario = self.scenario();
        scenario.runs.push((program.to_path_buf(), args.to_vec()));
        let name = program.file_name().unwrap_or(OsStr::new(""));
        if scenario.broken.contains(name) || !program.is_file() {
            return Err(io::Error::other("the program can't be started"));
        }
        Ok(scenario.answers.get(name).cloned().unwrap_or(Output {
            status: exit_status(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }))
    }
}

#[cfg(windows)]
fn exit_status(code: u32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    ExitStatus::from_raw(code)
}

#[cfg(unix)]
fn exit_status(code: u32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    // A wait status keeps the exit code in its second byte.
    ExitStatus::from_raw((code as i32) << 8)
}
