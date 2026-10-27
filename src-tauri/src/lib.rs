pub mod clock;
pub mod download;
pub mod events;
pub mod logs;
pub mod preset;
pub mod probe;
pub mod processes;
pub mod releases;
pub mod settings;
pub mod toolbox;
pub mod transcript;

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Builder, Manager, Runtime, State, WindowEvent};

use crate::clock::SystemClock;
use crate::download::{Added, Content, Downloader, OnClose};
use crate::events::{AppEvent, EventSink};
use crate::preset::Preset;
use crate::probe::Prober;
use crate::processes::SystemProcesses;
use crate::releases::github::GithubReleases;
use crate::settings::{Browser, Settings};
use crate::toolbox::{SetupState, Toolbox};

#[derive(Serialize)]
struct AppInfo {
    name: String,
    version: String,
}

#[tauri::command]
fn app_info<R: Runtime>(app: AppHandle<R>) -> AppInfo {
    let package = app.package_info();
    AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
    }
}

/// The UI channels that receive backend events.
#[derive(Default)]
struct Subscribers(Mutex<Vec<Channel<AppEvent>>>);

impl Subscribers {
    /// Starts the channel with the current state, read under the same lock
    /// that orders later events, so the UI misses no change.
    fn add(&self, channel: Channel<AppEvent>, current: impl FnOnce() -> Vec<AppEvent>) {
        let mut channels = self.0.lock().unwrap();
        if current()
            .into_iter()
            .all(|event| channel.send(event).is_ok())
        {
            channels.push(channel);
        }
    }
}

impl Subscribers {
    /// Sends the event to every channel; `false` when none got it.
    fn deliver(&self, event: AppEvent) -> bool {
        let mut channels = self.0.lock().unwrap();
        // A channel fails once its page is gone.
        channels.retain(|channel| channel.send(event.clone()).is_ok());
        !channels.is_empty()
    }
}

impl EventSink for Subscribers {
    fn send(&self, event: AppEvent) {
        self.deliver(event);
    }
}

#[tauri::command]
fn subscribe(
    events: Channel<AppEvent>,
    toolbox: State<'_, Arc<Toolbox>>,
    prober: State<'_, Arc<Prober>>,
    settings: State<'_, Arc<Settings>>,
    downloader: State<'_, Arc<Downloader>>,
    subscribers: State<'_, Arc<Subscribers>>,
) {
    subscribers.add(events, || {
        vec![
            AppEvent::Setup(toolbox.state()),
            AppEvent::Probe(prober.state()),
            AppEvent::Settings(settings.state()),
            AppEvent::Queue(downloader.state()),
        ]
    });
}

/// Prepares the tools in the background, then, once they are ready,
/// receives the Queued Downloads that waited for them; the UI follows along
/// through events.
#[tauri::command]
fn retry_setup(toolbox: State<'_, Arc<Toolbox>>, downloader: State<'_, Arc<Downloader>>) {
    let (toolbox, downloader) = (toolbox.inner().clone(), downloader.inner().clone());
    std::thread::spawn(move || {
        toolbox.prepare();
        // A Queued Download would only find yt-dlp missing and be dropped.
        if toolbox.state() == SetupState::Ready {
            downloader.run();
        }
    });
}

/// Probes a URL in the background; the UI follows along through events.
#[tauri::command]
fn probe(url: String, prober: State<'_, Arc<Prober>>) {
    let prober = prober.inner().clone();
    std::thread::spawn(move || prober.probe(url.trim()));
}

/// Adds a Download of the Video the latest Probe found to the Queue: its
/// media in a Video or Audio preset, its Transcript in a language, or both;
/// the Queue runs in the background and the UI follows along through
/// events. Nothing is added for a duplicate of a Queued or running
/// Download. Fails when no Probe found a Video, when neither is asked for,
/// or when the Video lacks the Preset or the language.
#[tauri::command]
fn add_download(
    preset: Option<Preset>,
    transcript: Option<String>,
    prober: State<'_, Arc<Prober>>,
    downloader: State<'_, Arc<Downloader>>,
) -> Result<Added, String> {
    let video = prober.video().ok_or("no probed Video")?;
    if let Some(preset) = preset
        && !video.has_preset(preset)
    {
        return Err(format!("the Video has no {} Preset", preset.name()));
    }
    let transcript = transcript
        .map(|language| {
            video
                .transcript_languages()
                .into_iter()
                .find(|offered| offered.language == language)
                .ok_or(format!("the Video has no Transcript in {language}"))
        })
        .transpose()?;
    let content = match (preset, transcript) {
        (Some(preset), None) => Content::Media(preset),
        (Some(preset), Some(language)) => Content::MediaWithTranscript(preset, language),
        (None, Some(language)) => Content::Transcript(language),
        (None, None) => return Err("neither a Preset nor a Transcript asked for".into()),
    };
    let added = downloader.add(&video, content);
    spawn_run(downloader.inner().clone());
    Ok(added)
}

/// Puts the Failed or Cancelled Download back in the Queue to download its
/// Video again, resuming from a Failed one's Partial; the UI follows along
/// through events.
#[tauri::command]
fn retry_download(id: u64, downloader: State<'_, Arc<Downloader>>) -> Result<Added, String> {
    let added = downloader
        .retry(id)
        .ok_or(format!("no Failed or Cancelled Download {id}"))?;
    spawn_run(downloader.inner().clone());
    Ok(added)
}

/// Cancels the Queued or Downloading Download, stopping its yt-dlp and
/// removing its Partial; the UI follows along through events.
#[tauri::command]
fn cancel_download(id: u64, downloader: State<'_, Arc<Downloader>>) -> Result<(), String> {
    if downloader.cancel(id) {
        Ok(())
    } else {
        Err(format!("no Queued or Downloading Download {id}"))
    }
}

/// Closes the app after the user confirmed it with a Download unfinished:
/// stops the Queue, keeping the Partials for the next launch, then exits.
#[tauri::command]
fn close_app<R: Runtime>(app: AppHandle<R>, downloader: State<'_, Arc<Downloader>>) {
    let downloader = downloader.inner().clone();
    // Waits for yt-dlp to exit, off the main thread.
    std::thread::spawn(move || {
        downloader.close();
        app.exit(0);
    });
}

/// Takes the Done, Failed or Cancelled Download out of the Queue.
#[tauri::command]
fn remove_download(id: u64, downloader: State<'_, Arc<Downloader>>) {
    downloader.remove(id);
}

/// Takes every Done Download out of the Queue.
#[tauri::command]
fn clear_done_downloads(downloader: State<'_, Arc<Downloader>>) {
    downloader.clear_done();
}

/// Opens the Done Download's file in its default program.
#[tauri::command]
fn open_downloaded_file(id: u64, downloader: State<'_, Arc<Downloader>>) -> Result<(), String> {
    let file = downloader.saved_file(id).ok_or("no Done Download")?;
    tauri_plugin_opener::open_path(&file, None::<&str>).map_err(|error| {
        eprintln!("can't open {}: {error}", file.display());
        error.to_string()
    })
}

/// Shows the Done Download's file selected in its folder.
#[tauri::command]
fn show_downloaded_file(id: u64, downloader: State<'_, Arc<Downloader>>) -> Result<(), String> {
    let file = downloader.saved_file(id).ok_or("no Done Download")?;
    tauri_plugin_opener::reveal_item_in_dir(&file).map_err(|error| {
        eprintln!("can't show {}: {error}", file.display());
        error.to_string()
    })
}

/// Saves the browser whose YouTube sign-in yt-dlp uses, or `None` to not
/// sign in; the change arrives as an event.
#[tauri::command]
fn set_sign_in_browser(
    browser: Option<Browser>,
    settings: State<'_, Arc<Settings>>,
) -> Result<(), String> {
    settings.set_sign_in_browser(browser).map_err(|error| {
        eprintln!("can't save the sign-in browser: {error}");
        error.to_string()
    })
}

/// Prepares the tools in the background; the UI follows along through events.
fn spawn_prepare(toolbox: Arc<Toolbox>) {
    std::thread::spawn(move || toolbox.prepare());
}

/// Receives the Queued Downloads in the background, unless another thread
/// does already.
fn spawn_run(downloader: Arc<Downloader>) {
    std::thread::spawn(move || downloader.run());
}

/// Plugins and commands shared by the real app and tests on the mock runtime.
fn app_builder<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_info,
            subscribe,
            retry_setup,
            probe,
            set_sign_in_browser,
            add_download,
            retry_download,
            cancel_download,
            close_app,
            remove_download,
            clear_done_downloads,
            open_downloaded_file,
            show_downloaded_file
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app_builder(tauri::Builder::default())
        .setup(|app| {
            let subscribers = Arc::new(Subscribers::default());
            let data_dir = app.path().app_local_data_dir()?;
            let settings = Arc::new(Settings::load(
                data_dir.join("settings.json"),
                subscribers.clone(),
            ));
            let toolbox = Arc::new(Toolbox::new(
                data_dir.join("tools"),
                Arc::new(GithubReleases::new()),
                Arc::new(SystemProcesses),
                Arc::new(SystemClock),
                subscribers.clone(),
            ));
            let prober = Arc::new(Prober::new(
                toolbox.clone(),
                settings.clone(),
                subscribers.clone(),
            ));
            let save_dir = app
                .path()
                .download_dir()
                .or_else(|_| app.path().home_dir().map(|home| home.join("Downloads")))
                .unwrap_or_else(|_| data_dir.join("downloads"));
            let downloader = Arc::new(Downloader::new(
                toolbox.clone(),
                settings.clone(),
                data_dir.join("partials"),
                save_dir,
                data_dir.join("logs"),
                Arc::new(SystemClock),
                subscribers.clone(),
            ));
            // Before the UI can add a Download that would resume from one.
            downloader.discard_old_partials();
            app.manage(subscribers);
            app.manage(settings);
            app.manage(toolbox.clone());
            app.manage(prober);
            app.manage(downloader);
            spawn_prepare(toolbox);
            Ok(())
        })
        .on_window_event(|window, event| {
            // With a Download unfinished, the UI asks first; `close_app`
            // closes the app once the user confirms. Without a page to ask
            // in, the window closes, so that it can't get stuck open.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let keep_open = match window.state::<Arc<Downloader>>().on_close() {
                    OnClose::Close => false,
                    OnClose::Ask => window
                        .state::<Arc<Subscribers>>()
                        .deliver(AppEvent::CloseRequested),
                    // `close_app` exits once the Queue has stopped; closing
                    // the window before would cut a file's move short.
                    OnClose::Wait => true,
                };
                if keep_open {
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::WebviewWindowBuilder;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;

    #[test]
    fn app_starts_on_mock_runtime_and_answers_app_info() {
        let app = app_builder(mock_builder())
            .build(tauri::generate_context!(test = true))
            .expect("app should build on mock runtime");
        // Config windows are created by the event loop, which the mock app doesn't run.
        let webview = WebviewWindowBuilder::from_config(&app, &app.config().app.windows[0])
            .unwrap()
            .build()
            .expect("main window from tauri.conf.json");

        let response = get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: "app_info".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("app_info should succeed");

        let package_json: serde_json::Value =
            serde_json::from_str(include_str!("../../package.json")).unwrap();
        assert_eq!(
            response.deserialize::<serde_json::Value>().unwrap(),
            serde_json::json!({
                "name": "download-video-app",
                "version": package_json["version"],
            })
        );
    }
}
