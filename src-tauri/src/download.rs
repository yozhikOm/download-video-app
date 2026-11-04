//! The Queue of Downloads of probed Videos, received one at a time: yt-dlp
//! works in the Partials folder, and the final files, the media and its
//! Transcript, are moved to the save folder under the name the settings'
//! template gives the Video.

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::events::{AppEvent, EventSink};
use crate::file_name::file_name;
use crate::logs::{Log, Logs};
use crate::preset::{AudioPreset, Preset, VideoPreset};
use crate::probe::{self, Format, ProbeError, Video, VideoCard, error_line};
use crate::processes::Stopper;
use crate::settings::{Browser, Settings};
use crate::toolbox::{SetupState, Tool, Toolbox};
use crate::transcript::{self, TranscriptLanguage};

/// A Download in the Queue, as the main screen shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    /// Tells the Download apart from the others in the Queue.
    pub id: u64,
    /// The Video's own page.
    pub url: String,
    pub video: VideoCard,
    /// `None` for a Format or the Transcript alone.
    pub preset: Option<Preset>,
    /// The id of the Format saved as it is instead of a Preset.
    pub format: Option<String>,
    /// The language of the Transcript, when it is saved.
    pub transcript_language: Option<String>,
    /// The Transcript is saved as `.srt` too, with its timing.
    pub transcript_srt: bool,
    #[serde(flatten)]
    pub status: DownloadStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum DownloadStatus {
    /// Waits for the Downloads ahead of it in the Queue.
    Queued,
    /// Each field but `receiving` is `None` while yt-dlp doesn't tell it.
    Downloading {
        receiving: Receiving,
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
    /// The UI shows its own text for each kind of failure; the message is
    /// for the log.
    Failed {
        failure: Failure,
        message: String,
        /// The last lines yt-dlp printed, but its progress and what only
        /// `--verbose` prints, for the user to copy into a bug report.
        output: Vec<String>,
        /// What became of the update of yt-dlp that a
        /// [`Failure::ExtractorBroken`] starts; `None` for other failures.
        ytdlp_update: Option<YtDlpUpdate>,
    },
    /// Stopped by the user; its Partial is removed.
    Cancelled,
}

impl DownloadStatus {
    /// Queued, Downloading or Processing.
    fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Downloading { .. } | Self::Processing
        )
    }

    /// Downloading `receiving`, before yt-dlp tells its progress.
    fn starting(receiving: Receiving) -> Self {
        Self::Downloading {
            receiving,
            percent: None,
            speed: None,
            eta: None,
        }
    }
}

/// Why a Download failed, as far as the user can do something about it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Failure {
    /// No connection to the internet, or the site doesn't answer.
    Offline,
    /// The site isn't supported, or the Video is gone from it.
    Unsupported,
    /// The Video is age-restricted, private or members-only, or YouTube
    /// asks to prove the user isn't a bot; signing in gets past it.
    SignInRequired,
    /// yt-dlp can't make out the site's page, likely changed since its
    /// release; a newer yt-dlp usually can.
    ExtractorBroken,
    /// The file can't be written: the disk is full, or the folder or the
    /// file is out of reach.
    Disk,
    /// Something else, told only by the message.
    Unknown,
}

/// What became of the update of yt-dlp that a Download failed with
/// [`Failure::ExtractorBroken`] starts. The Download stays Failed: the user
/// retries it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum YtDlpUpdate {
    Updating,
    /// A newer yt-dlp is installed; retrying likely works.
    Updated,
    /// The latest yt-dlp failed already; a fix awaits its next release.
    UpToDate,
    /// yt-dlp couldn't be updated, such as without the internet.
    Failed,
}

/// What a Downloading Download receives now.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Receiving {
    Transcript,
    /// A Format with the picture, with or without the sound.
    Video,
    /// A Format with the sound alone.
    Audio,
}

impl Receiving {
    /// What a Format of `media` is when yt-dlp doesn't tell.
    fn of(media: &Media) -> Self {
        match media {
            Media::Preset(Preset::Video(_)) => Self::Video,
            Media::Preset(Preset::Audio(_)) => Self::Audio,
            Media::Format(format) if format.has_picture() => Self::Video,
            Media::Format(_) => Self::Audio,
        }
    }
}

/// How a Download saves the Video's media.
#[derive(Clone, Debug, PartialEq)]
pub enum Media {
    /// As a Video or Audio preset makes it.
    Preset(Preset),
    /// One Format exactly as the site reports it, chosen in the Advanced
    /// mode: in an MP4 when it has the picture, or else in an M4A.
    Format(Box<Format>),
}

/// What tells the media of two Downloads of a Video apart: the Preset, or
/// the Format's id, whatever a Probe estimated of its size.
#[derive(PartialEq)]
enum MediaKey<'a> {
    Preset(Preset),
    Format(&'a str),
}

impl Media {
    fn key(&self) -> MediaKey<'_> {
        match self {
            Self::Preset(preset) => MediaKey::Preset(*preset),
            Self::Format(format) => MediaKey::Format(&format.id),
        }
    }

    /// What a Partial's name tells the media apart by: the Preset's name,
    /// or `format-` and the Format's id, so that it stays a file name and
    /// yt-dlp's template takes it as it is: letters, digits and `-` stay,
    /// and any other character becomes its code in hex between `_`, so
    /// that two ids never share a name.
    fn partial_name(&self) -> String {
        match self {
            Self::Preset(preset) => preset.name().into(),
            Self::Format(format) => {
                let id: String = format
                    .id
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' {
                            c.to_string()
                        } else {
                            format!("_{:x}_", u32::from(c))
                        }
                    })
                    .collect();
                format!("format-{id}")
            }
        }
    }
}

impl From<Preset> for Media {
    fn from(preset: Preset) -> Self {
        Self::Preset(preset)
    }
}

impl From<VideoPreset> for Media {
    fn from(preset: VideoPreset) -> Self {
        Self::Preset(preset.into())
    }
}

impl From<AudioPreset> for Media {
    fn from(preset: AudioPreset) -> Self {
        Self::Preset(preset.into())
    }
}

/// How a Download saves the Video's Transcript: as text, and as SubRip
/// (`.srt`) with its timing too when asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Transcript {
    pub language: TranscriptLanguage,
    pub srt: bool,
}

/// The Transcript in `language` as text alone.
impl From<TranscriptLanguage> for Transcript {
    fn from(language: TranscriptLanguage) -> Self {
        Self {
            language,
            srt: false,
        }
    }
}

/// What a Download saves of its Video.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    /// The media alone.
    Media(Media),
    /// The media, and its Transcript next to it under the same name.
    MediaWithTranscript(Media, Transcript),
    /// The Transcript alone, without the media.
    Transcript(Transcript),
}

impl Content {
    fn media(&self) -> Option<&Media> {
        match self {
            Self::Media(media) | Self::MediaWithTranscript(media, _) => Some(media),
            Self::Transcript(_) => None,
        }
    }

    fn transcript(&self) -> Option<&Transcript> {
        match self {
            Self::Media(_) => None,
            Self::MediaWithTranscript(_, transcript) | Self::Transcript(transcript) => {
                Some(transcript)
            }
        }
    }

    /// What a duplicate Download of the same Video shares with it: the
    /// Preset or Format, or the Transcript alone.
    fn duplicate_key(&self) -> (Option<MediaKey<'_>>, Option<&Transcript>) {
        match self {
            Self::Transcript(transcript) => (None, Some(transcript)),
            _ => (self.media().map(Media::key), None),
        }
    }

    /// What the Download receives first: the Transcript comes before the
    /// media.
    fn first(&self) -> Receiving {
        match self {
            Self::Media(media) => Receiving::of(media),
            Self::MediaWithTranscript(..) | Self::Transcript(_) => Receiving::Transcript,
        }
    }
}

impl From<Preset> for Content {
    fn from(preset: Preset) -> Self {
        Self::Media(preset.into())
    }
}

impl From<VideoPreset> for Content {
    fn from(preset: VideoPreset) -> Self {
        Self::Media(preset.into())
    }
}

impl From<AudioPreset> for Content {
    fn from(preset: AudioPreset) -> Self {
        Self::Media(preset.into())
    }
}

/// What became of a Download asked for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Added {
    /// It waits in the Queue.
    Queued,
    /// A Download of the same Video with the same Preset, or of its
    /// Transcript alone in the same language, is Queued or running
    /// already; nothing was added. `of` is that Download's id, so that the
    /// UI tells of it only while it is Queued or running.
    Duplicate { of: u64 },
}

/// What a request to close the window leads to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnClose {
    /// No Download is unfinished: the window closes.
    Close,
    /// A Download is Queued, Downloading or Processing: the user is asked
    /// first.
    Ask,
    /// The app is closing already: the window stays until
    /// [`Downloader::close`] has stopped the Queue and the app exits.
    Wait,
}

/// The Downloads in the order they run.
#[derive(Default)]
struct Queue {
    entries: Vec<Entry>,
    /// A call of [`Downloader::run`] receives the Queued Downloads.
    has_worker: bool,
    /// The id of the Download being received.
    running: Option<u64>,
    /// Stops the yt-dlp of the Download being received.
    stopper: Arc<Stopper>,
    /// The app is closing: no Queued Download starts.
    closing: bool,
    next_id: u64,
}

struct Entry {
    download: Download,
    video: Video,
    content: Content,
}

impl Entry {
    fn is_done(&self) -> bool {
        matches!(self.download.status, DownloadStatus::Done { .. })
    }

    /// Done, Failed or Cancelled: nothing more happens to it unless retried.
    fn has_ended(&self) -> bool {
        matches!(
            self.download.status,
            DownloadStatus::Done { .. } | DownloadStatus::Failed { .. } | DownloadStatus::Cancelled
        )
    }
}

impl Queue {
    fn shown(&self) -> Vec<Download> {
        self.entries
            .iter()
            .map(|entry| entry.download.clone())
            .collect()
    }

    fn on_close(&self) -> OnClose {
        if self.closing {
            OnClose::Wait
        } else if self
            .entries
            .iter()
            .any(|entry| entry.download.status.is_active())
        {
            OnClose::Ask
        } else {
            OnClose::Close
        }
    }

    fn running(&self) -> Option<&Entry> {
        let id = self.running?;
        self.entries.iter().find(|entry| entry.download.id == id)
    }

    /// The name prefixes of the Partial of `entry`, which isn't running,
    /// that the running Download doesn't use: it may receive the same
    /// Transcript.
    fn unused_partial(&self, entry: &Entry) -> Vec<String> {
        let in_use = self
            .running()
            .map(|running| partial_prefixes(&running.video, &running.content))
            .unwrap_or_default();
        partial_prefixes(&entry.video, &entry.content)
            .into_iter()
            .filter(|prefix| !in_use.contains(prefix))
            .collect()
    }

    fn running_mut(&mut self) -> Option<&mut Entry> {
        let id = self.running?;
        self.entries
            .iter_mut()
            .find(|entry| entry.download.id == id)
    }

    /// The id of the Queued or running Download that one of `content` of
    /// `video` would duplicate.
    fn duplicate_of(&self, video: &Video, content: &Content) -> Option<u64> {
        self.entries
            .iter()
            .find(|entry| {
                entry.download.status.is_active()
                    && entry.video.id == video.id
                    && entry.content.duplicate_key() == content.duplicate_key()
            })
            .map(|entry| entry.download.id)
    }

    /// Starts the first Queued Download and returns what it saves of which
    /// Video, and what stops its yt-dlp; when none waits, or the app is
    /// closing, the worker stops.
    fn start_next(&mut self) -> Option<(Video, Content, Arc<Stopper>)> {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.download.status == DownloadStatus::Queued)
            .filter(|_| !self.closing)
        else {
            self.has_worker = false;
            return None;
        };
        entry.download.status = DownloadStatus::starting(entry.content.first());
        self.running = Some(entry.download.id);
        self.stopper = Arc::default();
        Some((
            entry.video.clone(),
            entry.content.clone(),
            self.stopper.clone(),
        ))
    }
}

/// Why a Download ended before its files were received.
enum Stopped {
    /// yt-dlp failed to start; the setup screen is up now.
    ToolBroken,
    /// The message is for the log.
    Failed(Failure, String),
}

/// How often the UI gets the speed and time left while the percent stands.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(1);

/// How long a Partial untouched is kept for a later Download to resume.
const PARTIAL_LIFETIME: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Keeps the Queue and downloads its Videos with yt-dlp.
pub struct Downloader {
    toolbox: Arc<Toolbox>,
    settings: Arc<Settings>,
    partials_dir: PathBuf,
    logs: Logs,
    clock: Arc<dyn Clock>,
    events: Arc<dyn EventSink>,
    queue: Mutex<Queue>,
    /// Notified on every change to the Queue, such as the running Download
    /// ending.
    changed: Condvar,
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
        logs_dir: PathBuf,
        clock: Arc<dyn Clock>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            toolbox,
            settings,
            partials_dir,
            logs: Logs::new(logs_dir),
            clock,
            events,
            queue: Mutex::new(Queue::default()),
            changed: Condvar::new(),
            progress_sent: Mutex::new(None),
            sending: Mutex::new(()),
        }
    }

    pub fn state(&self) -> Vec<Download> {
        self.queue.lock().unwrap().shown()
    }

    /// What a request to close the window leads to: the user is asked while
    /// a Download is unfinished, and once the app is closing, the window
    /// waits for [`Downloader::close`].
    pub fn on_close(&self) -> OnClose {
        self.queue.lock().unwrap().on_close()
    }

    /// The saved file of the Download, once it is Done.
    pub fn saved_file(&self, id: u64) -> Option<PathBuf> {
        let queue = self.queue.lock().unwrap();
        let entry = queue.entries.iter().find(|entry| entry.download.id == id)?;
        match &entry.download.status {
            DownloadStatus::Done { file } => Some(file.clone()),
            _ => None,
        }
    }

    /// Adds a Download of `content` of `video` to the end of the Queue: the
    /// media as an MP4 in the resolution of a Video preset, or as the sound
    /// alone in the format of an Audio preset, and its Transcript as text.
    /// Nothing is added while the same Video with the same Preset, or its
    /// Transcript alone in the same language, is Queued or running.
    pub fn add(&self, video: &Video, content: Content) -> Added {
        let (preset, format) = match content.media() {
            Some(Media::Preset(preset)) => (Some(*preset), None),
            Some(Media::Format(format)) => (None, Some(format.id.clone())),
            None => (None, None),
        };
        self.update(|queue| {
            if let Some(of) = queue.duplicate_of(video, &content) {
                return Added::Duplicate { of };
            }
            let id = queue.next_id;
            queue.next_id += 1;
            queue.entries.push(Entry {
                download: Download {
                    id,
                    url: video.url.clone(),
                    video: video.card(),
                    preset,
                    format,
                    transcript_language: content
                        .transcript()
                        .map(|it| it.language.language.clone()),
                    transcript_srt: content.transcript().is_some_and(|it| it.srt),
                    status: DownloadStatus::Queued,
                },
                video: video.clone(),
                content,
            });
            Added::Queued
        })
    }

    /// Puts a Failed or Cancelled Download back at the end of the Queue, to
    /// download its Video with its Preset and Transcript again: yt-dlp
    /// resumes from a Failed one's Partial, and starts a Cancelled one from
    /// scratch. `None` when it is neither, or while a cancelled one's
    /// yt-dlp is still exiting; nothing changes for a duplicate of a Queued
    /// or running Download, as with [`Downloader::add`].
    pub fn retry(&self, id: u64) -> Option<Added> {
        self.update(|queue| {
            let at = queue.entries.iter().position(|entry| {
                entry.download.id == id
                    && matches!(
                        entry.download.status,
                        DownloadStatus::Failed { .. } | DownloadStatus::Cancelled
                    )
            })?;
            if queue.running == Some(id) {
                return None;
            }
            let retried = &queue.entries[at];
            if let Some(of) = queue.duplicate_of(&retried.video, &retried.content) {
                return Some(Added::Duplicate { of });
            }
            if retried.download.status == DownloadStatus::Cancelled {
                // Its Partial may have been in use when it was cancelled.
                remove_partial(&self.partials_dir, &queue.unused_partial(retried));
            }
            let mut entry = queue.entries.remove(at);
            entry.download.status = DownloadStatus::Queued;
            queue.entries.push(entry);
            Some(Added::Queued)
        })
    }

    /// Cancels a Queued or Downloading Download and removes its Partial, so
    /// that retrying it starts from scratch: a Downloading one's yt-dlp is
    /// killed, and its Partial is removed once it has exited. `false` when
    /// it is neither, once its yt-dlp has ended, or once the app is closing,
    /// which keeps the Partials.
    pub fn cancel(&self, id: u64) -> bool {
        self.update(|queue| {
            let Some(at) = queue
                .entries
                .iter()
                .position(|entry| entry.download.id == id)
                .filter(|_| !queue.closing)
            else {
                return false;
            };
            let entry = &queue.entries[at];
            match entry.download.status {
                DownloadStatus::Queued => {
                    remove_partial(&self.partials_dir, &queue.unused_partial(entry));
                }
                DownloadStatus::Downloading { .. } if queue.running == Some(id) => {
                    if !queue.stopper.stop() {
                        return false;
                    }
                }
                _ => return false,
            }
            queue.entries[at].download.status = DownloadStatus::Cancelled;
            true
        })
    }

    /// Stops the Queue for the app to close: the running Download's yt-dlp
    /// is killed, with the ffmpeg it runs, and no Queued one starts. Unlike
    /// [`Downloader::cancel`], it keeps the Partials, so that a Download of
    /// the same Video with the same Preset after the next launch resumes.
    /// Returns once the running Download has stopped, so that nothing has
    /// its files open or half moved.
    pub fn close(&self) {
        let mut queue = self.queue.lock().unwrap();
        queue.closing = true;
        queue.stopper.stop();
        let _stopped = self
            .changed
            .wait_while(queue, |queue| queue.running.is_some())
            .unwrap();
    }

    /// Removes the Partials untouched for longer than [`PARTIAL_LIFETIME`].
    /// Called at launch, before any Download is received.
    pub fn discard_old_partials(&self) {
        let now = self.clock.now();
        remove_partials_where(&self.partials_dir, |entry| {
            entry
                .metadata()
                .ok()
                .filter(|metadata| metadata.is_file())
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|untouched| untouched > PARTIAL_LIFETIME)
        });
    }

    /// Takes a Done, Failed or Cancelled Download out of the Queue; its
    /// files stay, and so does a Failed one's Partial, which a later Download
    /// of the same Video with the same Preset resumes. A Cancelled one's
    /// yt-dlp may still be exiting; its Partial is removed once it has.
    pub fn remove(&self, id: u64) {
        self.update(|queue| {
            queue
                .entries
                .retain(|entry| entry.download.id != id || !entry.has_ended());
        });
    }

    /// Takes every Done Download out of the Queue; their files stay.
    pub fn clear_done(&self) {
        self.update(|queue| queue.entries.retain(|entry| !entry.is_done()));
    }

    /// Receives the Queued Downloads one at a time, in the order of the
    /// Queue, until none waits. Returns at once while another call receives
    /// them; a Download added meanwhile is received by that one.
    pub fn run(&self) {
        {
            let mut queue = self.queue.lock().unwrap();
            if queue.has_worker {
                return;
            }
            queue.has_worker = true;
        }
        while let Some((video, content, stopper)) = self.update(Queue::start_next) {
            // Each Download's first progress goes out at once.
            *self.progress_sent.lock().unwrap() = None;
            if !self.download(&video, &content, &stopper) {
                // Setup has ended, this call's or one already running; when
                // it failed, the rest wait until Setup is retried and runs
                // the Queue. Its state is read under the Queue's lock, so that a
                // `run` after a Setup that ends Ready meanwhile either
                // finds the worker gone or leaves the Queue to it.
                let mut queue = self.queue.lock().unwrap();
                if self.toolbox.state() != SetupState::Ready {
                    queue.has_worker = false;
                    return;
                }
            }
        }
    }

    /// Receives `content` of `video` for the running Download and saves its
    /// files, unless `stopper` stops it first. `false` when yt-dlp failed to
    /// start: the Download is dropped and Setup restores yt-dlp; once it
    /// does, the user adds the Download again.
    fn download(&self, video: &Video, content: &Content, stopper: &Stopper) -> bool {
        // An update replaces yt-dlp only between Downloads, not between the
        // runs for the Transcript and the media.
        let ytdlp_in_use = self.toolbox.use_ytdlp();
        let ytdlp_version = self.toolbox.ytdlp_version();
        let sign_in_browser = self.settings.sign_in_browser_for(&video.url);
        let mut log = self.logs.start(self.clock.now(), &video.id);
        log.note(&format!("URL: {}", video.url));
        match content.media() {
            Some(Media::Preset(preset)) => log.note(&format!("Preset: {}", preset.name())),
            Some(Media::Format(format)) => log.note(&format!("Format: {}", format.id)),
            None => {}
        }
        if let Some(transcript) = content.transcript() {
            let srt = if transcript.srt { " with .srt" } else { "" };
            log.note(&format!(
                "Transcript: {}{srt}",
                transcript.language.language
            ));
        }
        let received = self.receive(video, content, sign_in_browser, &mut log, stopper);
        // Once yt-dlp has ended, the Download can no longer be cancelled.
        let stopped = stopper.finish();
        let cancelled = stopped && self.running_status() == Some(DownloadStatus::Cancelled);
        let status = match received {
            // Nothing uses its files now. A yt-dlp killed and then found
            // broken is cancelled all the same; the next Download tells
            // whether it is broken.
            _ if cancelled => {
                log.note("Cancelled");
                remove_partial(&self.partials_dir, &partial_prefixes(video, content));
                self.update(|queue| queue.running = None);
                return true;
            }
            // Stopped by the app closing, which keeps the Partial for the
            // next launch; one whose yt-dlp ended first is saved.
            Err(_) if stopped => {
                self.update(|queue| queue.running = None);
                return true;
            }
            Err(Stopped::ToolBroken) => {
                log.note("yt-dlp failed to start");
                self.update(|queue| {
                    let running = queue.running.take();
                    queue
                        .entries
                        .retain(|entry| Some(entry.download.id) != running);
                });
                self.toolbox.prepare();
                return false;
            }
            Err(Stopped::Failed(failure, message)) => DownloadStatus::Failed {
                failure,
                message,
                output: log.last_lines(),
                ytdlp_update: (failure == Failure::ExtractorBroken)
                    .then_some(YtDlpUpdate::Updating),
            },
            Ok(partials) => match self.save(&partials, video) {
                Ok(files) => DownloadStatus::Done {
                    file: files[0].clone(),
                },
                Err(error) => DownloadStatus::Failed {
                    failure: Failure::Disk,
                    message: format!("can't save {}: {error}", partials[0].display()),
                    output: log.last_lines(),
                    ytdlp_update: None,
                },
            },
        };
        match &status {
            DownloadStatus::Done { file } => log.note(&format!("Done: {}", file.display())),
            DownloadStatus::Failed { message, .. } => {
                log.note(&format!("Failed: {message}"));
                eprintln!("can't download {}: {message}", video.url);
            }
            _ => {}
        }
        let updates_ytdlp = matches!(
            status,
            DownloadStatus::Failed {
                ytdlp_update: Some(_),
                ..
            }
        );
        let id = self.update(|queue| {
            if let Some(entry) = queue.running_mut() {
                entry.download.status = status;
            }
            queue.running.take()
        });
        drop(ytdlp_in_use);
        if updates_ytdlp && let Some(id) = id {
            self.update_ytdlp_after(id, ytdlp_version, &mut log);
        }
        true
    }

    /// Updates yt-dlp after the Download `id` failed with a broken
    /// extractor, before the next Download starts, and tells the Download
    /// what became of it; `used` is the version it failed with.
    fn update_ytdlp_after(&self, id: u64, used: Option<String>, log: &mut Log) {
        let updated = self.toolbox.update_ytdlp();
        let installed = self.toolbox.ytdlp_version();
        let update = match updated {
            // An update the user asked for may have installed it meanwhile.
            _ if installed != used => YtDlpUpdate::Updated,
            Ok(_) => YtDlpUpdate::UpToDate,
            Err(error) => {
                log.note(&format!("yt-dlp can't be updated: {error:?}"));
                YtDlpUpdate::Failed
            }
        };
        if update == YtDlpUpdate::Updated {
            log.note(&format!(
                "yt-dlp updated to {}",
                installed.unwrap_or_default()
            ));
        }
        self.update(|queue| {
            // Unless the user retried or removed it meanwhile.
            if let Some(entry) = queue
                .entries
                .iter_mut()
                .find(|entry| entry.download.id == id)
                && let DownloadStatus::Failed { ytdlp_update, .. } = &mut entry.download.status
            {
                *ytdlp_update = Some(update);
            }
        });
    }

    /// Receives `content` of `video` into the Partials folder and returns
    /// its files, the media first, then the text of the Transcript and its
    /// `.srt`. The Transcript comes first, as yt-dlp takes subtitles before
    /// the media, so that a Download whose Transcript fails doesn't receive
    /// the media for nothing.
    fn receive(
        &self,
        video: &Video,
        content: &Content,
        sign_in_browser: Option<Browser>,
        log: &mut Log,
        stopper: &Stopper,
    ) -> Result<Vec<PathBuf>, Stopped> {
        let transcript = content
            .transcript()
            .map(|transcript| {
                self.receive_transcript(video, transcript, sign_in_browser, log, stopper)
            })
            .transpose()?;
        let media = content
            .media()
            .map(|media| {
                // The UI turns from the Transcript to the media at once,
                // while yt-dlp asks the site about the Video again.
                if transcript.is_some() {
                    self.set_status(DownloadStatus::starting(Receiving::of(media)));
                }
                self.receive_media(video, media, sign_in_browser, log, stopper)
            })
            .transpose()?;
        Ok(media
            .into_iter()
            .chain(transcript.into_iter().flatten())
            .collect())
    }

    /// Has yt-dlp download the media and returns its final file.
    fn receive_media(
        &self,
        video: &Video,
        media: &Media,
        sign_in_browser: Option<Browser>,
        log: &mut Log,
        stopper: &Stopper,
    ) -> Result<PathBuf, Stopped> {
        // Dated when received, not as the site last changed the Video, so
        // that a fresh Partial isn't discarded as old.
        let mut args: Vec<OsString> = vec!["--no-playlist".into(), "--no-mtime".into()];
        args.extend(media_args(media));
        args.extend::<[OsString; _]>([
            "-P".into(),
            self.partials_dir.clone().into_os_string(),
            "-o".into(),
            format!("%(id)s.{}.%(ext)s", media.partial_name()).into(),
            // `--print` makes yt-dlp quiet, and quiet hides the progress.
            "--progress".into(),
            "--newline".into(),
            "--progress-template".into(),
            // The Format's id places it among the separate Formats, and its
            // video codec tells whether it has the picture; a missing one
            // prints as `null`.
            r#"download:{"progress": %(progress)j, "format": %(info.format_id|null)j, "vcodec": %(info.vcodec|null)j}"#.into(),
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
        let output = self.run_ytdlp(
            &args,
            sign_in_browser,
            log,
            &mut |line| match line {
                Line::Formats(planned) => formats = planned,
                Line::Progress(line) => {
                    line.note(&mut formats);
                    self.report_progress(line.status(&formats, Receiving::of(media)));
                }
                Line::Processing => self.set_status(DownloadStatus::Processing),
                Line::FinalFile(path) => final_file = Some(path),
                Line::Other => {}
            },
            stopper,
        )?;
        self.failure_resuming(&output, &media_prefix(video, media))?;
        final_file.ok_or_else(|| {
            Stopped::Failed(
                Failure::Unknown,
                "yt-dlp didn't tell where the file is".into(),
            )
        })
    }

    /// Has yt-dlp download the subtitles or captions of the Transcript's
    /// language alone, and returns their text in a file of its own, and
    /// the `.srt` made of them when asked for.
    fn receive_transcript(
        &self,
        video: &Video,
        transcript: &Transcript,
        sign_in_browser: Option<Browser>,
        log: &mut Log,
        stopper: &Stopper,
    ) -> Result<Vec<PathBuf>, Stopped> {
        let language = &transcript.language;
        let name = transcript::ytdlp_name(language, &video.transcripts);
        let write = if language.automatic {
            "--write-auto-subs"
        } else {
            "--write-subs"
        };
        let mut args: Vec<OsString> = [
            "--no-playlist",
            "--skip-download",
            write,
            "--sub-langs",
            &name,
            // yt-dlp takes another format when the site has no WebVTT, and
            // ffmpeg converts it.
            "--sub-format",
            "vtt",
            "--convert-subs",
            "vtt",
            "-o",
            "subtitle:%(id)s.transcript.%(ext)s",
            "-P",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.extend([
            self.partials_dir.clone().into_os_string(),
            "--".into(),
            video.url.clone().into(),
        ]);
        // It prints nothing the app reads.
        let output = self.run_ytdlp(&args, sign_in_browser, log, &mut |_| {}, stopper)?;
        self.failure_resuming(&output, &transcript_prefix(video, language))?;
        // yt-dlp prints no path without the media, and succeeds without a
        // file when the site has no subtitles in the language; it puts the
        // language before the extension.
        let subtitles = self
            .partials_dir
            .join(format!("{}.transcript.{name}.vtt", video.id));
        let vtt = fs::read(&subtitles)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|error| {
                Stopped::Failed(
                    Failure::Unknown,
                    format!("no subtitles at {}: {error}", subtitles.display()),
                )
            })?;
        let mut files = vec![(subtitles.with_extension("txt"), transcript::text(&vtt))];
        if transcript.srt {
            files.push((subtitles.with_extension("srt"), transcript::srt(&vtt)));
        }
        for (file, content) in &files {
            fs::write(file, content).map_err(|error| {
                Stopped::Failed(
                    Failure::Disk,
                    format!("can't write {}: {error}", file.display()),
                )
            })?;
        }
        if let Err(error) = fs::remove_file(&subtitles) {
            eprintln!("can't remove {}: {error}", subtitles.display());
        }
        Ok(files.into_iter().map(|(file, _)| file).collect())
    }

    /// Runs yt-dlp with `args`, so that `stopper` can stop it, and hands
    /// what each line of its stdout tells to `on_line`; everything it prints
    /// but its progress goes to `log`, its stderr once it has exited, and
    /// all but what only `--verbose` prints are kept among the last ones.
    fn run_ytdlp(
        &self,
        args: &[OsString],
        sign_in_browser: Option<Browser>,
        log: &mut Log,
        on_line: &mut dyn FnMut(Line),
        stopper: &Stopper,
    ) -> Result<Output, Stopped> {
        // Verbose, so that the log tells the versions of yt-dlp and ffmpeg
        // and what ffmpeg printed.
        let args: Vec<OsString> = std::iter::once("--verbose".into())
            .chain(args.iter().cloned())
            .collect();
        let command: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
        log.note(&format!("> yt-dlp {}", command.join(" ")));
        let output = self
            .toolbox
            .run_streaming(
                Tool::YtDlp,
                &args,
                sign_in_browser,
                &mut |line| {
                    let read = read_line(line);
                    if !matches!(read, Line::Progress(_)) {
                        log.output(line);
                    }
                    on_line(read);
                },
                stopper,
            )
            .map_err(|_| Stopped::ToolBroken)?;
        for line in String::from_utf8_lossy(&output.stderr).lines() {
            if verbose_only(line) {
                log.note(line);
            } else {
                log.output(line);
            }
        }
        log.note(&format!("yt-dlp exited: {}", output.status));
        Ok(output)
    }

    /// Like [`failure`], for yt-dlp resuming the part of the Partial whose
    /// names begin with `prefix`. yt-dlp fails with HTTP Error 416 when that
    /// part is already whole: it asks for the bytes past its end, and can't
    /// tell it is done (yt-dlp#12994). Every retry would fail the same way,
    /// so the part is removed, and the next one receives it from scratch.
    fn failure_resuming(&self, output: &Output, prefix: &str) -> Result<(), Stopped> {
        failure(output).inspect_err(|stopped| {
            if let Stopped::Failed(_, message) = stopped
                && message.contains("HTTP Error 416")
            {
                remove_partial(&self.partials_dir, &[prefix.to_owned()]);
            }
        })
    }

    /// Moves the final files of `video` out of the Partials folder to the
    /// save folder under the first name free for all of them, each with its
    /// own extension: the name the template gives, then that name with
    /// " (1)", " (2)"…
    fn save(&self, partials: &[PathBuf], video: &Video) -> io::Result<Vec<PathBuf>> {
        let save_dir = self.settings.save_dir();
        let name = file_name(&self.settings.file_name_template(), video);
        fs::create_dir_all(&save_dir)?;
        for number in 0.. {
            let name = match number {
                0 => name.clone(),
                _ => format!("{name} ({number})"),
            };
            let files: Vec<PathBuf> = partials
                .iter()
                .map(|partial| {
                    let extension = partial.extension().unwrap_or_default().to_string_lossy();
                    save_dir.join(format!("{name}.{extension}"))
                })
                .collect();
            // Creating the files takes the names, so that another program
            // can't take them before the move.
            let mut taken = Vec::new();
            for file in &files {
                match create_new(file) {
                    Ok(Some(_)) => taken.push(file),
                    Ok(None) => break,
                    Err(error) => {
                        remove_all(&taken);
                        return Err(error);
                    }
                }
            }
            if taken.len() < files.len() {
                remove_all(&taken);
                continue;
            }
            for (at, (partial, file)) in partials.iter().zip(&files).enumerate() {
                if let Err(error) = move_file(partial, file) {
                    remove_all(&taken[at..]);
                    return Err(error);
                }
            }
            return Ok(files);
        }
        unreachable!("some number is free")
    }

    fn report_progress(&self, mut status: DownloadStatus) {
        let now = self.clock.now();
        let Some(shown) = self.running_status() else {
            return self.set_status(status);
        };
        // A size that turns out bigger than told would take the percent
        // back; it holds until the bytes catch up.
        if let DownloadStatus::Downloading {
            percent: Some(shown),
            ..
        } = shown
            && let DownloadStatus::Downloading { percent, .. } = &mut status
        {
            *percent = Some(percent.map_or(shown, |percent| percent.max(shown)));
        }
        // yt-dlp reports every chunk it receives; the UI gets each tenth of
        // a percent and the turn from the video to the audio at once, and
        // otherwise a line at most once a second, so the speed and time
        // left move even while the percent stands or is unknown.
        let step = |status: &DownloadStatus| match status {
            DownloadStatus::Downloading {
                receiving, percent, ..
            } => Some((*receiving, percent.map(|percent| (percent * 10.0) as u64))),
            _ => None,
        };
        if let DownloadStatus::Downloading { .. } = shown
            && step(&shown) == step(&status)
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

    fn running_status(&self) -> Option<DownloadStatus> {
        let mut queue = self.queue.lock().unwrap();
        Some(queue.running_mut()?.download.status.clone())
    }

    /// Sets the status of the running Download, unless it is cancelled.
    fn set_status(&self, status: DownloadStatus) {
        self.update(|queue| {
            if let Some(entry) = queue.running_mut()
                && entry.download.status != DownloadStatus::Cancelled
            {
                entry.download.status = status;
            }
        });
    }

    /// Changes the Queue and, when the UI would see the change, sends it;
    /// `sending` is held throughout, so that the UI gets the changes in the
    /// same order.
    fn update<T>(&self, change: impl FnOnce(&mut Queue) -> T) -> T {
        let _sending = self.sending.lock().unwrap();
        let (result, changed) = {
            let mut queue = self.queue.lock().unwrap();
            let before = queue.shown();
            let result = change(&mut queue);
            self.changed.notify_all();
            let after = queue.shown();
            (result, (after != before).then_some(after))
        };
        // Sent outside the Queue's lock: a new subscriber reads the Queue
        // while holding the subscribers' lock.
        if let Some(queue) = changed {
            self.events.send(AppEvent::Queue(queue));
        }
        result
    }
}

/// The line of yt-dlp's stderr that tells why it failed, when it did, and
/// what kind of failure it tells of.
fn failure(output: &Output) -> Result<(), Stopped> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut message = error_line(&stderr);
    // Without an error line, such as when yt-dlp crashes, its last line
    // tells why rather than the warnings before it.
    if !message.starts_with("ERROR:") {
        message = stderr
            .lines()
            .rfind(|line| !verbose_only(line))
            .unwrap_or_default()
            .to_owned();
    }
    Err(Stopped::Failed(read_failure(&message), message))
}

/// Whether a line of yt-dlp's stderr is one only `--verbose` prints and
/// tells nothing of what happened on its own: a debug line, or a frame of
/// the Python traceback printed after an error. They are too many to leave
/// room among the last lines for the ones that do tell, such as the
/// exception that ends a traceback.
fn verbose_only(line: &str) -> bool {
    line.starts_with("[debug] ")
        || line.starts_with(char::is_whitespace)
        || line.is_empty()
        || line == "Traceback (most recent call last):"
        || line.starts_with("During handling of the above exception")
        || line.starts_with("The above exception was the direct cause")
}

/// The kind of failure yt-dlp's error message tells of. A Video that needs
/// signing in or isn't there is told as a Probe tells it.
fn read_failure(message: &str) -> Failure {
    match probe::read_error(message) {
        ProbeError::SignInRequired | ProbeError::BotCheck => return Failure::SignInRequired,
        ProbeError::Unsupported | ProbeError::TruncatedUrl => return Failure::Unsupported,
        // About the Sign-in browser, not the disk the files go to.
        ProbeError::CookiesUnreadable { .. } => return Failure::Unknown,
        _ => {}
    }
    let lower = message.to_lowercase();
    let says = |phrases: &[&str]| phrases.iter().any(|phrase| lower.contains(phrase));
    if says(&[
        "no space left",
        "not enough space",
        "disk quota exceeded",
        "permission denied",
        "access is denied",
        "read-only file system",
        "being used by another process",
    ]) {
        Failure::Disk
    } else if says(&[
        "getaddrinfo failed",
        "failed to resolve",
        "name resolution",
        "timed out",
        "connection attempt failed",
        "connection aborted",
        "connection reset",
        "connection refused",
        "forcibly closed",
        "network is unreachable",
        "no route to host",
        "remote end closed connection",
        "incompleteread",
        "transporterror",
        "http error 500",
        "http error 502",
        "http error 503",
        "http error 504",
    ]) {
        Failure::Offline
    } else if says(&[
        "please report this issue",
        "unable to extract",
        "extraction failed",
        "requested format is not available",
    ]) {
        Failure::ExtractorBroken
    } else {
        Failure::Unknown
    }
}

/// What yt-dlp receives for `media` and how it makes the final file of it.
fn media_args(media: &Media) -> Vec<OsString> {
    match media {
        Media::Preset(preset) => preset_args(*preset),
        // That Format, remuxed into an MP4 without encoding it again, or
        // converted into one when an MP4 can't hold its codecs as they are.
        // A picture without sound gets the best sound, preferring AAC as the
        // Video presets do, or stays alone on a site without a Format of the
        // sound alone.
        Media::Format(format) if format.has_picture() => {
            let into_mp4 = if fits_mp4(format) {
                "--remux-video"
            } else {
                "--recode-video"
            };
            let id = &format.id;
            let mut args: Vec<OsString> = if format.has_sound() {
                vec!["-f".into(), id.into()]
            } else {
                vec![
                    "-f".into(),
                    format!("{id}+ba/{id}").into(),
                    "-S".into(),
                    "acodec:aac".into(),
                ]
            };
            args.extend([into_mp4.into(), "mp4".into()]);
            args
        }
        // The sound alone, extracted into an M4A as the M4A preset does.
        Media::Format(format) => ["-f", &format.id]
            .into_iter()
            .map(OsString::from)
            .chain(m4a_args())
            .collect(),
    }
}

/// Whether an MP4 holds the streams of `format` as they are: H.264, H.265,
/// VP9, AV1 or MPEG-4 picture, and AAC, Opus, MP3, FLAC or AC-3 sound. A
/// codec the site doesn't tell is taken to fit.
fn fits_mp4(format: &Format) -> bool {
    const PICTURE: [&str; 11] = [
        "avc1", "avc3", "h264", "hev1", "hvc1", "h265", "hevc", "vp09", "vp9", "av01", "mp4v",
    ];
    const SOUND: [&str; 9] = [
        "none", "mp4a", "aac", "opus", "mp3", "flac", "ac-3", "ac3", "ec-3",
    ];
    let fits = |codec: &Option<String>, known: &[&str]| {
        codec.as_deref().is_none_or(|codec| {
            let codec = codec.to_ascii_lowercase();
            known.iter().any(|known| codec.starts_with(known))
        })
    };
    fits(&format.video_codec, &PICTURE) && fits(&format.audio_codec, &SOUND)
}

/// How ffmpeg extracts the sound into an M4A: AAC sound is copied as it is,
/// and only other sound is encoded, as AAC at 192 kbit/s (yt-dlp ignores
/// the quality when it copies).
fn m4a_args() -> Vec<OsString> {
    ["-x", "--audio-format", "m4a", "--audio-quality", "192K"]
        .into_iter()
        .map(OsString::from)
        .collect()
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
        // M4A prefers AAC, which ffmpeg copies as it is.
        Preset::Audio(AudioPreset::M4a) => {
            return ["-f", "ba/b", "-S", "acodec:aac"]
                .into_iter()
                .map(OsString::from)
                .chain(m4a_args())
                .collect();
        }
        // LAME's VBR quality 1: about 190–250 kbit/s.
        Preset::Audio(AudioPreset::Mp3) => [
            "-f",
            "ba/b",
            "-x",
            "--audio-format",
            "mp3",
            "--audio-quality",
            "1",
        ]
        .map(String::from)
        .to_vec(),
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

/// The beginnings of the names of the files that a Download of `content` of
/// `video` leaves in the Partials folder: `<id>.<preset>.` for the media and
/// `<id>.transcript.<language>.` for the Transcript.
fn partial_prefixes(video: &Video, content: &Content) -> Vec<String> {
    let media = content.media().map(|media| media_prefix(video, media));
    let transcript = content
        .transcript()
        .map(|transcript| transcript_prefix(video, &transcript.language));
    media.into_iter().chain(transcript).collect()
}

fn media_prefix(video: &Video, media: &Media) -> String {
    format!("{}.{}.", video.id, media.partial_name())
}

fn transcript_prefix(video: &Video, language: &TranscriptLanguage) -> String {
    let name = transcript::ytdlp_name(language, &video.transcripts);
    format!("{}.transcript.{name}.", video.id)
}

/// Removes the files in the Partials folder whose names begin with one of
/// `prefixes`.
fn remove_partial(partials_dir: &Path, prefixes: &[String]) {
    remove_partials_where(partials_dir, |entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix.as_str()))
    });
}

/// Removes the files in the Partials folder that `remove` picks.
fn remove_partials_where(partials_dir: &Path, remove: impl Fn(&fs::DirEntry) -> bool) {
    let Ok(entries) = fs::read_dir(partials_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if remove(&entry)
            && let Err(error) = fs::remove_file(entry.path())
        {
            eprintln!(
                "can't remove the Partial {}: {error}",
                entry.path().display()
            );
        }
    }
}

/// Removes the empty files taken for a save that won't happen.
fn remove_all(files: &[&PathBuf]) {
    for file in files {
        let _ = fs::remove_file(file);
    }
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
    /// Its video codec; `none` when it has the sound alone.
    vcodec: Option<String>,
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

    /// What the Format being received is, when yt-dlp tells its codec.
    fn receiving(&self) -> Option<Receiving> {
        match self.vcodec.as_deref()? {
            "none" => Some(Receiving::Audio),
            _ => Some(Receiving::Video),
        }
    }

    /// The status this line tells; with the separate `formats`, its
    /// percent and time left cover all of them. A Format whose codec isn't
    /// told is received as `otherwise`.
    fn status(&self, formats: &[PlannedFormat], otherwise: Receiving) -> DownloadStatus {
        let progress = &self.progress;
        let receiving = self.receiving().unwrap_or(otherwise);
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
                receiving,
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
                receiving,
                percent: None,
                speed: progress.speed,
                eta: own_eta,
            };
        };
        let left = (all - done).max(0.0);
        DownloadStatus::Downloading {
            receiving,
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
