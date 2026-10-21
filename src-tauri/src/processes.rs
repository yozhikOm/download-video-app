//! The only seam to tool processes: everything that starts a program goes
//! through [`Processes`].

#[cfg(test)]
pub mod fake;

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

pub trait Processes: Send + Sync {
    /// Runs `program` and waits for it to exit; errs only when it can't be started.
    fn run(&self, program: &Path, args: &[OsString]) -> io::Result<Output>;
}

/// [`Processes`] started by the operating system, without a console window.
pub struct SystemProcesses;

impl Processes for SystemProcesses {
    fn run(&self, program: &Path, args: &[OsString]) -> io::Result<Output> {
        let mut command = Command::new(program);
        command.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.output()
    }
}
