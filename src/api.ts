// Типы и обёртки над Rust-командами. Зеркало src-tauri/src/models.rs.
import { invoke } from "@tauri-apps/api/core";

export interface SubtitleTrack {
  lang: string;
  name: string;
  auto: boolean;
}

export interface VideoInfo {
  id: string;
  url: string;
  title: string;
  uploader: string | null;
  thumbnail: string | null;
  /** Секунды. */
  duration: number | null;
  videoQualities: number[];
  subtitles: SubtitleTrack[];
}

export type MediaKind = "video" | "audio";
export type VideoContainer = "mp4" | "mkv" | "webm";
export type AudioFormat = "mp3" | "m4a" | "opus";

export interface DownloadOptions {
  url: string;
  kind: MediaKind;
  height: number | null;
  container: VideoContainer;
  audioFormat: AudioFormat;
  outDir: string;
}

export type TranscriptFormat = "txt" | "srt" | "vtt";

export interface TranscriptOptions {
  url: string;
  lang: string;
  auto: boolean;
  format: TranscriptFormat;
  outDir: string;
}

export type DownloadStatus =
  | "queued"
  | "downloading"
  | "processing"
  | "done"
  | "error"
  | "cancelled";

/** Событие "download://progress". */
export interface DownloadProgress {
  id: string;
  percent: number;
  speed: string | null;
  eta: string | null;
  status: DownloadStatus;
}

export const api = {
  fetchInfo: (url: string) => invoke<VideoInfo>("fetch_info", { url }),
  startDownload: (options: DownloadOptions) =>
    invoke<string>("start_download", { options }),
  cancelDownload: (id: string) => invoke<void>("cancel_download", { id }),
  downloadTranscript: (options: TranscriptOptions) =>
    invoke<string>("download_transcript", { options }),
  ytdlpVersion: () => invoke<string>("ytdlp_version"),
};

/** Ошибки из Rust приходят строкой, из JS — объектом Error. */
export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
