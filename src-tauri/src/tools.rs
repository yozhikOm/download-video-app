//! The only seam to yt-dlp, ffmpeg and their releases: everything that reaches
//! the network or starts a tool process goes through [`Tools`].

#[cfg(test)]
pub mod fake;
pub mod github;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::Output;

use serde::Serialize;

/// A release download that provides some of the tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Package {
    /// `yt-dlp.exe` from yt-dlp/yt-dlp.
    YtDlp,
    /// The zip with `ffmpeg.exe` and `ffprobe.exe` from yt-dlp/FFmpeg-Builds.
    Ffmpeg,
}

impl Package {
    /// Where the package's official release assets live.
    pub fn releases_url(self) -> &'static str {
        match self {
            Package::YtDlp => "https://github.com/yt-dlp/yt-dlp/releases/download/",
            Package::Ffmpeg => "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/",
        }
    }

    /// Whether `url` is an asset of the package's official releases.
    pub fn is_official(self, url: &str) -> bool {
        url.strip_prefix(self.releases_url()).is_some_and(|rest| {
            !rest.contains('%') && !rest.split('/').any(|part| part == ".." || part == ".")
        })
    }
}

/// The asset of a package's latest release that the app installs.
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub size: u64,
    /// Hex SHA-256 of the asset as published with the release.
    pub sha256: String,
}

#[derive(Debug)]
pub enum ToolsError {
    /// The release server can't be reached.
    Offline,
    Other(String),
}

pub trait Tools: Send + Sync {
    fn latest_release(&self, package: Package) -> Result<Release, ToolsError>;

    /// Streams the asset at `url` into `out`, reporting the bytes received so far.
    fn fetch(
        &self,
        url: &str,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), ToolsError>;

    /// Runs `program` and waits for it to exit; errs only when it can't be started.
    fn run(&self, program: &Path, args: &[OsString]) -> io::Result<Output>;
}
