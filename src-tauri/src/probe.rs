//! Probing a URL: asking yt-dlp what the Video offers before anything is
//! downloaded, and keeping the answer for the Download.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::process::Output;
use std::sync::{Arc, Mutex};

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::events::{AppEvent, EventSink};
use crate::settings::{Browser, Settings};
use crate::toolbox::{Tool, Toolbox};

/// What the main screen shows about the current Probe.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ProbeState {
    Idle,
    Probing,
    Probed {
        video: VideoCard,
    },
    #[serde(rename_all = "camelCase")]
    Failed {
        error: ProbeError,
        /// The sign-in browser the Probe ran with, which the user may have
        /// changed since.
        sign_in_browser: Option<Browser>,
    },
}

/// Why a URL can't be downloaded; the UI shows its own text for each.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProbeError {
    /// A playlist or channel, not a Video.
    Playlist,
    /// A stream that is live right now; it can be downloaded once it ends.
    Live,
    /// Age-restricted, private or members-only, and no sign-in browser is
    /// chosen or its account has no access.
    SignInRequired,
    /// YouTube asks to prove the user isn't a bot; it's about the network,
    /// not the Video. Signing in through a browser gets past it.
    BotCheck,
    /// yt-dlp couldn't read the sign-in browser's cookies.
    CookiesUnreadable { reason: CookiesError },
    /// The URL lost part of the Video's id, likely when it was copied.
    TruncatedUrl,
    /// The site isn't supported or there is no Video at the URL.
    Unsupported,
    /// The message is for the log; the UI shows a generic text.
    Other { message: String },
}

/// Why yt-dlp couldn't read the sign-in browser's cookies.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CookiesError {
    /// The browser isn't installed or has no cookies yet.
    BrowserNotFound,
    /// The browser keeps its cookies locked while it runs.
    BrowserOpen,
    /// The browser encrypts its cookies so that other programs can't read
    /// them; on Windows, recent Chrome, Edge and Brave do.
    CantDecrypt,
}

/// What the user sees of a probed Video to confirm it's the right one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VideoCard {
    pub title: String,
    pub channel: Option<String>,
    pub thumbnail: Option<String>,
    /// Seconds.
    pub duration: Option<f64>,
}

/// Everything a Probe learned about a Video.
#[derive(Clone, Debug, PartialEq)]
pub struct Video {
    /// The Video's own page, without the playlist a URL may have named.
    pub url: String,
    pub title: String,
    pub channel: Option<String>,
    pub thumbnail: Option<String>,
    /// Seconds.
    pub duration: Option<f64>,
    pub formats: Vec<Format>,
    pub transcripts: TranscriptLanguages,
    /// The language spoken in the Video, when the site tells.
    pub original_language: Option<String>,
    pub chapters: Vec<Chapter>,
    /// The Video is the recording of a finished stream.
    pub was_live: bool,
}

/// A raw stream variant as yt-dlp reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Format {
    pub id: String,
    pub ext: String,
    /// `none` for a Format without video; `None` when the site doesn't tell.
    pub video_codec: Option<String>,
    /// `none` for a Format without audio; `None` when the site doesn't tell.
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    /// Kbit/s.
    pub bitrate: Option<f64>,
    /// Bytes, exact or estimated.
    pub size: Option<u64>,
    pub note: Option<String>,
    pub language: Option<String>,
}

/// The languages a Transcript can be saved in, by the source of the text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TranscriptLanguages {
    /// Subtitles the author added.
    pub manual: Vec<String>,
    /// Captions the site generated, including machine translations.
    pub automatic: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    /// Seconds from the start.
    pub start: f64,
    pub end: f64,
    pub title: String,
}

impl Video {
    pub fn card(&self) -> VideoCard {
        VideoCard {
            title: self.title.clone(),
            channel: self.channel.clone(),
            thumbnail: self.thumbnail.clone(),
            duration: self.duration,
        }
    }
}

/// Probes URLs with yt-dlp; only the latest Probe counts.
pub struct Prober {
    toolbox: Arc<Toolbox>,
    settings: Arc<Settings>,
    events: Arc<dyn EventSink>,
    current: Mutex<Current>,
    /// Held while changing the state and sending it, so that the UI gets
    /// the changes in the same order.
    sending: Mutex<()>,
}

struct Current {
    /// Counts Probes, so that a late answer to an older one is dropped.
    generation: u64,
    state: ProbeState,
    video: Option<Video>,
}

impl Prober {
    pub fn new(toolbox: Arc<Toolbox>, settings: Arc<Settings>, events: Arc<dyn EventSink>) -> Self {
        Self {
            toolbox,
            settings,
            events,
            current: Mutex::new(Current {
                generation: 0,
                state: ProbeState::Idle,
                video: None,
            }),
            sending: Mutex::new(()),
        }
    }

    pub fn state(&self) -> ProbeState {
        self.current.lock().unwrap().state.clone()
    }

    /// The Video the latest Probe found, if it found one.
    pub fn video(&self) -> Option<Video> {
        self.current.lock().unwrap().video.clone()
    }

    /// Probes `url`, replacing whatever the previous Probe found. Blocks
    /// until yt-dlp answers.
    pub fn probe(&self, url: &str) {
        let generation = self.start();
        let sign_in_browser = self.settings.sign_in_browser();
        let args: Vec<OsString> = ["-J", "--no-playlist", "--flat-playlist", "--", url]
            .into_iter()
            .map(Into::into)
            .collect();
        match self.toolbox.run(Tool::YtDlp, &args, sign_in_browser) {
            Ok(output) => match read_answer(&output, url) {
                Ok(video) => self.finish(
                    generation,
                    ProbeState::Probed {
                        video: video.card(),
                    },
                    Some(video),
                ),
                Err(error) => {
                    if let ProbeError::Other { message } = &error {
                        eprintln!("can't probe {url}: {message}");
                    }
                    let state = ProbeState::Failed {
                        error,
                        sign_in_browser,
                    };
                    self.finish(generation, state, None);
                }
            },
            // The setup screen is up now; once yt-dlp is restored, the user
            // pastes the URL again.
            Err(_) => {
                self.finish(generation, ProbeState::Idle, None);
                self.toolbox.prepare();
            }
        }
    }

    fn start(&self) -> u64 {
        let _sending = self.sending.lock().unwrap();
        let generation = {
            let mut current = self.current.lock().unwrap();
            current.generation += 1;
            current.state = ProbeState::Probing;
            current.video = None;
            current.generation
        };
        self.events.send(AppEvent::Probe(ProbeState::Probing));
        generation
    }

    /// Ends the Probe unless a newer one has started since.
    fn finish(&self, generation: u64, state: ProbeState, video: Option<Video>) {
        let _sending = self.sending.lock().unwrap();
        {
            let mut current = self.current.lock().unwrap();
            if current.generation != generation {
                return;
            }
            current.state = state.clone();
            current.video = video;
        }
        // Sent outside the state lock: a new subscriber reads the state
        // while holding the subscribers' lock.
        self.events.send(AppEvent::Probe(state));
    }
}

/// The part of yt-dlp's `-J` answer the app uses.
#[derive(Deserialize)]
struct Info {
    #[serde(rename = "_type")]
    kind: Option<String>,
    webpage_url: Option<String>,
    title: Option<String>,
    channel: Option<String>,
    uploader: Option<String>,
    thumbnail: Option<String>,
    duration: Option<f64>,
    formats: Option<Vec<InfoFormat>>,
    subtitles: Option<BTreeMap<String, IgnoredAny>>,
    automatic_captions: Option<BTreeMap<String, IgnoredAny>>,
    language: Option<String>,
    chapters: Option<Vec<InfoChapter>>,
    live_status: Option<String>,
    is_live: Option<bool>,
    was_live: Option<bool>,
}

#[derive(Deserialize)]
struct InfoFormat {
    format_id: String,
    ext: Option<String>,
    vcodec: Option<String>,
    acodec: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<f64>,
    tbr: Option<f64>,
    filesize: Option<f64>,
    filesize_approx: Option<f64>,
    format_note: Option<String>,
    language: Option<String>,
}

#[derive(Deserialize)]
struct InfoChapter {
    start_time: f64,
    end_time: f64,
    title: Option<String>,
}

impl Info {
    fn into_video(self, url: &str) -> Video {
        // yt-dlp lists a stream's chat replay among the subtitles.
        let languages = |captions: Option<BTreeMap<String, IgnoredAny>>| {
            captions
                .unwrap_or_default()
                .into_keys()
                .filter(|language| language != "live_chat")
                .collect()
        };
        Video {
            url: self.webpage_url.unwrap_or_else(|| url.into()),
            title: self.title.unwrap_or_default(),
            channel: self.channel.or(self.uploader),
            thumbnail: self.thumbnail,
            duration: self.duration,
            formats: self
                .formats
                .unwrap_or_default()
                .into_iter()
                .filter(|f| !(is_none(&f.vcodec) && is_none(&f.acodec)))
                .map(InfoFormat::into_format)
                .collect(),
            transcripts: TranscriptLanguages {
                manual: languages(self.subtitles),
                automatic: languages(self.automatic_captions),
            },
            original_language: self.language,
            chapters: self
                .chapters
                .unwrap_or_default()
                .into_iter()
                .map(|chapter| Chapter {
                    start: chapter.start_time,
                    end: chapter.end_time,
                    title: chapter.title.unwrap_or_default(),
                })
                .collect(),
            was_live: self.was_live == Some(true)
                || matches!(self.live_status.as_deref(), Some("was_live" | "post_live")),
        }
    }
}

impl InfoFormat {
    fn into_format(self) -> Format {
        Format {
            id: self.format_id,
            ext: self.ext.unwrap_or_default(),
            video_codec: self.vcodec,
            audio_codec: self.acodec,
            width: self.width,
            height: self.height,
            fps: self.fps,
            bitrate: self.tbr,
            size: self
                .filesize
                .or(self.filesize_approx)
                .map(|size| size as u64),
            note: self.format_note,
            language: self.language,
        }
    }
}

/// A codec of `none` means the Format lacks that kind of stream; storyboards
/// lack both.
fn is_none(codec: &Option<String>) -> bool {
    codec.as_deref() == Some("none")
}

fn read_answer(output: &Output, url: &str) -> Result<Video, ProbeError> {
    if !output.status.success() {
        return Err(read_error(&String::from_utf8_lossy(&output.stderr)));
    }
    let info: Info = serde_json::from_slice(&output.stdout).map_err(|error| ProbeError::Other {
        message: format!("unreadable yt-dlp answer: {error}"),
    })?;
    if info.kind.as_deref() == Some("playlist") {
        return Err(ProbeError::Playlist);
    }
    if info.is_live == Some(true) || info.live_status.as_deref() == Some("is_live") {
        return Err(ProbeError::Live);
    }
    Ok(info.into_video(url))
}

/// Tells why yt-dlp failed from its error message.
fn read_error(stderr: &str) -> ProbeError {
    let message = stderr
        .lines()
        .rfind(|line| line.starts_with("ERROR:"))
        .unwrap_or(stderr.trim())
        .to_string();
    let lower = message.to_lowercase();
    let says = |phrases: &[&str]| phrases.iter().any(|phrase| lower.contains(phrase));
    let cookies_unreadable = |reason| ProbeError::CookiesUnreadable { reason };
    // YouTube's bot check asks to sign in too, so it's told apart before a
    // Video that needs signing in.
    if says(&["could not find"]) && says(&["cookies database"]) {
        cookies_unreadable(CookiesError::BrowserNotFound)
    } else if says(&["could not copy"]) && says(&["cookie database"]) {
        cookies_unreadable(CookiesError::BrowserOpen)
    } else if says(&["failed to decrypt with dpapi"]) {
        cookies_unreadable(CookiesError::CantDecrypt)
    } else if says(&["not a bot"]) {
        ProbeError::BotCheck
    } else if says(&[
        "sign in",
        "--cookies",
        "age-restricted",
        "members-only",
        "members on level",
        "private video",
    ]) {
        ProbeError::SignInRequired
    } else if says(&["incomplete youtube id", "looks truncated"]) {
        ProbeError::TruncatedUrl
    } else if says(&[
        "unsupported url",
        "is not a valid url",
        "video unavailable",
        "video is unavailable",
    ]) {
        ProbeError::Unsupported
    } else {
        ProbeError::Other { message }
    }
}
