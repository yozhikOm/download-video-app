use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use tempfile::TempDir;

use super::*;
use crate::clock::FixedClock;
use crate::events::{AppEvent, RecordingSink};
use crate::probe::{TranscriptLanguages, Video};
use crate::processes::fake::{FakeProcesses, exit_status};
use crate::releases::fake::FakeReleases;
use crate::settings::{Browser, Settings};
use crate::toolbox::SetupState;

const URL: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
const ID: &str = "jNQXAC9IVRw";

struct App {
    dir: TempDir,
    releases: Arc<FakeReleases>,
    toolbox: Arc<Toolbox>,
    processes: Arc<FakeProcesses>,
    settings: Arc<Settings>,
    events: Arc<RecordingSink>,
    downloader: Arc<Downloader>,
}

impl App {
    /// Starts the app with its tools already set up.
    fn start() -> Self {
        let dir = TempDir::new().unwrap();
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
        let downloader = Arc::new(Downloader::new(
            toolbox.clone(),
            settings.clone(),
            dir.path().join("partials"),
            dir.path().join("Downloads"),
            events.clone(),
        ));
        Self {
            dir,
            releases,
            toolbox,
            processes,
            settings,
            events,
            downloader,
        }
    }

    /// Makes yt-dlp act like it downloads the Video with this id: it prints
    /// `progress` lines, writes `content` where it is told and prints what
    /// it is asked to print.
    fn ytdlp_downloads(&self, id: &str, content: &[u8], progress: &[String]) {
        let (id, content, progress) = (id.to_owned(), content.to_owned(), progress.to_owned());
        self.processes.script("yt-dlp.exe", move |args| {
            let dir = PathBuf::from(after(args, "-P"));
            let name = after(args, "-o")
                .to_str()
                .unwrap()
                .replace("%(id)s", &id)
                .replace("%(ext)s", "mp4");
            fs::create_dir_all(&dir).unwrap();
            let file = dir.join(name);
            fs::write(&file, &content).unwrap();
            let mut stdout = progress.join("\n") + "\n";
            for print in values_of(args, "--print") {
                match print.to_str().unwrap() {
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

    fn download_states(&self) -> Vec<Option<Download>> {
        self.events
            .events()
            .into_iter()
            .filter_map(|event| match event {
                AppEvent::Download(state) => Some(state),
                _ => None,
            })
            .collect()
    }

    fn status(&self) -> Option<DownloadStatus> {
        self.downloader.state().map(|download| download.status)
    }
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

    app.downloader.download(&video(URL, "Me at the zoo"));

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

/// A progress line as yt-dlp prints it with `--progress-template
/// "download:%(progress)j"`, cut down to the fields the app reads.
fn progress(downloaded: u64, total: u64, speed: f64, eta: u64) -> String {
    serde_json::json!({
        "status": "downloading",
        "downloaded_bytes": downloaded,
        "total_bytes": total,
        "speed": speed,
        "eta": eta,
        "_percent_str": "  0.5%",
    })
    .to_string()
}

fn downloading(percent: f64, speed: f64, eta: u64) -> DownloadStatus {
    DownloadStatus::Downloading {
        percent: Some(percent),
        speed: Some(speed),
        eta: Some(eta),
    }
}

#[test]
fn a_download_goes_from_downloading_through_processing_to_done() {
    let app = App::start();
    app.ytdlp_downloads(
        ID,
        b"the video",
        &[
            progress(250, 1000, 100.0, 7),
            progress(1000, 1000, 400.0, 0),
        ],
    );

    app.downloader.download(&video(URL, "Me at the zoo"));

    let statuses: Vec<DownloadStatus> = app
        .download_states()
        .into_iter()
        .map(|download| download.unwrap().status)
        .collect();
    assert_eq!(
        statuses,
        [
            DownloadStatus::Downloading {
                percent: None,
                speed: None,
                eta: None
            },
            downloading(25.0, 100.0, 7),
            downloading(100.0, 400.0, 0),
            DownloadStatus::Processing,
            DownloadStatus::Done {
                file: app.save_dir().join("Me at the zoo.mp4")
            },
        ]
    );
    let card = app.downloader.state().unwrap().video;
    assert_eq!(card.title, "Me at the zoo");
}

#[test]
fn a_taken_name_gets_the_next_free_number_and_nothing_is_overwritten() {
    let app = App::start();
    fs::create_dir_all(app.save_dir()).unwrap();
    fs::write(app.save_dir().join("Me at the zoo.mp4"), b"first").unwrap();
    fs::write(app.save_dir().join("Me at the zoo (1).mp4"), b"second").unwrap();
    app.ytdlp_downloads(ID, b"third", &[]);

    app.downloader.download(&video(URL, "Me at the zoo"));

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

#[test]
fn characters_windows_forbids_in_a_title_are_replaced_by_lookalikes() {
    let app = App::start();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.downloader
        .download(&video(URL, "Why? A/B: \"test\" | <1> * \\ end. "));

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

        app.downloader.download(&video(URL, title));

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
        let partial = PathBuf::from(after(args, "-P")).join(format!("{ID}.default.f137.mp4.part"));
        fs::create_dir_all(partial.parent().unwrap()).unwrap();
        fs::write(&partial, b"half").unwrap();
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: b"WARNING: retrying\nERROR: unable to download video data: Read timed out.\n"
                .to_vec(),
        }
    });

    app.downloader.download(&video(URL, "Me at the zoo"));

    assert_eq!(
        app.status(),
        Some(DownloadStatus::Failed {
            message: "ERROR: unable to download video data: Read timed out.".into()
        })
    );
    assert_eq!(
        file_names(&app.partials_dir()),
        [format!("{ID}.default.f137.mp4.part")]
    );
    assert!(!app.save_dir().exists());
}

#[test]
fn a_broken_yt_dlp_is_restored_and_the_download_is_dropped() {
    let app = App::start();
    app.processes.break_tool("yt-dlp.exe");
    let fetched_before = app.releases.fetches().len();

    app.downloader.download(&video(URL, "Me at the zoo"));

    assert_eq!(app.downloader.state(), None);
    assert_eq!(app.download_states().last(), Some(&None));
    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(
        app.releases.fetches()[fetched_before..],
        [crate::releases::fake::YTDLP_URL]
    );
}

#[test]
fn a_youtube_download_runs_with_the_sign_in_browser_and_another_site_without() {
    let app = App::start();
    app.settings
        .set_sign_in_browser(Some(Browser::Firefox))
        .unwrap();
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.downloader.download(&video(URL, "Me at the zoo"));
    app.downloader.download(&video(
        "https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/",
        "Rutube",
    ));

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

#[test]
fn a_download_asked_for_while_another_runs_is_ignored() {
    let app = App::start();
    let downloader = Arc::downgrade(&app.downloader);
    let statuses_seen = Arc::new(Mutex::new(Vec::new()));
    let seen = statuses_seen.clone();
    app.processes.script("yt-dlp.exe", move |_| {
        let downloader = downloader.upgrade().unwrap();
        downloader.download(&video(URL, "Another"));
        seen.lock().unwrap().push(downloader.state().unwrap());
        Output {
            status: exit_status(1),
            stdout: Vec::new(),
            stderr: b"ERROR: stopped".to_vec(),
        }
    });

    app.downloader.download(&video(URL, "Me at the zoo"));

    let seen = statuses_seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "yt-dlp ran once");
    assert_eq!(seen[0].video.title, "Me at the zoo");
}

#[test]
fn a_download_asks_yt_dlp_for_this_one_video_merged_into_an_mp4() {
    let app = App::start();
    let url = "https://www.youtube.com/watch?v=jNQXAC9IVRw&list=PL123";
    app.ytdlp_downloads(ID, b"the video", &[]);

    app.downloader.download(&video(url, "Me at the zoo"));

    let (_, args) = app.processes.runs().pop().unwrap();
    assert!(args.contains(&"--no-playlist".into()), "{args:?}");
    assert_eq!(after(&args, "--merge-output-format"), "mp4");
    // The URL comes after `--`, so yt-dlp never reads it as an option.
    assert_eq!(args[args.len() - 2..], [OsString::from("--"), url.into()]);
}

#[test]
fn a_stream_in_fragments_reports_the_share_of_fragments_received() {
    let app = App::start();
    let fragments = serde_json::json!({
        "status": "downloading",
        "downloaded_bytes": 52_000,
        "total_bytes_estimate": null,
        "fragment_index": 3,
        "fragment_count": 12,
        "speed": null,
        "eta": null,
    });
    app.ytdlp_downloads(ID, b"the video", &[fragments.to_string()]);

    app.downloader.download(&video(URL, "Me at the zoo"));

    let statuses: Vec<DownloadStatus> = app
        .download_states()
        .into_iter()
        .map(|download| download.unwrap().status)
        .collect();
    assert_eq!(
        statuses[1],
        DownloadStatus::Downloading {
            percent: Some(25.0),
            speed: None,
            eta: None
        }
    );
}

/// Probes and downloads a short public Video with the real tools:
/// `cargo test --lib -- --ignored downloads_a_real_video`. YouTube may ask
/// to prove the user isn't a bot; then name a browser signed in to YouTube
/// in `SMOKE_SIGN_IN_BROWSER`, such as `firefox`.
#[test]
#[ignore = "downloads the real tools and a real Video"]
fn downloads_a_real_video_with_the_real_yt_dlp() {
    use crate::clock::SystemClock;
    use crate::probe::Prober;
    use crate::processes::SystemProcesses;
    use crate::releases::github::GithubReleases;

    let dir = TempDir::new().unwrap();
    let events = Arc::new(RecordingSink::default());
    let settings = Arc::new(Settings::load(
        dir.path().join("settings.json"),
        events.clone(),
    ));
    if let Ok(browser) = std::env::var("SMOKE_SIGN_IN_BROWSER") {
        let browser: Browser = serde_json::from_value(browser.into()).unwrap();
        settings.set_sign_in_browser(Some(browser)).unwrap();
    }
    let toolbox = Arc::new(Toolbox::new(
        dir.path().join("tools"),
        Arc::new(GithubReleases::new()),
        Arc::new(SystemProcesses),
        Arc::new(SystemClock),
        events.clone(),
    ));
    toolbox.prepare();
    assert_eq!(toolbox.state(), SetupState::Ready);
    let prober = Prober::new(toolbox.clone(), settings.clone(), events.clone());
    prober.probe(URL);
    let video = prober
        .video()
        .unwrap_or_else(|| panic!("{:?}", prober.state()));
    let downloader = Downloader::new(
        toolbox,
        settings,
        dir.path().join("partials"),
        dir.path().join("Downloads"),
        events.clone(),
    );

    downloader.download(&video);

    let saved = dir.path().join("Downloads").join("Me at the zoo.mp4");
    assert_eq!(
        downloader.state().map(|download| download.status),
        Some(DownloadStatus::Done {
            file: saved.clone()
        })
    );
    assert!(fs::metadata(&saved).unwrap().len() > 100_000);
    assert_eq!(file_names(&dir.path().join("partials")), [] as [String; 0]);
    let statuses: Vec<DownloadStatus> = events
        .events()
        .into_iter()
        .filter_map(|event| match event {
            AppEvent::Download(Some(download)) => Some(download.status),
            _ => None,
        })
        .collect();
    assert!(statuses.iter().any(|status| matches!(
        status,
        DownloadStatus::Downloading {
            percent: Some(_),
            ..
        }
    )));
    assert!(statuses.contains(&DownloadStatus::Processing));
}
