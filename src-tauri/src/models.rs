// Структуры, которыми обмениваются Rust и фронтенд. Зеркало — src/api.ts.
// Часть полей пока не читается: они понадобятся на этапе подключения yt-dlp.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub uploader: Option<String>,
    pub thumbnail: Option<String>,
    /// Длительность в секундах.
    pub duration: Option<f64>,
    /// Доступные высоты видео (1080, 720, ...), по убыванию.
    pub video_qualities: Vec<u32>,
    pub subtitles: Vec<SubtitleTrack>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub lang: String,
    pub name: String,
    /// true — автосгенерированные субтитры.
    pub auto: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    pub url: String,
    pub kind: MediaKind,
    pub height: Option<u32>,
    /// mp4 | mkv | webm
    pub container: String,
    /// mp3 | m4a | opus
    pub audio_format: String,
    pub out_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptFormat {
    Txt,
    Srt,
    Vtt,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptOptions {
    pub url: String,
    pub lang: String,
    pub auto: bool,
    pub format: TranscriptFormat,
    pub out_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Processing,
    Done,
    Error,
    Cancelled,
}

/// Событие "download://progress".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub percent: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub status: DownloadStatus,
}
