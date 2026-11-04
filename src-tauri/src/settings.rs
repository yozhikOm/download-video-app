//! The app's settings, kept in the app data folder between launches.

#[cfg(test)]
mod tests;

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use crate::events::{AppEvent, EventSink};
use crate::file_name::DEFAULT_TEMPLATE;
use crate::preset::{AudioPreset, VideoPreset};

/// A browser yt-dlp can read cookies from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Brave,
    Chrome,
    Chromium,
    Edge,
    Firefox,
    Opera,
    Vivaldi,
    Whale,
}

impl Browser {
    /// The name `--cookies-from-browser` takes.
    pub fn ytdlp_name(self) -> &'static str {
        match self {
            Browser::Brave => "brave",
            Browser::Chrome => "chrome",
            Browser::Chromium => "chromium",
            Browser::Edge => "edge",
            Browser::Firefox => "firefox",
            Browser::Opera => "opera",
            Browser::Vivaldi => "vivaldi",
            Browser::Whale => "whale",
        }
    }
}

/// What the settings show.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsState {
    pub sign_in_browser: Option<Browser>,
    /// The folder a Done Download's files are moved to.
    pub save_dir: PathBuf,
    /// The name a Done Download's files are saved under, with `{title}`,
    /// `{channel}` and `{date}` put in by the app.
    pub file_name_template: String,
    /// The Video preset chosen at first after a Probe, or the nearest
    /// lower one the Video has.
    pub default_video_preset: VideoPreset,
    /// The Audio preset chosen at first after a Probe.
    pub default_audio_preset: AudioPreset,
    /// The Advanced mode is on: the Video's Formats to download one of
    /// them. Off at first.
    pub advanced_mode: bool,
}

/// What the settings file holds: what the user chose, `None` for a default.
/// A value missing from it, as in the file of an older version, or one this
/// version can't read, as from a later one, means its default alone.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Saved {
    #[serde(deserialize_with = "or_none")]
    sign_in_browser: Option<Browser>,
    /// `None` for the user's Downloads folder, wherever the system has it.
    #[serde(deserialize_with = "or_none")]
    save_dir: Option<PathBuf>,
    /// `None` for [`DEFAULT_TEMPLATE`].
    #[serde(deserialize_with = "or_none")]
    file_name_template: Option<String>,
    /// `None` for [`DEFAULT_VIDEO_PRESET`].
    #[serde(deserialize_with = "or_none")]
    default_video_preset: Option<VideoPreset>,
    /// `None` for [`DEFAULT_AUDIO_PRESET`].
    #[serde(deserialize_with = "or_none")]
    default_audio_preset: Option<AudioPreset>,
    /// `None` for off.
    #[serde(deserialize_with = "or_none")]
    advanced_mode: Option<bool>,
}

/// The Video preset chosen at first until the user chooses another.
const DEFAULT_VIDEO_PRESET: VideoPreset = VideoPreset::P1080;

/// The Audio preset chosen at first until the user chooses another.
const DEFAULT_AUDIO_PRESET: AudioPreset = AudioPreset::M4a;

/// A value that isn't a `T` as if it were missing.
fn or_none<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

/// The settings file and what it holds; every change is saved at once.
pub struct Settings {
    path: PathBuf,
    /// The user's Downloads folder, the save folder unless they chose one.
    downloads_dir: PathBuf,
    events: Arc<dyn EventSink>,
    saved: Mutex<Saved>,
    /// Held while saving a change and sending it, so that the file and the
    /// UI get the changes in the same order.
    saving: Mutex<()>,
}

impl Settings {
    /// Reads the settings file; a missing or unreadable one means the
    /// defaults. Files are saved to `downloads_dir` until the user chooses
    /// another folder.
    pub fn load(path: PathBuf, downloads_dir: PathBuf, events: Arc<dyn EventSink>) -> Self {
        let saved = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            downloads_dir,
            events,
            saved: Mutex::new(saved),
            saving: Mutex::new(()),
        }
    }

    pub fn state(&self) -> SettingsState {
        self.shown(&self.saved.lock().unwrap())
    }

    /// The browser whose YouTube sign-in yt-dlp uses, if any.
    pub fn sign_in_browser(&self) -> Option<Browser> {
        self.saved.lock().unwrap().sign_in_browser
    }

    /// The folder a Done Download's files are moved to.
    pub fn save_dir(&self) -> PathBuf {
        self.state().save_dir
    }

    /// The name a Done Download's files are saved under, before the app
    /// puts the Video's title, channel and date in.
    pub fn file_name_template(&self) -> String {
        self.state().file_name_template
    }

    /// The Video preset chosen at first after a Probe, when the Video has it.
    pub fn default_video_preset(&self) -> VideoPreset {
        self.state().default_video_preset
    }

    /// The Audio preset chosen at first after a Probe.
    pub fn default_audio_preset(&self) -> AudioPreset {
        self.state().default_audio_preset
    }

    /// The sign-in browser to ask about `url` with. Signing in is for
    /// YouTube; the user's sign-in to other sites stays in the browser.
    pub fn sign_in_browser_for(&self, url: &str) -> Option<Browser> {
        self.sign_in_browser().filter(|_| is_youtube(url))
    }

    /// Saves the choice; when it can't be saved, nothing changes.
    pub fn set_sign_in_browser(&self, browser: Option<Browser>) -> io::Result<()> {
        self.change(|saved| saved.sign_in_browser = browser)
    }

    /// Saves the folder files are saved to from now on; when it can't be
    /// saved, nothing changes.
    pub fn set_save_dir(&self, dir: PathBuf) -> io::Result<()> {
        self.change(|saved| saved.save_dir = Some(dir))
    }

    /// Saves the file name template; a blank one means the default. When it
    /// can't be saved, nothing changes.
    pub fn set_file_name_template(&self, template: String) -> io::Result<()> {
        let template = Some(template).filter(|template| !template.trim().is_empty());
        self.change(|saved| saved.file_name_template = template)
    }

    /// Saves the Video preset chosen at first after a Probe; when it can't
    /// be saved, nothing changes.
    pub fn set_default_video_preset(&self, preset: VideoPreset) -> io::Result<()> {
        self.change(|saved| saved.default_video_preset = Some(preset))
    }

    /// Saves the Audio preset chosen at first after a Probe; when it can't
    /// be saved, nothing changes.
    pub fn set_default_audio_preset(&self, preset: AudioPreset) -> io::Result<()> {
        self.change(|saved| saved.default_audio_preset = Some(preset))
    }

    /// Turns the Advanced mode on or off; when it can't be saved, nothing
    /// changes.
    pub fn set_advanced_mode(&self, on: bool) -> io::Result<()> {
        self.change(|saved| saved.advanced_mode = Some(on))
    }

    /// Saves the settings as `change` leaves them and sends them to the UI;
    /// when they can't be saved, nothing changes.
    fn change(&self, change: impl FnOnce(&mut Saved)) -> io::Result<()> {
        let _saving = self.saving.lock().unwrap();
        let mut saved = self.saved.lock().unwrap().clone();
        change(&mut saved);
        self.save(&saved)?;
        let state = self.shown(&saved);
        *self.saved.lock().unwrap() = saved;
        // Sent outside the settings' lock: a new subscriber reads the
        // state while holding the subscribers' lock.
        self.events.send(AppEvent::Settings(state));
        Ok(())
    }

    /// The settings with the defaults for what the user hasn't chosen.
    fn shown(&self, saved: &Saved) -> SettingsState {
        SettingsState {
            sign_in_browser: saved.sign_in_browser,
            save_dir: saved
                .save_dir
                .clone()
                .unwrap_or_else(|| self.downloads_dir.clone()),
            file_name_template: saved
                .file_name_template
                .clone()
                .unwrap_or_else(|| DEFAULT_TEMPLATE.into()),
            default_video_preset: saved.default_video_preset.unwrap_or(DEFAULT_VIDEO_PRESET),
            default_audio_preset: saved.default_audio_preset.unwrap_or(DEFAULT_AUDIO_PRESET),
            advanced_mode: saved.advanced_mode.unwrap_or(false),
        }
    }

    fn save(&self, saved: &Saved) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut temp = self.path.clone().into_os_string();
        temp.push(".tmp");
        let temp = PathBuf::from(temp);
        fs::write(&temp, serde_json::to_vec_pretty(saved)?)?;
        fs::rename(&temp, &self.path)
    }
}

/// The same hosts as `youtubeHosts` in `MainScreen.tsx`.
const YOUTUBE_HOSTS: [&str; 3] = ["youtube.com", "youtu.be", "youtube-nocookie.com"];

fn is_youtube(url: &str) -> bool {
    let url = url.trim();
    // yt-dlp accepts a URL without "https://".
    let parsed = if url.contains("://") {
        Url::parse(url)
    } else {
        Url::parse(&format!("https://{url}"))
    };
    let Some(host) = parsed.as_ref().ok().and_then(Url::host_str) else {
        return false;
    };
    YOUTUBE_HOSTS
        .iter()
        .any(|site| host == *site || host.ends_with(&format!(".{site}")))
}
