use std::sync::Arc;
use std::time::SystemTime;

use tempfile::TempDir;

use super::*;
use crate::clock::FixedClock;
use crate::events::{AppEvent, RecordingSink};
use crate::processes::fake::FakeProcesses;
use crate::releases::fake::FakeReleases;
use crate::settings::{Browser, Settings};
use crate::toolbox::SetupState;

const VIDEO: &[u8] = include_bytes!("fixtures/youtube-video.json");

struct App {
    _dir: TempDir,
    releases: Arc<FakeReleases>,
    processes: Arc<FakeProcesses>,
    toolbox: Arc<Toolbox>,
    settings: Arc<Settings>,
    events: Arc<RecordingSink>,
    prober: Prober,
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
            settings.clone(),
            events.clone(),
        ));
        toolbox.prepare();
        Self {
            _dir: dir,
            releases,
            processes,
            prober: Prober::new(toolbox.clone(), settings.clone(), events.clone()),
            toolbox,
            settings,
            events,
        }
    }

    /// Makes yt-dlp answer every Probe with this stdout.
    fn ytdlp_prints(&self, stdout: &[u8]) {
        self.processes.succeed_with("yt-dlp.exe", stdout);
    }

    /// Makes yt-dlp fail every Probe with this stderr.
    fn ytdlp_fails(&self, stderr: &[u8]) {
        self.processes.fail_with("yt-dlp.exe", stderr);
    }

    fn probe_states(&self) -> Vec<ProbeState> {
        self.events
            .events()
            .into_iter()
            .filter_map(|event| match event {
                AppEvent::Probe(state) => Some(state),
                _ => None,
            })
            .collect()
    }
}

/// A failed Probe that ran without a sign-in browser.
fn failed(error: ProbeError) -> ProbeState {
    ProbeState::Failed {
        error,
        sign_in_browser: None,
    }
}

#[test]
fn a_video_url_shows_the_video_card() {
    let app = App::start();
    app.ytdlp_prints(VIDEO);

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    let card = VideoCard {
        title: "A Simple Way to Break a Bad Habit | Judson Brewer | TED".into(),
        channel: Some("TED".into()),
        thumbnail: Some("https://i.ytimg.com/vi/-moW9jvvMr4/maxresdefault.jpg".into()),
        duration: Some(565.0),
    };
    assert_eq!(
        app.probe_states(),
        [
            ProbeState::Probing,
            ProbeState::Probed {
                video: card.clone()
            }
        ]
    );
    assert_eq!(app.prober.state(), ProbeState::Probed { video: card });
}

#[test]
fn a_url_of_a_video_in_a_playlist_probes_only_that_video() {
    let app = App::start();
    app.ytdlp_prints(VIDEO);
    let url = "https://www.youtube.com/watch?v=-moW9jvvMr4&list=PLOGi5-fAu8bH_VbadQVK_GFsP2lbd6BmL";

    app.prober.probe(url);

    let (program, args) = app.processes.runs().pop().unwrap();
    assert_eq!(program.file_name().unwrap(), "yt-dlp.exe");
    assert!(args.contains(&OsString::from("--no-playlist")), "{args:?}");
    // The URL comes after `--`, so yt-dlp never reads it as an option.
    assert_eq!(args[args.len() - 2..], [OsString::from("--"), url.into()]);
    assert!(matches!(app.prober.state(), ProbeState::Probed { .. }));
}

#[test]
fn a_playlist_or_channel_is_rejected_without_a_card() {
    for (url, answer) in [
        (
            "https://www.youtube.com/playlist?list=PLbpi6ZahtOH6Blw3RGYpWkSByi_T7Rygb",
            &include_bytes!("fixtures/youtube-playlist.json")[..],
        ),
        (
            "https://www.youtube.com/@YouTube",
            &include_bytes!("fixtures/youtube-channel.json")[..],
        ),
    ] {
        let app = App::start();
        app.ytdlp_prints(answer);

        app.prober.probe(url);

        assert_eq!(app.prober.state(), failed(ProbeError::Playlist), "{url}");
        // Entries stay unresolved, so a long playlist is rejected quickly.
        let (_, args) = app.processes.runs().pop().unwrap();
        assert!(
            args.contains(&OsString::from("--flat-playlist")),
            "{args:?}"
        );
    }
}

#[test]
fn a_stream_that_is_live_now_is_rejected() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/youtube-live.json"));

    app.prober
        .probe("https://www.youtube.com/watch?v=0sfDth4lMLc");

    assert_eq!(app.prober.state(), failed(ProbeError::Live));
}

#[test]
fn a_finished_stream_is_an_ordinary_video() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/youtube-finished-stream.json"));

    app.prober
        .probe("https://www.youtube.com/watch?v=pZMrdjuU-CY");

    assert_eq!(
        app.prober.state(),
        ProbeState::Probed {
            video: VideoCard {
                title: "Around the World with Peppa Pig | Full Live Stream Replay 🌍".into(),
                channel: Some("Travel with Peppa".into()),
                thumbnail: Some("https://i.ytimg.com/vi/pZMrdjuU-CY/maxresdefault.jpg".into()),
                duration: Some(14339.0),
            }
        }
    );
}

#[test]
fn a_video_that_needs_signing_in_is_rejected() {
    for (url, stderr) in [
        (
            "https://www.youtube.com/watch?v=LvCtTbfm2O8",
            &include_bytes!("fixtures/youtube-members-only.stderr")[..],
        ),
        (
            "https://www.youtube.com/watch?v=Tq92D6wQ1mg",
            &include_bytes!("fixtures/youtube-age-restricted.stderr")[..],
        ),
        // Not captured, no private video was at hand: YouTube's reason
        // followed by yt-dlp's hint, as yt-dlp builds it.
        (
            "https://www.youtube.com/watch?v=yZIXLfi8CZQ",
            &b"ERROR: [youtube] yZIXLfi8CZQ: Private video. Sign in if you've been granted access to this video. Use --cookies-from-browser or --cookies for the authentication.
"[..],
        ),
    ] {
        let app = App::start();
        app.ytdlp_fails(stderr);

        app.prober.probe(url);

        assert_eq!(
            app.prober.state(),
            failed(ProbeError::SignInRequired),
            "{url}"
        );
    }
}

#[test]
fn an_unsupported_site_or_a_url_without_a_video_is_rejected() {
    for (url, stderr) in [
        (
            "https://example.com/",
            &include_bytes!("fixtures/unsupported-url.stderr")[..],
        ),
        (
            "https://www.youtube.com/watch?v=yZIXLfi8CZQ",
            &include_bytes!("fixtures/youtube-unavailable.stderr")[..],
        ),
    ] {
        let app = App::start();
        app.ytdlp_fails(stderr);

        app.prober.probe(url);

        assert_eq!(app.prober.state(), failed(ProbeError::Unsupported), "{url}");
    }
}

#[test]
fn a_url_with_a_truncated_id_is_rejected() {
    let app = App::start();
    app.ytdlp_fails(include_bytes!("fixtures/youtube-truncated-id.stderr"));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr");

    assert_eq!(app.prober.state(), failed(ProbeError::TruncatedUrl));
}

#[test]
fn a_bot_check_is_told_apart_from_a_sign_in() {
    let app = App::start();
    let stderr = include_bytes!("fixtures/youtube-bot-check.stderr");
    app.ytdlp_fails(stderr);

    app.prober
        .probe("https://www.youtube.com/watch?v=jNQXAC9IVRw");

    assert_eq!(app.prober.state(), failed(ProbeError::BotCheck));
}

#[test]
fn cookies_that_cant_be_read_say_why() {
    for (browser, stderr, reason) in [
        // Not installed, or never opened.
        (
            Browser::Vivaldi,
            &include_bytes!("fixtures/cookies-browser-not-found.stderr")[..],
            CookiesError::BrowserNotFound,
        ),
        // Edge and Chrome lock their cookies while they run.
        (
            Browser::Edge,
            &include_bytes!("fixtures/cookies-browser-open.stderr")[..],
            CookiesError::BrowserOpen,
        ),
        // Chromium-based browsers on Windows encrypt cookies for themselves.
        (
            Browser::Brave,
            &include_bytes!("fixtures/cookies-cant-decrypt.stderr")[..],
            CookiesError::CantDecrypt,
        ),
    ] {
        let app = App::start();
        app.settings.set_sign_in_browser(Some(browser)).unwrap();
        app.ytdlp_fails(stderr);

        app.prober
            .probe("https://www.youtube.com/watch?v=jNQXAC9IVRw");

        assert_eq!(
            app.prober.state(),
            ProbeState::Failed {
                error: ProbeError::CookiesUnreadable { reason },
                sign_in_browser: Some(browser),
            },
            "{browser:?}"
        );
    }
}

#[test]
fn a_failed_probe_names_the_sign_in_browser_it_ran_with() {
    let app = App::start();
    app.settings
        .set_sign_in_browser(Some(Browser::Firefox))
        .unwrap();
    app.ytdlp_fails(include_bytes!("fixtures/youtube-bot-check.stderr"));
    app.prober
        .probe("https://www.youtube.com/watch?v=jNQXAC9IVRw");

    // Choosing another browser doesn't change what the Probe ran with.
    app.settings.set_sign_in_browser(None).unwrap();

    assert_eq!(
        app.prober.state(),
        ProbeState::Failed {
            error: ProbeError::BotCheck,
            sign_in_browser: Some(Browser::Firefox),
        }
    );
}

#[test]
fn the_probed_video_keeps_what_downloads_need() {
    let app = App::start();
    app.ytdlp_prints(VIDEO);

    app.prober.probe(
        "https://www.youtube.com/watch?v=-moW9jvvMr4&list=PLOGi5-fAu8bH_VbadQVK_GFsP2lbd6BmL",
    );

    let video = app.prober.video().expect("a probed Video");
    assert_eq!(video.url, "https://www.youtube.com/watch?v=-moW9jvvMr4");
    // Storyboards (sb0–sb3) are pictures, not streams.
    let ids: Vec<&str> = video.formats.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "249-drc", "250-drc", "249", "250", "140-drc", "251-drc", "140", "251", "160", "278",
            "394", "133", "242", "395", "134", "18", "243", "396", "135", "244", "397", "136",
            "247", "398", "248-sr", "399-sr"
        ]
    );
    assert_eq!(
        video.formats[15],
        Format {
            id: "18".into(),
            ext: "mp4".into(),
            video_codec: Some("avc1.42001E".into()),
            audio_codec: Some("mp4a.40.2".into()),
            width: Some(640),
            height: Some(360),
            fps: Some(30.0),
            bitrate: Some(250.869),
            size: Some(17707023),
            note: Some("360p".into()),
            language: Some("en".into()),
        }
    );
    assert_eq!(video.transcripts.manual.len(), 43);
    assert!(video.transcripts.manual.contains(&"ru".into()));
    assert_eq!(video.transcripts.automatic.len(), 157);
    assert!(video.transcripts.automatic.contains(&"en-orig".into()));
    assert_eq!(video.original_language.as_deref(), Some("en"));
    assert_eq!(video.chapters.len(), 6);
    assert_eq!(
        video.chapters[1],
        Chapter {
            start: 40.0,
            end: 208.0,
            title: "Why Is It So Hard To Pay Attention".into(),
        }
    );
    assert!(!video.was_live);
}

#[test]
fn a_finished_stream_is_marked_as_one() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/youtube-finished-stream.json"));

    app.prober
        .probe("https://www.youtube.com/watch?v=pZMrdjuU-CY");

    assert!(app.prober.video().unwrap().was_live);
}

#[test]
fn the_chat_replay_of_a_finished_stream_is_not_a_transcript() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/youtube-stream-with-chat.json"));

    app.prober
        .probe("https://www.youtube.com/watch?v=fp_7X3VWIuA");

    let video = app.prober.video().unwrap();
    assert_eq!(video.transcripts.manual, Vec::<String>::new());
    assert_eq!(video.transcripts.automatic.len(), 157);
}

#[test]
fn a_video_from_another_site_may_lack_a_channel_language_and_transcripts() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/rutube-video.json"));

    app.prober
        .probe("https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/");

    let video = app.prober.video().unwrap();
    // Without a channel, the uploader stands in.
    assert_eq!(video.card().channel.as_deref(), Some("Cyber Potato"));
    assert_eq!(video.formats.len(), 24);
    assert_eq!(video.original_language, None);
    assert!(video.transcripts.manual.is_empty() && video.transcripts.automatic.is_empty());
    assert!(video.chapters.is_empty());
}

#[test]
fn a_failed_probe_forgets_the_previous_video() {
    let app = App::start();
    app.ytdlp_prints(VIDEO);
    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");
    app.ytdlp_fails(include_bytes!("fixtures/unsupported-url.stderr"));

    app.prober.probe("https://example.com/");

    assert_eq!(app.prober.video(), None);
}

#[test]
fn a_broken_yt_dlp_is_restored_and_the_probe_is_dropped() {
    let app = App::start();
    app.processes.break_tool("yt-dlp.exe");
    let fetched_before = app.releases.fetches().len();

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(app.prober.state(), ProbeState::Idle);
    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(
        app.releases.fetches()[fetched_before..],
        [crate::releases::fake::YTDLP_URL]
    );
}
