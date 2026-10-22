use std::sync::Arc;

use tempfile::TempDir;

use super::*;
use crate::events::{AppEvent, RecordingSink};

struct App {
    dir: TempDir,
    events: Arc<RecordingSink>,
    settings: Settings,
}

impl App {
    fn start() -> Self {
        Self::start_with(TempDir::new().unwrap())
    }

    /// Starts the app again on the same data folder, like a relaunch.
    fn restart(self) -> Self {
        Self::start_with(self.dir)
    }

    fn start_with(dir: TempDir) -> Self {
        let events = Arc::new(RecordingSink::default());
        let settings = Settings::load(dir.path().join("settings.json"), events.clone());
        Self {
            dir,
            events,
            settings,
        }
    }
}

#[test]
fn no_sign_in_browser_is_used_by_default() {
    let app = App::start();

    assert_eq!(app.settings.state().sign_in_browser, None);
}

#[test]
fn the_sign_in_browser_survives_a_relaunch() {
    let app = App::start();

    app.settings
        .set_sign_in_browser(Some(Browser::Firefox))
        .unwrap();
    let app = app.restart();

    assert_eq!(app.settings.state().sign_in_browser, Some(Browser::Firefox));
}

#[test]
fn choosing_not_to_sign_in_survives_a_relaunch() {
    let app = App::start();
    app.settings
        .set_sign_in_browser(Some(Browser::Firefox))
        .unwrap();

    app.settings.set_sign_in_browser(None).unwrap();
    let app = app.restart();

    assert_eq!(app.settings.state().sign_in_browser, None);
}

#[test]
fn a_change_is_sent_to_the_ui() {
    let app = App::start();

    app.settings
        .set_sign_in_browser(Some(Browser::Edge))
        .unwrap();

    assert_eq!(
        app.events.events(),
        [AppEvent::Settings(SettingsState {
            sign_in_browser: Some(Browser::Edge)
        })]
    );
}

#[test]
fn an_unreadable_settings_file_means_the_defaults() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("settings.json"), b"{ not json").unwrap();

    let app = App::start_with(dir);

    assert_eq!(app.settings.state(), SettingsState::default());
}

#[test]
fn a_choice_that_cant_be_saved_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let events = Arc::new(RecordingSink::default());
    // The settings file's folder is a file, so nothing can be saved there.
    std::fs::write(dir.path().join("data"), b"").unwrap();
    let settings = Settings::load(
        dir.path().join("data").join("settings.json"),
        events.clone(),
    );

    let result = settings.set_sign_in_browser(Some(Browser::Firefox));

    assert!(result.is_err());
    assert_eq!(settings.state().sign_in_browser, None);
    assert_eq!(events.events(), []);
}
