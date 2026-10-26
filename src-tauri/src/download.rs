//! Downloading a probed Video: yt-dlp works in the Partials folder, and the
//! final file is moved to the save folder under the Video's title.

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::events::{AppEvent, EventSink};
use crate::preset::{AudioPreset, Preset, VideoPreset};
use crate::probe::{Video, VideoCard, error_line};
use crate::settings::Settings;
use crate::toolbox::{Tool, Toolbox};

/// The Download the main screen shows.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Download {
    pub video: VideoCard,
    #[serde(flatten)]
    pub status: DownloadStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum DownloadStatus {
    /// Each field is `None` while yt-dlp doesn't tell it.
    Downloading {
        /// 0 to 100, of all the Formats yt-dlp receives; `None` while the
        /// size of one is unknown. Never goes down during a Download.
        percent: Option<f64>,
        /// Bytes per second.
        speed: Option<f64>,
        /// Seconds left.
        eta: Option<u64>,
    },
    /// ffmpeg merges the received Formats into the MP4, or extracts the
    /// sound into its own file.
    Processing,
    Done {
        file: PathBuf,
    },
    /// The message is for the log; the UI shows a generic text.
    Failed {
        message: String,
    },
}

/// How often the UI gets the speed and time left while the percent stands.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(1);

/// Downloads Videos with yt-dlp.
pub struct Downloader {
    toolbox: Arc<Toolbox>,
    settings: Arc<Settings>,
    partials_dir: PathBuf,
    save_dir: PathBuf,
    clock: Arc<dyn Clock>,
    events: Arc<dyn EventSink>,
    current: Mutex<Option<Download>>,
    /// The current Download's Video and Preset, for retrying it once it is
    /// Failed.
    to_retry: Mutex<Option<(Video, Preset)>>,
    /// When the UI last got the progress.
    progress_sent: Mutex<Option<SystemTime>>,
    /// Held while changing the state and sending it, so that the UI gets
    /// the changes in the same order.
    sending: Mutex<()>,
}

impl Downloader {
    pub fn new(
        toolbox: Arc<Toolbox>,
        settings: Arc<Settings>,
        partials_dir: PathBuf,
        save_dir: PathBuf,
        clock: Arc<dyn Clock>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            toolbox,
            settings,
            partials_dir,
            save_dir,
            clock,
            events,
            current: Mutex::new(None),
            to_retry: Mutex::new(None),
            progress_sent: Mutex::new(None),
            sending: Mutex::new(()),
        }
    }

    pub fn state(&self) -> Option<Download> {
        self.current.lock().unwrap().clone()
    }

    /// The saved file of the current Download, once it is Done.
    pub fn saved_file(&self) -> Option<PathBuf> {
        match self.state()?.status {
            DownloadStatus::Done { file } => Some(file),
            _ => None,
        }
    }

    /// Downloads the current Download's Video again with its Preset if it is
    /// Failed; yt-dlp resumes from its Partial. Blocks until the Download
    /// ends.
    pub fn retry(&self) {
        let failed = matches!(
            self.state(),
            Some(Download {
                status: DownloadStatus::Failed { .. },
                ..
            })
        );
        let video = self.to_retry.lock().unwrap().clone();
        if let (true, Some((video, preset))) = (failed, video) {
            self.download(&video, preset);
        }
    }

    /// Downloads `video` with `preset`: as an MP4 in the resolution of a
    /// Video preset, or as the sound alone in the format of an Audio
    /// preset. Blocks until the Download ends. Returns at once if another
    /// Download is running.
    pub fn download(&self, video: &Video, preset: Preset) {
        if !self.start(video, preset) {
            return;
        }
        let sign_in_browser = self.settings.sign_in_browser_for(&video.url);
        let mut args: Vec<OsString> = vec!["--no-playlist".into()];
        args.extend(preset_args(preset));
        args.extend::<[OsString; _]>([
            "-P".into(),
            self.partials_dir.clone().into_os_string(),
            "-o".into(),
            format!("%(id)s.{}.%(ext)s", preset.name()).into(),
            // `--print` makes yt-dlp quiet, and quiet hides the progress.
            "--progress".into(),
            "--newline".into(),
            "--progress-template".into(),
            // The Format's id places it among the separate Formats; a
            // missing one prints as `null`.
            r#"download:{"progress": %(progress)j, "format": %(info.format_id|null)j}"#.into(),
            // The separate Formats and their sizes, before any is received,
            // so that the progress covers all of them; `null` for a single
            // Format.
            "--print".into(),
            r#"before_dl:{"formats": %(requested_formats.:.{format_id,filesize,filesize_approx}|null)j}"#.into(),
            // Printed right before ffmpeg merges the video and audio or
            // extracts the sound.
            "--print".into(),
            "post_process:[processing]".into(),
            // A JSON string, so that a path with non-ASCII letters survives
            // the console's code page.
            "--print".into(),
            "after_move:%(filepath)j".into(),
        ]);
        args.extend(["--".into(), video.url.clone().into()]);
        let mut formats = Vec::new();
        let mut final_file = None;
        let result = self
            .toolbox
            .run_streaming(
                Tool::YtDlp,
                &args,
                sign_in_browser,
                &mut |line| match read_line(line) {
                    Line::Formats(planned) => formats = planned,
                    Line::Progress(line) => {
                        line.note(&mut formats);
                        self.report_progress(line.status(&formats));
                    }
                    Line::Processing => self.set_status(DownloadStatus::Processing),
                    Line::FinalFile(path) => final_file = Some(path),
                    Line::Other => {}
                },
            );
        let status = match result {
            // The setup screen is up now; once yt-dlp is restored, the user
            // starts the Download again.
            Err(_) => {
                self.set_state(None);
                self.toolbox.prepare();
                return;
            }
            Ok(output) if !output.status.success() => DownloadStatus::Failed {
                message: error_line(&String::from_utf8_lossy(&output.stderr)),
            },
            Ok(_) => match final_file {
                Some(partial) => match self.save(&partial, &file_name(video)) {
                    Ok(file) => DownloadStatus::Done { file },
                    Err(error) => DownloadStatus::Failed {
                        message: format!("can't save {}: {error}", partial.display()),
                    },
                },
                None => DownloadStatus::Failed {
                    message: "yt-dlp didn't tell where the file is".into(),
                },
            },
        };
        if let DownloadStatus::Failed { message } = &status {
            eprintln!("can't download {}: {message}", video.url);
        }
        self.set_status(status);
    }

    /// Makes `video` the current Download, unless one is running.
    fn start(&self, video: &Video, preset: Preset) -> bool {
        let _sending = self.sending.lock().unwrap();
        let download = Download {
            video: video.card(),
            status: DownloadStatus::Downloading {
                percent: None,
                speed: None,
                eta: None,
            },
        };
        {
            let mut current = self.current.lock().unwrap();
            if let Some(Download {
                status: DownloadStatus::Downloading { .. } | DownloadStatus::Processing,
                ..
            }) = *current
            {
                return false;
            }
            *current = Some(download.clone());
            *self.to_retry.lock().unwrap() = Some((video.clone(), preset));
        }
        // The new Download's first progress goes out at once.
        *self.progress_sent.lock().unwrap() = None;
        self.events.send(AppEvent::Download(Some(download)));
        true
    }

    /// Moves the final file out of the Partials folder under the first
    /// free name: the title, then the title with " (1)", " (2)"…
    fn save(&self, partial: &Path, name: &str) -> io::Result<PathBuf> {
        fs::create_dir_all(&self.save_dir)?;
        let extension = partial.extension().unwrap_or_default().to_string_lossy();
        for number in 0.. {
            let name = match number {
                0 => format!("{name}.{extension}"),
                _ => format!("{name} ({number}).{extension}"),
            };
            let file = self.save_dir.join(name);
            // Creating the file takes the name, so that another program
            // can't take it before the move.
            if create_new(&file)?.is_none() {
                continue;
            }
            return match move_file(partial, &file) {
                Ok(()) => Ok(file),
                Err(error) => {
                    let _ = fs::remove_file(&file);
                    Err(error)
                }
            };
        }
        unreachable!("some number is free")
    }

    fn report_progress(&self, mut status: DownloadStatus) {
        let now = self.clock.now();
        let Some(shown) = self.state() else {
            return self.set_status(status);
        };
        // A size that turns out bigger than told would take the percent
        // back; it holds until the bytes catch up.
        if let DownloadStatus::Downloading {
            percent: Some(shown),
            ..
        } = shown.status
            && let DownloadStatus::Downloading { percent, .. } = &mut status
        {
            *percent = Some(percent.map_or(shown, |percent| percent.max(shown)));
        }
        // yt-dlp reports every chunk it receives; the UI gets each tenth of
        // a percent at once, and otherwise a line at most once a second, so
        // the speed and time left move even while the percent stands or is
        // unknown.
        let permille = |status: &DownloadStatus| match status {
            DownloadStatus::Downloading { percent, .. } => {
                percent.map(|percent| (percent * 10.0) as u64)
            }
            _ => None,
        };
        if let DownloadStatus::Downloading { .. } = shown.status
            && permille(&shown.status) == permille(&status)
            && self.progress_sent.lock().unwrap().is_some_and(|sent| {
                // A clock set back counts as the interval gone.
                now.duration_since(sent)
                    .is_ok_and(|elapsed| elapsed < PROGRESS_INTERVAL)
            })
        {
            return;
        }
        *self.progress_sent.lock().unwrap() = Some(now);
        self.set_status(status);
    }

    fn set_status(&self, status: DownloadStatus) {
        let download = self.state().map(|download| Download { status, ..download });
        self.set_state(download);
    }

    fn set_state(&self, state: Option<Download>) {
        let _sending = self.sending.lock().unwrap();
        *self.current.lock().unwrap() = state.clone();
        // Sent outside the state lock: a new subscriber reads the state
        // while holding the subscribers' lock.
        self.events.send(AppEvent::Download(state));
    }
}

/// What yt-dlp receives for `preset` and how it makes the final file of it.
fn preset_args(preset: Preset) -> Vec<OsString> {
    let args = match preset {
        Preset::Video(preset) => vec![
            // Separate video and audio: preferring AAC would otherwise pick
            // a Format with both, which YouTube sends slowly over HLS. A
            // site with only such Formats falls back to the best of them.
            "-f".into(),
            "bv+ba/b".into(),
            "-S".into(),
            format_sort(preset),
            "--merge-output-format".into(),
            "mp4".into(),
        ],
        // The best sound alone, or a Format with both on a site without
        // one, extracted by ffmpeg into a file of the Preset's format.
        Preset::Audio(preset) => {
            let mut args = vec![
                "-f".into(),
                "ba/b".into(),
                "-x".into(),
                "--audio-format".into(),
                preset.name().into(),
            ];
            args.extend(match preset {
                // ffmpeg copies AAC sound into an M4A as it is; only a site
                // without AAC gets its sound encoded, as AAC at 192 kbit/s.
                AudioPreset::M4a => vec![
                    "-S".into(),
                    "acodec:aac".into(),
                    "--audio-quality".into(),
                    "192K".into(),
                ],
                // LAME's VBR quality 1: about 190–250 kbit/s.
                AudioPreset::Mp3 => vec!["--audio-quality".into(), "1".into()],
            });
            args
        }
    };
    args.into_iter().map(OsString::from).collect()
}

/// yt-dlp's order of the Formats for `preset`: the highest resolution up to
/// the Preset's, then H.264 video and AAC audio, which any player plays,
/// over VP9, AV1 and Opus of the same resolution.
fn format_sort(preset: VideoPreset) -> String {
    let resolution = match preset.resolution() {
        Some(resolution) => format!("res:{resolution}"),
        None => "res".into(),
    };
    format!("{resolution},vcodec:h264,acodec:aac")
}

/// Longer titles are cut, so that the path fits Windows' limit.
const MAX_NAME_CHARS: usize = 150;

/// The saved file's name without its extension: the Video's title with the
/// characters Windows forbids replaced by lookalikes, as yt-dlp does, or the
/// Video's id when the title can't be a file name.
fn file_name(video: &Video) -> String {
    let replaced: String = video
        .title
        .chars()
        .map(|c| match c {
            '"' => '＂',
            '*' => '＊',
            ':' => '：',
            '<' => '＜',
            '>' => '＞',
            '?' => '？',
            '|' => '｜',
            '/' => '⧸',
            '\\' => '⧹',
            c if c.is_control() => ' ',
            c => c,
        })
        .take(MAX_NAME_CHARS)
        .collect();
    // Windows drops a trailing dot or space from a file name.
    let name = replaced
        .trim_start()
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    if name.is_empty() || is_reserved(name) {
        if video.id.is_empty() {
            "video".into()
        } else {
            video.id.clone()
        }
    } else {
        name.into()
    }
}

/// Names that Windows keeps for devices, whatever extension follows them.
fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper[3..].chars().all(|c| ('1'..='9').contains(&c)))
}

/// Moves `from` over the empty file `to`. The save folder may be on another
/// drive than the Partials folder; then the file is copied under a temporary
/// name first, so that `to` never holds half a file.
fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_then_rename(from, to)
}

/// Copies `from` next to `to` under the first free temporary name, then
/// renames the copy over `to`. The temporary name is taken by creating the
/// file, so that a file of the user's named like it is neither overwritten
/// nor removed.
fn copy_then_rename(from: &Path, to: &Path) -> io::Result<()> {
    let mut source = File::open(from)?;
    let (temp, mut copy) = create_temporary(to)?;
    let copied = io::copy(&mut source, &mut copy);
    // Windows can't rename a file that is still open.
    drop(copy);
    if let Err(error) = copied.and_then(|_| fs::rename(&temp, to)) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = fs::remove_file(from) {
        eprintln!("can't remove the Partial {}: {error}", from.display());
    }
    Ok(())
}

/// Creates the first free file of `<to>.download`, `<to>.1.download`,
/// `<to>.2.download`…
fn create_temporary(to: &Path) -> io::Result<(PathBuf, File)> {
    for number in 0.. {
        let mut temp = to.as_os_str().to_owned();
        if number > 0 {
            temp.push(format!(".{number}"));
        }
        temp.push(".download");
        let temp = PathBuf::from(temp);
        if let Some(file) = create_new(&temp)? {
            return Ok((temp, file));
        }
    }
    unreachable!("some number is free")
}

/// Creates the file at `path`; `None` when a file is already there.
fn create_new(path: &Path) -> io::Result<Option<File>> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(None),
        Err(error) => Err(error),
    }
}

/// What a line of yt-dlp's stdout tells.
enum Line {
    /// The separate Formats yt-dlp is about to receive, in order.
    Formats(Vec<PlannedFormat>),
    Progress(ProgressLine),
    Processing,
    /// The final file in the Partials folder.
    FinalFile(PathBuf),
    Other,
}

/// The line printed before anything is received.
#[derive(Deserialize)]
struct FormatsLine {
    formats: Option<Vec<PlannedFormat>>,
}

#[derive(Deserialize)]
struct PlannedFormat {
    format_id: String,
    filesize: Option<f64>,
    filesize_approx: Option<f64>,
    /// The size yt-dlp's progress tells once the Format is being received.
    #[serde(skip)]
    received: Option<f64>,
}

impl PlannedFormat {
    fn size(&self) -> Option<f64> {
        self.filesize.or(self.filesize_approx)
    }
}

/// A line printed by the app's `--progress-template`.
#[derive(Deserialize)]
struct ProgressLine {
    progress: Progress,
    /// The id of the Format being received.
    format: Option<String>,
}

/// The part of yt-dlp's progress the app uses.
#[derive(Deserialize)]
struct Progress {
    downloaded_bytes: Option<f64>,
    total_bytes: Option<f64>,
    total_bytes_estimate: Option<f64>,
    /// A Format that comes in fragments may tell only how many there are.
    fragment_index: Option<f64>,
    fragment_count: Option<f64>,
    speed: Option<f64>,
    eta: Option<f64>,
}

fn read_line(line: &str) -> Line {
    if line == "[processing]" {
        return Line::Processing;
    }
    if line.starts_with('{') {
        if let Ok(progress) = serde_json::from_str::<ProgressLine>(line) {
            return Line::Progress(progress);
        }
        return match serde_json::from_str::<FormatsLine>(line) {
            Ok(formats) => Line::Formats(formats.formats.unwrap_or_default()),
            Err(_) => Line::Other,
        };
    }
    match serde_json::from_str::<PathBuf>(line) {
        Ok(path) => Line::FinalFile(path),
        Err(_) => Line::Other,
    }
}

impl ProgressLine {
    fn total(&self) -> Option<f64> {
        self.progress
            .total_bytes
            .or(self.progress.total_bytes_estimate)
    }

    /// 0 to 1, of the fragments of a Format that comes in fragments. Its
    /// size is only estimated from the fragments received so far, which
    /// YouTube's HLS makes far too small, so the share of fragments is
    /// closer.
    fn share_of_fragments(&self) -> Option<f64> {
        let (index, count) = (self.progress.fragment_index?, self.progress.fragment_count?);
        (count > 0.0).then(|| (index / count).clamp(0.0, 1.0))
    }

    /// Remembers the size of the Format being received among the separate
    /// `formats`, so that it counts as received rather than as told.
    fn note(&self, formats: &mut [PlannedFormat]) {
        if let Some(planned) = formats
            .iter_mut()
            .find(|planned| Some(&planned.format_id) == self.format.as_ref())
            && let Some(size) = self.total().or(self.progress.downloaded_bytes)
        {
            planned.received = Some(size);
        }
    }

    /// The status this line tells; with the separate `formats`, its
    /// percent and time left cover all of them.
    fn status(&self, formats: &[PlannedFormat]) -> DownloadStatus {
        let progress = &self.progress;
        let total = self.total();
        let percent_of = |done: Option<f64>, total: Option<f64>| {
            let (done, total) = (done?, total?);
            (total > 0.0).then(|| (done / total).clamp(0.0, 1.0) * 100.0)
        };
        let own_eta = progress.eta.map(|eta| eta.max(0.0).round() as u64);
        let Some(at) = formats
            .iter()
            .position(|planned| Some(&planned.format_id) == self.format.as_ref())
        else {
            return DownloadStatus::Downloading {
                percent: self
                    .share_of_fragments()
                    .map(|share| share * 100.0)
                    .or_else(|| percent_of(progress.downloaded_bytes, total)),
                speed: progress.speed,
                eta: own_eta,
            };
        };
        // This Format's own percent would fall back once the others join,
        // so there is none until all the sizes are known.
        let Some((done, all)) = self.of_all(formats, at, total) else {
            return DownloadStatus::Downloading {
                percent: None,
                speed: progress.speed,
                eta: own_eta,
            };
        };
        let left = (all - done).max(0.0);
        DownloadStatus::Downloading {
            percent: percent_of(Some(done), Some(all)),
            speed: progress.speed,
            eta: progress
                .speed
                .filter(|speed| *speed > 0.0)
                .map(|speed| (left / speed).round() as u64)
                .or(own_eta),
        }
    }

    /// Bytes received of all the separate `formats`, of which this is the
    /// one `at`, and their total; `None` when a size is unknown. The Formats
    /// before it count with the size their progress told.
    fn of_all(
        &self,
        formats: &[PlannedFormat],
        at: usize,
        total: Option<f64>,
    ) -> Option<(f64, f64)> {
        let before = formats[..at]
            .iter()
            .map(|planned| planned.received.or(planned.size()))
            .sum::<Option<f64>>()?;
        let after = formats[at + 1..]
            .iter()
            .map(PlannedFormat::size)
            .sum::<Option<f64>>()?;
        let (downloaded, total) = match self.share_of_fragments() {
            // Its told size stands in for the estimate.
            Some(share) => {
                let size = formats[at].size()?;
                (share * size, size)
            }
            None => (
                self.progress.downloaded_bytes?,
                total.or(formats[at].size())?,
            ),
        };
        Some((before + downloaded, before + total + after))
    }
}
