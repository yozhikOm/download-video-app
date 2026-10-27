use std::sync::Arc;
use std::time::SystemTime;

use tempfile::TempDir;

use super::*;
use crate::clock::FixedClock;
use crate::events::{AppEvent, RecordingSink};
use crate::preset::{AudioPreset, Preset, VideoPreset};
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
            dir.path().join("Downloads"),
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

    /// The browser the last yt-dlp run read cookies from, if any.
    fn cookies_browser(&self) -> Option<OsString> {
        let (_, args) = self.processes.runs().pop().unwrap();
        let cookies = args
            .iter()
            .position(|arg| arg == "--cookies-from-browser")?;
        Some(args[cookies + 1].clone())
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
    let probed = ProbeState::Probed {
        video: card,
        video_presets: vec![
            VideoPreset::P1080,
            VideoPreset::P720,
            VideoPreset::P480,
            VideoPreset::P360,
        ],
        default_video_preset: VideoPreset::P1080,
        audio_presets: vec![AudioPreset::M4a, AudioPreset::Mp3],
        m4a_keeps_sound: true,
        // Which ones, the Transcript tests tell.
        transcript_languages: offered_transcripts(&app).0,
        default_transcript_language: Some("en".into()),
    };
    assert_eq!(app.probe_states(), [ProbeState::Probing, probed.clone()]);
    assert_eq!(app.prober.state(), probed);
}

/// The Video presets the latest Probe offers, and the one chosen at first.
fn offered_presets(app: &App) -> (Vec<VideoPreset>, VideoPreset) {
    match app.prober.state() {
        ProbeState::Probed {
            video_presets,
            default_video_preset,
            ..
        } => (video_presets, default_video_preset),
        state => panic!("not probed: {state:?}"),
    }
}

#[test]
fn a_probed_video_offers_only_the_video_presets_it_has_with_1080p_chosen() {
    let app = App::start();
    app.ytdlp_prints(VIDEO);

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(
        offered_presets(&app),
        (
            vec![
                VideoPreset::P1080,
                VideoPreset::P720,
                VideoPreset::P480,
                VideoPreset::P360
            ],
            VideoPreset::P1080
        )
    );
}

/// The `-J` answer of `youtube-video.json` with its Formats changed by
/// `edit`.
fn video_with_formats(edit: impl FnOnce(&mut Vec<serde_json::Value>)) -> Vec<u8> {
    let mut answer: serde_json::Value = serde_json::from_slice(VIDEO).unwrap();
    let serde_json::Value::Array(formats) = &mut answer["formats"] else {
        panic!("no formats");
    };
    edit(formats);
    serde_json::to_vec(&answer).unwrap()
}

/// The `-J` answer of `youtube-video.json` without the Formats taller than
/// `height`.
fn video_up_to(height: u64) -> Vec<u8> {
    video_with_formats(|formats| {
        formats.retain(|format| format["height"].as_u64().is_none_or(|it| it <= height))
    })
}

#[test]
fn without_1080p_the_nearest_lower_video_preset_is_chosen() {
    for (height, presets, chosen) in [
        (
            720,
            vec![VideoPreset::P720, VideoPreset::P480, VideoPreset::P360],
            VideoPreset::P720,
        ),
        (
            480,
            vec![VideoPreset::P480, VideoPreset::P360],
            VideoPreset::P480,
        ),
        (360, vec![VideoPreset::P360], VideoPreset::P360),
    ] {
        let app = App::start();
        app.ytdlp_prints(&video_up_to(height));

        app.prober
            .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

        assert_eq!(offered_presets(&app), (presets, chosen), "up to {height}p");
    }
}

#[test]
fn a_video_above_1080p_offers_its_higher_presets_with_1080p_still_chosen() {
    let app = App::start();
    // As YouTube offers them: above 1080p only VP9 and AV1.
    app.ytdlp_prints(&video_with_formats(|formats| {
        formats.extend([
            serde_json::json!({"format_id": "271", "ext": "webm", "vcodec": "vp9", "acodec": "none", "width": 2560, "height": 1440}),
            serde_json::json!({"format_id": "401", "ext": "mp4", "vcodec": "av01.0.12M.08", "acodec": "none", "width": 3840, "height": 2160}),
        ])
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(
        offered_presets(&app),
        (
            vec![
                VideoPreset::P2160,
                VideoPreset::P1440,
                VideoPreset::P1080,
                VideoPreset::P720,
                VideoPreset::P480,
                VideoPreset::P360
            ],
            VideoPreset::P1080
        )
    );
}

#[test]
fn a_video_above_2160p_offers_its_best_too() {
    let app = App::start();
    app.ytdlp_prints(&video_with_formats(|formats| {
        formats.push(serde_json::json!({"format_id": "571", "ext": "mp4", "vcodec": "av01.0.16M.08", "acodec": "none", "width": 7680, "height": 4320}))
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(
        offered_presets(&app),
        (
            vec![
                VideoPreset::Best,
                VideoPreset::P1080,
                VideoPreset::P720,
                VideoPreset::P480,
                VideoPreset::P360
            ],
            VideoPreset::P1080
        )
    );
}

#[test]
fn a_video_of_unknown_size_offers_only_its_best() {
    let app = App::start();
    app.ytdlp_prints(&video_with_formats(|formats| {
        formats.retain(|format| format["height"].is_null());
        formats.push(serde_json::json!({"format_id": "18", "ext": "mp4", "vcodec": "avc1.42001E", "acodec": "mp4a.40.2"}))
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(
        offered_presets(&app),
        (vec![VideoPreset::Best], VideoPreset::Best)
    );
}

#[test]
fn without_1080p_or_lower_the_nearest_higher_video_preset_is_chosen() {
    let app = App::start();
    app.ytdlp_prints(&video_with_formats(|formats| {
        formats.retain(|format| format["height"].is_null());
        formats.extend([
            serde_json::json!({"format_id": "271", "ext": "webm", "vcodec": "vp9", "acodec": "none", "width": 2560, "height": 1440}),
            serde_json::json!({"format_id": "401", "ext": "mp4", "vcodec": "av01.0.12M.08", "acodec": "none", "width": 3840, "height": 2160}),
        ])
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(
        offered_presets(&app),
        (
            vec![VideoPreset::P2160, VideoPreset::P1440],
            VideoPreset::P1440
        )
    );
}

#[test]
fn the_default_video_preset_from_the_settings_is_chosen() {
    let app = App::start();
    app.settings
        .set_default_video_preset(VideoPreset::P480)
        .unwrap();
    app.ytdlp_prints(VIDEO);

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(offered_presets(&app).1, VideoPreset::P480);
}

#[test]
fn without_the_default_video_preset_from_the_settings_the_nearest_lower_is_chosen() {
    let app = App::start();
    app.settings
        .set_default_video_preset(VideoPreset::P2160)
        .unwrap();
    app.ytdlp_prints(VIDEO);

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(offered_presets(&app).1, VideoPreset::P1080);
}

#[test]
fn a_default_video_preset_below_any_the_video_has_gives_way_to_the_nearest_higher() {
    let app = App::start();
    app.settings
        .set_default_video_preset(VideoPreset::P360)
        .unwrap();
    app.ytdlp_prints(&video_with_formats(|formats| {
        formats.retain(|format| format["height"].is_null());
        formats.extend([
            serde_json::json!({"format_id": "271", "ext": "webm", "vcodec": "vp9", "acodec": "none", "width": 2560, "height": 1440}),
            serde_json::json!({"format_id": "401", "ext": "mp4", "vcodec": "av01.0.12M.08", "acodec": "none", "width": 3840, "height": 2160}),
        ])
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(offered_presets(&app).1, VideoPreset::P1440);
}

#[test]
fn a_default_of_the_best_chooses_the_videos_best_or_else_its_highest_preset() {
    let above_2160p = video_with_formats(|formats| {
        formats.push(serde_json::json!({"format_id": "571", "ext": "mp4", "vcodec": "av01.0.16M.08", "acodec": "none", "width": 7680, "height": 4320}))
    });
    for (answer, chosen) in [
        (above_2160p.as_slice(), VideoPreset::Best),
        (VIDEO, VideoPreset::P1080),
    ] {
        let app = App::start();
        app.settings
            .set_default_video_preset(VideoPreset::Best)
            .unwrap();
        app.ytdlp_prints(answer);

        app.prober
            .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

        assert_eq!(offered_presets(&app).1, chosen);
    }
}

#[test]
fn a_widescreen_or_vertical_video_offers_presets_by_the_smaller_side() {
    // The picture's width and height for each of the fixture's heights.
    let widescreen = |height: u64| (height * 16 / 9, height * 800 / 1080);
    let vertical = |height: u64| (height, height * 16 / 9);
    for (shape, size) in [
        ("widescreen", &widescreen as &dyn Fn(u64) -> (u64, u64)),
        ("vertical", &vertical),
    ] {
        let app = App::start();
        app.ytdlp_prints(&video_with_formats(|formats| {
            for format in formats.iter_mut() {
                if let Some(height) = format["height"].as_u64() {
                    let (width, height) = size(height);
                    format["width"] = width.into();
                    format["height"] = height.into();
                }
            }
        }));

        app.prober
            .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

        assert_eq!(offered_presets(&app).1, VideoPreset::P1080, "{shape}");
    }
}

#[test]
fn a_video_with_video_without_sound_offers_presets_only_from_it() {
    let app = App::start();
    app.ytdlp_prints(&video_with_formats(|formats| {
        // Its 720p comes only with sound, which a Download doesn't take.
        formats.retain(|format| format["height"].as_u64() != Some(720));
        formats.push(serde_json::json!({"format_id": "22", "ext": "mp4", "vcodec": "avc1.64001F", "acodec": "mp4a.40.2", "width": 1280, "height": 720}));
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(
        offered_presets(&app).0,
        vec![VideoPreset::P1080, VideoPreset::P480, VideoPreset::P360]
    );
}

#[test]
fn a_video_preset_goes_to_the_ui_and_back_by_its_name() {
    for (preset, name) in [(VideoPreset::Best, "best"), (VideoPreset::P360, "360p")] {
        assert_eq!(serde_json::to_value(preset).unwrap(), name);
        assert_eq!(
            serde_json::from_value::<VideoPreset>(name.into()).unwrap(),
            preset
        );
    }
    assert!(serde_json::from_value::<VideoPreset>("4320p".into()).is_err());
}

/// The Audio presets the latest Probe offers.
fn offered_audio_presets(app: &App) -> Vec<AudioPreset> {
    match app.prober.state() {
        ProbeState::Probed { audio_presets, .. } => audio_presets,
        state => panic!("not probed: {state:?}"),
    }
}

#[test]
fn a_video_with_sound_offers_m4a_and_mp3() {
    for (url, answer) in [
        ("https://www.youtube.com/watch?v=-moW9jvvMr4", VIDEO),
        // Its Formats have video and sound together.
        (
            "https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/",
            include_bytes!("fixtures/rutube-video.json"),
        ),
    ] {
        let app = App::start();
        app.ytdlp_prints(answer);

        app.prober.probe(url);

        assert_eq!(
            offered_audio_presets(&app),
            vec![AudioPreset::M4a, AudioPreset::Mp3],
            "{url}"
        );
    }
}

#[test]
fn a_video_without_sound_offers_no_audio_presets() {
    let app = App::start();
    app.ytdlp_prints(&video_with_formats(|formats| {
        formats.retain(|format| format["acodec"] == "none")
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(offered_audio_presets(&app), []);
}

#[test]
fn a_video_that_lists_no_formats_offers_m4a_and_mp3() {
    let app = App::start();
    app.ytdlp_prints(&video_with_formats(Vec::clear));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    assert_eq!(
        offered_audio_presets(&app),
        vec![AudioPreset::M4a, AudioPreset::Mp3]
    );
}

/// Whether the latest Probe says M4A saves the Video's sound as it is.
fn m4a_keeps_sound(app: &App) -> bool {
    match app.prober.state() {
        ProbeState::Probed {
            m4a_keeps_sound, ..
        } => m4a_keeps_sound,
        state => panic!("not probed: {state:?}"),
    }
}

#[test]
fn m4a_keeps_the_sound_of_a_video_with_aac() {
    for (url, answer) in [
        ("https://www.youtube.com/watch?v=-moW9jvvMr4", VIDEO),
        (
            "https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/",
            include_bytes!("fixtures/rutube-video.json"),
        ),
    ] {
        let app = App::start();
        app.ytdlp_prints(answer);

        app.prober.probe(url);

        assert!(m4a_keeps_sound(&app), "{url}");
    }
}

#[test]
fn m4a_encodes_the_sound_of_a_video_without_aac() {
    for answer in [
        video_with_formats(|formats| {
            formats.retain(|format| !format["acodec"].as_str().unwrap().starts_with("mp4a"))
        }),
        video_with_formats(Vec::clear),
    ] {
        let app = App::start();
        app.ytdlp_prints(&answer);

        app.prober
            .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

        assert!(!m4a_keeps_sound(&app));
    }
}

#[test]
fn a_preset_comes_from_the_ui_by_its_name() {
    for (name, preset) in [
        ("1080p", Preset::Video(VideoPreset::P1080)),
        ("m4a", Preset::Audio(AudioPreset::M4a)),
        ("mp3", Preset::Audio(AudioPreset::Mp3)),
    ] {
        assert_eq!(
            serde_json::from_value::<Preset>(name.into()).unwrap(),
            preset
        );
    }
    assert_eq!(serde_json::to_value(AudioPreset::Mp3).unwrap(), "mp3");
    assert_eq!(
        serde_json::from_value::<Preset>("flac".into())
            .unwrap_err()
            .to_string(),
        "no Preset is named flac"
    );
}

#[test]
fn a_video_from_another_site_offers_presets_from_its_formats_with_sound() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/rutube-video.json"));

    app.prober
        .probe("https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/");

    assert_eq!(
        offered_presets(&app),
        (
            vec![
                VideoPreset::P1080,
                VideoPreset::P720,
                VideoPreset::P480,
                VideoPreset::P360
            ],
            VideoPreset::P1080
        )
    );
}

/// The Transcript languages the latest Probe offers, and the one chosen at
/// first.
fn offered_transcripts(app: &App) -> (Vec<TranscriptLanguage>, Option<String>) {
    match app.prober.state() {
        ProbeState::Probed {
            transcript_languages,
            default_transcript_language,
            ..
        } => (transcript_languages, default_transcript_language),
        state => panic!("not probed: {state:?}"),
    }
}

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
fn a_video_with_subtitles_offers_them_over_captions_with_its_own_language_chosen() {
    let app = App::start();
    app.ytdlp_prints(VIDEO);

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    let (transcripts, chosen) = offered_transcripts(&app);
    // English has both; its subtitles stand in for the captions.
    assert_eq!(transcripts.len(), 43);
    assert!(
        transcripts.iter().all(|it| !it.automatic),
        "{transcripts:?}"
    );
    assert!(transcripts.contains(&manual("ru")));
    assert!(transcripts.contains(&manual("en")));
    assert_eq!(chosen.as_deref(), Some("en"));
}

#[test]
fn a_video_with_captions_alone_offers_its_own_language_without_machine_translations() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/youtube-finished-stream.json"));

    app.prober
        .probe("https://www.youtube.com/watch?v=pZMrdjuU-CY");

    assert_eq!(
        offered_transcripts(&app),
        (vec![automatic("en")], Some("en".into()))
    );
}

#[test]
fn a_video_without_subtitles_or_captions_offers_no_transcript() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/rutube-video.json"));

    app.prober
        .probe("https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/");

    assert_eq!(offered_transcripts(&app), (vec![], None));
}

/// The `-J` answer of `youtube-video.json` changed by `edit`.
fn video_with(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let mut answer: serde_json::Value = serde_json::from_slice(VIDEO).unwrap();
    edit(&mut answer);
    serde_json::to_vec(&answer).unwrap()
}

#[test]
fn without_subtitles_in_its_own_language_the_captions_of_it_are_chosen() {
    let app = App::start();
    app.ytdlp_prints(&video_with(|answer| {
        answer["subtitles"].as_object_mut().unwrap().remove("en");
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    let (transcripts, chosen) = offered_transcripts(&app);
    assert_eq!(transcripts.len(), 43);
    assert!(transcripts.contains(&automatic("en")));
    assert!(!transcripts.contains(&manual("en")));
    assert_eq!(chosen.as_deref(), Some("en"));
}

#[test]
fn subtitles_of_a_regional_variant_count_as_the_videos_own_language() {
    let app = App::start();
    app.ytdlp_prints(&video_with(|answer| {
        let subtitles = answer["subtitles"].as_object_mut().unwrap();
        let english = subtitles.remove("en").unwrap();
        subtitles.insert("en-GB".into(), english);
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    let (transcripts, chosen) = offered_transcripts(&app);
    // The captions are in plain English, which the subtitles don't have.
    assert!(transcripts.contains(&manual("en-GB")));
    assert!(transcripts.contains(&automatic("en")));
    assert_eq!(chosen.as_deref(), Some("en-GB"));
}

#[test]
fn a_video_of_unknown_language_gets_the_language_of_its_captions_chosen() {
    let app = App::start();
    app.ytdlp_prints(&video_with(|answer| {
        answer.as_object_mut().unwrap().remove("language");
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    // The captions are of what is spoken.
    assert_eq!(offered_transcripts(&app).1.as_deref(), Some("en"));
}

#[test]
fn a_video_of_unknown_language_without_captions_gets_its_first_subtitles_chosen() {
    let app = App::start();
    app.ytdlp_prints(&video_with(|answer| {
        let answer = answer.as_object_mut().unwrap();
        answer.remove("language");
        answer.remove("automatic_captions");
    }));

    app.prober
        .probe("https://www.youtube.com/watch?v=-moW9jvvMr4");

    let (transcripts, chosen) = offered_transcripts(&app);
    assert_eq!(chosen, Some(transcripts[0].language.clone()));
}

#[test]
fn the_chat_replay_of_a_stream_is_not_offered_as_a_transcript() {
    let app = App::start();
    app.ytdlp_prints(include_bytes!("fixtures/youtube-stream-with-chat.json"));

    app.prober
        .probe("https://www.youtube.com/watch?v=fp_7X3VWIuA");

    assert_eq!(offered_transcripts(&app).0, [automatic("en")]);
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
            },
            video_presets: vec![
                VideoPreset::P1080,
                VideoPreset::P720,
                VideoPreset::P480,
                VideoPreset::P360,
            ],
            default_video_preset: VideoPreset::P1080,
            audio_presets: vec![AudioPreset::M4a, AudioPreset::Mp3],
            m4a_keeps_sound: true,
            transcript_languages: vec![automatic("en")],
            default_transcript_language: Some("en".into()),
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
fn a_probe_runs_yt_dlp_with_the_sign_in_browser_it_names() {
    let app = App::start();
    app.settings
        .set_sign_in_browser(Some(Browser::Firefox))
        .unwrap();
    app.ytdlp_fails(include_bytes!("fixtures/youtube-bot-check.stderr"));

    app.prober
        .probe("https://www.youtube.com/watch?v=jNQXAC9IVRw");

    let runs = app.processes.runs();
    let (_, args) = runs.last().unwrap();
    let cookies = args
        .iter()
        .position(|arg| arg == "--cookies-from-browser")
        .expect("--cookies-from-browser");
    assert_eq!(args[cookies + 1], "firefox");
    assert_eq!(
        app.prober.state(),
        ProbeState::Failed {
            error: ProbeError::BotCheck,
            sign_in_browser: Some(Browser::Firefox),
        }
    );
}

#[test]
fn every_youtube_url_runs_with_the_sign_in_browser() {
    for url in [
        "youtu.be/jNQXAC9IVRw",
        "https://m.youtube.com/watch?v=jNQXAC9IVRw",
        "https://www.youtube-nocookie.com/embed/jNQXAC9IVRw",
    ] {
        let app = App::start();
        app.settings
            .set_sign_in_browser(Some(Browser::Firefox))
            .unwrap();
        app.ytdlp_prints(VIDEO);

        app.prober.probe(url);

        assert_eq!(app.cookies_browser(), Some("firefox".into()), "{url}");
    }
}

#[test]
fn a_url_of_another_site_runs_without_the_sign_in_browser() {
    let app = App::start();
    app.settings
        .set_sign_in_browser(Some(Browser::Edge))
        .unwrap();
    app.ytdlp_fails(b"ERROR: Unsupported URL: https://rutube.ru/");

    app.prober
        .probe("https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/");

    // The user's sign-in to other sites stays in the browser.
    assert_eq!(app.cookies_browser(), None);
    assert_eq!(app.prober.state(), failed(ProbeError::Unsupported));
}

#[test]
fn a_host_that_only_looks_like_youtube_runs_without_the_sign_in_browser() {
    for url in [
        "https://notyoutube.com/watch?v=jNQXAC9IVRw",
        "https://youtube.com.example.org/watch?v=jNQXAC9IVRw",
    ] {
        let app = App::start();
        app.settings
            .set_sign_in_browser(Some(Browser::Firefox))
            .unwrap();
        app.ytdlp_prints(VIDEO);

        app.prober.probe(url);

        assert_eq!(app.cookies_browser(), None, "{url}");
    }
}

#[test]
fn the_probed_video_keeps_what_downloads_need() {
    let app = App::start();
    app.ytdlp_prints(VIDEO);

    app.prober.probe(
        "https://www.youtube.com/watch?v=-moW9jvvMr4&list=PLOGi5-fAu8bH_VbadQVK_GFsP2lbd6BmL",
    );

    let video = app.prober.video().expect("a probed Video");
    assert_eq!(video.id, "-moW9jvvMr4");
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
