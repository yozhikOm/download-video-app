//! Scripted [`Processes`] for tests: records the programs started, answers
//! with scripted output and lets a test break a tool.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::{Processes, Stopper};

#[derive(Default)]
pub struct FakeProcesses(Mutex<Scenario>);

/// Answers a run from its arguments, and may act like the program on disk.
type Script = Arc<dyn Fn(&[OsString]) -> Output + Send + Sync>;

#[derive(Default)]
struct Scenario {
    broken: HashSet<OsString>,
    answers: HashMap<OsString, Script>,
    runs: Vec<(PathBuf, Vec<OsString>)>,
    kills: usize,
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

    /// Makes any program with this file name answer with what `script`
    /// returns for the arguments it was started with.
    pub fn script(
        &self,
        file_name: &str,
        script: impl Fn(&[OsString]) -> Output + Send + Sync + 'static,
    ) {
        self.scenario()
            .answers
            .insert(file_name.into(), Arc::new(script));
    }

    /// Programs started so far with their arguments, in order.
    pub fn runs(&self) -> Vec<(PathBuf, Vec<OsString>)> {
        self.scenario().runs.clone()
    }

    /// How many programs were killed while they ran.
    pub fn kills(&self) -> usize {
        self.scenario().kills
    }

    fn answer(&self, file_name: &str, status: ExitStatus, stdout: &[u8], stderr: &[u8]) {
        let output = Output {
            status,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        };
        self.script(file_name, move |_| output.clone());
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
        let Some(script) = scenario.answers.get(name).cloned() else {
            return Ok(Output {
                status: exit_status(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });
        };
        // The script may start programs of its own.
        drop(scenario);
        Ok(script(args))
    }

    fn run_streaming(
        &self,
        program: &Path,
        args: &[OsString],
        on_line: &mut dyn FnMut(&str),
        stopper: &Stopper,
    ) -> io::Result<Output> {
        let killed = Arc::new(AtomicBool::new(false));
        let kill = killed.clone();
        stopper.on_stop(move || kill.store(true, Ordering::SeqCst));
        let output = self.run(program, args);
        stopper.exited();
        let mut output = output?;
        if killed.load(Ordering::SeqCst) {
            self.scenario().kills += 1;
            // A killed program prints nothing more and exits with an error.
            return Ok(Output {
                status: exit_status(1),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            on_line(line);
        }
        output.stdout.clear();
        Ok(output)
    }
}

#[cfg(windows)]
pub fn exit_status(code: u32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    ExitStatus::from_raw(code)
}

#[cfg(unix)]
pub fn exit_status(code: u32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    // A wait status keeps the exit code in its second byte.
    ExitStatus::from_raw((code as i32) << 8)
}
