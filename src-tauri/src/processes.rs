//! The only seam to tool processes: everything that starts a program goes
//! through [`Processes`].

#[cfg(test)]
pub mod fake;
#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;

pub trait Processes: Send + Sync {
    /// Runs `program` and waits for it to exit; errs only when it can't be started.
    fn run(&self, program: &Path, args: &[OsString]) -> io::Result<Output>;

    /// Like [`Processes::run`], but hands each line `program` prints to
    /// stdout to `on_line` as soon as it is printed, and `stopper` kills it,
    /// with the programs it started, from another thread. The returned
    /// stdout is empty: its lines went to `on_line`.
    fn run_streaming(
        &self,
        program: &Path,
        args: &[OsString],
        on_line: &mut dyn FnMut(&str),
        stopper: &Stopper,
    ) -> io::Result<Output>;
}

/// Stops the work of one caller from another thread: kills the program it
/// runs through [`Processes::run_streaming`] now, and any it starts later,
/// until the work is finished.
#[derive(Default)]
pub struct Stopper(Mutex<StopperState>);

#[derive(Default)]
struct StopperState {
    stopped: bool,
    finished: bool,
    /// Kills the program running now.
    kill: Option<Box<dyn FnOnce() + Send>>,
}

impl Stopper {
    /// Kills the program running now; `false` once the work is finished,
    /// when there is nothing left to stop.
    pub fn stop(&self) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.finished {
            return false;
        }
        state.stopped = true;
        // Killed under the lock, so that the program can't be found exited
        // and its process id given to another program meanwhile.
        if let Some(kill) = state.kill.take() {
            kill();
        }
        true
    }

    /// Ends the work, so that it can no longer be stopped; returns whether
    /// it was.
    pub fn finish(&self) -> bool {
        let mut state = self.0.lock().unwrap();
        state.finished = true;
        state.kill = None;
        state.stopped
    }

    /// For [`Processes`]: `kill` kills the program just started when the
    /// work is stopped, at once when it is already.
    pub fn on_stop(&self, kill: impl FnOnce() + Send + 'static) {
        let mut state = self.0.lock().unwrap();
        if state.stopped {
            drop(state);
            kill();
        } else {
            state.kill = Some(Box::new(kill));
        }
    }

    /// For [`Processes`]: the program has exited, so that its process id,
    /// which another program may get next, is no longer killed.
    pub fn exited(&self) {
        self.0.lock().unwrap().kill = None;
    }
}

/// [`Processes`] started by the operating system, without a console window.
pub struct SystemProcesses;

impl Processes for SystemProcesses {
    fn run(&self, program: &Path, args: &[OsString]) -> io::Result<Output> {
        command(program, args).output()
    }

    fn run_streaming(
        &self,
        program: &Path,
        args: &[OsString],
        on_line: &mut dyn FnMut(&str),
        stopper: &Stopper,
    ) -> io::Result<Output> {
        let mut child = command(program, args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let pid = child.id();
        stopper.on_stop(move || kill_tree(pid));
        // Read on its own thread, so that a program that fills the stderr
        // pipe doesn't stall waiting for it while stdout is being read.
        let mut stderr_pipe = child.stderr.take().expect("stderr is piped");
        let stderr = std::thread::spawn(move || {
            let mut stderr = Vec::new();
            let _ = stderr_pipe.read_to_end(&mut stderr);
            stderr
        });
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        for line in stdout.split(b'\n') {
            let Ok(line) = line else { break };
            let line = String::from_utf8_lossy(&line);
            on_line(line.trim_end_matches('\r'));
        }
        let status = child.wait();
        stopper.exited();
        let status = status?;
        Ok(Output {
            status,
            stdout: Vec::new(),
            stderr: stderr.join().unwrap_or_default(),
        })
    }
}

/// Kills the program with the process id `pid` and the programs it started,
/// such as the ffmpeg yt-dlp runs, which would otherwise keep its files
/// open. Returns once they are killed, so that whoever goes on after the
/// program has exited finds its files let go, whatever order they die in.
fn kill_tree(pid: u32) {
    #[cfg(windows)]
    let killer = command(
        Path::new("taskkill"),
        &[
            "/F".into(),
            "/T".into(),
            "/PID".into(),
            pid.to_string().into(),
        ],
    )
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();
    // Elsewhere only the program itself is killed.
    #[cfg(not(windows))]
    let killer = command(Path::new("kill"), &["-KILL".into(), pid.to_string().into()]).status();
    if let Err(error) = killer {
        eprintln!("can't kill the program {pid}: {error}");
    }
}

fn command(program: &Path, args: &[OsString]) -> Command {
    let mut command = Command::new(program);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
