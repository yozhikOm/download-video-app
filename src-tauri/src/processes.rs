//! The only seam to tool processes: everything that starts a program goes
//! through [`Processes`].

#[cfg(test)]
pub mod fake;

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Output, Stdio};

pub trait Processes: Send + Sync {
    /// Runs `program` and waits for it to exit; errs only when it can't be started.
    fn run(&self, program: &Path, args: &[OsString]) -> io::Result<Output>;

    /// Like [`Processes::run`], but hands each line `program` prints to
    /// stdout to `on_line` as soon as it is printed. The returned stdout is
    /// empty: its lines went to `on_line`.
    fn run_streaming(
        &self,
        program: &Path,
        args: &[OsString],
        on_line: &mut dyn FnMut(&str),
    ) -> io::Result<Output>;
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
    ) -> io::Result<Output> {
        let mut child = command(program, args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
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
        let status = child.wait()?;
        Ok(Output {
            status,
            stdout: Vec::new(),
            stderr: stderr.join().unwrap_or_default(),
        })
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
