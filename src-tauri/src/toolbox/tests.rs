use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tempfile::TempDir;

use super::*;
use crate::clock::FixedClock;
use crate::events::{AppEvent, RecordingSink};
use crate::processes::fake::FakeProcesses;
use crate::releases::fake::{
    DENO_BYTES, DENO_URL, FFMPEG_BYTES, FFMPEG_URL, FFPROBE_BYTES, FakeReleases, NEWER_YTDLP_BYTES,
    NEWER_YTDLP_URL, NEWER_YTDLP_VERSION, YTDLP_BYTES, YTDLP_URL,
};
use crate::settings::Browser;

const NOW: Duration = Duration::from_secs(1_790_000_000);
const HOUR: Duration = Duration::from_secs(60 * 60);

struct App {
    dir: TempDir,
    releases: Arc<FakeReleases>,
    processes: Arc<FakeProcesses>,
    events: Arc<RecordingSink>,
    toolbox: Toolbox,
}

impl App {
    fn start() -> Self {
        Self::start_with(
            Arc::new(FakeReleases::new()),
            Arc::new(FakeProcesses::new()),
            TempDir::new().unwrap(),
            NOW,
        )
    }

    /// Starts the app again on the same data folder, like a relaunch.
    fn restart(self) -> Self {
        self.restart_at(NOW)
    }

    /// Like [`App::restart`], with the clock at `now` since the Unix epoch.
    fn restart_at(self, now: Duration) -> Self {
        Self::start_with(self.releases, self.processes, self.dir, now)
    }

    fn start_with(
        releases: Arc<FakeReleases>,
        processes: Arc<FakeProcesses>,
        dir: TempDir,
        now: Duration,
    ) -> Self {
        let events = Arc::new(RecordingSink::default());
        let toolbox = Toolbox::new(
            dir.path().join("tools"),
            releases.clone(),
            processes.clone(),
            Arc::new(FixedClock(SystemTime::UNIX_EPOCH + now)),
            events.clone(),
        );
        Self {
            dir,
            releases,
            processes,
            events,
            toolbox,
        }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.path().join("tools").join(name)
    }

    /// Asset URLs fetched after the first `skip` fetches.
    fn fetches_after(&self, skip: usize) -> Vec<String> {
        self.releases.fetches().split_off(skip)
    }

    /// The states of the updates of yt-dlp asked for, as each event sent it.
    fn update_states(&self) -> Vec<YtDlpUpdateState> {
        let events = self.events.events();
        events
            .into_iter()
            .filter_map(|event| match event {
                AppEvent::YtDlpUpdate(state) => Some(state),
                _ => None,
            })
            .collect()
    }

    fn states(&self) -> Vec<SetupState> {
        let events = self.events.events();
        events
            .into_iter()
            .filter_map(|event| match event {
                AppEvent::Setup(state) => Some(state),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn first_run_installs_all_four_tools_and_becomes_ready() {
    let app = App::start();

    app.toolbox.prepare();

    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(std::fs::read(app.file("yt-dlp.exe")).unwrap(), YTDLP_BYTES);
    assert_eq!(std::fs::read(app.file("ffmpeg.exe")).unwrap(), FFMPEG_BYTES);
    assert_eq!(
        std::fs::read(app.file("ffprobe.exe")).unwrap(),
        FFPROBE_BYTES
    );
    assert_eq!(std::fs::read(app.file("deno.exe")).unwrap(), DENO_BYTES);
}

#[test]
fn a_prepare_while_another_prepares_waits_for_it_to_end_without_downloading_again() {
    let app = App::start();
    app.releases.hold_fetches();

    std::thread::scope(|scope| {
        let toolbox = &app.toolbox;
        scope.spawn(|| toolbox.prepare());
        app.releases.wait_until_fetch_held();
        let (ended, state_after) = std::sync::mpsc::channel();
        scope.spawn(move || {
            toolbox.prepare();
            ended.send(toolbox.state()).unwrap();
        });
        let returned_early = state_after.recv_timeout(Duration::from_millis(200));
        // Let the first prepare end before asserting, or a failure would
        // leave the scope waiting for it.
        app.releases.release_fetches();
        assert_eq!(
            returned_early.ok(),
            None,
            "the second prepare returned while the first one ran"
        );
        assert_eq!(state_after.recv().unwrap(), SetupState::Ready);
    });

    assert_eq!(app.releases.fetches().len(), 3);
}

#[test]
fn setup_reports_download_progress_up_to_the_total_before_ready() {
    let app = App::start();

    app.toolbox.prepare();

    let progress: Vec<(u64, u64)> = app
        .states()
        .into_iter()
        .filter_map(|state| match state {
            SetupState::Installing {
                downloaded, total, ..
            } if total > 0 => Some((downloaded, total)),
            _ => None,
        })
        .collect();
    let total = progress[0].1;
    assert!(
        progress
            .windows(2)
            .all(|w| w[0].0 <= w[1].0 && w[1].1 == total)
    );
    let published: u64 = [Package::YtDlp, Package::Ffmpeg, Package::Deno]
        .into_iter()
        .map(|package| app.releases.latest_release(package).unwrap().size)
        .sum();
    assert_eq!(total, published);
    assert_eq!(progress.first().unwrap().0, 0);
    assert_eq!(progress.last().unwrap().0, total);
    assert_eq!(app.states().last(), Some(&SetupState::Ready));
}

#[test]
fn first_run_shows_the_setup_screen_before_asking_github_for_releases() {
    let app = App::start();
    app.releases.set_online(false);

    app.toolbox.prepare();

    assert_eq!(
        app.states(),
        [
            SetupState::Checking,
            SetupState::Installing {
                downloaded: 0,
                total: 0,
                broken: None,
            },
            SetupState::Failed {
                error: SetupError::Offline,
                broken: None,
            },
        ]
    );
}

#[test]
fn relaunch_with_tools_in_place_is_ready_without_the_network() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.set_online(false);
    let fetched_before = app.releases.fetches().len();
    let app = app.restart();

    app.toolbox.prepare();

    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(app.fetches_after(fetched_before), Vec::<String>::new());
    // Nothing to download, so the setup screen doesn't flash.
    assert_eq!(app.states(), [SetupState::Checking, SetupState::Ready]);
}

#[test]
fn first_run_offline_fails_and_retry_online_succeeds() {
    let app = App::start();
    app.releases.set_online(false);

    app.toolbox.prepare();

    assert_eq!(
        app.toolbox.state(),
        SetupState::Failed {
            error: SetupError::Offline,
            broken: None,
        }
    );

    app.releases.set_online(true);
    app.toolbox.prepare();

    assert_eq!(app.toolbox.state(), SetupState::Ready);
}

#[test]
fn relaunch_restores_a_deleted_tool_downloading_only_its_package() {
    let app = App::start();
    app.toolbox.prepare();
    std::fs::remove_file(app.file("ffprobe.exe")).unwrap();
    let fetched_before = app.releases.fetches().len();
    let app = app.restart();

    app.toolbox.prepare();

    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(
        std::fs::read(app.file("ffprobe.exe")).unwrap(),
        FFPROBE_BYTES
    );
    assert_eq!(app.fetches_after(fetched_before), [FFMPEG_URL]);
}

#[test]
fn relaunch_restores_a_tool_whose_size_changed() {
    let app = App::start();
    app.toolbox.prepare();
    std::fs::write(app.file("yt-dlp.exe"), b"truncated").unwrap();
    let fetched_before = app.releases.fetches().len();
    let app = app.restart();

    app.toolbox.prepare();

    assert_eq!(std::fs::read(app.file("yt-dlp.exe")).unwrap(), YTDLP_BYTES);
    assert_eq!(app.fetches_after(fetched_before), [YTDLP_URL]);
}

#[test]
fn relaunch_restores_a_deleted_or_resized_deno() {
    for damage in [
        |path: &Path| std::fs::remove_file(path).unwrap(),
        |path: &Path| std::fs::write(path, b"truncated").unwrap(),
    ] {
        let app = App::start();
        app.toolbox.prepare();
        damage(&app.file("deno.exe"));
        let fetched_before = app.releases.fetches().len();
        let app = app.restart();

        app.toolbox.prepare();

        assert_eq!(app.toolbox.state(), SetupState::Ready);
        assert_eq!(std::fs::read(app.file("deno.exe")).unwrap(), DENO_BYTES);
        assert_eq!(app.fetches_after(fetched_before), [DENO_URL]);
    }
}

#[test]
fn relaunch_without_the_manifest_installs_everything_again() {
    let app = App::start();
    app.toolbox.prepare();
    std::fs::remove_file(app.file("manifest.json")).unwrap();
    let fetched_before = app.releases.fetches().len();
    let app = app.restart();

    app.toolbox.prepare();

    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(
        app.fetches_after(fetched_before),
        [YTDLP_URL, FFMPEG_URL, DENO_URL]
    );
}

#[test]
fn installing_yt_dlp_records_it_as_just_checked_for_updates() {
    let app = App::start();

    app.toolbox.prepare();

    assert_eq!(
        app.toolbox.ytdlp_checked_at(),
        Some(SystemTime::UNIX_EPOCH + NOW)
    );
}

#[test]
fn checksum_mismatch_fails_setup_and_leaves_no_unverified_file() {
    for (package, files) in [
        (Package::YtDlp, vec!["yt-dlp.exe"]),
        (Package::Ffmpeg, vec!["ffmpeg.exe", "ffprobe.exe"]),
        (Package::Deno, vec!["deno.exe"]),
    ] {
        let app = App::start();
        app.releases.tamper_checksum(package);

        app.toolbox.prepare();

        assert_eq!(
            app.toolbox.state(),
            SetupState::Failed {
                error: SetupError::ChecksumMismatch { package },
                broken: None,
            }
        );
        let left: Vec<String> = std::fs::read_dir(app.file(""))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        for file in files {
            assert!(!left.iter().any(|name| name.starts_with(file)), "{left:?}");
        }
        assert!(
            !left.iter().any(|name| name.ends_with(".download")),
            "{left:?}"
        );
    }
}

#[test]
fn setup_refuses_assets_outside_the_official_github_releases() {
    for (package, url) in [
        (
            Package::YtDlp,
            "http://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp.exe",
        ),
        (Package::YtDlp, "https://example.com/yt-dlp.exe"),
        (
            Package::YtDlp,
            "https://github.com/someone/yt-dlp/releases/download/2026.08.19/yt-dlp.exe",
        ),
        (
            Package::YtDlp,
            "https://github.com/yt-dlp/yt-dlp/releases/download/%2e%2e/%2e%2e/someone/yt-dlp.exe",
        ),
        (
            Package::Deno,
            "https://github.com/someone/deno/releases/download/v2.9.8/deno-x86_64-pc-windows-msvc.zip",
        ),
    ] {
        let app = App::start();
        app.releases.move_asset(package, url);

        app.toolbox.prepare();

        assert!(
            matches!(app.toolbox.state(), SetupState::Failed { .. }),
            "{url}"
        );
        assert!(!app.releases.fetches().iter().any(|f| f == url), "{url}");
    }
}

#[test]
fn tools_run_from_the_app_folder_and_yt_dlp_uses_the_apps_own_ffmpeg_and_deno() {
    let app = App::start();
    app.toolbox.prepare();

    app.toolbox
        .run(Tool::YtDlp, &["--version".into()], None)
        .expect("yt-dlp runs");
    app.toolbox
        .run(Tool::Ffprobe, &["-version".into()], None)
        .expect("ffprobe runs");

    let ffmpeg = app.file("ffmpeg.exe").into_os_string();
    let mut deno = OsString::from("deno:");
    deno.push(app.file("deno.exe"));
    assert_eq!(
        app.processes.runs(),
        vec![
            (
                app.file("yt-dlp.exe"),
                vec![
                    "--ignore-config".into(),
                    "--ffmpeg-location".into(),
                    ffmpeg,
                    "--no-js-runtimes".into(),
                    "--js-runtimes".into(),
                    deno,
                    "--version".into()
                ]
            ),
            (app.file("ffprobe.exe"), vec!["-version".into()]),
        ]
    );
}

#[test]
fn yt_dlp_reads_the_cookies_of_the_sign_in_browser_it_is_given() {
    let app = App::start();
    app.toolbox.prepare();

    app.toolbox
        .run(Tool::YtDlp, &["--version".into()], Some(Browser::Firefox))
        .expect("yt-dlp runs");
    app.toolbox
        .run(Tool::Ffprobe, &["-version".into()], Some(Browser::Firefox))
        .expect("ffprobe runs");

    let runs = app.processes.runs();
    let (_, ytdlp_args) = &runs[0];
    let cookies = ytdlp_args
        .iter()
        .position(|arg| arg == "--cookies-from-browser")
        .expect("--cookies-from-browser");
    assert_eq!(ytdlp_args[cookies + 1], "firefox");
    // The app's own options come before the caller's.
    assert_eq!(ytdlp_args.last().unwrap(), "--version");
    assert_eq!(runs[1].1, [OsString::from("-version")]);
}

#[test]
fn a_tool_that_fails_to_start_is_broken_and_setup_restores_it() {
    let app = App::start();
    app.toolbox.prepare();
    app.processes.break_tool("ffmpeg.exe");
    let fetched_before = app.releases.fetches().len();

    let result = app.toolbox.run(Tool::Ffmpeg, &["-version".into()], None);

    assert_eq!(result.unwrap_err(), ToolBroken(Tool::Ffmpeg));
    assert_eq!(
        app.toolbox.state(),
        SetupState::Installing {
            downloaded: 0,
            total: 0,
            broken: Some(Tool::Ffmpeg),
        }
    );

    app.toolbox.prepare();

    let states = app.states();
    let restoring = states
        .iter()
        .rev()
        .skip(1)
        .take_while(|s| **s != SetupState::Ready);
    assert!(restoring.clone().count() > 0);
    assert!(restoring.into_iter().all(|state| matches!(
        state,
        SetupState::Installing {
            broken: Some(Tool::Ffmpeg),
            ..
        }
    )));
    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(app.fetches_after(fetched_before), [FFMPEG_URL]);
}

#[test]
fn a_broken_tool_is_restored_on_the_next_launch_too() {
    let app = App::start();
    app.toolbox.prepare();
    app.processes.break_tool("yt-dlp.exe");
    let _ = app.toolbox.run(Tool::YtDlp, &[], None);
    let fetched_before = app.releases.fetches().len();
    let app = app.restart();

    app.toolbox.prepare();

    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(app.fetches_after(fetched_before), [YTDLP_URL]);
}

#[test]
fn a_deno_that_fails_to_start_is_broken_and_setup_restores_it() {
    let app = App::start();
    app.toolbox.prepare();
    app.processes.break_tool("deno.exe");
    let fetched_before = app.releases.fetches().len();

    let result = app.toolbox.run(Tool::Deno, &["--version".into()], None);

    assert_eq!(result.unwrap_err(), ToolBroken(Tool::Deno));
    app.toolbox.prepare();
    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert_eq!(app.fetches_after(fetched_before), [DENO_URL]);
}

#[test]
fn unverified_files_left_by_an_interrupted_launch_are_removed() {
    let app = App::start();
    app.toolbox.prepare();
    for name in [
        "yt-dlp.exe.download",
        "ffmpeg.zip.download",
        "deno.zip.download",
    ] {
        std::fs::write(app.file(name), b"half a download").unwrap();
    }
    let app = app.restart();

    app.toolbox.prepare();

    assert_eq!(app.toolbox.state(), SetupState::Ready);
    assert!(!app.file("yt-dlp.exe.download").exists());
    assert!(!app.file("ffmpeg.zip.download").exists());
    assert!(!app.file("deno.zip.download").exists());
}

#[test]
fn a_launch_more_than_a_day_after_the_last_check_installs_a_newer_yt_dlp() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.publish_newer_ytdlp();
    let later = NOW + 24 * HOUR + Duration::from_secs(1);
    let app = app.restart_at(later);
    app.toolbox.prepare();

    app.toolbox.update_ytdlp_if_due();

    assert_eq!(
        std::fs::read(app.file("yt-dlp.exe")).unwrap(),
        NEWER_YTDLP_BYTES
    );
    assert_eq!(
        app.toolbox.ytdlp_checked_at(),
        Some(SystemTime::UNIX_EPOCH + later)
    );
    assert_eq!(
        app.toolbox.ytdlp_version().as_deref(),
        Some(NEWER_YTDLP_VERSION)
    );
}

#[test]
fn a_launch_within_a_day_of_the_last_check_doesnt_check_for_updates() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.publish_newer_ytdlp();
    let fetched_before = app.releases.fetches().len();
    let app = app.restart_at(NOW + 24 * HOUR);
    app.toolbox.prepare();

    app.toolbox.update_ytdlp_if_due();

    assert_eq!(std::fs::read(app.file("yt-dlp.exe")).unwrap(), YTDLP_BYTES);
    assert_eq!(app.fetches_after(fetched_before), Vec::<String>::new());
    assert_eq!(
        app.toolbox.ytdlp_checked_at(),
        Some(SystemTime::UNIX_EPOCH + NOW)
    );
}

#[test]
fn a_launch_with_the_clock_set_back_before_the_last_check_checks_for_updates() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.publish_newer_ytdlp();
    let app = app.restart_at(NOW - HOUR);
    app.toolbox.prepare();

    app.toolbox.update_ytdlp_if_due();

    assert_eq!(
        std::fs::read(app.file("yt-dlp.exe")).unwrap(),
        NEWER_YTDLP_BYTES
    );
}

#[test]
fn an_update_on_request_installs_a_newer_yt_dlp_and_tells_its_version() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.publish_newer_ytdlp();

    app.toolbox.update_ytdlp_on_request();

    assert_eq!(
        app.update_states(),
        [
            YtDlpUpdateState::Updating,
            YtDlpUpdateState::Updated {
                version: NEWER_YTDLP_VERSION.into()
            },
        ]
    );
    assert_eq!(app.toolbox.ytdlp_update_state(), app.update_states()[1]);
    assert_eq!(
        std::fs::read(app.file("yt-dlp.exe")).unwrap(),
        NEWER_YTDLP_BYTES
    );
    assert_eq!(app.releases.fetches().last().unwrap(), NEWER_YTDLP_URL);
}

#[test]
fn an_update_on_request_with_the_latest_installed_downloads_nothing() {
    let app = App::start();
    app.toolbox.prepare();
    let fetched_before = app.releases.fetches().len();
    let later = NOW + HOUR;
    let app = app.restart_at(later);
    app.toolbox.prepare();

    app.toolbox.update_ytdlp_on_request();

    assert_eq!(
        app.toolbox.ytdlp_update_state(),
        YtDlpUpdateState::UpToDate {
            version: "2026.08.19".into()
        }
    );
    assert_eq!(app.fetches_after(fetched_before), Vec::<String>::new());
    assert_eq!(
        app.toolbox.ytdlp_checked_at(),
        Some(SystemTime::UNIX_EPOCH + later)
    );
}

#[test]
fn an_update_that_fails_its_checksum_keeps_the_installed_yt_dlp() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.publish_newer_ytdlp();
    app.releases.tamper_checksum(Package::YtDlp);

    app.toolbox.update_ytdlp_on_request();

    assert_eq!(
        app.toolbox.ytdlp_update_state(),
        YtDlpUpdateState::Failed {
            error: SetupError::ChecksumMismatch {
                package: Package::YtDlp
            }
        }
    );
    assert_eq!(std::fs::read(app.file("yt-dlp.exe")).unwrap(), YTDLP_BYTES);
    assert!(!app.file("yt-dlp.exe.update").exists());
    // The tools stay usable.
    assert_eq!(app.toolbox.state(), SetupState::Ready);
}

#[test]
fn an_update_offline_fails_and_leaves_the_check_to_the_next_launch() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.set_online(false);
    let app = app.restart_at(NOW + 25 * HOUR);
    app.toolbox.prepare();

    app.toolbox.update_ytdlp_on_request();

    assert_eq!(
        app.toolbox.ytdlp_update_state(),
        YtDlpUpdateState::Failed {
            error: SetupError::Offline
        }
    );
    assert_eq!(
        app.toolbox.ytdlp_checked_at(),
        Some(SystemTime::UNIX_EPOCH + NOW)
    );
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
fn an_update_replaces_yt_dlp_only_once_it_is_no_longer_in_use() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.publish_newer_ytdlp();
    let in_use = app.toolbox.use_ytdlp();

    std::thread::scope(|scope| {
        let toolbox = &app.toolbox;
        let update = scope.spawn(|| toolbox.update_ytdlp_on_request());
        wait_until(|| toolbox.ytdlp_update_state() == YtDlpUpdateState::Waiting);
        assert_eq!(std::fs::read(app.file("yt-dlp.exe")).unwrap(), YTDLP_BYTES);

        drop(in_use);
        update.join().unwrap();
    });

    assert_eq!(
        std::fs::read(app.file("yt-dlp.exe")).unwrap(),
        NEWER_YTDLP_BYTES
    );
    assert_eq!(
        app.update_states(),
        [
            YtDlpUpdateState::Updating,
            YtDlpUpdateState::Waiting,
            YtDlpUpdateState::Updated {
                version: NEWER_YTDLP_VERSION.into()
            },
        ]
    );
}

#[test]
fn a_use_of_yt_dlp_starting_while_an_update_waits_gets_the_new_yt_dlp() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.publish_newer_ytdlp();
    let in_use = app.toolbox.use_ytdlp();

    std::thread::scope(|scope| {
        let toolbox = &app.toolbox;
        let file = app.file("yt-dlp.exe");
        let update = scope.spawn(|| toolbox.update_ytdlp_on_request());
        wait_until(|| toolbox.ytdlp_update_state() == YtDlpUpdateState::Waiting);
        let (used, found) = std::sync::mpsc::channel();
        scope.spawn(move || {
            let _next = toolbox.use_ytdlp();
            used.send(std::fs::read(file).unwrap()).unwrap();
        });
        let started_early = found.recv_timeout(Duration::from_millis(200));
        drop(in_use);
        assert_eq!(started_early.ok(), None, "it didn't wait for the update");
        assert_eq!(found.recv().unwrap(), NEWER_YTDLP_BYTES);
        update.join().unwrap();
    });
}

#[test]
fn an_update_waits_for_a_running_yt_dlp_to_exit() {
    let app = App::start();
    app.toolbox.prepare();
    app.releases.publish_newer_ytdlp();
    let (started, running) = std::sync::mpsc::channel();
    let (exit, exited) = std::sync::mpsc::channel::<()>();
    let (started, exited) = (Mutex::new(started), Mutex::new(exited));
    app.processes.script("yt-dlp.exe", move |_| {
        started.lock().unwrap().send(()).unwrap();
        exited.lock().unwrap().recv().unwrap();
        Output {
            status: crate::processes::fake::exit_status(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    });

    std::thread::scope(|scope| {
        let toolbox = &app.toolbox;
        let run = scope.spawn(|| toolbox.run(Tool::YtDlp, &["--version".into()], None));
        running.recv().unwrap();
        let update = scope.spawn(|| toolbox.update_ytdlp_on_request());
        wait_until(|| toolbox.ytdlp_update_state() == YtDlpUpdateState::Waiting);
        assert_eq!(std::fs::read(app.file("yt-dlp.exe")).unwrap(), YTDLP_BYTES);

        exit.send(()).unwrap();
        run.join().unwrap().unwrap();
        update.join().unwrap();
    });

    assert_eq!(
        std::fs::read(app.file("yt-dlp.exe")).unwrap(),
        NEWER_YTDLP_BYTES
    );
}
