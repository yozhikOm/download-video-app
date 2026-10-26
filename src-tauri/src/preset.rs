//! Presets: the user-facing choices of what to save, built on top of a
//! Video's Formats.

use serde::{Deserialize, Serialize};

use crate::probe::Format;

/// A Preset of either group, as the UI names it when it asks for a
/// Download.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Preset {
    Video(VideoPreset),
    Audio(AudioPreset),
}

impl Preset {
    /// The name of this Preset in the UI and in a Partial's name, such as
    /// `1080p` or `mp3`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Video(preset) => preset.name(),
            Self::Audio(preset) => preset.name(),
        }
    }
}

impl From<VideoPreset> for Preset {
    fn from(preset: VideoPreset) -> Self {
        Self::Video(preset)
    }
}

impl From<AudioPreset> for Preset {
    fn from(preset: AudioPreset) -> Self {
        Self::Audio(preset)
    }
}

/// A Video preset: the quality of the saved MP4. The UI knows it by its
/// [`name`](VideoPreset::name).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "&'static str", try_from = "String")]
pub enum VideoPreset {
    /// The highest resolution the Video has, offered only above 2160p or
    /// when the Video has no other Preset.
    Best,
    P2160,
    P1440,
    P1080,
    P720,
    P480,
    P360,
}

/// The Video presets with a resolution, highest first, and their
/// resolutions as yt-dlp counts them: the smaller side of the picture.
const RESOLUTIONS: [(VideoPreset, u32); 6] = [
    (VideoPreset::P2160, 2160),
    (VideoPreset::P1440, 1440),
    (VideoPreset::P1080, 1080),
    (VideoPreset::P720, 720),
    (VideoPreset::P480, 480),
    (VideoPreset::P360, 360),
];

/// The resolution of the Video preset chosen at first, when the Video has
/// it.
const PREFERRED_RESOLUTION: u32 = 1080;

impl VideoPreset {
    /// The highest resolution this Preset saves, as yt-dlp counts it: the
    /// smaller side of the picture. `None` for the Video's best.
    pub fn resolution(self) -> Option<u32> {
        RESOLUTIONS
            .into_iter()
            .find(|(preset, _)| *preset == self)
            .map(|(_, resolution)| resolution)
    }

    /// The name of this Preset in the UI and in a Partial's name, such as
    /// `1080p`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::P2160 => "2160p",
            Self::P1440 => "1440p",
            Self::P1080 => "1080p",
            Self::P720 => "720p",
            Self::P480 => "480p",
            Self::P360 => "360p",
        }
    }
}

impl From<VideoPreset> for &'static str {
    fn from(preset: VideoPreset) -> Self {
        preset.name()
    }
}

impl TryFrom<String> for VideoPreset {
    type Error = String;

    fn try_from(name: String) -> Result<Self, String> {
        [VideoPreset::Best]
            .into_iter()
            .chain(RESOLUTIONS.map(|(preset, _)| preset))
            .find(|preset| preset.name() == name)
            .ok_or_else(|| format!("no Video preset is named {name}"))
    }
}

/// The Video presets a Video with these Formats has: each resolution that
/// some video Format falls under, highest first, after the Video's best
/// when it saves more than the highest of them or there is none. A Format
/// falls under the Preset whose resolution it reaches without reaching the
/// next one up, so that a widescreen 1920×800 counts as 1080p, and so does
/// a vertical 1080×1920. 360p takes all below it. Only video-only Formats
/// count when the Video has any, since a Download takes one of them
/// (`bv`) rather than a Format with video and sound together.
pub fn video_presets(formats: &[Format]) -> Vec<VideoPreset> {
    let without_sound: Vec<u32> = formats
        .iter()
        .filter(|format| !format.has_sound())
        .filter_map(Format::resolution)
        .collect();
    let resolutions = if without_sound.is_empty() {
        formats.iter().filter_map(Format::resolution).collect()
    } else {
        without_sound
    };
    let lower_bounds = RESOLUTIONS
        .iter()
        .skip(1)
        .map(|(_, resolution)| *resolution)
        .chain([0]);
    let with_resolution: Vec<VideoPreset> = RESOLUTIONS
        .into_iter()
        .zip(lower_bounds)
        .filter(|((_, highest), below)| {
            resolutions
                .iter()
                .any(|resolution| below < resolution && resolution <= highest)
        })
        .map(|((preset, _), _)| preset)
        .collect();
    // Otherwise the highest of them saves the same as the Video's best.
    let (_, highest) = RESOLUTIONS[0];
    let best_saves_more =
        with_resolution.is_empty() || resolutions.iter().any(|resolution| *resolution > highest);
    let best = best_saves_more.then_some(VideoPreset::Best);
    best.into_iter().chain(with_resolution).collect()
}

/// The Video preset chosen at first among `presets`, as [`video_presets`]
/// orders them: 1080p, or the nearest lower one, or the nearest higher one
/// when there is none lower, or the Video's best when there is no other.
pub fn default_video_preset(presets: &[VideoPreset]) -> VideoPreset {
    let with_resolution = || {
        presets
            .iter()
            .filter_map(|preset| Some((*preset, preset.resolution()?)))
    };
    with_resolution()
        .find(|(_, resolution)| *resolution <= PREFERRED_RESOLUTION)
        .or_else(|| with_resolution().next_back())
        .map_or(VideoPreset::Best, |(preset, _)| preset)
}

/// An Audio preset: the format of the saved sound, without the picture. The
/// UI knows it by its [`name`](AudioPreset::name).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "&'static str", try_from = "String")]
pub enum AudioPreset {
    /// The site's AAC sound as it is, without encoding it again.
    M4a,
    /// The sound encoded as a high-quality VBR MP3.
    Mp3,
}

/// The Audio presets in the order the UI offers them.
const AUDIO_PRESETS: [AudioPreset; 2] = [AudioPreset::M4a, AudioPreset::Mp3];

impl AudioPreset {
    /// The name of this Preset in the UI and in a Partial's name, and the
    /// extension of the saved file.
    pub fn name(self) -> &'static str {
        match self {
            Self::M4a => "m4a",
            Self::Mp3 => "mp3",
        }
    }
}

impl From<AudioPreset> for &'static str {
    fn from(preset: AudioPreset) -> Self {
        preset.name()
    }
}

impl TryFrom<String> for AudioPreset {
    type Error = String;

    fn try_from(name: String) -> Result<Self, String> {
        AUDIO_PRESETS
            .into_iter()
            .find(|preset| preset.name() == name)
            .ok_or_else(|| format!("no Audio preset is named {name}"))
    }
}

/// The Audio presets a Video with these Formats has: all of them when some
/// Format has sound or doesn't tell, none when every Format is without
/// sound.
pub fn audio_presets(formats: &[Format]) -> Vec<AudioPreset> {
    if formats.iter().any(Format::has_sound) {
        AUDIO_PRESETS.to_vec()
    } else {
        Vec::new()
    }
}
