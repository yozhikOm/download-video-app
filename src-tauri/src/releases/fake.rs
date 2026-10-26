//! Scripted [`Releases`] for tests: publishes small fake releases and lets a
//! test take the network down or tamper with a checksum.

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::{Condvar, Mutex};

use sha2::{Digest, Sha256};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::{Package, Release, Releases, ReleasesError};

pub const YTDLP_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp.exe";
pub const FFMPEG_URL: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";
pub const DENO_URL: &str =
    "https://github.com/denoland/deno/releases/download/v2.9.8/deno-x86_64-pc-windows-msvc.zip";
pub const YTDLP_BYTES: &[u8] = b"fake yt-dlp 2026.08.19";
pub const FFMPEG_BYTES: &[u8] = b"fake ffmpeg";
pub const FFPROBE_BYTES: &[u8] = b"fake ffprobe";
pub const DENO_BYTES: &[u8] = b"fake deno 2.9.8";

/// Bytes per progress report, so even tiny fake downloads report progress.
const CHUNK: usize = 4;

pub struct FakeReleases {
    scenario: Mutex<Scenario>,
    /// Woken when a fetch starts waiting or fetches are let through.
    held_changed: Condvar,
}

struct Scenario {
    online: bool,
    releases: HashMap<Package, (Release, Vec<u8>)>,
    fetches: Vec<String>,
    /// Fetches wait until the test lets them through.
    held: bool,
    /// Fetches waiting while held.
    waiting: usize,
}

impl FakeReleases {
    /// Online, with a yt-dlp release, an FFmpeg-Builds zip and a deno zip
    /// published.
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
            (Package::Deno, release("v2.9.8", DENO_URL, deno_zip())),
        ]);
        Self {
            scenario: Mutex::new(Scenario {
                online: true,
                releases,
                fetches: Vec::new(),
                held: false,
                waiting: 0,
            }),
            held_changed: Condvar::new(),
        }
    }

    /// Makes each fetch wait until [`FakeReleases::release_fetches`], as a
    /// slow download would.
    pub fn hold_fetches(&self) {
        self.scenario().held = true;
    }

    /// Blocks until a held fetch waits.
    pub fn wait_until_fetch_held(&self) {
        let scenario = self.scenario();
        drop(
            self.held_changed
                .wait_while(scenario, |scenario| scenario.waiting == 0)
                .unwrap(),
        );
    }

    /// Lets the held fetches through, and the later ones at once.
    pub fn release_fetches(&self) {
        self.scenario().held = false;
        self.held_changed.notify_all();
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

    /// Asset URLs fetched so far, in order.
    pub fn fetches(&self) -> Vec<String> {
        self.scenario().fetches.clone()
    }

    fn scenario(&self) -> std::sync::MutexGuard<'_, Scenario> {
        self.scenario.lock().unwrap()
    }
}

impl Default for FakeReleases {
    fn default() -> Self {
        Self::new()
    }
}

impl Releases for FakeReleases {
    fn latest_release(&self, package: Package) -> Result<Release, ReleasesError> {
        let scenario = self.scenario();
        if !scenario.online {
            return Err(ReleasesError::Offline);
        }
        Ok(scenario.releases[&package].0.clone())
    }

    fn fetch(
        &self,
        url: &str,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), ReleasesError> {
        let bytes = {
            let mut scenario = self.scenario();
            if scenario.held {
                scenario.waiting += 1;
                self.held_changed.notify_all();
                scenario = self
                    .held_changed
                    .wait_while(scenario, |scenario| scenario.held)
                    .unwrap();
                scenario.waiting -= 1;
            }
            if !scenario.online {
                return Err(ReleasesError::Offline);
            }
            scenario.fetches.push(url.to_string());
            let found = scenario.releases.values().find(|(r, _)| r.url == url);
            found.map(|(_, bytes)| bytes.clone())
        };
        let bytes = bytes.ok_or_else(|| ReleasesError::Other(format!("404 {url}")))?;
        let mut received = 0;
        for chunk in bytes.chunks(CHUNK) {
            out.write_all(chunk).unwrap();
            received += chunk.len() as u64;
            progress(received);
        }
        Ok(())
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

/// Laid out like the real deno zip: just `deno.exe`.
fn deno_zip() -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("deno.exe", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(DENO_BYTES).unwrap();
    zip.finish().unwrap().into_inner()
}
