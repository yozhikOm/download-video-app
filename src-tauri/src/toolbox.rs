//! The app's own copies of yt-dlp, ffmpeg, ffprobe and deno: installing and
//! restoring them, and running them. Copies on PATH are never used.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::clock::Clock;
use crate::events::{AppEvent, EventSink};
use crate::processes::Processes;
use crate::releases::{Package, Release, Releases, ReleasesError};

const MANIFEST: &str = "manifest.json";
/// Suffix of files that are still downloading or unverified.
const TEMP_SUFFIX: &str = ".download";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    YtDlp,
    Ffmpeg,
    Ffprobe,
    /// The JavaScript runtime that yt-dlp needs to extract YouTube.
    Deno,
}

impl Tool {
    const ALL: [Tool; 4] = [Tool::YtDlp, Tool::Ffmpeg, Tool::Ffprobe, Tool::Deno];

    fn file_name(self) -> &'static str {
        match self {
            Tool::YtDlp => "yt-dlp.exe",
            Tool::Ffmpeg => "ffmpeg.exe",
            Tool::Ffprobe => "ffprobe.exe",
            Tool::Deno => "deno.exe",
        }
    }

    fn package(self) -> Package {
        match self {
            Tool::YtDlp => Package::YtDlp,
            Tool::Ffmpeg | Tool::Ffprobe => Package::Ffmpeg,
            Tool::Deno => Package::Deno,
        }
    }

    /// Whether `entry` is this tool in its package's zip.
    fn is_archived_as(self, entry: &str) -> bool {
        match self {
            Tool::YtDlp => false,
            Tool::Ffmpeg | Tool::Ffprobe => entry.ends_with(&format!("/bin/{}", self.file_name())),
            Tool::Deno => entry == self.file_name(),
        }
    }
}

/// What the setup screen shows. Only [`SetupState::Ready`] lets the app past it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum SetupState {
    Checking,
    #[serde(rename_all = "camelCase")]
    Installing {
        downloaded: u64,
        /// Zero while the download size isn't known yet.
        total: u64,
        /// The tool that failed to start and is being restored.
        broken: Option<Tool>,
    },
    #[serde(rename_all = "camelCase")]
    Failed {
        error: SetupError,
        broken: Option<Tool>,
    },
    Ready,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SetupError {
    Offline,
    ChecksumMismatch {
        package: Package,
    },
    /// The message is for the log; the UI shows a generic text.
    Other {
        message: String,
    },
}

/// A tool failed to start, so the app needs setup again to restore it.
#[derive(Debug, PartialEq)]
pub struct ToolBroken(pub Tool);

/// What was installed, so a launch can check the tools without hashing them.
#[derive(Default, Serialize, Deserialize)]
struct Manifest {
    files: BTreeMap<String, InstalledFile>,
    /// Seconds since the Unix epoch.
    ytdlp_checked_at: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct InstalledFile {
    version: String,
    size: u64,
    sha256: String,
}

/// The app's own installed tools: setting them up and running them.
pub struct Toolbox {
    dir: PathBuf,
    releases: Arc<dyn Releases>,
    processes: Arc<dyn Processes>,
    clock: Arc<dyn Clock>,
    events: Arc<dyn EventSink>,
    state: Mutex<SetupState>,
    /// Held while preparing, so concurrent requests don't download twice.
    preparing: Mutex<()>,
    /// The tool being restored after it failed to start.
    broken: Mutex<Option<Tool>>,
}

impl Toolbox {
    pub fn new(
        dir: PathBuf,
        releases: Arc<dyn Releases>,
        processes: Arc<dyn Processes>,
        clock: Arc<dyn Clock>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            dir,
            releases,
            processes,
            clock,
            events,
            state: Mutex::new(SetupState::Checking),
            preparing: Mutex::new(()),
            broken: Mutex::new(None),
        }
    }

    pub fn state(&self) -> SetupState {
        self.state.lock().unwrap().clone()
    }

    /// Makes sure all tools are in place, downloading whatever is missing.
    /// Blocks until it ends in [`SetupState::Ready`] or [`SetupState::Failed`].
    /// Returns at once if another call is already preparing.
    pub fn prepare(&self) {
        let Ok(_preparing) = self.preparing.try_lock() else {
            return;
        };
        let broken = *self.broken.lock().unwrap();
        // When restoring a broken tool, the setup screen is already up.
        if broken.is_none() {
            self.set_state(SetupState::Checking);
        }
        match self.install_missing() {
            Ok(()) => {
                *self.broken.lock().unwrap() = None;
                self.set_state(SetupState::Ready);
            }
            Err(error) => self.set_state(SetupState::Failed { error, broken }),
        }
    }

    /// Runs a tool from the app's own folder; yt-dlp ignores the user's
    /// config files and is pointed at the app's own ffmpeg and deno, never
    /// at a JavaScript runtime installed elsewhere.
    ///
    /// On [`ToolBroken`] the setup screen is already showing, and the caller
    /// starts [`Toolbox::prepare`] to restore the tool.
    pub fn run(&self, tool: Tool, args: &[OsString]) -> Result<Output, ToolBroken> {
        let mut full_args = Vec::new();
        if tool == Tool::YtDlp {
            let mut deno = OsString::from("deno:");
            deno.push(self.path(Tool::Deno));
            full_args.extend([
                "--ignore-config".into(),
                "--ffmpeg-location".into(),
                self.path(Tool::Ffmpeg).into_os_string(),
                "--no-js-runtimes".into(),
                "--js-runtimes".into(),
                deno,
            ]);
        }
        full_args.extend_from_slice(args);
        self.processes
            .run(&self.path(tool), &full_args)
            .map_err(|_| {
                self.forget(tool);
                ToolBroken(tool)
            })
    }

    /// When yt-dlp was last installed or checked for updates.
    pub fn ytdlp_checked_at(&self) -> Option<SystemTime> {
        let seconds = self.load_manifest().ytdlp_checked_at?;
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
    }

    /// Drops a tool that failed to start from the manifest, so that the next
    /// [`Toolbox::prepare`], in this launch or a later one, reinstalls it.
    fn forget(&self, tool: Tool) {
        let mut manifest = self.load_manifest();
        manifest.files.remove(tool.file_name());
        if let Err(error) = self.save_manifest(&manifest) {
            eprintln!("can't forget broken {}: {error:?}", tool.file_name());
        }
        *self.broken.lock().unwrap() = Some(tool);
        self.report_progress(0, 0);
    }

    fn set_state(&self, state: SetupState) {
        *self.state.lock().unwrap() = state.clone();
        // Sent outside the lock so a subscriber may read the state meanwhile.
        self.events.send(AppEvent::Setup(state));
    }

    fn install_missing(&self) -> Result<(), SetupError> {
        fs::create_dir_all(&self.dir)?;
        self.remove_unverified()?;
        let mut manifest = self.load_manifest();
        let mut missing: Vec<Package> = Vec::new();
        for tool in Tool::ALL {
            if !self.is_intact(&manifest, tool) && !missing.contains(&tool.package()) {
                missing.push(tool.package());
            }
        }
        if missing.is_empty() {
            return Ok(());
        }

        // Show the setup screen while asking GitHub for the releases.
        self.report_progress(0, 0);
        let mut releases = Vec::new();
        for package in missing {
            let release = self.releases.latest_release(package)?;
            if !package.is_official(&release.url) {
                return Err(other(format!("unofficial asset {}", release.url)));
            }
            releases.push((package, release));
        }
        let total = releases.iter().map(|(_, release)| release.size).sum();
        let mut done = 0;
        self.report_progress(0, total);
        for (package, release) in releases {
            let asset = self.fetch_verified(package, &release, done, total)?;
            match package {
                Package::YtDlp => {
                    let installed = self.install_file(Tool::YtDlp, asset, &release)?;
                    manifest
                        .files
                        .insert(Tool::YtDlp.file_name().into(), installed);
                    manifest.ytdlp_checked_at = Some(self.unix_now());
                }
                Package::Ffmpeg | Package::Deno => {
                    let result = self.extract(package, &asset.path, &release, &mut manifest);
                    remove_if_exists(&asset.path);
                    result?;
                }
            }
            self.save_manifest(&manifest)?;
            done += release.size;
        }
        Ok(())
    }

    fn is_intact(&self, manifest: &Manifest, tool: Tool) -> bool {
        let Some(installed) = manifest.files.get(tool.file_name()) else {
            return false;
        };
        fs::metadata(self.path(tool)).is_ok_and(|m| m.is_file() && m.len() == installed.size)
    }

    /// Removes files that an interrupted launch left before verifying them.
    fn remove_unverified(&self) -> io::Result<()> {
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.to_string_lossy().ends_with(TEMP_SUFFIX) {
                remove_if_exists(&path);
            }
        }
        Ok(())
    }

    /// Fetches the release asset under a temporary name and verifies its
    /// published checksum; an asset that doesn't match is deleted.
    fn fetch_verified(
        &self,
        package: Package,
        release: &Release,
        done: u64,
        total: u64,
    ) -> Result<TempFile, SetupError> {
        let name = match package {
            Package::YtDlp => Tool::YtDlp.file_name(),
            Package::Ffmpeg => "ffmpeg.zip",
            Package::Deno => "deno.zip",
        };
        let path = self.dir.join(format!("{name}{TEMP_SUFFIX}"));
        let result = HashingWriter::create(&path)
            .map_err(SetupError::from)
            .and_then(|mut file| {
                let mut progress = |received: u64| self.report_progress(done + received, total);
                self.releases
                    .fetch(&release.url, &mut file, &mut progress)?;
                Ok(file.finish(path.clone())?)
            })
            .and_then(|file| {
                if file.sha256.eq_ignore_ascii_case(&release.sha256) {
                    Ok(file)
                } else {
                    Err(SetupError::ChecksumMismatch { package })
                }
            });
        if result.is_err() {
            remove_if_exists(&path);
        }
        result
    }

    /// Moves a verified file into place.
    fn install_file(
        &self,
        tool: Tool,
        file: TempFile,
        release: &Release,
    ) -> Result<InstalledFile, SetupError> {
        if let Err(error) = fs::rename(&file.path, self.path(tool)) {
            remove_if_exists(&file.path);
            return Err(error.into());
        }
        Ok(InstalledFile {
            version: release.version.clone(),
            size: file.size,
            sha256: file.sha256,
        })
    }

    /// Installs the package's tools from its verified zip.
    fn extract(
        &self,
        package: Package,
        archive: &Path,
        release: &Release,
        manifest: &mut Manifest,
    ) -> Result<(), SetupError> {
        let mut zip = ZipArchive::new(File::open(archive)?).map_err(other)?;
        for tool in Tool::ALL
            .into_iter()
            .filter(|tool| tool.package() == package)
        {
            let entry = zip
                .file_names()
                .filter_map(Result::ok)
                .find(|name| tool.is_archived_as(name))
                .map(|name| name.into_owned())
                .ok_or_else(|| {
                    other(format!("{} is missing from the archive", tool.file_name()))
                })?;
            let temp = self.dir.join(format!("{}{TEMP_SUFFIX}", tool.file_name()));
            let result = zip
                .by_name(&entry)
                .map_err(other)
                .and_then(|mut entry| {
                    let mut file = HashingWriter::create(&temp)?;
                    io::copy(&mut entry, &mut file)?;
                    Ok(file.finish(temp.clone())?)
                })
                .and_then(|file| self.install_file(tool, file, release));
            match result {
                Ok(installed) => {
                    manifest.files.insert(tool.file_name().into(), installed);
                }
                Err(error) => {
                    remove_if_exists(&temp);
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn report_progress(&self, downloaded: u64, total: u64) {
        let downloaded = downloaded.min(total);
        // A large download arrives in thousands of chunks; report each
        // thousandth of the total once.
        let permille = |bytes: u64| bytes * 1000 / total.max(1);
        if let SetupState::Installing {
            downloaded: shown,
            total: shown_total,
            ..
        } = self.state()
            && shown_total == total
            && 0 < downloaded
            && downloaded < total
            && permille(shown) == permille(downloaded)
        {
            return;
        }
        self.set_state(SetupState::Installing {
            downloaded,
            total,
            broken: *self.broken.lock().unwrap(),
        });
    }

    fn path(&self, tool: Tool) -> PathBuf {
        self.dir.join(tool.file_name())
    }

    fn load_manifest(&self) -> Manifest {
        // A missing or unreadable manifest means nothing is installed.
        fs::read(self.dir.join(MANIFEST))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save_manifest(&self, manifest: &Manifest) -> Result<(), SetupError> {
        let temp = self.dir.join(format!("{MANIFEST}{TEMP_SUFFIX}"));
        fs::write(&temp, serde_json::to_vec_pretty(manifest).map_err(other)?)?;
        fs::rename(&temp, self.dir.join(MANIFEST))?;
        Ok(())
    }

    fn unix_now(&self) -> u64 {
        let now = self.clock.now();
        now.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

impl From<io::Error> for SetupError {
    fn from(error: io::Error) -> Self {
        other(error)
    }
}

impl From<ReleasesError> for SetupError {
    fn from(error: ReleasesError) -> Self {
        match error {
            ReleasesError::Offline => SetupError::Offline,
            ReleasesError::Other(message) => SetupError::Other { message },
        }
    }
}

fn other(error: impl ToString) -> SetupError {
    SetupError::Other {
        message: error.to_string(),
    }
}

/// A file written under a temporary name, with its size and hex SHA-256.
struct TempFile {
    path: PathBuf,
    size: u64,
    sha256: String,
}

/// Hashes a file while it is being written.
struct HashingWriter {
    file: File,
    hasher: Sha256,
    size: u64,
}

impl HashingWriter {
    fn create(path: &Path) -> io::Result<Self> {
        Ok(Self {
            file: File::create(path)?,
            hasher: Sha256::new(),
            size: 0,
        })
    }

    fn finish(mut self, path: PathBuf) -> io::Result<TempFile> {
        self.file.flush()?;
        Ok(TempFile {
            path,
            size: self.size,
            sha256: hex(&self.hasher.finalize()),
        })
    }
}

impl Write for HashingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.file.write(buf)?;
        self.hasher.update(&buf[..written]);
        self.size += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn remove_if_exists(path: &Path) {
    let _ = fs::remove_file(path);
}
