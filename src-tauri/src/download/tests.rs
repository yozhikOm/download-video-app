use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, SystemTime};

use tempfile::TempDir;

use super::*;
use crate::clock::{Clock, FixedClock};
use crate::events::{AppEvent, RecordingSink};
use crate::preset::{AudioPreset, VideoPreset};
use crate::probe::{TranscriptLanguages, Video};
use crate::processes::fake::{FakeProcesses, exit_status};
use crate::releases::fake::{FakeReleases, NEWER_YTDLP_BYTES, YTDLP_BYTES};
use crate::settings::{Browser, Settings};
use crate::toolbox::{SetupState, YtDlpUpdateState};
use crate::transcript::TranscriptLanguage;

const URL: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
const ID: &str = "jNQXAC9IVRw";

struct App {
    dir: TempDir,
    releases: Arc<FakeReleases>,
    toolbox: Arc<Toolbox>,
    processes: Arc<FakeProcesses>,
    settings: Arc<Settings>,
    events: Arc<RecordingSink>,
    clock: Arc<SteppingClock>,
    downloader: Arc<Downloader>,
}

/// Each reading is `step` later than the one before; the step is zero until
/// a test sets it.
struct SteppingClock {
    now: Mutex<SystemTime>,
    step: Mutex<Duration>,
}

impl SteppingClock {
    fn new() -> Self {
        Self {
            now: Mutex::new(SystemTime::UNIX_EPOCH),
            step: Mutex::new(Duration::ZERO),
        }
    }

    fn step_by(&self, step: Duration) {
        *self.step.lock().unwrap() = step;
    }

    fn set(&self, now: SystemTime) {
        *self.now.lock().unwrap() = now;
    }
}

impl Clock for SteppingClock {
    fn now(&self) -> SystemTime {
        let mut now = self.now.lock().unwrap();
        *now += *self.step.lock().unwrap();
        *now
    }
}

impl App {
    /// Starts the app with its tools set up.
    fn start() -> Self {
        Self::start_in(TempDir::new().unwrap())
    }

    /// Closes the app and launches it again on the same folders, with an
    /// empty Queue.
    fn relaunch(self) -> Self {
        Self::start_in(self.dir)
    }

    /// Starts the app in `dir`, setting its tools up there unless they are.
    fn start_in(dir: TempDir) -> Self {
        let releases = Arc::new(FakeReleases::new());
        let processes = Arc::new(FakeProcesses::new());
        let events = Arc::new(RecordingSink::default());
        let settings = Arc::new(Settings::load(
            dir.path().join("settings.json"),
            events.clone(),
        ));
        let toolbox = Arc::new(Toolbox::new(
            dir.path().join("tools"),
            releases.clone(),
            processes.clone(),
            Arc::new(FixedClock(SystemTime::UNIX_EPOCH)),
            events.clone(),
        ));
        toolbox.prepare();
        let clock = Arc::new(SteppingClock::new());
        let downloader = Arc::new(Downloader::new(
            toolbox.clone(),
            settings.clone(),
            dir.path().join("partials"),
            dir.path().join("Downloads"),
            dir.path().join("logs"),
            clock.clone(),
            events.clone(),
        ));
        Self {
            dir,
            releases,
            toolbox,
            processes,
            settings,
            events,
            clock,
            downloader,
        }
    }

    /// Makes yt-dlp act like it downloads the Video with this id: it prints
    /// `progress` lines, writes `content` where it is told and prints what
    /// it is asked to print.
    fn ytdlp_downloads(&self, id: &str, content: &[u8], progress: &[String]) {
        self.ytdlp_downloads_formats(id, content, serde_json::Value::Null, progress);
    }

    /// Like [`App::ytdlp_downloads`], for a Video that yt-dlp receives as
    /// the separate `formats`, each `{format_id, filesize, filesize_approx}`.
    fn ytdlp_downloads_formats(
        &self,
        id: &str,
        content: &[u8],
        formats: serde_json::Value,
        progress: &[String],
    ) {
        self.ytdlp_acts(id, content, formats, progress, None);
    }

    /// Like [`App::ytdlp_downloads`], and asked for the subtitles alone,
    /// yt-dlp writes `subtitles` where it is told.
    fn ytdlp_downloads_with_subtitles(&self, id: &str, content: &[u8], subtitles: &[u8]) {
        self.ytdlp_acts(id, content, serde_json::Value::Null, &[], Some(subtitles));
    }

    /// Makes yt-dlp act like it downloads the Video with this id; asked for
    /// the subtitles alone, it writes `subtitles`, or nothing as when the
    /// site has none in the language asked for.
    fn ytdlp_acts(
        &self,
        id: &str,
        content: &[u8],
        formats: serde_json::Value,
        progress: &[String],
        subtitles: Option<&[u8]>,
    ) {
        let (id, content, progress) = (id.to_owned(), content.to_owned(), progress.to_owned());
        let subtitles = subtitles.map(<[u8]>::to_vec);
        self.processes.script("yt-dlp.exe", move |args| {
            if args.contains(&"--skip-download".into()) {
                return write_subtitles(args, &id, subtitles.as_deref());
            }
            let dir = PathBuf::from(after(args, "-P"));
            // The sound alone is saved in the format it was extracted to.
            let extension = values_of(args, "--audio-format")
                .first()
                .map_or("mp4".into(), |format| format.to_string_lossy().into_owned());
            let name = after(args, "-o")
                .to_str()
                .unwrap()
                .replace("%(id)s", &id)
                .replace("%(ext)s", &extension);
            fs::create_dir_all(&dir).unwrap();
            let file = dir.join(name);
            fs::write(&file, &content).unwrap();
            let mut stdout = String::new();
            let prints = values_of(args, "--print");
            // Printed before anything is received.
            if prints
                .iter()
                .any(|print| print.to_str().unwrap().starts_with("before_dl:"))
            {
                stdout += &serde_json::json!({ "formats": formats }).to_string();
                stdout += "\n";
            }
            stdout += &(progress.join("\n") + "\n");
            for print in prints {
                match print.to_str().unwrap() {
                    print if print.starts_with("before_dl:") => {}
                    "post_process:[processing]" => stdout += "[processing]\n",
                    "after_move:%(filepath)j" => {
                        stdout += &serde_json::to_string(&file).unwrap();
                        stdout += "\n";
                    }
                    other => panic!("unexpected --print {other}"),
                }
            }
            Output {
                status: exit_status(0),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
            }
        });
    }

    fn save_dir(&self) -> PathBuf {
        self.dir.path().join("Downloads")
    }

    fn partials_dir(&self) -> PathBuf {
        self.dir.path().join("partials")
    }

    fn logs_dir(&self) -> PathBuf {
        self.dir.path().join("logs")
    }

    /// Adds a Download to the Queue and runs the Queue until none waits.
    fn download(&self, video: &Video, content: Content) {
        download(&self.downloader, video, content);
    }

    /// Retries the Download added last and runs the Queue.
    fn retry(&self) {
        let id = self.downloader.state().last().unwrap().id;
        self.downloader.retry(id);
        self.downloader.run();
    }

    /// The Queue as each event sent it.
    fn queues(&self) -> Vec<Vec<Download>> {
        queues(&self.events)
    }

    /// The status of the Download added last, as each event sent it.
    fn download_statuses(&self) -> Vec<DownloadStatus> {
        self.queues()
            .into_iter()
            .filter_map(|queue| Some(queue.last()?.status.clone()))
            .collect()
    }

    /// The status of the Download added last.
    fn status(&self) -> Option<DownloadStatus> {
        last_status(&self.downloader)
    }
}

/// Adds a Download to the Queue and runs the Queue until none waits.
fn download(downloader: &Downloader, video: &Video, content: Content) {
    assert_eq!(downloader.add(video, content), Added::Queued);
    downloader.run();
}

/// The statuses of the Downloads in the Queue, in order.
fn statuses(downloader: &Downloader) -> Vec<DownloadStatus> {
    downloader
        .state()
        .into_iter()
        .map(|download| download.status)
        .collect()
}

/// The titles of the Videos of the Downloads in the Queue, in order.
fn titles(downloader: &Downloader) -> Vec<String> {
    downloader
        .state()
        .into_iter()
        .map(|download| download.video.title)
        .collect()
}

/// The status of the Download added last.
fn last_status(downloader: &Downloader) -> Option<DownloadStatus> {
    Some(downloader.state().last()?.status.clone())
}

/// The Queue as each event sent it.
fn queues(events: &RecordingSink) -> Vec<Vec<Download>> {
    events
        .events()
        .into_iter()
        .filter_map(|event| match event {
            AppEvent::Queue(queue) => Some(queue),
            _ => None,
        })
        .collect()
}

/// The argument that follows `option`.
fn after<'a>(args: &'a [OsString], option: &str) -> &'a OsString {
    let index = args.iter().position(|arg| arg == option).unwrap();
    &args[index + 1]
}

/// Every argument that follows `option`.
fn values_of<'a>(args: &'a [OsString], option: &str) -> Vec<&'a OsString> {
    args.windows(2)
        .filter(|pair| pair[0] == option)
        .map(|pair| &pair[1])
        .collect()
}

/// Acts like yt-dlp asked for the subtitles alone: writes `subtitles` in
/// the `-P` folder under the `subtitle:` template, with the language
/// before the extension, or nothing when `subtitles` is `None`.
fn write_subtitles(args: &[OsString], id: &str, subtitles: Option<&[u8]>) -> Output {
    if let Some(subtitles) = subtitles {
        let dir = PathBuf::from(after(args, "-P"));
        let language = after(args, "--sub-langs").to_str().unwrap();
        let template = values_of(args, "-o")
            .into_iter()
            .find_map(|template| template.to_str().unwrap().strip_prefix("subtitle:"))
            .unwrap();
        let name = template
            .replace("%(id)s", id)
            .replace("%(ext)s", &format!("{language}.vtt"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), subtitles).unwrap();
    }
    Output {
        status: exit_status(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

fn video(url: &str, title: &str) -> Video {
    Video {
        id: ID.into(),
        url: url.into(),
        title: title.into(),
        channel: Some("jawed".into()),
        thumbnail: None,
        duration: Some(19.0),
        formats: Vec::new(),
        transcripts: TranscriptLanguages::default(),
        original_language: None,
        chapters: Vec::new(),
        was_live: false,
    }
}

/// The names of the files in `dir`, sorted.
fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn a_downloaded_video_is_moved_to_the_save_folder_under_its_title() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    let saved = app.save_dir().join("Me at the zoo.mp4");
    assert_eq!(
        app.status(),
        Some(DownloadStatus::Done {
            file: saved.clone()
        })
    );
    assert_eq!(file_names(&app.save_dir()), ["Me at the zoo.mp4"]);
    assert_eq!(fs::read(&saved).unwrap(), b"the video");
    assert_eq!(file_names(&app.partials_dir()), [] as [String; 0]);
}

#[test]
fn an_audio_download_goes_through_processing_to_a_file_of_its_format() {
    for (preset, name) in [
        (AudioPreset::M4a, "Me at the zoo.m4a"),
        (AudioPreset::Mp3, "Me at the zoo.mp3"),
    ] {
        let app = App::start();
        app.ytdlp_downloads(
            ID,
            b"the sound",
            &[progress_of(AUDIO_ONLY, 1000, 1000, 400.0, 0)],
        );

        app.download(&video(URL, "Me at the zoo"), preset.into());

        let saved = app.save_dir().join(name);
        assert_eq!(
            app.download_statuses(),
            [
                DownloadStatus::Queued,
                DownloadStatus::starting(Receiving::Audio),
                downloading(Receiving::Audio, 100.0, 400.0, 0),
                DownloadStatus::Processing,
                DownloadStatus::Done {
                    file: saved.clone()
                },
            ],
            "{preset:?}"
        );
        assert_eq!(fs::read(&saved).unwrap(), b"the sound");
        assert_eq!(file_names(&app.partials_dir()), [] as [String; 0]);
    }
}

/// Format ids as YouTube's.
const VIDEO_AND_AUDIO: &str = "18";
const VIDEO_ONLY: &str = "395";
const AUDIO_ONLY: &str = "251";

/// A progress line of a Format with both video and audio, cut down to the
/// fields the app reads.
fn progress(downloaded: u64, total: u64, speed: f64, eta: u64) -> String {
    progress_of(VIDEO_AND_AUDIO, downloaded, total, speed, eta)
}

/// A progress line of this Format.
fn progress_of(format: &str, downloaded: u64, total: u64, speed: f64, eta: u64) -> String {
    progress_line(
        format,
        serde_json::json!({
            "status": "downloading",
            "downloaded_bytes": downloaded,
            "total_bytes": total,
            "speed": speed,
            "eta": eta,
            "_percent_str": "  0.5%",
        }),
    )
}

/// A line as yt-dlp prints it with the app's `--progress-template`; only
/// [`AUDIO_ONLY`] has no picture.
fn progress_line(format: &str, progress: serde_json::Value) -> String {
    let vcodec = if format == AUDIO_ONLY { "none" } else { "avc1" };
    serde_json::json!({ "progress": progress, "format": format, "vcodec": vcodec }).to_string()
}

/// The separate Formats yt-dlp names before receiving them, with their
/// sizes when the site tells.
fn formats(sizes: &[(&str, Option<u64>)]) -> serde_json::Value {
    sizes
        .iter()
        .map(|(id, size)| {
            serde_json::json!({ "format_id": id, "filesize": size, "filesize_approx": null })
        })
        .collect()
}

/// The separate Formats with only the approximate sizes the site tells.
fn approx_formats(sizes: &[(&str, u64)]) -> serde_json::Value {
    sizes
        .iter()
        .map(|(id, size)| {
            serde_json::json!({ "format_id": id, "filesize": null, "filesize_approx": size })
        })
        .collect()
}

fn downloading(receiving: Receiving, percent: f64, speed: f64, eta: u64) -> DownloadStatus {
    DownloadStatus::Downloading {
        receiving,
        percent: Some(percent),
        speed: Some(speed),
        eta: Some(eta),
    }
}

#[test]
fn a_download_goes_from_queued_through_downloading_and_processing_to_done() {
    let app = App::start();
    app.ytdlp_downloads(
        ID,
        b"the video",
        &[
            progress(250, 1000, 100.0, 7),
            progress(1000, 1000, 400.0, 0),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        app.download_statuses(),
        [
            DownloadStatus::Queued,
            DownloadStatus::starting(Receiving::Video),
            downloading(Receiving::Video, 25.0, 100.0, 7),
            downloading(Receiving::Video, 100.0, 400.0, 0),
            DownloadStatus::Processing,
            DownloadStatus::Done {
                file: app.save_dir().join("Me at the zoo.mp4")
            },
        ]
    );
    let download = &app.downloader.state()[0];
    assert_eq!(download.video.title, "Me at the zoo");
    assert_eq!(download.preset, Some(VideoPreset::P1080.into()));
    assert_eq!(download.transcript_language, None);
}

#[test]
fn a_video_received_as_separate_video_and_audio_shows_one_progress_for_both() {
    let app = App::start();
    app.ytdlp_downloads_formats(
        ID,
        b"the video",
        formats(&[(VIDEO_ONLY, Some(1500)), (AUDIO_ONLY, Some(500))]),
        &[
            progress_of(VIDEO_ONLY, 500, 1500, 100.0, 10),
            progress_of(VIDEO_ONLY, 1500, 1500, 100.0, 0),
            progress_of(AUDIO_ONLY, 250, 500, 50.0, 5),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    // Of the 2000 bytes of both, the time left is what remains of both at
    // the current speed.
    assert_eq!(
        app.download_statuses()[2..5],
        [
            downloading(Receiving::Video, 25.0, 100.0, 15),
            downloading(Receiving::Video, 75.0, 100.0, 5),
            downloading(Receiving::Audio, 87.5, 50.0, 5),
        ]
    );
}

#[test]
fn the_turn_from_the_video_to_the_audio_shows_at_once() {
    let app = App::start();
    app.ytdlp_downloads_formats(
        ID,
        b"the video",
        formats(&[(VIDEO_ONLY, Some(1000)), (AUDIO_ONLY, Some(1000))]),
        &[
            progress_of(VIDEO_ONLY, 1000, 1000, 100.0, 0),
            progress_of(AUDIO_ONLY, 0, 1000, 100.0, 10),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    // The clock stands still and the percent stays at half.
    assert_eq!(
        app.download_statuses()[2..4],
        [
            downloading(Receiving::Video, 50.0, 100.0, 10),
            downloading(Receiving::Audio, 50.0, 100.0, 10),
        ]
    );
}

#[test]
fn a_format_whose_codec_yt_dlp_doesnt_tell_counts_as_what_its_preset_saves() {
    for (preset, receiving) in [
        (Preset::from(VideoPreset::P1080), Receiving::Video),
        (AudioPreset::Mp3.into(), Receiving::Audio),
    ] {
        let app = App::start();
        let line = serde_json::json!({
            "progress": { "downloaded_bytes": 250, "total_bytes": 1000, "speed": 100.0, "eta": 7 },
            "format": "http-0",
            "vcodec": null,
        });
        app.ytdlp_downloads(ID, b"the media", &[line.to_string()]);

        app.download(&video(URL, "Me at the zoo"), preset.into());

        assert_eq!(
            app.download_statuses()[2],
            downloading(receiving, 25.0, 100.0, 7),
            "{preset:?}"
        );
    }
}

#[test]
fn a_download_shows_no_percent_until_the_sizes_of_its_formats_are_known() {
    let app = App::start();
    app.ytdlp_downloads_formats(
        ID,
        b"the video",
        formats(&[(VIDEO_ONLY, Some(1500)), (AUDIO_ONLY, None)]),
        &[
            progress_of(VIDEO_ONLY, 750, 1500, 100.0, 8),
            progress_of(AUDIO_ONLY, 250, 500, 50.0, 6),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    // The video's own percent would fall back once the audio's size is
    // known; only its speed and time left are shown.
    assert_eq!(
        app.download_statuses()[2..4],
        [
            DownloadStatus::Downloading {
                receiving: Receiving::Video,
                percent: None,
                speed: Some(100.0),
                eta: Some(8)
            },
            downloading(Receiving::Audio, 87.5, 50.0, 5),
        ]
    );
}

#[test]
fn a_received_format_of_approximate_size_counts_with_the_bytes_received() {
    let app = App::start();
    app.ytdlp_downloads_formats(
        ID,
        b"the video",
        approx_formats(&[(VIDEO_ONLY, 1000), (AUDIO_ONLY, 500)]),
        &[
            progress_of(VIDEO_ONLY, 1500, 1500, 100.0, 0),
            progress_of(AUDIO_ONLY, 50, 500, 50.0, 9),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        app.download_statuses()[2..4],
        [
            downloading(Receiving::Video, 75.0, 100.0, 5),
            downloading(Receiving::Audio, 77.5, 50.0, 9)
        ]
    );
}

#[test]
fn the_percent_holds_when_a_format_turns_out_bigger_than_told() {
    let app = App::start();
    app.ytdlp_downloads_formats(
        ID,
        b"the video",
        formats(&[(VIDEO_ONLY, Some(1500)), (AUDIO_ONLY, Some(500))]),
        &[
            progress_of(VIDEO_ONLY, 1500, 1500, 100.0, 0),
            progress_of(AUDIO_ONLY, 100, 1000, 50.0, 18),
            progress_of(AUDIO_ONLY, 1000, 1000, 50.0, 0),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    // 1600 of 2500 bytes would be 64%; the bar holds at 75% instead, as the
    // turn to the audio shows.
    assert_eq!(percents(&app), [None, Some(75.0), Some(75.0), Some(100.0)]);
}

#[test]
fn the_speed_and_time_left_update_each_second_while_the_percent_stands() {
    let app = App::start();
    app.clock.step_by(Duration::from_millis(600));
    app.ytdlp_downloads(
        ID,
        b"the video",
        &[
            progress(250, 1000, 100.0, 9),
            progress(250, 1000, 80.0, 10),
            progress(250, 1000, 50.0, 15),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    // The second line comes 0.6 s after the first, the third 1.2 s.
    assert_eq!(
        app.download_statuses()[2..4],
        [
            downloading(Receiving::Video, 25.0, 100.0, 9),
            downloading(Receiving::Video, 25.0, 50.0, 15)
        ]
    );
}

#[test]
fn the_speed_and_time_left_update_each_second_while_the_percent_is_unknown() {
    let app = App::start();
    app.clock.step_by(Duration::from_millis(600));
    app.ytdlp_downloads_formats(
        ID,
        b"the video",
        formats(&[(VIDEO_ONLY, Some(1500)), (AUDIO_ONLY, None)]),
        &[
            progress_of(VIDEO_ONLY, 250, 1500, 100.0, 9),
            progress_of(VIDEO_ONLY, 500, 1500, 80.0, 10),
            progress_of(VIDEO_ONLY, 750, 1500, 50.0, 15),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    // The second line comes 0.6 s after the first, the third 1.2 s.
    assert_eq!(
        app.download_statuses()[2..5],
        [
            DownloadStatus::Downloading {
                receiving: Receiving::Video,
                percent: None,
                speed: Some(100.0),
                eta: Some(9)
            },
            DownloadStatus::Downloading {
                receiving: Receiving::Video,
                percent: None,
                speed: Some(50.0),
                eta: Some(15)
            },
            DownloadStatus::Processing,
        ]
    );
}

#[test]
fn a_new_download_shows_its_first_progress_at_once() {
    let app = App::start();
    app.ytdlp_downloads_formats(
        ID,
        b"the video",
        formats(&[(VIDEO_ONLY, Some(1500)), (AUDIO_ONLY, None)]),
        &[progress_of(VIDEO_ONLY, 250, 1500, 100.0, 9)],
    );
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());
    let first = app.download_statuses().len();

    // The clock stands still, so the line comes at once after the last
    // one of the Download before.
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        app.download_statuses()[first + 2],
        DownloadStatus::Downloading {
            receiving: Receiving::Video,
            percent: None,
            speed: Some(100.0),
            eta: Some(9)
        }
    );
}

#[test]
fn a_taken_name_gets_the_next_free_number_and_nothing_is_overwritten() {
    let app = App::start();
    fs::create_dir_all(app.save_dir()).unwrap();
    fs::write(app.save_dir().join("Me at the zoo.mp4"), b"first").unwrap();
    fs::write(app.save_dir().join("Me at the zoo (1).mp4"), b"second").unwrap();
    app.ytdlp_downloads(ID, b"third", &[]);

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    let saved = app.save_dir().join("Me at the zoo (2).mp4");
    assert_eq!(
        app.status(),
        Some(DownloadStatus::Done {
            file: saved.clone()
        })
    );
    assert_eq!(fs::read(&saved).unwrap(), b"third");
    assert_eq!(
        fs::read(app.save_dir().join("Me at the zoo.mp4")).unwrap(),
        b"first"
    );
    assert_eq!(
        fs::read(app.save_dir().join("Me at the zoo (1).mp4")).unwrap(),
        b"second"
    );
}

/// A Partial, the empty file that took its name in the save folder, and a
/// file of the user's named like the temporary copy.
fn copy_across_drives(partial_content: Option<&[u8]>) -> (TempDir, PathBuf, PathBuf, PathBuf) {
    let dir = TempDir::new().unwrap();
    let partials = dir.path().join("partials");
    let save_dir = dir.path().join("Downloads");
    fs::create_dir_all(&partials).unwrap();
    fs::create_dir_all(&save_dir).unwrap();
    let partial = partials.join(format!("{ID}.1080p.mp4"));
    if let Some(content) = partial_content {
        fs::write(&partial, content).unwrap();
    }
    let to = save_dir.join("Me at the zoo.mp4");
    fs::write(&to, b"").unwrap();
    let users = save_dir.join("Me at the zoo.mp4.download");
    fs::write(&users, b"the user's").unwrap();
    (dir, partial, to, users)
}

#[test]
fn a_copy_across_drives_leaves_a_file_named_like_its_temporary_alone() {
    let (_dir, partial, to, users) = copy_across_drives(Some(b"the video"));

    copy_then_rename(&partial, &to).unwrap();

    assert_eq!(fs::read(&to).unwrap(), b"the video");
    assert_eq!(fs::read(&users).unwrap(), b"the user's");
    assert_eq!(
        file_names(to.parent().unwrap()),
        ["Me at the zoo.mp4", "Me at the zoo.mp4.download"]
    );
    assert!(!partial.exists());
}

#[test]
fn a_failed_copy_across_drives_leaves_a_file_named_like_its_temporary_alone() {
    let (_dir, partial, to, users) = copy_across_drives(None);

    assert!(copy_then_rename(&partial, &to).is_err());

    assert_eq!(fs::read(&users).unwrap(), b"the user's");
    assert_eq!(
        file_names(to.parent().unwrap()),
        ["Me at the zoo.mp4", "Me at the zoo.mp4.download"]
    );
}

#[test]
fn characters_windows_forbids_in_a_title_are_replaced_by_lookalikes() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.download(
        &video(URL, "Why? A/B: \"test\" | <1> * \\ end. "),
        VideoPreset::P1080.into(),
    );

    assert_eq!(
        file_names(&app.save_dir()),
        ["Why？ A⧸B： ＂test＂ ｜ ＜1＞ ＊ ⧹ end.mp4"]
    );
}

#[test]
fn a_title_that_cant_be_a_file_name_gives_way_to_the_video_id() {
    for title in ["", " . ", "CON", "nul"] {
        let app = App::start();
        app.ytdlp_downloads(ID, b"the video", &[]);

        app.download(&video(URL, title), VideoPreset::P1080.into());

        assert_eq!(
            file_names(&app.save_dir()),
            [format!("{ID}.mp4")],
            "{title:?}"
        );
    }
}

#[test]
fn a_download_that_yt_dlp_fails_is_failed_and_keeps_its_partial() {
    let app = App::start();
    app.processes.script("yt-dlp.exe", |args| {
        let partial = PathBuf::from(after(args, "-P")).join(format!("{ID}.1080p.f137.mp4.part"));
        fs::create_dir_all(partial.parent().unwrap()).unwrap();
        fs::write(&partial, b"half").unwrap();
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: b"WARNING: retrying\nERROR: unable to download video data: Read timed out.\n"
                .to_vec(),
        }
    });

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        app.status(),
        Some(DownloadStatus::Failed {
            failure: Failure::Offline,
            message: "ERROR: unable to download video data: Read timed out.".into(),
            output: vec![
                "WARNING: retrying".into(),
                "ERROR: unable to download video data: Read timed out.".into()
            ],
            ytdlp_update: None,
        })
    );
    assert_eq!(
        file_names(&app.partials_dir()),
        [format!("{ID}.1080p.f137.mp4.part")]
    );
    assert!(!app.save_dir().exists());
}

/// The kind of failure a Download that yt-dlp fails with `stderr` ends with.
fn failure_of(stderr: &str) -> Failure {
    let app = App::start();
    app.processes.fail_with("yt-dlp.exe", stderr.as_bytes());
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());
    match app.status() {
        Some(DownloadStatus::Failed { failure, .. }) => failure,
        status => panic!("not Failed: {status:?}"),
    }
}

#[test]
fn a_download_yt_dlp_fails_tells_what_kind_of_failure_it_is() {
    let cases = [
        (
            "ERROR: [youtube] jNQXAC9IVRw: Unable to download API page: <urlopen error [Errno 11001] getaddrinfo failed> (caused by TransportError('<urlopen error [Errno 11001] getaddrinfo failed>'))",
            Failure::Offline,
        ),
        (
            "ERROR: [youtube] jNQXAC9IVRw: Unable to download API page: HTTPSConnectionPool(host='www.youtube.com', port=443): Max retries exceeded with url: /youtubei/v1/player (Caused by NameResolutionError(\"<urllib3.connection.HTTPSConnection object at 0x0000020F>: Failed to resolve 'www.youtube.com' ([Errno 11001] getaddrinfo failed)\")) (caused by TransportError(...))",
            Failure::Offline,
        ),
        (
            "ERROR: unable to download video data: ('Connection aborted.', ConnectionResetError(10054, 'An existing connection was forcibly closed by the remote host', None, 10054, None))",
            Failure::Offline,
        ),
        (
            "ERROR: unable to download video data: <urlopen error [WinError 10060] A connection attempt failed because the connected party did not properly respond after a period of time>",
            Failure::Offline,
        ),
        (
            "ERROR: Unsupported URL: https://example.com/",
            Failure::Unsupported,
        ),
        (
            "ERROR: [youtube] jNQXAC9IVRw: Video unavailable. This video has been removed by the uploader",
            Failure::Unsupported,
        ),
        (
            "ERROR: [youtube] jNQXAC9IVRw: Sign in to confirm your age. This video may be inappropriate for some users. Use --cookies-from-browser or --cookies for the authentication.",
            Failure::SignInRequired,
        ),
        (
            "ERROR: [youtube] jNQXAC9IVRw: Sign in to confirm you’re not a bot. Use --cookies-from-browser or --cookies for the authentication.",
            Failure::SignInRequired,
        ),
        (
            "ERROR: [youtube] jNQXAC9IVRw: Join this channel to get access to members-only content like this video, and other exclusive perks.",
            Failure::SignInRequired,
        ),
        (
            "ERROR: [youtube] jNQXAC9IVRw: Unable to extract initial player response; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= , filling out the appropriate issue template. Confirm you are on the latest version using  yt-dlp -U",
            Failure::ExtractorBroken,
        ),
        (
            "ERROR: [youtube] jNQXAC9IVRw: Requested format is not available. Use --list-formats for a list of available formats",
            Failure::ExtractorBroken,
        ),
        (
            "ERROR: unable to open for writing: [Errno 28] No space left on device",
            Failure::Disk,
        ),
        (
            "ERROR: Postprocessing: [WinError 112] There is not enough space on the disk",
            Failure::Disk,
        ),
        (
            "ERROR: unable to open for writing: [Errno 13] Permission denied: 'C:\\\\Users\\\\me\\\\AppData\\\\Local\\\\partials\\\\jNQXAC9IVRw.1080p.f137.mp4.part'",
            Failure::Disk,
        ),
        (
            "ERROR: Unable to rename file: [WinError 32] The process cannot access the file because it is being used by another process",
            Failure::Disk,
        ),
        (
            "ERROR: unable to download video data: HTTP Error 503: Service Unavailable",
            Failure::Offline,
        ),
        // Geo-blocks and login walls of other sites refuse with 403 too.
        (
            "ERROR: unable to download video data: HTTP Error 403: Forbidden",
            Failure::Unknown,
        ),
        // The Sign-in browser's cookies are about the browser, not the
        // disk the files go to.
        (
            "ERROR: Could not copy Chrome cookie database. See  https://github.com/yt-dlp/yt-dlp/issues/7271  for more info",
            Failure::Unknown,
        ),
        (
            "ERROR: Postprocessing: Conversion failed!",
            Failure::Unknown,
        ),
    ];
    for (stderr, expected) in cases {
        assert_eq!(failure_of(&format!("{stderr}\n")), expected, "{stderr}");
    }
}

/// Makes yt-dlp print `stdout` and `stderr` and fail.
fn ytdlp_fails_printing(app: &App, stdout: String, stderr: String) {
    app.processes.script("yt-dlp.exe", move |_| Output {
        status: exit_status(1),
        stdout: stdout.clone().into_bytes(),
        stderr: stderr.clone().into_bytes(),
    });
}

/// The last lines yt-dlp printed that the Download added last keeps.
fn output(app: &App) -> Vec<String> {
    match app.status() {
        Some(DownloadStatus::Failed { output, .. }) => output,
        status => panic!("not Failed: {status:?}"),
    }
}

#[test]
fn a_failed_download_keeps_what_yt_dlp_printed_but_its_progress() {
    let app = App::start();
    ytdlp_fails_printing(
        &app,
        format!(
            "[info] jNQXAC9IVRw: Downloading 1 format(s): 137+140\n{}\n{}\n",
            progress(10, 100, 1.0, 9),
            progress(20, 100, 1.0, 8)
        ),
        "WARNING: [youtube] Falling back to generic n function search\nERROR: unable to download video data: Read timed out.\n".into(),
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        output(&app),
        [
            "[info] jNQXAC9IVRw: Downloading 1 format(s): 137+140",
            "WARNING: [youtube] Falling back to generic n function search",
            "ERROR: unable to download video data: Read timed out.",
        ]
    );
}

#[test]
fn a_failed_download_keeps_only_the_last_twenty_lines_yt_dlp_printed() {
    let app = App::start();
    let lines: Vec<String> = (1..=30).map(|n| format!("WARNING: number {n}")).collect();
    ytdlp_fails_printing(&app, String::new(), lines.join("\n") + "\n");

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(output(&app), lines[10..]);
}

#[test]
fn a_download_in_the_queue_tells_its_videos_url() {
    let app = App::start();

    app.downloader
        .add(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(app.downloader.state()[0].url, URL);
}

#[test]
fn a_download_logs_all_yt_dlp_printed_under_the_time_it_started() {
    let app = App::start();
    // 2026-10-27 11:04:20 UTC.
    app.clock
        .set(SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_099_060));
    let lines: Vec<String> = (1..=30).map(|n| format!("WARNING: number {n}")).collect();
    ytdlp_fails_printing(
        &app,
        "[info] jNQXAC9IVRw: Downloading 1 format(s): 137+140\n".into(),
        lines.join("\n") + "\n",
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        file_names(&app.logs_dir()),
        ["2026-10-27 11-04-20 jNQXAC9IVRw.log"]
    );
    let log =
        fs::read_to_string(app.logs_dir().join("2026-10-27 11-04-20 jNQXAC9IVRw.log")).unwrap();
    assert!(log.contains(URL), "{log}");
    assert!(
        log.contains("[info] jNQXAC9IVRw: Downloading 1 format(s): 137+140\n"),
        "{log}"
    );
    assert!(log.contains(&(lines.join("\n") + "\n")), "{log}");
}

#[test]
fn only_the_ten_newest_logs_are_kept() {
    let app = App::start();
    app.clock.step_by(Duration::from_secs(1));
    app.ytdlp_downloads(ID, b"the video", &[]);
    let zoo = video(URL, "Me at the zoo");
    app.download(&zoo, VideoPreset::P1080.into());
    app.download(&zoo, VideoPreset::P1080.into());
    let oldest = file_names(&app.logs_dir());
    assert_eq!(oldest.len(), 2);

    for _ in 0..10 {
        app.download(&zoo, VideoPreset::P1080.into());
    }

    let kept = file_names(&app.logs_dir());
    assert_eq!(kept.len(), 10, "{kept:?}");
    assert!(oldest.iter().all(|log| !kept.contains(log)), "{kept:?}");
}

#[test]
fn a_yt_dlp_that_fails_without_an_error_line_is_told_by_its_last_line_not_its_warnings() {
    assert_eq!(
        failure_of(
            "WARNING: [youtube] jNQXAC9IVRw: nsig extraction failed: Some formats may be missing; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q=\nTraceback (most recent call last):\n  File \"yt_dlp\\YoutubeDL.py\", line 1730, in wrapper\nKeyError: 'formats'\n"
        ),
        Failure::Unknown
    );
}

#[test]
fn a_download_runs_yt_dlp_verbose_and_logs_its_debug_lines_without_keeping_them() {
    let app = App::start();
    app.processes.script("yt-dlp.exe", |args| {
        let mut stderr = String::new();
        if args.contains(&"--verbose".into()) {
            stderr += "[debug] yt-dlp version stable@2026.10.21 from yt-dlp/yt-dlp [8cdf4fea4] (win_exe)\n";
        }
        stderr += "ERROR: Postprocessing: Conversion failed!\n";
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: stderr.into_bytes(),
        }
    });

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(output(&app), ["ERROR: Postprocessing: Conversion failed!"]);
    let logs = file_names(&app.logs_dir());
    let log = fs::read_to_string(app.logs_dir().join(&logs[0])).unwrap();
    assert!(
        log.contains("[debug] yt-dlp version stable@2026.10.21"),
        "{log}"
    );
}

/// What yt-dlp `--verbose` prints after an error: the frames of where it
/// was raised and of its cause, then the cause.
const TRACEBACK: &str = "  File \"yt_dlp\\extractor\\common.py\", line 742, in extract
  File \"yt_dlp\\extractor\\generic.py\", line 2393, in _real_extract
  File \"yt_dlp\\extractor\\common.py\", line 911, in _request_webpage

  File \"yt_dlp\\extractor\\common.py\", line 898, in _request_webpage
  File \"yt_dlp\\YoutubeDL.py\", line 4172, in urlopen
  File \"yt_dlp\\networking\\common.py\", line 117, in send
yt_dlp.networking.exceptions.HTTPError: HTTP Error 404: Not Found
";

#[test]
fn a_failed_download_logs_the_traceback_yt_dlp_printed_but_keeps_only_its_exception() {
    let app = App::start();
    ytdlp_fails_printing(
        &app,
        String::new(),
        format!(
            "WARNING: [generic] Falling back on generic information extractor\nERROR: [generic] Unable to download webpage: HTTP Error 404: Not Found\n{TRACEBACK}"
        ),
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        output(&app),
        [
            "WARNING: [generic] Falling back on generic information extractor",
            "ERROR: [generic] Unable to download webpage: HTTP Error 404: Not Found",
            "yt_dlp.networking.exceptions.HTTPError: HTTP Error 404: Not Found",
        ]
    );
    let logs = file_names(&app.logs_dir());
    let log = fs::read_to_string(app.logs_dir().join(&logs[0])).unwrap();
    assert!(log.contains(TRACEBACK), "{log}");
}

#[test]
fn a_yt_dlp_that_crashes_is_told_by_its_exception_not_its_debug_lines_or_traceback() {
    let app = App::start();
    ytdlp_fails_printing(
        &app,
        String::new(),
        "[debug] Command-line config: ['--verbose']\n[debug] yt-dlp version stable@2026.10.21\nTraceback (most recent call last):\n  File \"yt_dlp\\__main__.py\", line 17, in <module>\n  File \"yt_dlp\\YoutubeDL.py\", line 1730, in wrapper\nKeyError: 'formats'\n\nDuring handling of the above exception, another exception occurred:\n\nTraceback (most recent call last):\n  File \"yt_dlp\\__init__.py\", line 1093, in main\nSystemExit: 1\n".into(),
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    match app.status() {
        Some(DownloadStatus::Failed {
            failure, message, ..
        }) => assert_eq!(
            (failure, message.as_str()),
            (Failure::Unknown, "SystemExit: 1")
        ),
        status => panic!("not Failed: {status:?}"),
    }
    assert_eq!(output(&app), ["KeyError: 'formats'", "SystemExit: 1"]);
}

#[test]
fn of_logs_started_in_the_same_second_the_oldest_go_first() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);
    let zoo = video(URL, "Me at the zoo");
    app.download(&zoo, VideoPreset::P1080.into());
    let first = file_names(&app.logs_dir());

    for _ in 0..10 {
        app.download(&zoo, VideoPreset::P1080.into());
    }

    let kept = file_names(&app.logs_dir());
    assert_eq!(kept.len(), 10, "{kept:?}");
    assert!(!kept.contains(&first[0]), "{kept:?}");
}

#[test]
fn a_download_whose_files_cant_be_saved_is_failed_for_the_disk() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);
    // A file stands where the save folder should be.
    fs::write(app.save_dir(), b"").unwrap();

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert!(
        matches!(
            app.status(),
            Some(DownloadStatus::Failed {
                failure: Failure::Disk,
                ..
            })
        ),
        "{:?}",
        app.status()
    );
}

#[test]
fn a_download_that_yt_dlp_cant_resume_is_failed_without_its_partial() {
    let app = App::start();
    app.processes.script("yt-dlp.exe", |args| {
        let partial = PathBuf::from(after(args, "-P")).join(format!("{ID}.1080p.f137.mp4.part"));
        fs::create_dir_all(partial.parent().unwrap()).unwrap();
        fs::write(&partial, b"whole").unwrap();
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: b"ERROR: unable to download video data: HTTP Error 416: Requested range not satisfiable\n"
                .to_vec(),
        }
    });

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        app.status(),
        Some(DownloadStatus::Failed {
            failure: Failure::Unknown,
            message: "ERROR: unable to download video data: HTTP Error 416: Requested range not satisfiable".into(),
            output: vec!["ERROR: unable to download video data: HTTP Error 416: Requested range not satisfiable".into()],
            ytdlp_update: None,
        })
    );
    assert_eq!(file_names(&app.partials_dir()), Vec::<String>::new());
}

#[test]
fn a_transcript_yt_dlp_cant_resume_leaves_the_medias_partial() {
    let app = App::start();
    let media = format!("{ID}.1080p.f137.mp4.part");
    fs::create_dir_all(app.partials_dir()).unwrap();
    fs::write(app.partials_dir().join(&media), b"half").unwrap();
    app.processes.fail_with(
        "yt-dlp.exe",
        b"ERROR: Unable to download video subtitles: HTTP Error 416: Requested range not satisfiable\n",
    );

    app.download(
        &video(URL, "Me at the zoo"),
        Content::MediaWithTranscript(VideoPreset::P1080.into(), manual("en")),
    );

    assert!(matches!(app.status(), Some(DownloadStatus::Failed { .. })));
    assert_eq!(file_names(&app.partials_dir()), [media]);
}

/// The yt-dlp runs so far, each with its arguments.
fn ytdlp_runs(app: &App) -> Vec<Vec<OsString>> {
    app.processes
        .runs()
        .into_iter()
        .filter(|(program, _)| program.ends_with("yt-dlp.exe"))
        .map(|(_, args)| args)
        .collect()
}

#[test]
fn a_failed_download_is_retried_with_its_own_video_under_the_same_partial_name() {
    let app = App::start();
    app.processes
        .fail_with("yt-dlp.exe", b"ERROR: Read timed out.\n");
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.retry();

    assert_eq!(
        app.status(),
        Some(DownloadStatus::Done {
            file: app.save_dir().join("Me at the zoo.mp4")
        })
    );
    let runs = ytdlp_runs(&app);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0], runs[1]);
}

#[test]
fn a_broken_yt_dlp_is_restored_and_the_download_is_dropped() {
    let app = App::start();
    app.processes.break_tool("yt-dlp.exe");
    let fetched_before = app.releases.fetches().len();

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(app.downloader.state(), []);
    assert_eq!(app.queues().last(), Some(&vec![]));
    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(
        app.releases.fetches()[fetched_before..],
        [crate::releases::fake::YTDLP_URL]
    );
}

#[test]
fn the_queue_waits_while_a_broken_yt_dlp_cant_be_restored() {
    let app = App::start();
    app.processes.break_tool("yt-dlp.exe");
    app.releases.set_online(false);
    for id in ["first", "second"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }

    app.downloader.run();

    assert!(matches!(app.toolbox.state(), SetupState::Failed { .. }));
    assert_eq!(ytdlp_runs(&app).len(), 1);
    assert_eq!(titles(&app.downloader), ["second"]);
    assert_eq!(statuses(&app.downloader), [DownloadStatus::Queued]);
}

#[test]
fn the_queue_goes_on_once_a_setup_already_running_restores_yt_dlp() {
    let app = App::start();
    app.ytdlp_downloads("second", b"the video", &[]);
    fs::remove_file(app.dir.path().join("tools").join("yt-dlp.exe")).unwrap();
    for id in ["first", "second"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    app.releases.hold_fetches();

    std::thread::scope(|scope| {
        // A Probe found yt-dlp broken first and restores it.
        assert!(app.toolbox.run(Tool::YtDlp, &[], None).is_err());
        scope.spawn(|| app.toolbox.prepare());
        app.releases.wait_until_fetch_held();
        let worker = scope.spawn(|| app.downloader.run());
        // The first Download is dropped once yt-dlp fails to start for it.
        while app.downloader.state().len() != 1 {
            std::thread::yield_now();
        }
        app.releases.release_fetches();
        worker.join().unwrap();
    });

    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(titles(&app.downloader), ["second"]);
    assert!(matches!(
        last_status(&app.downloader),
        Some(DownloadStatus::Done { .. })
    ));
}

#[test]
fn a_youtube_download_runs_with_the_sign_in_browser_and_another_site_without() {
    let app = App::start();
    app.settings
        .set_sign_in_browser(Some(Browser::Firefox))
        .unwrap();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());
    app.download(
        &video(
            "https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/",
            "Rutube",
        ),
        VideoPreset::P1080.into(),
    );

    let browsers: Vec<Option<OsString>> = app
        .processes
        .runs()
        .into_iter()
        .filter(|(program, _)| program.ends_with("yt-dlp.exe"))
        .map(|(_, args)| {
            let at = args
                .iter()
                .position(|arg| arg == "--cookies-from-browser")?;
            Some(args[at + 1].clone())
        })
        .collect();
    assert_eq!(browsers, [Some("firefox".into()), None]);
}

/// A Video with its own id, so that its Downloads are no duplicates of
/// another's.
fn video_with_id(id: &str, title: &str) -> Video {
    Video {
        id: id.into(),
        url: format!("https://www.youtube.com/watch?v={id}"),
        ..video(URL, title)
    }
}

/// The id of the Video yt-dlp is run for, as [`video_with_id`] puts it at
/// the end of its URL.
fn id_of(args: &[OsString]) -> &str {
    let url = args.last().unwrap().to_str().unwrap();
    url.rsplit('=').next().unwrap()
}

/// The ids of the Videos yt-dlp was run for, in order.
fn ids_received(app: &App) -> Vec<String> {
    ytdlp_runs(app)
        .into_iter()
        .map(|args| id_of(&args).to_owned())
        .collect()
}

/// Makes yt-dlp act like it downloads any Video, saving its id as its
/// content.
fn ytdlp_downloads_any(app: &App) {
    ytdlp_fails_for(app, None);
}

/// Makes yt-dlp fail the Video with the id `failing`, and act like it
/// downloads any other.
fn ytdlp_fails_for(app: &App, failing: Option<&'static str>) {
    app.processes.script("yt-dlp.exe", move |args| {
        let id = id_of(args);
        if Some(id) == failing {
            return Output {
                status: exit_status(1),
                stdout: Vec::new(),
                stderr: b"ERROR: Read timed out.".to_vec(),
            };
        }
        let dir = PathBuf::from(after(args, "-P"));
        let name = after(args, "-o").to_str().unwrap().replace("%(id)s", id);
        let file = dir.join(name.replace("%(ext)s", "mp4"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(&file, id).unwrap();
        Output {
            status: exit_status(0),
            stdout: format!("{}\n", serde_json::to_string(&file).unwrap()).into_bytes(),
            stderr: Vec::new(),
        }
    });
}

#[test]
fn queued_downloads_run_one_at_a_time_in_the_order_added() {
    let app = App::start();
    ytdlp_downloads_any(&app);

    for id in ["first", "second", "third"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    let queued = statuses(&app.downloader);
    app.downloader.run();

    assert_eq!(queued, vec![DownloadStatus::Queued; 3]);
    assert_eq!(ids_received(&app), ["first", "second", "third"]);
    let saved: Vec<PathBuf> = app
        .downloader
        .state()
        .into_iter()
        .map(|download| match download.status {
            DownloadStatus::Done { file } => file,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        saved,
        ["first", "second", "third"].map(|title| app.save_dir().join(format!("{title}.mp4")))
    );
}

#[test]
fn a_download_added_while_another_runs_waits_for_it_and_runs_next() {
    let app = App::start();
    let downloader = Arc::downgrade(&app.downloader);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_by_ytdlp = seen.clone();
    app.processes.script("yt-dlp.exe", move |args| {
        let downloader = downloader.upgrade().unwrap();
        let url = args.last().unwrap().to_str().unwrap().to_owned();
        if url.ends_with("first") {
            downloader.add(
                &video_with_id("second", "second"),
                VideoPreset::P1080.into(),
            );
            // A second worker leaves the Queue to the one running.
            downloader.run();
        }
        seen_by_ytdlp.lock().unwrap().push(statuses(&downloader));
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: b"ERROR: stopped".to_vec(),
        }
    });

    app.download(&video_with_id("first", "first"), VideoPreset::P1080.into());

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "yt-dlp ran once for each");
    assert_eq!(
        seen[0],
        [
            DownloadStatus::starting(Receiving::Video),
            DownloadStatus::Queued
        ]
    );
    assert!(matches!(seen[1][0], DownloadStatus::Failed { .. }));
    assert_eq!(seen[1][1], DownloadStatus::starting(Receiving::Video));
}

#[test]
fn the_same_video_with_the_same_preset_is_not_added_while_queued() {
    let app = App::start();
    let zoo = video(URL, "Me at the zoo");
    app.downloader.add(&zoo, VideoPreset::P1080.into());
    let events_before = app.events.events().len();

    let added = app.downloader.add(&zoo, VideoPreset::P1080.into());

    let queued = app.downloader.state()[0].id;
    assert_eq!(added, Added::Duplicate { of: queued });
    assert_eq!(statuses(&app.downloader), [DownloadStatus::Queued]);
    assert_eq!(app.events.events().len(), events_before, "nothing is sent");
}

#[test]
fn the_same_video_with_the_same_preset_is_not_added_while_it_downloads() {
    let app = App::start();
    let downloader = Arc::downgrade(&app.downloader);
    let added = Arc::new(Mutex::new(None));
    let added_by_ytdlp = added.clone();
    app.processes.script("yt-dlp.exe", move |_| {
        let downloader = downloader.upgrade().unwrap();
        *added_by_ytdlp.lock().unwrap() =
            Some(downloader.add(&video(URL, "Me at the zoo"), VideoPreset::P1080.into()));
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: b"ERROR: stopped".to_vec(),
        }
    });

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    let running = app.downloader.state()[0].id;
    assert_eq!(
        *added.lock().unwrap(),
        Some(Added::Duplicate { of: running })
    );
    assert_eq!(app.downloader.state().len(), 1);
}

#[test]
fn the_same_video_is_added_with_another_preset_or_once_done() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);
    let zoo = video(URL, "Me at the zoo");
    app.download(&zoo, VideoPreset::P1080.into());

    let added = [
        app.downloader.add(&zoo, VideoPreset::P1080.into()),
        app.downloader.add(&zoo, VideoPreset::P720.into()),
        app.downloader.add(&zoo, AudioPreset::Mp3.into()),
        app.downloader.add(
            &zoo,
            Content::MediaWithTranscript(AudioPreset::M4a.into(), manual("en")),
        ),
        app.downloader.add(&zoo, Content::Transcript(manual("en"))),
        app.downloader.add(&zoo, Content::Transcript(manual("ru"))),
        // The media with another Transcript, or with none, is still the
        // same Preset.
        app.downloader.add(
            &zoo,
            Content::MediaWithTranscript(VideoPreset::P720.into(), manual("en")),
        ),
        app.downloader.add(&zoo, Content::Transcript(manual("en"))),
    ];

    let ids: Vec<u64> = app.downloader.state().iter().map(|it| it.id).collect();
    assert_eq!(
        added,
        [
            Added::Queued,
            Added::Queued,
            Added::Queued,
            Added::Queued,
            Added::Queued,
            Added::Queued,
            // The 720p, and the English Transcript alone.
            Added::Duplicate { of: ids[2] },
            Added::Duplicate { of: ids[5] },
        ]
    );
}

#[test]
fn a_failed_download_retried_goes_to_the_end_of_the_queue() {
    let app = App::start();
    ytdlp_fails_for(&app, Some("first"));
    for id in ["first", "second"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    app.downloader.run();
    let failed = app.downloader.state()[0].id;
    ytdlp_downloads_any(&app);
    app.downloader
        .add(&video_with_id("third", "third"), VideoPreset::P1080.into());

    let retried = app.downloader.retry(failed);
    let ids: Vec<u64> = app.downloader.state().iter().map(|it| it.id).collect();
    app.downloader.run();

    assert_eq!(retried, Some(Added::Queued));
    assert_eq!(ids[2], failed);
    assert_eq!(ids_received(&app), ["first", "second", "third", "first"]);
    assert!(
        statuses(&app.downloader)
            .iter()
            .all(|status| matches!(status, DownloadStatus::Done { .. }))
    );
}

#[test]
fn a_failed_download_is_not_retried_while_its_duplicate_is_queued() {
    let app = App::start();
    app.processes
        .fail_with("yt-dlp.exe", b"ERROR: Read timed out.\n");
    let zoo = video(URL, "Me at the zoo");
    app.download(&zoo, VideoPreset::P1080.into());
    let failed = app.downloader.state()[0].id;
    app.downloader.add(&zoo, VideoPreset::P1080.into());
    let queued = app.downloader.state()[1].id;

    assert_eq!(
        app.downloader.retry(failed),
        Some(Added::Duplicate { of: queued })
    );
    assert!(matches!(
        statuses(&app.downloader)[..],
        [DownloadStatus::Failed { .. }, DownloadStatus::Queued]
    ));
}

#[test]
fn only_a_failed_or_cancelled_download_is_retried() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());
    app.downloader
        .add(&video(URL, "Me at the zoo"), VideoPreset::P720.into());

    for download in app.downloader.state() {
        assert_eq!(app.downloader.retry(download.id), None);
    }
    assert_eq!(app.downloader.retry(99), None);
}

/// Leaves a Partial in the Partials folder, as a Download of the Video
/// with this id in 1080p that stopped would; returns its file's name.
fn leave_partial(app: &App, id: &str) -> String {
    leave_partial_in(&app.partials_dir(), id)
}

/// Like [`leave_partial`], in `dir`, as yt-dlp would.
fn leave_partial_in(dir: &Path, id: &str) -> String {
    let name = format!("{id}.1080p.f137.mp4.part");
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(&name), b"half").unwrap();
    name
}

#[test]
fn a_queued_download_cancelled_is_never_received_and_its_partial_is_removed() {
    let app = App::start();
    ytdlp_downloads_any(&app);
    leave_partial(&app, "second");
    for id in ["first", "second"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    let second = app.downloader.state()[1].id;

    assert!(app.downloader.cancel(second));
    app.downloader.run();

    assert_eq!(ids_received(&app), ["first"]);
    assert_eq!(statuses(&app.downloader)[1..], [DownloadStatus::Cancelled]);
    assert_eq!(file_names(&app.partials_dir()), Vec::<String>::new());
}

/// Makes yt-dlp leave a Partial of the Video with the id `cancelled` and
/// have its Download cancelled while it receives it, and act like it
/// downloads any other Video, saving its id as its content.
fn ytdlp_cancelled_for(app: &App, cancelled: &'static str) {
    let downloader = Arc::downgrade(&app.downloader);
    app.processes.script("yt-dlp.exe", move |args| {
        let id = id_of(args);
        let dir = PathBuf::from(after(args, "-P"));
        let name = after(args, "-o").to_str().unwrap().replace("%(id)s", id);
        fs::create_dir_all(&dir).unwrap();
        if id == cancelled {
            fs::write(dir.join(name.replace("%(ext)s", "f137.mp4.part")), b"half").unwrap();
            let downloader = downloader.upgrade().unwrap();
            let running = downloader
                .state()
                .into_iter()
                .find(|download| matches!(download.status, DownloadStatus::Downloading { .. }))
                .unwrap();
            assert!(downloader.cancel(running.id));
        }
        let file = dir.join(name.replace("%(ext)s", "mp4"));
        fs::write(&file, id).unwrap();
        Output {
            status: exit_status(0),
            stdout: format!("{}\n", serde_json::to_string(&file).unwrap()).into_bytes(),
            stderr: Vec::new(),
        }
    });
}

#[test]
fn a_download_cancelled_while_downloading_stops_yt_dlp_and_removes_its_partial() {
    let app = App::start();
    ytdlp_cancelled_for(&app, "first");
    for id in ["first", "second"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }

    app.downloader.run();

    assert_eq!(app.processes.kills(), 1);
    assert_eq!(
        statuses(&app.downloader),
        [
            DownloadStatus::Cancelled,
            DownloadStatus::Done {
                file: app.save_dir().join("second.mp4")
            }
        ]
    );
    assert_eq!(file_names(&app.partials_dir()), Vec::<String>::new());
    assert_eq!(file_names(&app.save_dir()), ["second.mp4"]);
}

#[test]
fn a_download_cancelled_as_its_yt_dlp_breaks_stays_cancelled() {
    let app = App::start();
    let downloader = Arc::downgrade(&app.downloader);
    let processes = Arc::downgrade(&app.processes);
    app.processes.script("yt-dlp.exe", move |args| {
        let dir = PathBuf::from(after(args, "-P"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{ID}.1080p.f137.mp4.part")), b"half").unwrap();
        let downloader = downloader.upgrade().unwrap();
        assert!(downloader.cancel(downloader.state()[0].id));
        processes.upgrade().unwrap().break_tool("yt-dlp.exe");
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    });
    let fetched_before = app.releases.fetches().len();

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(app.status(), Some(DownloadStatus::Cancelled));
    assert_eq!(
        app.releases.fetches().len(),
        fetched_before,
        "Setup doesn't run"
    );
    assert_eq!(file_names(&app.partials_dir()), Vec::<String>::new());
}

/// Makes yt-dlp act like it downloads any Video, saving its id as its
/// content; returns the files it finds in the Partials folder each time it
/// starts.
fn ytdlp_finds_partials(app: &App) -> Arc<Mutex<Vec<Vec<String>>>> {
    let found = Arc::new(Mutex::new(Vec::new()));
    let found_by_ytdlp = found.clone();
    app.processes.script("yt-dlp.exe", move |args| {
        let dir = PathBuf::from(after(args, "-P"));
        fs::create_dir_all(&dir).unwrap();
        found_by_ytdlp.lock().unwrap().push(file_names(&dir));
        let id = id_of(args);
        let name = after(args, "-o").to_str().unwrap().replace("%(id)s", id);
        let file = dir.join(name.replace("%(ext)s", "mp4"));
        fs::write(&file, id).unwrap();
        Output {
            status: exit_status(0),
            stdout: format!("{}\n", serde_json::to_string(&file).unwrap()).into_bytes(),
            stderr: Vec::new(),
        }
    });
    found
}

#[test]
fn a_queued_download_cancelled_leaves_the_transcript_the_running_one_receives() {
    let app = App::start();
    let zoo = video(URL, "Me at the zoo");
    app.downloader.add(
        &zoo,
        Content::MediaWithTranscript(VideoPreset::P1080.into(), manual("en")),
    );
    app.downloader.add(&zoo, Content::Transcript(manual("en")));
    let downloader = Arc::downgrade(&app.downloader);
    app.processes.script("yt-dlp.exe", move |args| {
        let downloader = downloader.upgrade().unwrap();
        if args.contains(&"--skip-download".into()) {
            let output = write_subtitles(args, ID, Some(TED_SUBTITLES));
            if let [running, queued] = &downloader.state()[..]
                && queued.status == DownloadStatus::Queued
            {
                assert_eq!(
                    running.status,
                    DownloadStatus::starting(Receiving::Transcript)
                );
                assert!(downloader.cancel(queued.id));
            }
            return output;
        }
        let file = PathBuf::from(after(args, "-P")).join(format!("{ID}.1080p.mp4"));
        fs::write(&file, b"the video").unwrap();
        Output {
            status: exit_status(0),
            stdout: format!("{}\n", serde_json::to_string(&file).unwrap()).into_bytes(),
            stderr: Vec::new(),
        }
    });

    app.downloader.run();

    assert_eq!(
        statuses(&app.downloader),
        [
            DownloadStatus::Done {
                file: app.save_dir().join("Me at the zoo.mp4")
            },
            DownloadStatus::Cancelled
        ]
    );
    assert_eq!(
        file_names(&app.save_dir()),
        ["Me at the zoo.mp4", "Me at the zoo.txt"]
    );
}

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[test]
fn partials_untouched_for_seven_days_are_discarded_and_newer_ones_stay() {
    let app = App::start();
    // At first launch there is no Partials folder yet.
    app.downloader.discard_old_partials();
    let now = SystemTime::UNIX_EPOCH + 100 * DAY;
    app.clock.set(now);
    fs::create_dir_all(app.partials_dir()).unwrap();
    for (name, untouched) in [
        ("month.1080p.f137.mp4.part", 30 * DAY),
        (
            "week.1080p.f137.mp4.part",
            7 * DAY + Duration::from_secs(60),
        ),
        (
            "days.1080p.f137.mp4.part",
            7 * DAY - Duration::from_secs(60),
        ),
        ("today.mp3.webm.part", Duration::ZERO),
    ] {
        let file = fs::File::create(app.partials_dir().join(name)).unwrap();
        file.set_modified(now - untouched).unwrap();
    }

    app.downloader.discard_old_partials();

    assert_eq!(
        file_names(&app.partials_dir()),
        ["days.1080p.f137.mp4.part", "today.mp3.webm.part"]
    );
}

#[test]
fn a_done_failed_or_cancelled_download_isnt_cancelled() {
    let app = App::start();
    ytdlp_fails_for(&app, Some("failed"));
    for id in ["done", "failed", "cancelled"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    let cancelled = app.downloader.state()[2].id;
    app.downloader.cancel(cancelled);
    app.downloader.run();
    leave_partial(&app, "failed");
    let before = app.downloader.state();

    for download in &before {
        assert!(!app.downloader.cancel(download.id));
    }
    assert!(!app.downloader.cancel(99));

    assert_eq!(app.downloader.state(), before);
    assert_eq!(
        file_names(&app.partials_dir()),
        ["failed.1080p.f137.mp4.part"]
    );
}

#[test]
fn a_failed_download_is_retried_from_its_partial() {
    let app = App::start();
    ytdlp_fails_for(&app, Some("first"));
    let partial = leave_partial(&app, "first");
    app.download(&video_with_id("first", "first"), VideoPreset::P1080.into());
    let found = ytdlp_finds_partials(&app);

    app.retry();

    assert_eq!(*found.lock().unwrap(), [[partial]]);
    assert_eq!(
        app.status(),
        Some(DownloadStatus::Done {
            file: app.save_dir().join("first.mp4")
        })
    );
}

#[test]
fn a_cancelled_download_isnt_retried_until_its_yt_dlp_has_exited() {
    let app = App::start();
    let downloader = Arc::downgrade(&app.downloader);
    let retried = Arc::new(Mutex::new(None));
    let retried_meanwhile = retried.clone();
    app.processes.script("yt-dlp.exe", move |_| {
        let downloader = downloader.upgrade().unwrap();
        let id = downloader.state()[0].id;
        assert!(downloader.cancel(id));
        *retried_meanwhile.lock().unwrap() = Some(downloader.retry(id));
        Output {
            status: exit_status(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    });

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(*retried.lock().unwrap(), Some(None));
    assert_eq!(app.status(), Some(DownloadStatus::Cancelled));
    ytdlp_downloads_any(&app);
    app.retry();
    assert!(matches!(app.status(), Some(DownloadStatus::Done { .. })));
}

#[test]
fn a_cancelled_download_is_retried_from_scratch() {
    let app = App::start();
    ytdlp_cancelled_for(&app, "first");
    app.download(&video_with_id("first", "first"), VideoPreset::P1080.into());
    let found = ytdlp_finds_partials(&app);

    app.retry();

    assert_eq!(*found.lock().unwrap(), [Vec::<String>::new()]);
    assert_eq!(
        app.status(),
        Some(DownloadStatus::Done {
            file: app.save_dir().join("first.mp4")
        })
    );
}

#[test]
fn a_download_cancelled_while_its_transcript_downloads_doesnt_receive_the_media() {
    let app = App::start();
    let downloader = Arc::downgrade(&app.downloader);
    app.processes.script("yt-dlp.exe", move |args| {
        let output = write_subtitles(args, ID, Some(TED_SUBTITLES));
        let downloader = downloader.upgrade().unwrap();
        assert!(downloader.cancel(downloader.state()[0].id));
        output
    });

    app.download(
        &video(URL, "Me at the zoo"),
        Content::MediaWithTranscript(VideoPreset::P1080.into(), manual("en")),
    );

    assert_eq!(app.status(), Some(DownloadStatus::Cancelled));
    assert_eq!(ytdlp_runs(&app).len(), 1, "the media isn't downloaded");
    assert_eq!(file_names(&app.partials_dir()), Vec::<String>::new());
    assert!(!app.save_dir().exists());
}

#[test]
fn an_ended_download_is_removed_and_the_others_stay() {
    let app = App::start();
    ytdlp_fails_for(&app, Some("failed"));
    for id in ["done", "failed"] {
        app.download(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    for id in ["cancelled", "queued"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    let ids: Vec<u64> = app.downloader.state().iter().map(|it| it.id).collect();
    assert!(app.downloader.cancel(ids[2]));

    for id in &ids {
        app.downloader.remove(*id);
    }

    assert_eq!(titles(&app.downloader), ["queued"]);
    assert_eq!(app.queues().last().unwrap().len(), 1, "the change is sent");
}

#[test]
fn a_removed_failed_download_keeps_its_partial() {
    let app = App::start();
    app.processes
        .fail_with("yt-dlp.exe", b"ERROR: Read timed out.\n");
    fs::create_dir_all(app.partials_dir()).unwrap();
    fs::write(
        app.partials_dir().join(format!("{ID}.1080p.f137.mp4.part")),
        b"half",
    )
    .unwrap();
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());
    let id = app.downloader.state()[0].id;

    app.downloader.remove(id);

    assert_eq!(titles(&app.downloader), Vec::<String>::new());
    assert_eq!(
        file_names(&app.partials_dir()),
        [format!("{ID}.1080p.f137.mp4.part")]
    );
}

#[test]
fn clearing_done_downloads_leaves_the_others() {
    let app = App::start();
    ytdlp_fails_for(&app, Some("failed"));
    for id in ["done", "failed", "also done"] {
        app.download(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    app.downloader.add(
        &video_with_id("queued", "queued"),
        VideoPreset::P1080.into(),
    );

    app.downloader.clear_done();

    assert_eq!(titles(&app.downloader), ["failed", "queued"]);
    assert_eq!(app.queues().last().unwrap().len(), 2, "the change is sent");
}

#[test]
fn a_removed_download_no_longer_opens_its_file() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());
    let id = app.downloader.state()[0].id;
    assert_eq!(
        app.downloader.saved_file(id),
        Some(app.save_dir().join("Me at the zoo.mp4"))
    );

    app.downloader.remove(id);

    assert_eq!(app.downloader.saved_file(id), None);
    assert!(app.save_dir().join("Me at the zoo.mp4").exists());
}

#[test]
fn closing_asks_while_a_download_is_queued_or_downloading_and_not_once_they_end() {
    let app = App::start();
    assert_eq!(
        app.downloader.on_close(),
        OnClose::Close,
        "the Queue is empty"
    );
    ytdlp_fails_for(&app, Some("failed"));
    for id in ["done", "failed"] {
        app.download(&video_with_id(id, id), VideoPreset::P1080.into());
    }
    app.downloader.add(
        &video_with_id("cancelled", "cancelled"),
        VideoPreset::P1080.into(),
    );
    assert_eq!(app.downloader.on_close(), OnClose::Ask, "one is Queued");
    assert!(app.downloader.cancel(app.downloader.state()[2].id));
    assert_eq!(app.downloader.on_close(), OnClose::Close);

    let downloader = Arc::downgrade(&app.downloader);
    let on_close = Arc::new(Mutex::new(None));
    let on_close_meanwhile = on_close.clone();
    app.processes.script("yt-dlp.exe", move |_| {
        *on_close_meanwhile.lock().unwrap() = Some(downloader.upgrade().unwrap().on_close());
        Output {
            status: exit_status(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    });
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        *on_close.lock().unwrap(),
        Some(OnClose::Ask),
        "one is Downloading"
    );
}

/// Makes yt-dlp leave a Partial of the Video it is run for and go on until
/// it is killed; `started` gets a message once it has left it.
fn ytdlp_runs_until_killed(app: &App) -> mpsc::Receiver<()> {
    let (started_tx, started) = mpsc::channel();
    let processes = Arc::downgrade(&app.processes);
    app.processes.script("yt-dlp.exe", move |args| {
        leave_partial_in(&PathBuf::from(after(args, "-P")), id_of(args));
        started_tx.send(()).unwrap();
        processes.upgrade().unwrap().wait_until_killed();
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    });
    started
}

/// Runs the Queue on a thread of its own, as the app does, and closes the
/// app once yt-dlp has `started`; returns when both are over.
fn close_while_downloading(app: &App, started: &mpsc::Receiver<()>) {
    let downloader = app.downloader.clone();
    let worker = std::thread::spawn(move || downloader.run());
    started.recv().unwrap();
    app.downloader.close();
    assert_eq!(
        app.processes.kills(),
        1,
        "yt-dlp has exited by the time it returns"
    );
    worker.join().unwrap();
}

#[test]
fn closing_the_app_stops_yt_dlp_keeps_its_partial_and_starts_nothing_more() {
    let app = App::start();
    let started = ytdlp_runs_until_killed(&app);
    leave_partial(&app, "second");
    for id in ["first", "second"] {
        app.downloader
            .add(&video_with_id(id, id), VideoPreset::P1080.into());
    }

    close_while_downloading(&app, &started);

    assert_eq!(ids_received(&app), ["first"]);
    assert_eq!(
        app.downloader.on_close(),
        OnClose::Wait,
        "nothing more to ask about"
    );
    let second = app.downloader.state()[1].id;
    assert!(
        !app.downloader.cancel(second),
        "a cancel meanwhile keeps the Partial"
    );
    assert_eq!(
        file_names(&app.partials_dir()),
        ["first.1080p.f137.mp4.part", "second.1080p.f137.mp4.part"]
    );
    assert!(
        !statuses(&app.downloader).iter().any(|status| matches!(
            status,
            DownloadStatus::Cancelled | DownloadStatus::Failed { .. }
        )),
        "closing is neither cancelling nor failing"
    );
}

#[test]
fn a_close_request_while_the_app_is_closing_waits_for_the_queue_to_stop() {
    let app = App::start();
    let (started_tx, started) = mpsc::channel();
    let downloader = Arc::downgrade(&app.downloader);
    let processes = Arc::downgrade(&app.processes);
    let on_close = Arc::new(Mutex::new(None));
    let on_close_meanwhile = on_close.clone();
    app.processes.script("yt-dlp.exe", move |_| {
        started_tx.send(()).unwrap();
        processes.upgrade().unwrap().wait_until_killed();
        *on_close_meanwhile.lock().unwrap() = Some(downloader.upgrade().unwrap().on_close());
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    });
    app.downloader
        .add(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    close_while_downloading(&app, &started);

    assert_eq!(
        *on_close.lock().unwrap(),
        Some(OnClose::Wait),
        "the window stays until yt-dlp has exited"
    );
    assert_eq!(app.downloader.on_close(), OnClose::Wait, "and after");
}

#[test]
fn a_download_stopped_by_closing_the_app_resumes_from_its_partial_after_relaunch() {
    let app = App::start();
    let started = ytdlp_runs_until_killed(&app);
    let first = video_with_id("first", "first");
    app.downloader.add(&first, VideoPreset::P1080.into());
    close_while_downloading(&app, &started);

    let app = app.relaunch();
    app.downloader.discard_old_partials();
    let found = ytdlp_finds_partials(&app);
    app.download(&first, VideoPreset::P1080.into());

    assert_eq!(*found.lock().unwrap(), [["first.1080p.f137.mp4.part"]]);
    assert_eq!(
        app.status(),
        Some(DownloadStatus::Done {
            file: app.save_dir().join("first.mp4")
        })
    );
}

#[test]
fn a_download_asks_yt_dlp_for_this_one_video_merged_into_an_mp4() {
    let app = App::start();
    let url = "https://www.youtube.com/watch?v=jNQXAC9IVRw&list=PL123";
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.download(&video(url, "Me at the zoo"), VideoPreset::P1080.into());

    let (_, args) = app.processes.runs().pop().unwrap();
    assert!(args.contains(&"--no-playlist".into()), "{args:?}");
    assert_eq!(after(&args, "--merge-output-format"), "mp4");
    // The URL comes after `--`, so yt-dlp never reads it as an option.
    assert_eq!(args[args.len() - 2..], [OsString::from("--"), url.into()]);
}

#[test]
fn a_download_dates_its_files_when_received_so_its_partial_isnt_taken_for_old() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    let (_, args) = app.processes.runs().pop().unwrap();
    // Otherwise yt-dlp may date them as the site last changed the Video.
    assert!(args.contains(&"--no-mtime".into()), "{args:?}");
}

#[test]
fn a_video_preset_limits_the_resolution_and_prefers_h264_and_aac_at_equal_resolution() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);

    for preset in [VideoPreset::Best, VideoPreset::P1440, VideoPreset::P480] {
        app.download(&video(URL, "Me at the zoo"), preset.into());
    }

    let sorts: Vec<OsString> = ytdlp_runs(&app)
        .iter()
        .map(|args| after(args, "-S").clone())
        .collect();
    assert_eq!(
        sorts,
        [
            "res,vcodec:h264,acodec:aac",
            "res:1440,vcodec:h264,acodec:aac",
            "res:480,vcodec:h264,acodec:aac"
        ]
    );
}

#[test]
fn a_download_takes_separate_video_and_audio_over_a_format_with_both() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P720.into());

    // Preferring AAC would otherwise pick a Format with both, which YouTube
    // sends slowly over HLS; a site with only such Formats falls back to one.
    let (_, args) = app.processes.runs().pop().unwrap();
    assert_eq!(after(&args, "-f"), "bv+ba/b");
}

#[test]
fn each_video_preset_of_a_video_keeps_a_partial_of_its_own() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.download(&video(URL, "Me at the zoo"), VideoPreset::Best.into());
    app.download(&video(URL, "Me at the zoo"), VideoPreset::P720.into());

    let names: Vec<OsString> = ytdlp_runs(&app)
        .iter()
        .map(|args| after(args, "-o").clone())
        .collect();
    assert_eq!(names, ["%(id)s.best.%(ext)s", "%(id)s.720p.%(ext)s"]);
}

#[test]
fn m4a_keeps_the_sites_aac_sound_and_encodes_other_sound_at_192k() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the sound", &[]);

    app.download(&video(URL, "Me at the zoo"), AudioPreset::M4a.into());

    // ffmpeg copies AAC sound into an M4A, where yt-dlp ignores the
    // quality; only a site without AAC gets its sound encoded.
    let (_, args) = app.processes.runs().pop().unwrap();
    assert_eq!(after(&args, "-f"), "ba/b");
    assert_eq!(after(&args, "-S"), "acodec:aac");
    assert!(args.contains(&"-x".into()), "{args:?}");
    assert_eq!(after(&args, "--audio-format"), "m4a");
    assert_eq!(after(&args, "--audio-quality"), "192K");
    assert!(!args.contains(&"--merge-output-format".into()), "{args:?}");
}

#[test]
fn mp3_encodes_the_best_sound_as_high_quality_vbr() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the sound", &[]);

    app.download(&video(URL, "Me at the zoo"), AudioPreset::Mp3.into());

    // LAME's VBR quality 1 makes about 190–250 kbit/s.
    let (_, args) = app.processes.runs().pop().unwrap();
    assert_eq!(after(&args, "-f"), "ba/b");
    assert!(!args.contains(&"-S".into()), "{args:?}");
    assert!(args.contains(&"-x".into()), "{args:?}");
    assert_eq!(after(&args, "--audio-format"), "mp3");
    assert_eq!(after(&args, "--audio-quality"), "1");
    assert!(!args.contains(&"--merge-output-format".into()), "{args:?}");
}

#[test]
fn each_audio_preset_of_a_video_keeps_a_partial_of_its_own() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the sound", &[]);

    app.download(&video(URL, "Me at the zoo"), AudioPreset::M4a.into());
    app.download(&video(URL, "Me at the zoo"), AudioPreset::Mp3.into());

    let names: Vec<OsString> = ytdlp_runs(&app)
        .iter()
        .map(|args| after(args, "-o").clone())
        .collect();
    assert_eq!(names, ["%(id)s.m4a.%(ext)s", "%(id)s.mp3.%(ext)s"]);
}

#[test]
fn a_failed_audio_download_is_retried_with_its_audio_preset() {
    let app = App::start();
    app.processes
        .fail_with("yt-dlp.exe", b"ERROR: Read timed out.\n");
    app.download(&video(URL, "Me at the zoo"), AudioPreset::Mp3.into());
    app.ytdlp_downloads(ID, b"the sound", &[]);

    app.retry();

    assert_eq!(
        app.status(),
        Some(DownloadStatus::Done {
            file: app.save_dir().join("Me at the zoo.mp3")
        })
    );
    let runs = ytdlp_runs(&app);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0], runs[1]);
}

#[test]
fn a_format_in_fragments_reports_the_share_of_fragments_received() {
    let app = App::start();
    let fragments = progress_line(
        "hls-1080",
        serde_json::json!({
            "status": "downloading",
            "downloaded_bytes": 52_000,
            "total_bytes_estimate": null,
            "fragment_index": 3,
            "fragment_count": 12,
            "speed": null,
            "eta": null,
        }),
    );
    app.ytdlp_downloads(ID, b"the video", &[fragments]);

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(
        app.download_statuses()[2],
        DownloadStatus::Downloading {
            receiving: Receiving::Video,
            percent: Some(25.0),
            speed: None,
            eta: None
        }
    );
}

/// A progress line of a Format in fragments, whose size yt-dlp only
/// estimates from the fragments received so far.
fn fragment_progress(
    format: &str,
    downloaded: u64,
    estimate: u64,
    index: u64,
    count: u64,
) -> String {
    progress_line(
        format,
        serde_json::json!({
            "status": "downloading",
            "downloaded_bytes": downloaded,
            "total_bytes_estimate": estimate,
            "fragment_index": index,
            "fragment_count": count,
            "speed": 160_967.0,
            "eta": 1426,
        }),
    )
}

/// The percents a Download showed while downloading, in order.
fn percents(app: &App) -> Vec<Option<f64>> {
    app.download_statuses()
        .into_iter()
        .filter_map(|status| match status {
            DownloadStatus::Downloading { percent, .. } => Some(percent),
            _ => None,
        })
        .collect()
}

#[test]
fn a_format_in_fragments_shows_the_share_of_fragments_rather_than_its_size_estimate() {
    let app = App::start();
    // As YouTube's HLS sends them: the first chunk estimates the whole
    // Format at its own size, and later estimates stay far too small.
    app.ytdlp_downloads(
        ID,
        b"the video",
        &[
            fragment_progress("95", 1024, 1024, 0, 1183),
            fragment_progress("95", 4_059_672, 224_815_968, 21, 1183),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P720.into());

    assert_eq!(
        percents(&app),
        [None, Some(0.0), Some(21.0 / 1183.0 * 100.0)]
    );
}

#[test]
fn a_format_in_fragments_among_separate_formats_counts_by_its_fragments() {
    let app = App::start();
    app.ytdlp_downloads_formats(
        ID,
        b"the video",
        formats(&[("398", Some(1_000_000)), (AUDIO_ONLY, Some(100_000))]),
        &[
            fragment_progress("398", 1024, 1024, 0, 100),
            fragment_progress("398", 300_000, 310_000, 50, 100),
        ],
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P720.into());

    // Half of the video's fragments are half its told size.
    assert_eq!(
        percents(&app),
        [None, Some(0.0), Some(500_000.0 / 1_100_000.0 * 100.0)]
    );
}

/// Real subtitles of a TED talk from ted.com, cut down; see
/// `fixtures/README.md`.
const TED_SUBTITLES: &[u8] = include_bytes!("fixtures/ted-talk.en.vtt");

/// The text of [`TED_SUBTITLES`].
const TED_TEXT: &str = "\
When I was first learning to meditate,
the instruction was to simply
pay attention to my breath,
and when my mind wandered,
to bring it back.
Sounded simple enough.
Yet I'd sit on these silent retreats,
sweating through T-shirts
in the middle of winter.
I'd take naps every chance I got
because it was really hard work.
Actually, it was exhausting.
The instruction was simple enough
but I was missing something
really important.
So why is it so hard to pay attention?
and repeat.
Thank you.
(Applause)
";

/// YouTube's captions of what is spoken, which repeat each line in the next
/// caption; see `fixtures/README.md`.
const YOUTUBE_CAPTIONS: &[u8] = include_bytes!("fixtures/youtube-automatic.en-orig.vtt");

fn manual(language: &str) -> TranscriptLanguage {
    TranscriptLanguage {
        language: language.into(),
        automatic: false,
    }
}

fn automatic(language: &str) -> TranscriptLanguage {
    TranscriptLanguage {
        language: language.into(),
        automatic: true,
    }
}

#[test]
fn a_transcript_alone_is_saved_as_text_under_the_title_without_the_media() {
    let app = App::start();
    app.ytdlp_downloads_with_subtitles(ID, b"the video", TED_SUBTITLES);

    app.download(
        &video(URL, "Me at the zoo"),
        Content::Transcript(manual("en")),
    );

    let saved = app.save_dir().join("Me at the zoo.txt");
    assert_eq!(
        app.download_statuses(),
        [
            DownloadStatus::Queued,
            DownloadStatus::starting(Receiving::Transcript),
            DownloadStatus::Done {
                file: saved.clone()
            },
        ]
    );
    assert_eq!(file_names(&app.save_dir()), ["Me at the zoo.txt"]);
    assert_eq!(fs::read_to_string(&saved).unwrap(), TED_TEXT);
    assert_eq!(file_names(&app.partials_dir()), [] as [String; 0]);
    let runs = ytdlp_runs(&app);
    assert_eq!(runs.len(), 1);
    assert!(runs[0].contains(&"--skip-download".into()), "{runs:?}");
}

#[test]
fn a_transcript_asks_for_the_subtitles_or_for_the_captions_of_what_is_spoken() {
    let app = App::start();
    app.ytdlp_downloads_with_subtitles(ID, b"the video", TED_SUBTITLES);
    // As YouTube lists them: the captions of what is spoken, and their
    // machine translations.
    let video = Video {
        transcripts: TranscriptLanguages {
            manual: vec!["en".into()],
            automatic: vec!["de".into(), "en".into(), "en-orig".into()],
        },
        ..video(URL, "Me at the zoo")
    };

    for language in [manual("en"), automatic("en")] {
        app.download(&video, Content::Transcript(language));
    }

    let runs = ytdlp_runs(&app);
    let asked: Vec<(bool, bool, &OsString)> = runs
        .iter()
        .map(|args| {
            (
                args.contains(&"--write-subs".into()),
                args.contains(&"--write-auto-subs".into()),
                after(args, "--sub-langs"),
            )
        })
        .collect();
    // YouTube's captions in `en` may be a machine translation of the
    // captions in another language; `en-orig` never is.
    assert_eq!(
        asked,
        [
            (true, false, &"en".into()),
            (false, true, &"en-orig".into())
        ]
    );
}

#[test]
fn a_transcript_of_captions_has_neither_their_timing_nor_the_lines_they_repeat() {
    let app = App::start();
    app.ytdlp_downloads_with_subtitles(ID, b"the video", YOUTUBE_CAPTIONS);

    app.download(
        &video(URL, "Me at the zoo"),
        Content::Transcript(automatic("en")),
    );

    // YouTube writes a word it bleeps out as `[&nbsp;__&nbsp;]`.
    assert_eq!(
        fs::read_to_string(app.save_dir().join("Me at the zoo.txt")).unwrap(),
        "\
when I was first learning to meditate
the instruction was to Simply pay
attention to my breath and when my mind
just by being curiously aware when she
smoked was that smoking tastes like
[ __ ] now she moved from knowledge to to
wisdom she moved from knowing in her
curious feel the joy of Letting Go and
repeat thank you
"
    );
}

#[test]
fn a_transcript_with_the_media_is_saved_next_to_it_under_the_same_name() {
    let app = App::start();
    app.ytdlp_downloads_with_subtitles(ID, b"the video", TED_SUBTITLES);

    app.download(
        &video(URL, "Me at the zoo"),
        Content::MediaWithTranscript(VideoPreset::P1080.into(), manual("en")),
    );

    let saved = app.save_dir().join("Me at the zoo.mp4");
    // The Transcript comes first, then the media before yt-dlp tells its
    // progress.
    assert_eq!(
        app.download_statuses(),
        [
            DownloadStatus::Queued,
            DownloadStatus::starting(Receiving::Transcript),
            DownloadStatus::starting(Receiving::Video),
            DownloadStatus::Processing,
            DownloadStatus::Done {
                file: saved.clone()
            },
        ]
    );
    assert_eq!(
        file_names(&app.save_dir()),
        ["Me at the zoo.mp4", "Me at the zoo.txt"]
    );
    assert_eq!(fs::read(&saved).unwrap(), b"the video");
    assert_eq!(
        fs::read_to_string(app.save_dir().join("Me at the zoo.txt")).unwrap(),
        TED_TEXT
    );
    assert_eq!(file_names(&app.partials_dir()), [] as [String; 0]);
}

#[test]
fn the_media_and_its_transcript_take_the_first_number_free_for_both() {
    let app = App::start();
    fs::create_dir_all(app.save_dir()).unwrap();
    fs::write(
        app.save_dir().join("Me at the zoo.txt"),
        b"the user's notes",
    )
    .unwrap();
    app.ytdlp_downloads_with_subtitles(ID, b"the video", TED_SUBTITLES);

    app.download(
        &video(URL, "Me at the zoo"),
        Content::MediaWithTranscript(AudioPreset::Mp3.into(), manual("en")),
    );

    assert_eq!(
        file_names(&app.save_dir()),
        [
            "Me at the zoo (1).mp3",
            "Me at the zoo (1).txt",
            "Me at the zoo.txt"
        ]
    );
    assert_eq!(
        fs::read(app.save_dir().join("Me at the zoo.txt")).unwrap(),
        b"the user's notes"
    );
}

#[test]
fn a_transcript_yt_dlp_doesnt_save_fails_the_download_before_the_media() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.download(
        &video(URL, "Me at the zoo"),
        Content::MediaWithTranscript(VideoPreset::P1080.into(), manual("en")),
    );

    assert!(
        matches!(app.status(), Some(DownloadStatus::Failed { .. })),
        "{:?}",
        app.status()
    );
    assert_eq!(ytdlp_runs(&app).len(), 1, "the media isn't downloaded");
    assert!(!app.save_dir().exists());
}

#[test]
fn a_failed_transcript_is_retried_with_the_media_and_the_transcript() {
    let app = App::start();
    app.processes
        .fail_with("yt-dlp.exe", b"ERROR: Unable to download video subtitles\n");
    app.download(
        &video(URL, "Me at the zoo"),
        Content::MediaWithTranscript(VideoPreset::P1080.into(), manual("en")),
    );
    assert_eq!(
        app.status(),
        Some(DownloadStatus::Failed {
            failure: Failure::Unknown,
            message: "ERROR: Unable to download video subtitles".into(),
            output: vec!["ERROR: Unable to download video subtitles".into()],
            ytdlp_update: None,
        })
    );
    app.ytdlp_downloads_with_subtitles(ID, b"the video", TED_SUBTITLES);

    app.retry();

    assert_eq!(
        file_names(&app.save_dir()),
        ["Me at the zoo.mp4", "Me at the zoo.txt"]
    );
}

#[test]
fn a_broken_yt_dlp_is_restored_and_the_transcript_is_dropped() {
    let app = App::start();
    app.processes.break_tool("yt-dlp.exe");

    app.download(
        &video(URL, "Me at the zoo"),
        Content::Transcript(manual("en")),
    );

    assert_eq!(app.downloader.state(), []);
    assert_eq!(app.toolbox.state(), SetupState::Ready);
}

/// Probes and downloads a short public Video with the real tools:
/// `cargo test --lib -- --ignored downloads_a_real_video`. YouTube may ask
/// to prove the user isn't a bot; then name a browser signed in to YouTube
/// in `SMOKE_SIGN_IN_BROWSER`, such as `firefox`.
#[test]
#[ignore = "downloads the real tools and a real Video"]
fn downloads_a_real_video_with_the_real_yt_dlp() {
    let dir = TempDir::new().unwrap();
    let events = Arc::new(RecordingSink::default());
    let (video, _, downloader) = probe_with_real_tools(dir.path(), URL, events.clone());

    download(&downloader, &video, VideoPreset::P1080.into());

    let saved = dir.path().join("Downloads").join("Me at the zoo.mp4");
    assert_eq!(
        last_status(&downloader),
        Some(DownloadStatus::Done {
            file: saved.clone()
        })
    );
    assert!(fs::metadata(&saved).unwrap().len() > 100_000);
    assert_eq!(file_names(&dir.path().join("partials")), [] as [String; 0]);
    let statuses: Vec<DownloadStatus> = queues(&events)
        .into_iter()
        .filter_map(|queue| Some(queue.last()?.status.clone()))
        .collect();
    // "Me at the zoo" comes as separate video and audio; one progress
    // covers both, so it never goes back.
    let percents: Vec<f64> = statuses
        .iter()
        .filter_map(|status| match status {
            DownloadStatus::Downloading { percent, .. } => *percent,
            _ => None,
        })
        .collect();
    assert!(
        percents.windows(2).all(|pair| pair[0] <= pair[1]),
        "{percents:?}"
    );
    assert_eq!(percents.last(), Some(&100.0), "{percents:?}");
    // yt-dlp tells the codec of each, so the video turns to the audio.
    let receiving: Vec<Receiving> = statuses
        .iter()
        .filter_map(|status| match status {
            DownloadStatus::Downloading { receiving, .. } => Some(*receiving),
            _ => None,
        })
        .collect();
    assert!(
        receiving.contains(&Receiving::Video) && receiving.last() == Some(&Receiving::Audio),
        "{receiving:?}"
    );
    assert!(statuses.contains(&DownloadStatus::Processing));
}

/// Sets up the real tools in `dir`, Probes `url` with them and returns the
/// Video, the tools and a `Downloader` that saves to `dir`'s `Downloads`.
/// YouTube may ask to prove the user isn't a bot; then name a browser
/// signed in to YouTube in `SMOKE_SIGN_IN_BROWSER`, such as `firefox`.
fn probe_with_real_tools(
    dir: &Path,
    url: &str,
    events: Arc<RecordingSink>,
) -> (Video, Arc<Toolbox>, Downloader) {
    use crate::clock::SystemClock;
    use crate::probe::Prober;
    use crate::processes::SystemProcesses;
    use crate::releases::github::GithubReleases;

    let settings = Arc::new(Settings::load(dir.join("settings.json"), events.clone()));
    if let Ok(browser) = std::env::var("SMOKE_SIGN_IN_BROWSER") {
        let browser: Browser = serde_json::from_value(browser.into()).unwrap();
        settings.set_sign_in_browser(Some(browser)).unwrap();
    }
    let toolbox = Arc::new(Toolbox::new(
        dir.join("tools"),
        Arc::new(GithubReleases::new()),
        Arc::new(SystemProcesses),
        Arc::new(SystemClock),
        events.clone(),
    ));
    toolbox.prepare();
    assert_eq!(toolbox.state(), SetupState::Ready);
    let prober = Prober::new(toolbox.clone(), settings.clone(), events.clone());
    prober.probe(url);
    let video = prober
        .video()
        .unwrap_or_else(|| panic!("{:?}", prober.state()));
    let downloader = Downloader::new(
        toolbox.clone(),
        settings,
        dir.join("partials"),
        dir.join("Downloads"),
        dir.join("logs"),
        Arc::new(SystemClock),
        events,
    );
    (video, toolbox, downloader)
}

/// A 12-second public Video with 1080p in H.264 and, above it, only 1440p
/// in VP9, as YouTube offered it on 2026-10-25.
const PRESET_URL: &str = "https://www.youtube.com/watch?v=QiucFPLFMVc";

/// Downloads a Video that has 1080p in H.264 and, above it, only 1440p in
/// VP9 or AV1, with 1440p and with 1080p:
/// `cargo test --lib -- --ignored video_presets_pick`. Another such Video
/// can be named in `SMOKE_PRESET_URL`. See [`probe_with_real_tools`] for
/// `SMOKE_SIGN_IN_BROWSER`.
#[test]
#[ignore = "downloads the real tools and a real Video"]
fn video_presets_pick_the_resolution_then_h264_and_aac_with_the_real_yt_dlp() {
    let url = std::env::var("SMOKE_PRESET_URL").unwrap_or_else(|_| PRESET_URL.into());
    let dir = TempDir::new().unwrap();
    let events = Arc::new(RecordingSink::default());
    let (video, toolbox, downloader) = probe_with_real_tools(dir.path(), &url, events);
    assert_eq!(
        video.video_presets()[..2],
        [VideoPreset::P1440, VideoPreset::P1080],
        "the Video's highest Preset is 1440p"
    );

    for (preset, resolution, video_codec) in [
        (VideoPreset::P1440, 1440, None),
        (VideoPreset::P1080, 1080, Some("h264")),
    ] {
        download(&downloader, &video, preset.into());

        let Some(DownloadStatus::Done { file }) = last_status(&downloader) else {
            panic!("{preset:?}: {:?}", downloader.state());
        };
        assert_eq!(file.extension().unwrap(), "mp4", "{preset:?}");
        let tracks = tracks(&toolbox, &file);
        assert_eq!(tracks.resolution, resolution, "{preset:?}");
        match video_codec {
            Some(codec) => assert_eq!(tracks.video_codec, codec, "{preset:?}"),
            // Above 1080p YouTube has no H.264.
            None => assert_ne!(tracks.video_codec, "h264", "{preset:?}"),
        }
        assert_eq!(tracks.audio_codec, "aac", "{preset:?}");
    }
}

/// Downloads "Me at the zoo" with each Audio preset:
/// `cargo test --lib -- --ignored audio_presets_save`. See
/// [`probe_with_real_tools`] for `SMOKE_SIGN_IN_BROWSER`.
#[test]
#[ignore = "downloads the real tools and a real Video"]
fn audio_presets_save_the_sound_alone_with_the_real_yt_dlp() {
    let dir = TempDir::new().unwrap();
    let events = Arc::new(RecordingSink::default());
    let (video, toolbox, downloader) = probe_with_real_tools(dir.path(), URL, events.clone());
    assert_eq!(video.audio_presets(), [AudioPreset::M4a, AudioPreset::Mp3]);

    for (preset, name, codec) in [
        (AudioPreset::M4a, "Me at the zoo.m4a", "aac"),
        (AudioPreset::Mp3, "Me at the zoo.mp3", "mp3"),
    ] {
        download(&downloader, &video, preset.into());

        let saved = dir.path().join("Downloads").join(name);
        assert_eq!(
            last_status(&downloader),
            Some(DownloadStatus::Done {
                file: saved.clone()
            }),
            "{preset:?}"
        );
        let tracks = track_entries(&toolbox, &saved);
        let kinds: Vec<&serde_json::Value> =
            tracks.iter().map(|track| &track["codec_type"]).collect();
        assert_eq!(kinds, ["audio"], "{preset:?}");
        assert_eq!(tracks[0]["codec_name"], codec, "{preset:?}");
    }
    assert_eq!(file_names(&dir.path().join("partials")), [] as [String; 0]);
    let processed = queues(&events)
        .iter()
        .flatten()
        .filter(|download| download.status == DownloadStatus::Processing)
        .count();
    assert_eq!(processed, 2);
}

/// A TED talk with subtitles in English and YouTube's captions of it, as
/// in `probe/fixtures/youtube-video.json`.
const TRANSCRIPT_URL: &str = "https://www.youtube.com/watch?v=-moW9jvvMr4";

/// Saves the Transcript alone of a Video with subtitles and captions, from
/// each: `cargo test --lib -- --ignored transcripts_save`. See
/// [`probe_with_real_tools`] for `SMOKE_SIGN_IN_BROWSER`.
#[test]
#[ignore = "downloads the real tools and a real Transcript"]
fn transcripts_save_text_alone_with_the_real_yt_dlp() {
    let dir = TempDir::new().unwrap();
    let events = Arc::new(RecordingSink::default());
    let (video, _, downloader) = probe_with_real_tools(dir.path(), TRANSCRIPT_URL, events);
    let languages = video.transcript_languages();
    assert!(languages.contains(&manual("en")), "{languages:?}");

    // The captions aren't offered beside the subtitles in English, but
    // they are what repeats its lines.
    for language in [manual("en"), automatic("en")] {
        download(&downloader, &video, Content::Transcript(language.clone()));

        let Some(DownloadStatus::Done { file }) = last_status(&downloader) else {
            panic!("{language:?}: {:?}", downloader.state());
        };
        assert_eq!(file.extension().unwrap(), "txt", "{language:?}");
        let text = fs::read_to_string(&file).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 50, "{language:?}: {text}");
        assert!(
            !text.contains("-->") && !text.contains('<'),
            "{language:?}: {text}"
        );
        assert!(
            lines.windows(2).all(|pair| pair[0] != pair[1]),
            "{language:?}: {text}"
        );
        fs::remove_file(&file).unwrap();
    }
    assert_eq!(file_names(&dir.path().join("partials")), [] as [String; 0]);
}

/// What ffprobe tells of a saved file's video and audio tracks.
struct Tracks {
    video_codec: String,
    /// The smaller side of the picture.
    resolution: u64,
    audio_codec: String,
}

fn tracks(toolbox: &Toolbox, file: &Path) -> Tracks {
    let entries = track_entries(toolbox, file);
    let of_type = |kind: &str| {
        entries
            .iter()
            .find(|track| track["codec_type"] == kind)
            .unwrap_or_else(|| panic!("no {kind} in {entries:?}"))
    };
    let (video, audio) = (of_type("video"), of_type("audio"));
    Tracks {
        video_codec: video["codec_name"].as_str().unwrap().into(),
        resolution: video["width"]
            .as_u64()
            .unwrap()
            .min(video["height"].as_u64().unwrap()),
        audio_codec: audio["codec_name"].as_str().unwrap().into(),
    }
}

/// The tracks of a saved file as ffprobe tells them, each with its
/// `codec_type`, `codec_name`, `width` and `height`.
fn track_entries(toolbox: &Toolbox, file: &Path) -> Vec<serde_json::Value> {
    let args: Vec<OsString> = [
        "-v",
        "error",
        "-show_entries",
        "stream=codec_type,codec_name,width,height",
        "-of",
        "json",
    ]
    .into_iter()
    .map(Into::into)
    .chain([file.as_os_str().to_owned()])
    .collect();
    let output = toolbox.run(Tool::Ffprobe, &args, None).unwrap();
    let answer: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    answer["streams"].as_array().unwrap().clone()
}

const EXTRACTOR_BROKEN: &[u8] = b"ERROR: [youtube] jNQXAC9IVRw: Unable to extract initial player response; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= , filling out the appropriate issue template. Confirm you are on the latest version using  yt-dlp -U\n";

/// What became of the update of yt-dlp the Download added last started, as
/// each event sent it.
fn ytdlp_updates(app: &App) -> Vec<Option<YtDlpUpdate>> {
    app.download_statuses()
        .into_iter()
        .filter_map(|status| match status {
            DownloadStatus::Failed { ytdlp_update, .. } => Some(ytdlp_update),
            _ => None,
        })
        .collect()
}

fn installed_ytdlp(app: &App) -> Vec<u8> {
    fs::read(app.dir.path().join("tools").join("yt-dlp.exe")).unwrap()
}

#[test]
fn a_download_failing_with_a_broken_extractor_updates_yt_dlp_and_stays_failed() {
    let app = App::start();
    app.releases.publish_newer_ytdlp();
    app.processes.fail_with("yt-dlp.exe", EXTRACTOR_BROKEN);

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(installed_ytdlp(&app), NEWER_YTDLP_BYTES);
    assert_eq!(
        ytdlp_updates(&app),
        [Some(YtDlpUpdate::Updating), Some(YtDlpUpdate::Updated)]
    );
    assert!(matches!(
        app.status(),
        Some(DownloadStatus::Failed {
            failure: Failure::ExtractorBroken,
            ..
        })
    ));
    // The user retries it.
    assert_eq!(ytdlp_runs(&app).len(), 1);
}

#[test]
fn a_broken_extractor_tells_when_yt_dlp_couldnt_be_updated() {
    for (offline, expected) in [(false, YtDlpUpdate::UpToDate), (true, YtDlpUpdate::Failed)] {
        let app = App::start();
        app.releases.set_online(!offline);
        app.processes.fail_with("yt-dlp.exe", EXTRACTOR_BROKEN);

        app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

        assert_eq!(installed_ytdlp(&app), YTDLP_BYTES);
        assert_eq!(ytdlp_updates(&app).last(), Some(&Some(expected)));
    }
}

#[test]
fn a_download_failing_otherwise_doesnt_update_yt_dlp() {
    let app = App::start();
    app.releases.publish_newer_ytdlp();
    let fetched_before = app.releases.fetches().len();
    app.processes.fail_with(
        "yt-dlp.exe",
        b"ERROR: unable to download video data: HTTP Error 403: Forbidden\n",
    );

    app.download(&video(URL, "Me at the zoo"), VideoPreset::P1080.into());

    assert_eq!(app.releases.fetches().len(), fetched_before);
    assert_eq!(ytdlp_updates(&app), [None]);
}

/// Waits until `done` holds, failing the test after a few seconds.
fn wait_until(done: impl Fn() -> bool) {
    let start = std::time::Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(5), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn an_update_asked_for_during_a_download_waits_for_the_whole_download_to_end() {
    let app = App::start();
    app.releases.publish_newer_ytdlp();
    let toolbox = app.toolbox.clone();
    let ytdlp = app.dir.path().join("tools").join("yt-dlp.exe");
    let media_run_used = Arc::new(Mutex::new(Vec::new()));
    let used = media_run_used.clone();
    app.processes.script("yt-dlp.exe", move |args| {
        let dir = PathBuf::from(after(args, "-P"));
        if args.contains(&"--skip-download".into()) {
            // The user asks for an update while the Transcript is received.
            let updating = toolbox.clone();
            std::thread::spawn(move || updating.update_ytdlp_on_request());
            wait_until(|| toolbox.ytdlp_update_state() == YtDlpUpdateState::Waiting);
            return write_subtitles(args, ID, Some(b"WEBVTT\n\n00:00.000 --> 00:01.000\nhi\n"));
        }
        *used.lock().unwrap() = fs::read(&ytdlp).unwrap();
        let name = after(args, "-o")
            .to_str()
            .unwrap()
            .replace("%(id)s", ID)
            .replace("%(ext)s", "mp4");
        fs::write(dir.join(&name), b"the video").unwrap();
        Output {
            status: exit_status(0),
            stdout: (serde_json::to_string(&dir.join(name)).unwrap() + "\n").into_bytes(),
            stderr: Vec::new(),
        }
    });

    app.download(
        &video(URL, "Me at the zoo"),
        Content::MediaWithTranscript(VideoPreset::P1080.into(), manual("en")),
    );
    wait_until(|| {
        matches!(
            app.toolbox.ytdlp_update_state(),
            YtDlpUpdateState::Updated { .. }
        )
    });

    assert!(matches!(app.status(), Some(DownloadStatus::Done { .. })));
    assert_eq!(*media_run_used.lock().unwrap(), YTDLP_BYTES);
    assert_eq!(installed_ytdlp(&app), NEWER_YTDLP_BYTES);
}
