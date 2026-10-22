//! The app's settings, kept in the app data folder between launches.

#[cfg(test)]
mod tests;

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::events::{AppEvent, EventSink};

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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SettingsState {
    pub sign_in_browser: Option<Browser>,
}

/// The settings file and what it holds; every change is saved at once.
pub struct Settings {
    path: PathBuf,
    events: Arc<dyn EventSink>,
    state: Mutex<SettingsState>,
    /// Held while saving a change and sending it, so that the file and the
    /// UI get the changes in the same order.
    saving: Mutex<()>,
}

impl Settings {
    /// Reads the settings file; a missing or unreadable one means the defaults.
    pub fn load(path: PathBuf, events: Arc<dyn EventSink>) -> Self {
        let state = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            events,
            state: Mutex::new(state),
            saving: Mutex::new(()),
        }
    }

    pub fn state(&self) -> SettingsState {
        self.state.lock().unwrap().clone()
    }

    /// The browser whose YouTube sign-in yt-dlp uses, if any.
    pub fn sign_in_browser(&self) -> Option<Browser> {
        self.state.lock().unwrap().sign_in_browser
    }

    /// Saves the choice; when it can't be saved, nothing changes.
    pub fn set_sign_in_browser(&self, browser: Option<Browser>) -> io::Result<()> {
        let _saving = self.saving.lock().unwrap();
        let state = SettingsState {
            sign_in_browser: browser,
        };
        self.save(&state)?;
        *self.state.lock().unwrap() = state.clone();
        // Sent outside the state lock: a new subscriber reads the state
        // while holding the subscribers' lock.
        self.events.send(AppEvent::Settings(state));
        Ok(())
    }

    fn save(&self, state: &SettingsState) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut temp = self.path.clone().into_os_string();
        temp.push(".tmp");
        let temp = PathBuf::from(temp);
        fs::write(&temp, serde_json::to_vec_pretty(state)?)?;
        fs::rename(&temp, &self.path)
    }
}
