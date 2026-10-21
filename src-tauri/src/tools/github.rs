//! [`Tools`](super::Tools) backed by the official GitHub releases.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use super::{Package, Release, Tools, ToolsError};

const USER_AGENT: &str = concat!("download-video-app/", env!("CARGO_PKG_VERSION"));
/// A fetch that receives nothing for this long counts as a lost connection.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);

pub struct GithubTools {
    agent: ureq::Agent,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    updated_at: String,
}

struct Source {
    /// GitHub API URL of the release to install.
    api_url: &'static str,
    asset: &'static str,
    checksums: &'static str,
}

impl GithubTools {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .user_agent(USER_AGENT)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .build();
        Self {
            agent: config.into(),
        }
    }

    fn get(&self, url: &str) -> Result<ureq::http::Response<ureq::Body>, ToolsError> {
        self.agent.get(url).call().map_err(tools_error)
    }
}

impl Default for GithubTools {
    fn default() -> Self {
        Self::new()
    }
}

impl Tools for GithubTools {
    fn latest_release(&self, package: Package) -> Result<Release, ToolsError> {
        let source = source(package);
        let release: GithubRelease = self
            .get(source.api_url)?
            .body_mut()
            .read_json()
            .map_err(tools_error)?;
        let find = |name: &str| {
            let asset = release.assets.iter().find(|asset| asset.name == name);
            asset.ok_or_else(|| ToolsError::Other(format!("the release has no {name}")))
        };
        let asset = find(source.asset)?;
        let checksums_url = &find(source.checksums)?.browser_download_url;
        if !package.is_official(checksums_url) {
            return Err(ToolsError::Other(format!(
                "unofficial checksums {checksums_url}"
            )));
        }
        let sums = self
            .get(checksums_url)?
            .body_mut()
            .read_to_string()
            .map_err(tools_error)?;
        let sha256 = checksum_for(&sums, source.asset).ok_or_else(|| {
            ToolsError::Other(format!(
                "{} has no checksum for {}",
                source.checksums, source.asset
            ))
        })?;
        let version = match package {
            Package::YtDlp => release.tag_name,
            // FFmpeg-Builds rebuilds its rolling "latest" release in place.
            Package::Ffmpeg => asset.updated_at.clone(),
        };
        Ok(Release {
            version,
            url: asset.browser_download_url.clone(),
            size: asset.size,
            sha256,
        })
    }

    fn fetch(
        &self,
        url: &str,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), ToolsError> {
        let response = self.get(url)?;
        let mut body = response
            .into_body()
            .into_with_config()
            .limit(u64::MAX)
            .reader();
        // ureq only limits the whole body's time, so reads happen on a thread
        // that this one stops waiting for once the connection stalls.
        let (sender, chunks) = mpsc::sync_channel::<io::Result<Vec<u8>>>(4);
        thread::spawn(move || {
            let mut buffer = vec![0; 64 * 1024];
            loop {
                let chunk = body.read(&mut buffer).map(|read| buffer[..read].to_vec());
                let last = !matches!(&chunk, Ok(bytes) if !bytes.is_empty());
                if sender.send(chunk).is_err() || last {
                    return;
                }
            }
        });
        let mut received = 0;
        loop {
            let chunk = chunks
                .recv_timeout(STALL_TIMEOUT)
                .map_err(|_| ToolsError::Offline)?
                .map_err(connection_lost)?;
            if chunk.is_empty() {
                return Ok(());
            }
            out.write_all(&chunk)
                .map_err(|e| ToolsError::Other(e.to_string()))?;
            received += chunk.len() as u64;
            progress(received);
        }
    }

    fn run(&self, program: &Path, args: &[OsString]) -> io::Result<Output> {
        let mut command = Command::new(program);
        command.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.output()
    }
}

fn source(package: Package) -> Source {
    match package {
        Package::YtDlp => Source {
            api_url: "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest",
            asset: "yt-dlp.exe",
            checksums: "SHA2-256SUMS",
        },
        Package::Ffmpeg => Source {
            api_url: "https://api.github.com/repos/yt-dlp/FFmpeg-Builds/releases/tags/latest",
            asset: "ffmpeg-master-latest-win64-gpl.zip",
            checksums: "checksums.sha256",
        },
    }
}

fn tools_error(error: ureq::Error) -> ToolsError {
    match error {
        ureq::Error::Io(error) => connection_lost(error),
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed | ureq::Error::Timeout(_) => {
            ToolsError::Offline
        }
        error => ToolsError::Other(error.to_string()),
    }
}

/// A network read that fails means the connection dropped.
fn connection_lost(_: io::Error) -> ToolsError {
    ToolsError::Offline
}

/// Finds a file's hex SHA-256 in a `sha256sum`-style list, as published by
/// yt-dlp (`SHA2-256SUMS`) and FFmpeg-Builds (`checksums.sha256`).
fn checksum_for(sums: &str, file_name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (sum, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim_start().trim_start_matches('*');
        let is_sha256 = sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit());
        (name == file_name && is_sha256).then(|| sum.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_checksum_of_yt_dlp_exe_in_sha2_256sums() {
        let sums = "\
1fa6733c37ea6fb51c99ad8fe785e7b7e5f3246c9b980230329d4fb72ed8d4d6  yt-dlp
66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a  yt-dlp.exe
05b438997bafc3affdfda9d041353c9d73e04dc842207254b655b0887c4445b0  yt-dlp_arm64.exe
";
        assert_eq!(
            checksum_for(sums, "yt-dlp.exe").as_deref(),
            Some("66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a")
        );
    }

    #[test]
    fn finds_the_checksum_of_the_win64_zip_in_ffmpeg_builds_checksums() {
        let sums = "\
8f3b7dfd3c0a8d2f3bf7aedb2b0d95846a49e4c830fc1a5bf93d606b223e6aea  ffmpeg-master-latest-win64-gpl-shared.zip\r
75104ceafcb51a7982b248df5113d3fbb449d1c32425f1edf184d5ce391c3b9b  ffmpeg-master-latest-win64-gpl.zip\r
";
        assert_eq!(
            checksum_for(sums, "ffmpeg-master-latest-win64-gpl.zip").as_deref(),
            Some("75104ceafcb51a7982b248df5113d3fbb449d1c32425f1edf184d5ce391c3b9b")
        );
    }

    #[test]
    fn a_file_missing_from_the_list_has_no_checksum() {
        let sums = "66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a  yt-dlp.exe\n";
        assert_eq!(checksum_for(sums, "yt-dlp_x86.exe"), None);
        assert_eq!(checksum_for("garbage  yt-dlp.exe\n", "yt-dlp.exe"), None);
    }

    /// Installs the real tools from GitHub (~220 MB):
    /// `cargo test --lib -- --ignored installs_the_real_tools`.
    #[test]
    #[ignore = "downloads the real tools from GitHub"]
    fn installs_the_real_tools_from_github() {
        use std::sync::Arc;

        use crate::clock::SystemClock;
        use crate::events::RecordingSink;
        use crate::toolbox::{SetupState, Tool, Toolbox};

        let dir = tempfile::TempDir::new().unwrap();
        let toolbox = Toolbox::new(
            dir.path().to_path_buf(),
            Arc::new(GithubTools::new()),
            Arc::new(SystemClock),
            Arc::new(RecordingSink::default()),
        );

        toolbox.prepare();

        assert_eq!(toolbox.state(), SetupState::Ready);
        let output = toolbox.run(Tool::YtDlp, &["--version".into()]).unwrap();
        assert!(output.status.success(), "{output:?}");
        let output = toolbox.run(Tool::Ffprobe, &["-version".into()]).unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("ffprobe version"));
    }
}
