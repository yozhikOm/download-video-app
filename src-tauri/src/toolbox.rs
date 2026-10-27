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
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::clock::Clock;
use crate::events::{AppEvent, EventSink};
use crate::processes::{Processes, Stopper};
use crate::releases::{Package, Release, Releases, ReleasesError};
use crate::settings::Browser;

const MANIFEST: &str = "manifest.json";
/// Suffix of files that are still downloading or unverified.
const TEMP_SUFFIX: &str = ".download";
/// Where an update downloads the new yt-dlp. Unlike [`TEMP_SUFFIX`], Setup
/// doesn't remove it, so that Setup and an update may run at once.
const YTDLP_UPDATE_FILE: &str = "yt-dlp.exe.update";
/// How long after the last check a launch checks for a newer yt-dlp again.
const UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

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

/// How the update of yt-dlp the user asked for goes.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum YtDlpUpdateState {
    /// None was asked for since launch.
    Idle,
    /// Asking for yt-dlp's latest release and downloading it.
    Updating,
    /// The new yt-dlp is downloaded and verified; it replaces the old one
    /// once the running Download ends.
    Waiting,
    Updated {
        version: String,
    },
    /// The latest release is installed already.
    UpToDate {
        version: String,
    },
    Failed {
        error: SetupError,
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

/// Who uses yt-dlp now, so that an update replaces it only while nobody
/// does.
#[derive(Default)]
struct YtDlpUsers {
    count: usize,
    /// An update waits to replace yt-dlp.
    update_waiting: bool,
}

/// Keeps yt-dlp from being replaced while it lives.
pub struct YtDlpInUse<'a>(&'a Toolbox);

impl Drop for YtDlpInUse<'_> {
    fn drop(&mut self) {
        self.0.ytdlp_users.lock().unwrap().count -= 1;
        self.0.ytdlp_users_changed.notify_all();
    }
}

/// The app's own installed tools: setting them up and running them.
pub struct Toolbox {
    dir: PathBuf,
    releases: Arc<dyn Releases>,
    processes: Arc<dyn Processes>,
    clock: Arc<dyn Clock>,
    events: Arc<dyn EventSink>,
    state: Mutex<SetupState>,
    /// Held while preparing, so concurrent requests don't download twice and
    /// wait for the one preparing.
    preparing: Mutex<()>,
    /// The tool being restored after it failed to start.
    broken: Mutex<Option<Tool>>,
    /// Held while updating yt-dlp, so that updates run one at a time.
    updating: Mutex<()>,
    /// How the update the user asked for goes.
    update_state: Mutex<YtDlpUpdateState>,
    /// Held while an update replaces yt-dlp, so that none starts meanwhile.
    ytdlp_users: Mutex<YtDlpUsers>,
    /// Notified when yt-dlp is no longer used or has been replaced.
    ytdlp_users_changed: Condvar,
    /// Held while installing or replacing tools and recording it in the
    /// manifest, so that Setup and an update don't do it at once.
    installing: Mutex<()>,
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
            updating: Mutex::new(()),
            update_state: Mutex::new(YtDlpUpdateState::Idle),
            ytdlp_users: Mutex::new(YtDlpUsers::default()),
            ytdlp_users_changed: Condvar::new(),
            installing: Mutex::new(()),
        }
    }

    pub fn state(&self) -> SetupState {
        self.state.lock().unwrap().clone()
    }

    /// Makes sure all tools are in place, downloading whatever is missing.
    /// Blocks until it ends in [`SetupState::Ready`] or [`SetupState::Failed`].
    /// When another call is already preparing, waits for it to end instead,
    /// so that the caller reads how Setup ended either way.
    pub fn prepare(&self) {
        let Ok(_preparing) = self.preparing.try_lock() else {
            drop(self.preparing.lock().unwrap());
            return;
        };
        let broken = *self.broken.lock().unwrap();
        // When restoring a broken tool, the setup screen is already up.
        if broken.is_none() {
            self.set_state(SetupState::Checking);
        }
        let installed = {
            let _installing = self.installing.lock().unwrap();
            self.install_missing()
        };
        match installed {
            Ok(()) => {
                *self.broken.lock().unwrap() = None;
                self.set_state(SetupState::Ready);
            }
            Err(error) => self.set_state(SetupState::Failed { error, broken }),
        }
    }

    /// Runs a tool from the app's own folder; yt-dlp ignores the user's
    /// config files and is pointed at the app's own ffmpeg and deno, never
    /// at a JavaScript runtime installed elsewhere, and at the cookies of the
    /// sign-in browser the caller passes, so that the caller knows which one
    /// the run used.
    ///
    /// On [`ToolBroken`] the setup screen is already showing, and the caller
    /// starts [`Toolbox::prepare`] to restore the tool.
    pub fn run(
        &self,
        tool: Tool,
        args: &[OsString],
        sign_in_browser: Option<Browser>,
    ) -> Result<Output, ToolBroken> {
        let args = self.full_args(tool, args, sign_in_browser);
        let _in_use = self.in_use(tool);
        self.processes
            .run(&self.path(tool), &args)
            .map_err(|_| self.broke(tool))
    }

    /// Like [`Toolbox::run`], but hands each line the tool prints to stdout
    /// to `on_line` as soon as it is printed, and `stopper` kills it.
    pub fn run_streaming(
        &self,
        tool: Tool,
        args: &[OsString],
        sign_in_browser: Option<Browser>,
        on_line: &mut dyn FnMut(&str),
        stopper: &Stopper,
    ) -> Result<Output, ToolBroken> {
        let args = self.full_args(tool, args, sign_in_browser);
        let _in_use = self.in_use(tool);
        self.processes
            .run_streaming(&self.path(tool), &args, on_line, stopper)
            .map_err(|_| self.broke(tool))
    }

    /// Keeps yt-dlp from being replaced by an update until the returned
    /// guard is dropped, as for all the runs of a Download. While an update
    /// waits to replace it, waits for the update first, so that one
    /// Download after another doesn't keep it waiting.
    pub fn use_ytdlp(&self) -> YtDlpInUse<'_> {
        let users = self.ytdlp_users.lock().unwrap();
        let mut users = self
            .ytdlp_users_changed
            .wait_while(users, |users| users.update_waiting)
            .unwrap();
        users.count += 1;
        YtDlpInUse(self)
    }

    /// Keeps yt-dlp from being replaced while it runs. Unlike
    /// [`Toolbox::use_ytdlp`], doesn't wait for an update that waits, which
    /// may wait for the caller's own use; an update replacing it now holds
    /// the users' lock throughout.
    fn in_use(&self, tool: Tool) -> Option<YtDlpInUse<'_>> {
        (tool == Tool::YtDlp).then(|| {
            self.ytdlp_users.lock().unwrap().count += 1;
            YtDlpInUse(self)
        })
    }

    fn full_args(
        &self,
        tool: Tool,
        args: &[OsString],
        sign_in_browser: Option<Browser>,
    ) -> Vec<OsString> {
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
            // yt-dlp reads the cookies itself on every run; the app never
            // copies them.
            if let Some(browser) = sign_in_browser {
                full_args.extend(["--cookies-from-browser".into(), browser.ytdlp_name().into()]);
            }
        }
        full_args.extend_from_slice(args);
        full_args
    }

    fn broke(&self, tool: Tool) -> ToolBroken {
        self.forget(tool);
        ToolBroken(tool)
    }

    /// When yt-dlp was last installed or checked for updates.
    pub fn ytdlp_checked_at(&self) -> Option<SystemTime> {
        let seconds = self.load_manifest().ytdlp_checked_at?;
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
    }

    /// The version of the installed yt-dlp.
    pub fn ytdlp_version(&self) -> Option<String> {
        let manifest = self.load_manifest();
        Some(manifest.files.get(Tool::YtDlp.file_name())?.version.clone())
    }

    /// Updates yt-dlp when more than a day has passed since it was last
    /// checked for updates, as at launch once Setup is Ready. A failed
    /// update leaves it to the next launch.
    pub fn update_ytdlp_if_due(&self) {
        let now = self.clock.now();
        let due = self.ytdlp_checked_at().is_none_or(|checked| {
            // A clock set back counts as the interval gone.
            now.duration_since(checked)
                .map_or(true, |elapsed| elapsed > UPDATE_INTERVAL)
        });
        if due && let Err(error) = self.update_ytdlp() {
            eprintln!("can't update yt-dlp: {error:?}");
        }
    }

    pub fn ytdlp_update_state(&self) -> YtDlpUpdateState {
        self.update_state.lock().unwrap().clone()
    }

    /// Updates yt-dlp as the user asked, telling how it goes through
    /// [`YtDlpUpdateState`] events.
    pub fn update_ytdlp_on_request(&self) {
        self.set_update_state(YtDlpUpdateState::Updating);
        let state = self
            .update_ytdlp_with_waiting(&|| self.set_update_state(YtDlpUpdateState::Waiting))
            .unwrap_or_else(|error| YtDlpUpdateState::Failed { error });
        self.set_update_state(state);
    }

    fn set_update_state(&self, state: YtDlpUpdateState) {
        *self.update_state.lock().unwrap() = state.clone();
        // Sent outside the lock so a subscriber may read the state meanwhile.
        self.events.send(AppEvent::YtDlpUpdate(state));
    }

    /// Installs yt-dlp's latest release unless it is installed already,
    /// verified against its published checksum as Setup does, and records
    /// the check. The new yt-dlp replaces the old one once nothing uses it:
    /// it waits for the running Download to end. Returns
    /// [`YtDlpUpdateState::Updated`] or [`YtDlpUpdateState::UpToDate`].
    pub fn update_ytdlp(&self) -> Result<YtDlpUpdateState, SetupError> {
        self.update_ytdlp_with_waiting(&|| {})
    }

    /// Like [`Toolbox::update_ytdlp`], calling `on_waiting` when the new
    /// yt-dlp waits for the old one to be no longer used.
    fn update_ytdlp_with_waiting(
        &self,
        on_waiting: &dyn Fn(),
    ) -> Result<YtDlpUpdateState, SetupError> {
        let _updating = self.updating.lock().unwrap();
        let release = self.releases.latest_release(Package::YtDlp)?;
        if !Package::YtDlp.is_official(&release.url) {
            return Err(other(format!("unofficial asset {}", release.url)));
        }
        if self.ytdlp_version().as_ref() == Some(&release.version) {
            self.record_ytdlp(None)?;
            return Ok(YtDlpUpdateState::UpToDate {
                version: release.version,
            });
        }
        let file = self.fetch_verified(
            Package::YtDlp,
            &release,
            &self.dir.join(YTDLP_UPDATE_FILE),
            &mut |_| {},
        )?;
        let in_use = {
            let mut users = self.ytdlp_users.lock().unwrap();
            users.update_waiting = true;
            users.count > 0
        };
        // Told outside the lock, which a Download that ends meanwhile needs.
        if in_use {
            on_waiting();
        }
        let users = self.ytdlp_users.lock().unwrap();
        let mut users = self
            .ytdlp_users_changed
            .wait_while(users, |users| users.count > 0)
            .unwrap();
        let replaced = self.record_ytdlp(Some((file, &release)));
        users.update_waiting = false;
        drop(users);
        self.ytdlp_users_changed.notify_all();
        replaced?;
        Ok(YtDlpUpdateState::Updated {
            version: release.version,
        })
    }

    /// Moves the verified new yt-dlp into place, if any, and records it and
    /// the check in the manifest; fails unless Setup is Ready.
    fn record_ytdlp(&self, new: Option<(TempFile, &Release)>) -> Result<(), SetupError> {
        let _installing = self.installing.lock().unwrap();
        if self.state() != SetupState::Ready {
            if let Some((file, _)) = new {
                remove_if_exists(&file.path);
            }
            return Err(other("Setup isn't Ready"));
        }
        let mut manifest = self.load_manifest();
        if let Some((file, release)) = new {
            let installed = self.install_file(Tool::YtDlp, file, release)?;
            manifest
                .files
                .insert(Tool::YtDlp.file_name().into(), installed);
        }
        manifest.ytdlp_checked_at = Some(self.unix_now());
        self.save_manifest(&manifest)
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
            let mut progress = |received: u64| self.report_progress(done + received, total);
            let asset =
                self.fetch_verified(package, &release, &self.temp_path(package), &mut progress)?;
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

    /// Where Setup fetches the package's release asset.
    fn temp_path(&self, package: Package) -> PathBuf {
        let name = match package {
            Package::YtDlp => Tool::YtDlp.file_name(),
            Package::Ffmpeg => "ffmpeg.zip",
            Package::Deno => "deno.zip",
        };
        self.dir.join(format!("{name}{TEMP_SUFFIX}"))
    }

    /// Fetches the release asset to `path`, reporting the bytes received so
    /// far, and verifies its published checksum; an asset that doesn't
    /// match is deleted.
    fn fetch_verified(
        &self,
        package: Package,
        release: &Release,
        path: &Path,
        progress: &mut dyn FnMut(u64),
    ) -> Result<TempFile, SetupError> {
        let result = HashingWriter::create(path)
            .map_err(SetupError::from)
            .and_then(|mut file| {
                self.releases.fetch(&release.url, &mut file, progress)?;
                Ok(file.finish(path.to_path_buf())?)
            })
            .and_then(|file| {
                if file.sha256.eq_ignore_ascii_case(&release.sha256) {
                    Ok(file)
                } else {
                    Err(SetupError::ChecksumMismatch { package })
                }
            });
        if result.is_err() {
            remove_if_exists(path);
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
