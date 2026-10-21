//! Scripted [`Tools`] for tests: publishes small fake releases and lets a test
//! take the network down, tamper with a checksum or break a tool.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::{Package, Release, Tools, ToolsError};

pub const YTDLP_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp.exe";
pub const FFMPEG_URL: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";
pub const YTDLP_BYTES: &[u8] = b"fake yt-dlp 2026.08.19";
pub const FFMPEG_BYTES: &[u8] = b"fake ffmpeg";
pub const FFPROBE_BYTES: &[u8] = b"fake ffprobe";

/// Bytes per progress report, so even tiny fake downloads report progress.
const CHUNK: usize = 4;

pub struct FakeTools(Mutex<Scenario>);

struct Scenario {
    online: bool,
    releases: HashMap<Package, (Release, Vec<u8>)>,
    broken: HashSet<OsString>,
    fetches: Vec<String>,
    runs: Vec<(PathBuf, Vec<OsString>)>,
}

impl FakeTools {
    /// Online, with a yt-dlp release and an FFmpeg-Builds zip published.
    pub fn new() -> Self {
        let releases = HashMap::from([
            (
                Package::YtDlp,
                release("2026.08.19", YTDLP_URL, YTDLP_BYTES.to_vec()),
            ),
            (
                Package::Ffmpeg,
                release("2026-10-20T18:32:17Z", FFMPEG_URL, ffmpeg_zip()),
            ),
        ]);
        Self(Mutex::new(Scenario {
            online: true,
            releases,
            broken: HashSet::new(),
            fetches: Vec::new(),
            runs: Vec::new(),
        }))
    }

    pub fn set_online(&self, online: bool) {
        self.scenario().online = online;
    }

    /// Makes the release publish a checksum that its asset doesn't match.
    pub fn tamper_checksum(&self, package: Package) {
        self.scenario().releases.get_mut(&package).unwrap().0.sha256 = "0".repeat(64);
    }

    /// Makes the release point at `url` instead of its official asset.
    pub fn move_asset(&self, package: Package, url: &str) {
        let mut scenario = self.scenario();
        let (release, bytes) = scenario.releases.remove(&package).unwrap();
        let moved = (
            Release {
                url: url.to_string(),
                ..release
            },
            bytes,
        );
        scenario.releases.insert(package, moved);
    }

    /// Makes any program with this file name fail to start.
    pub fn break_tool(&self, file_name: &str) {
        self.scenario().broken.insert(file_name.into());
    }

    /// Asset URLs fetched so far, in order.
    pub fn fetches(&self) -> Vec<String> {
        self.scenario().fetches.clone()
    }

    /// Programs started so far with their arguments, in order.
    pub fn runs(&self) -> Vec<(PathBuf, Vec<OsString>)> {
        self.scenario().runs.clone()
    }

    fn scenario(&self) -> std::sync::MutexGuard<'_, Scenario> {
        self.0.lock().unwrap()
    }
}

impl Default for FakeTools {
    fn default() -> Self {
        Self::new()
    }
}

impl Tools for FakeTools {
    fn latest_release(&self, package: Package) -> Result<Release, ToolsError> {
        let scenario = self.scenario();
        if !scenario.online {
            return Err(ToolsError::Offline);
        }
        Ok(scenario.releases[&package].0.clone())
    }

    fn fetch(
        &self,
        url: &str,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), ToolsError> {
        let bytes = {
            let mut scenario = self.scenario();
            if !scenario.online {
                return Err(ToolsError::Offline);
            }
            scenario.fetches.push(url.to_string());
            let found = scenario.releases.values().find(|(r, _)| r.url == url);
            found.map(|(_, bytes)| bytes.clone())
        };
        let bytes = bytes.ok_or_else(|| ToolsError::Other(format!("404 {url}")))?;
        let mut received = 0;
        for chunk in bytes.chunks(CHUNK) {
            out.write_all(chunk).unwrap();
            received += chunk.len() as u64;
            progress(received);
        }
        Ok(())
    }

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

fn release(version: &str, url: &str, bytes: Vec<u8>) -> (Release, Vec<u8>) {
    let sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let release = Release {
        version: version.to_string(),
        url: url.to_string(),
        size: bytes.len() as u64,
        sha256,
    };
    (release, bytes)
}

/// Laid out like the real FFmpeg-Builds zip.
fn ffmpeg_zip() -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let root = "ffmpeg-master-latest-win64-gpl";
    for (name, bytes) in [
        ("LICENSE.txt", b"GPL".as_slice()),
        ("bin/ffmpeg.exe", FFMPEG_BYTES),
        ("bin/ffplay.exe", b"fake ffplay"),
        ("bin/ffprobe.exe", FFPROBE_BYTES),
    ] {
        zip.start_file(format!("{root}/{name}"), SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
