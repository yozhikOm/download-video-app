// Команды, доступные фронтенду через invoke().
// Пока это заглушки: реальная работа с yt-dlp появится на следующем этапе.

use crate::models::{DownloadOptions, SubtitleTrack, TranscriptOptions, VideoInfo};

const NOT_IMPLEMENTED: &str = "Пока не реализовано";

#[tauri::command]
pub async fn fetch_info(url: String) -> Result<VideoInfo, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Вставьте ссылку на видео".into());
    }

    // Заглушка вместо `yt-dlp -J`.
    Ok(VideoInfo {
        id: "demo".into(),
        url: url.into(),
        title: "Демо-видео (данные-заглушка)".into(),
        uploader: Some("Demo Channel".into()),
        thumbnail: None,
        duration: Some(754.0),
        video_qualities: vec![2160, 1440, 1080, 720, 480, 360],
        subtitles: vec![
            SubtitleTrack { lang: "ru".into(), name: "Русский".into(), auto: false },
            SubtitleTrack { lang: "en".into(), name: "English".into(), auto: true },
        ],
    })
}

#[tauri::command]
pub async fn start_download(options: DownloadOptions) -> Result<String, String> {
    let _ = options;
    Err(NOT_IMPLEMENTED.into())
}

#[tauri::command]
pub async fn cancel_download(id: String) -> Result<(), String> {
    let _ = id;
    Err(NOT_IMPLEMENTED.into())
}

#[tauri::command]
pub async fn download_transcript(options: TranscriptOptions) -> Result<String, String> {
    let _ = options;
    Err(NOT_IMPLEMENTED.into())
}

#[tauri::command]
pub async fn ytdlp_version() -> Result<String, String> {
    Err("yt-dlp ещё не подключён".into())
}
