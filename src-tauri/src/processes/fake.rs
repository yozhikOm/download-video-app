//! Scripted [`Processes`] for tests: records the programs started and lets a
//! test break a tool.

use std::collections::HashSet;
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

    /// Programs started so far with their arguments, in order.
    pub fn runs(&self) -> Vec<(PathBuf, Vec<OsString>)> {
        self.scenario().runs.clone()
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
        Ok(Output {
            status: ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}
