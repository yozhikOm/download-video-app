//! The Windows job that holds every Tool process, so that the system kills
//! them when the app ends, however it ends: Windows leaves the programs a
//! process started running after it exits, and a yt-dlp left so keeps the
//! Partials it writes open.

use std::ffi::c_void;
use std::io;
use std::mem;
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::ptr;
use std::sync::Mutex;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

/// The job's handle, never closed: the system closes it when the app ends,
/// which kills the programs in the job.
#[derive(Clone, Copy)]
struct Job(HANDLE);

// The handle is only passed to the system, which takes it on any thread.
unsafe impl Send for Job {}

/// Puts `child`, started suspended, into the job and lets it run. Programs
/// it starts join the job too; it can't start one before it is in.
pub(super) fn adopt(child: &Child) -> io::Result<()> {
    let job = job()?;
    // SAFETY: both handles are open: the job's for as long as the app runs,
    // the child's for as long as `child` lives.
    if unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    resume(child.id())
}

/// Whether the program of `child` is in the job.
#[cfg(test)]
pub(super) fn holds(child: &Child) -> io::Result<bool> {
    use windows_sys::Win32::System::JobObjects::IsProcessInJob;

    let job = job()?;
    let mut held = 0;
    // SAFETY: both handles are open, as in `adopt`.
    if unsafe { IsProcessInJob(child.as_raw_handle(), job.0, &mut held) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(held != 0)
}

/// The job, created once; a failure is tried again with the next program.
fn job() -> io::Result<Job> {
    static JOB: Mutex<Option<Job>> = Mutex::new(None);
    let mut job = JOB.lock().unwrap();
    if job.is_none() {
        *job = Some(create()?);
    }
    Ok(job.expect("created above"))
}

fn create() -> io::Result<Job> {
    // SAFETY: no name and default security; the limits are a valid struct
    // of the size passed.
    unsafe {
        let job = CreateJobObjectW(ptr::null(), ptr::null());
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast::<c_void>(),
            mem::size_of_val(&limits) as u32,
        );
        if set == 0 {
            let error = io::Error::last_os_error();
            CloseHandle(job);
            return Err(error);
        }
        Ok(Job(job))
    }
}

/// Resumes the program `pid`, started suspended: the system has no call for
/// a process, so each of its threads is resumed, only its first while it is
/// suspended.
fn resume(pid: u32) -> io::Result<()> {
    // SAFETY: the snapshot is closed below, and nowhere else.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let resumed = resume_threads(snapshot, pid);
        CloseHandle(snapshot);
        resumed
    }
}

/// # Safety
///
/// `snapshot` is an open snapshot of the threads.
unsafe fn resume_threads(snapshot: HANDLE, pid: u32) -> io::Result<()> {
    // SAFETY: the entry is a valid struct whose size is set, as the calls
    // require; each thread handle is closed once used.
    unsafe {
        let mut entry = THREADENTRY32 {
            dwSize: mem::size_of::<THREADENTRY32>() as u32,
            ..mem::zeroed()
        };
        let mut resumed = false;
        let mut found = Thread32First(snapshot, &mut entry) != 0;
        while found {
            if entry.th32OwnerProcessID == pid {
                let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if thread.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let count = ResumeThread(thread);
                let error = io::Error::last_os_error();
                CloseHandle(thread);
                if count == u32::MAX {
                    return Err(error);
                }
                resumed = true;
            }
            found = Thread32Next(snapshot, &mut entry) != 0;
        }
        if resumed {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no thread of the program {pid} to resume"),
            ))
        }
    }
}
