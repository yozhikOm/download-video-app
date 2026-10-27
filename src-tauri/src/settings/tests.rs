use std::sync::Arc;

use tempfile::TempDir;

use super::*;
use crate::events::{AppEvent, RecordingSink};
use crate::file_name::DEFAULT_TEMPLATE;

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
        let settings = Settings::load(
            dir.path().join("settings.json"),
            dir.path().join("Downloads"),
            events.clone(),
        );
        Self {
            dir,
            events,
            settings,
        }
    }

    /// The user's Downloads folder, as the system reports it.
    fn downloads_dir(&self) -> PathBuf {
        self.dir.path().join("Downloads")
    }

    /// The settings of an app just installed.
    fn defaults(&self) -> SettingsState {
        SettingsState {
            sign_in_browser: None,
            save_dir: self.downloads_dir(),
            file_name_template: DEFAULT_TEMPLATE.into(),
            default_video_preset: VideoPreset::P1080,
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
            sign_in_browser: Some(Browser::Edge),
            ..app.defaults()
        })]
    );
}

#[test]
fn an_unreadable_settings_file_means_the_defaults() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("settings.json"), b"{ not json").unwrap();

    let app = App::start_with(dir);

    assert_eq!(app.settings.state(), app.defaults());
}

#[test]
fn a_settings_file_of_an_older_version_keeps_its_sign_in_browser() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{ "signInBrowser": "firefox" }"#,
    )
    .unwrap();

    let app = App::start_with(dir);

    assert_eq!(
        app.settings.state(),
        SettingsState {
            sign_in_browser: Some(Browser::Firefox),
            ..app.defaults()
        }
    );
}

#[test]
fn a_value_this_version_cant_read_means_its_default_alone() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{ "signInBrowser": "firefox", "defaultVideoPreset": "4320p", "fileNameTemplate": 5 }"#,
    )
    .unwrap();

    let app = App::start_with(dir);

    assert_eq!(
        app.settings.state(),
        SettingsState {
            sign_in_browser: Some(Browser::Firefox),
            ..app.defaults()
        }
    );
}

#[test]
fn files_are_saved_to_the_downloads_folder_by_default() {
    let app = App::start();

    assert_eq!(app.settings.save_dir(), app.downloads_dir());
}

#[test]
fn the_save_folder_survives_a_relaunch() {
    let app = App::start();
    let chosen = app.dir.path().join("Videos");

    app.settings.set_save_dir(chosen.clone()).unwrap();
    let app = app.restart();

    assert_eq!(app.settings.save_dir(), chosen);
    assert_eq!(app.settings.state().save_dir, chosen);
}

#[test]
fn files_are_named_by_the_title_by_default() {
    let app = App::start();

    assert_eq!(app.settings.file_name_template(), "{title}");
}

#[test]
fn the_file_name_template_survives_a_relaunch() {
    let app = App::start();

    app.settings
        .set_file_name_template("{date} {title}".into())
        .unwrap();
    let app = app.restart();

    assert_eq!(app.settings.file_name_template(), "{date} {title}");
}

#[test]
fn a_blank_file_name_template_means_the_default() {
    let app = App::start();
    app.settings
        .set_file_name_template("{date} {title}".into())
        .unwrap();

    app.settings.set_file_name_template("  ".into()).unwrap();

    assert_eq!(app.settings.file_name_template(), "{title}");
}

#[test]
fn the_default_video_preset_is_1080p_at_first() {
    let app = App::start();

    assert_eq!(app.settings.default_video_preset(), VideoPreset::P1080);
}

#[test]
fn the_default_video_preset_survives_a_relaunch() {
    let app = App::start();

    app.settings
        .set_default_video_preset(VideoPreset::P720)
        .unwrap();
    let app = app.restart();

    assert_eq!(app.settings.default_video_preset(), VideoPreset::P720);
}

#[test]
fn each_setting_is_kept_when_another_changes() {
    let app = App::start();
    let chosen = app.dir.path().join("Videos");
    app.settings
        .set_sign_in_browser(Some(Browser::Firefox))
        .unwrap();
    app.settings.set_save_dir(chosen.clone()).unwrap();
    app.settings
        .set_file_name_template("{channel} - {title}".into())
        .unwrap();

    app.settings
        .set_default_video_preset(VideoPreset::P480)
        .unwrap();
    let app = app.restart();

    assert_eq!(
        app.settings.state(),
        SettingsState {
            sign_in_browser: Some(Browser::Firefox),
            save_dir: chosen,
            file_name_template: "{channel} - {title}".into(),
            default_video_preset: VideoPreset::P480,
        }
    );
}

#[test]
fn a_choice_that_cant_be_saved_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let events = Arc::new(RecordingSink::default());
    // The settings file's folder is a file, so nothing can be saved there.
    std::fs::write(dir.path().join("data"), b"").unwrap();
    let settings = Settings::load(
        dir.path().join("data").join("settings.json"),
        dir.path().join("Downloads"),
        events.clone(),
    );

    let result = settings.set_sign_in_browser(Some(Browser::Firefox));

    assert!(result.is_err());
    assert_eq!(settings.state().sign_in_browser, None);
    assert_eq!(events.events(), []);
}
