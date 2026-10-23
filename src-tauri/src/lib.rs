pub mod clock;
pub mod events;
pub mod probe;
pub mod processes;
pub mod releases;
pub mod settings;
pub mod toolbox;

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Builder, Manager, Runtime, State};

use crate::clock::SystemClock;
use crate::events::{AppEvent, EventSink};
use crate::probe::Prober;
use crate::processes::SystemProcesses;
use crate::releases::github::GithubReleases;
use crate::settings::{Browser, Settings};
use crate::toolbox::Toolbox;

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

impl EventSink for Subscribers {
    fn send(&self, event: AppEvent) {
        let mut channels = self.0.lock().unwrap();
        // A channel fails once its page is gone.
        channels.retain(|channel| channel.send(event.clone()).is_ok());
    }
}

#[tauri::command]
fn subscribe(
    events: Channel<AppEvent>,
    toolbox: State<'_, Arc<Toolbox>>,
    prober: State<'_, Arc<Prober>>,
    settings: State<'_, Arc<Settings>>,
    subscribers: State<'_, Arc<Subscribers>>,
) {
    subscribers.add(events, || {
        vec![
            AppEvent::Setup(toolbox.state()),
            AppEvent::Probe(prober.state()),
            AppEvent::Settings(settings.state()),
        ]
    });
}

#[tauri::command]
fn retry_setup(toolbox: State<'_, Arc<Toolbox>>) {
    spawn_prepare(toolbox.inner().clone());
}

/// Probes a URL in the background; the UI follows along through events.
#[tauri::command]
fn probe(url: String, prober: State<'_, Arc<Prober>>) {
    let prober = prober.inner().clone();
    std::thread::spawn(move || prober.probe(url.trim()));
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

/// Plugins and commands shared by the real app and tests on the mock runtime.
fn app_builder<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_info,
            subscribe,
            retry_setup,
            probe,
            set_sign_in_browser
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
            app.manage(subscribers);
            app.manage(settings);
            app.manage(toolbox.clone());
            app.manage(prober);
            spawn_prepare(toolbox);
            Ok(())
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
