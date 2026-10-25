//! Presets: the user-facing choices of what to save, built on top of a
//! Video's Formats.

use serde::{Deserialize, Serialize};

use crate::probe::Format;

/// A Video preset: the quality of the saved MP4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoPreset {
    /// The highest resolution the Video has, offered only above 2160p or
    /// when the Video has no other Preset.
    #[serde(rename = "best")]
    Best,
    #[serde(rename = "2160p")]
    P2160,
    #[serde(rename = "1440p")]
    P1440,
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "480p")]
    P480,
    #[serde(rename = "360p")]
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

    /// The name of this Preset in a Partial's name, such as `1080p`.
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

/// The Video presets a Video with these Formats has: each resolution that
/// some video Format falls under, highest first, after the Video's best
/// when it saves more than the highest of them or there is none. A Format
/// falls under the Preset whose resolution it reaches without reaching the
/// next one up, so that a widescreen 1920×800 counts as 1080p, and so does
/// a vertical 1080×1920. 360p takes all below it.
pub fn video_presets(formats: &[Format]) -> Vec<VideoPreset> {
    let resolutions: Vec<u32> = formats.iter().filter_map(Format::resolution).collect();
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
