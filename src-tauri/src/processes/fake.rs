//! Scripted [`Processes`] for tests: records the programs started, answers
//! with scripted output and lets a test break a tool.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Condvar, Mutex};

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
    /// Whether the program running through [`Processes::run_streaming`] now
    /// was killed.
    running: Option<Arc<Killed>>,
}

/// Set once a program is killed.
#[derive(Default)]
struct Killed(Mutex<bool>, Condvar);

impl Killed {
    /// Sets it and wakes whoever waits for it.
    fn kill(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_all();
    }

    /// Whether the program was killed.
    fn is_set(&self) -> bool {
        *self.0.lock().unwrap()
    }

    /// Waits until the program is killed.
    fn wait(&self) {
        let _killed = self.1.wait_while(self.0.lock().unwrap(), |killed| !*killed);
    }
}

impl FakeProcesses {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes any program with this file name fail to start, and one
    /// running now fail to be waited for.
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

    /// For a script: acts like a program that runs until it is killed.
    /// Returns at once when it was already, or when it can't be killed.
    pub fn wait_until_killed(&self) {
        // Not under the scenario's lock, which the kill may need.
        let running = self.scenario().running.clone();
        if let Some(killed) = running {
            killed.wait();
        }
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
        let killed = Arc::new(Killed::default());
        self.scenario().running = Some(killed.clone());
        let kill = killed.clone();
        stopper.on_stop(move || kill.kill());
        let output = self.run(program, args);
        stopper.exited();
        self.scenario().running = None;
        let mut output = output?;
        // One broken while it ran can't be waited for.
        let name = program.file_name().unwrap_or(OsStr::new(""));
        if self.scenario().broken.contains(name) {
            return Err(io::Error::other("the program can't be waited for"));
        }
        if killed.is_set() {
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
