import { Channel, invoke } from "@tauri-apps/api/core";

export type Tool = "ytDlp" | "ffmpeg" | "ffprobe" | "deno";
export type Package = "ytDlp" | "ffmpeg" | "deno";

export type SetupError =
  | { kind: "offline" }
  | { kind: "checksumMismatch"; package: Package }
  /** `message` is for the log; the UI shows a generic text. */
  | { kind: "other"; message: string };

export type SetupState =
  | { status: "checking" }
  | {
      status: "installing";
      downloaded: number;
      /** Zero while the download size isn't known yet. */
      total: number;
      /** The tool that failed to start and is being restored. */
      broken: Tool | null;
    }
  | { status: "failed"; error: SetupError; broken: Tool | null }
  | { status: "ready" };

/** How the update of yt-dlp the user asked for goes. */
export type YtDlpUpdateState =
  /** None was asked for since launch. */
  | { status: "idle" }
  | { status: "updating" }
  /**
   * The new yt-dlp is downloaded and verified; it replaces the old one once
   * the running Download ends.
   */
  | { status: "waiting" }
  | { status: "updated"; version: string }
  /** The latest release is installed already. */
  | { status: "upToDate"; version: string }
  | { status: "failed"; error: SetupError };

/** What the user sees of a probed Video to confirm it's the right one. */
export type VideoCard = {
  title: string;
  channel: string | null;
  thumbnail: string | null;
  /** Seconds. */
  duration: number | null;
};

/**
 * A Video preset: the quality of the saved MP4; `best` is the highest the
 * Video has, offered only above 2160p or when the Video has no other.
 */
export type VideoPreset =
  "best" | "2160p" | "1440p" | "1080p" | "720p" | "480p" | "360p";

/**
 * An Audio preset: the format of the saved sound, without the picture;
 * `m4a` keeps the site's AAC sound as it is, `mp3` encodes it as VBR.
 */
export type AudioPreset = "m4a" | "mp3";

/** A Preset of either group, as the backend knows it by its name. */
export type Preset = VideoPreset | AudioPreset;

/** A language a Transcript can be saved in, and its source. */
export type TranscriptLanguage = {
  /** The site's code of the language, such as `en` or `pt-BR`. */
  language: string;
  /**
   * The text comes from captions the site generated rather than from
   * subtitles the author added.
   */
  automatic: boolean;
};

/** A browser yt-dlp can read the YouTube sign-in from. */
export type Browser =
  | "brave"
  | "chrome"
  | "chromium"
  | "edge"
  | "firefox"
  | "opera"
  | "vivaldi"
  | "whale";

export type SettingsState = {
  /** The browser whose YouTube sign-in yt-dlp uses; `null` to not sign in. */
  signInBrowser: Browser | null;
};

/** Why yt-dlp couldn't read the sign-in browser's cookies. */
export type CookiesError = "browserNotFound" | "browserOpen" | "cantDecrypt";

/** Why a URL can't be downloaded. */
export type ProbeError =
  | { kind: "playlist" }
  | { kind: "live" }
  /** No sign-in browser is chosen, or its account has no access. */
  | { kind: "signInRequired" }
  /** YouTube asks to prove the user isn't a bot; it's about the network, not the Video. */
  | { kind: "botCheck" }
  | { kind: "cookiesUnreadable"; reason: CookiesError }
  | { kind: "truncatedUrl" }
  | { kind: "unsupported" }
  /** `message` is for the log; the UI shows a generic text. */
  | { kind: "other"; message: string };

export type ProbeState =
  | { status: "idle" }
  | { status: "probing" }
  | {
      status: "probed";
      video: VideoCard;
      /**
       * The Video presets the Video has, highest first, after `best` when it
       * saves more than the highest of them or there is no other.
       */
      videoPresets: VideoPreset[];
      /** The Video preset chosen at first. */
      defaultVideoPreset: VideoPreset;
      /** The Audio presets the Video has; none when it has no sound. */
      audioPresets: AudioPreset[];
      /** Whether M4A saves the Video's sound as it is, without encoding it again. */
      m4aKeepsSound: boolean;
      /**
       * The languages a Transcript can be saved in, subtitles over captions,
       * without machine translations; none when the Video has no Transcript.
       */
      transcriptLanguages: TranscriptLanguage[];
      /** The Transcript language chosen at first. */
      defaultTranscriptLanguage: string | null;
    }
  | {
      status: "failed";
      error: ProbeError;
      /** The sign-in browser the Probe ran with; the choice may have changed since. */
      signInBrowser: Browser | null;
    };

/**
 * What a Downloading Download receives now: its Transcript, a Format with
 * the picture (with or without the sound), or one with the sound alone.
 */
export type Receiving = "transcript" | "video" | "audio";

/**
 * Why a Download failed: no network or the site doesn't answer, the site
 * isn't supported or the Video is gone, the Video needs signing in, yt-dlp
 * can't make out the site's page, the file can't be written, or something
 * else.
 */
export type Failure =
  | "offline"
  | "unsupported"
  | "signInRequired"
  | "extractorBroken"
  | "disk"
  | "unknown";

/**
 * What became of the update of yt-dlp that a Download failed with a broken
 * extractor starts: it runs, a newer yt-dlp is installed, the latest one
 * failed already, or it couldn't be updated. The user retries the Download.
 */
export type YtDlpUpdate = "updating" | "updated" | "upToDate" | "failed";

/** A Download in the Queue, as the main screen shows it. */
export type Download = {
  /** Tells the Download apart from the others in the Queue. */
  id: number;
  /** The Video's own page. */
  url: string;
  video: VideoCard;
  /** `null` for the Transcript alone. */
  preset: Preset | null;
  /** The language of the Transcript, when it is saved. */
  transcriptLanguage: string | null;
} &
  /** Waits for the Downloads ahead of it in the Queue. */
  (
    | { status: "queued" }
    | {
        status: "downloading";
        receiving: Receiving;
        /**
         * 0 to 100, of all the Formats yt-dlp receives; `null` while the size
         * of one is unknown. Never goes down during a Download.
         */
        percent: number | null;
        /** Bytes per second. */
        speed: number | null;
        /** Seconds left. */
        eta: number | null;
      }
    /** ffmpeg merges the received Formats into the MP4, or extracts the sound. */
    | { status: "processing" }
    /** `file` is the saved file's full path: the media, or the Transcript alone. */
    | { status: "done"; file: string }
    /**
     * The UI shows its own text for each `failure`; `message` and `output`,
     * the last lines yt-dlp printed but its progress, go into the details
     * the user copies. `ytdlpUpdate` is `null` but for a broken extractor.
     */
    | {
        status: "failed";
        failure: Failure;
        message: string;
        output: string[];
        ytdlpUpdate: YtDlpUpdate | null;
      }
    /** Stopped by the user; its Partial is removed. */
    | { status: "cancelled" }
  );

/** What the backend pushes to the UI; the backend owns the state. */
export type AppEvent =
  | { type: "setup"; data: SetupState }
  | { type: "ytDlpUpdate"; data: YtDlpUpdateState }
  | { type: "probe"; data: ProbeState }
  | { type: "settings"; data: SettingsState }
  /** The Downloads in the Queue, in the order they run. */
  | { type: "queue"; data: Download[] }
  /**
   * The user closes the window while a Download is unfinished; the app stays
   * open until `closeApp` confirms it.
   */
  | { type: "closeRequested" };

/**
 * What became of a Download asked for: it waits in the Queue, or nothing
 * was added since the same Video with the same Preset, or its Transcript
 * alone in the same language, is Queued or running; `of` is that one's id.
 */
export type Added = { status: "queued" } | { status: "duplicate"; of: number };

export type AppInfo = {
  name: string;
  version: string;
};

/**
 * Receives the current state, then every change, in order.
 * Returns a function that stops delivery.
 */
export function subscribe(onEvent: (event: AppEvent) => void): () => void {
  const channel = new Channel<AppEvent>(onEvent);
  invoke("subscribe", { events: channel }).catch((error: unknown) =>
    console.error("subscribe failed", error),
  );
  return () => {
    channel.onmessage = () => {};
  };
}

export function retrySetup(): Promise<void> {
  return invoke("retry_setup");
}

/**
 * Updates yt-dlp once the running Download ends; how it goes arrives as
 * ytDlpUpdate events.
 */
export function updateYtdlp(): Promise<void> {
  return invoke("update_ytdlp");
}

/** Asks the backend to Probe a URL; the result arrives as a probe event. */
export function probe(url: string): Promise<void> {
  return invoke("probe", { url });
}

/**
 * Adds a Download of the probed Video to the Queue: its media with a Video
 * or Audio preset, its Transcript in a language next to it under the same
 * name, or the Transcript alone without a Preset; the Queue arrives as
 * queue events. Rejects when no Probe found a Video, when neither is asked
 * for, or when the Video lacks the Preset or the language.
 */
export function addDownload(
  preset: Preset | null,
  transcript: string | null,
): Promise<Added> {
  return invoke<Added>("add_download", { preset, transcript });
}

/**
 * Puts the Failed or Cancelled Download back at the end of the Queue to
 * download its Video again, resuming from a Failed one's Partial.
 */
export function retryDownload(id: number): Promise<Added> {
  return invoke<Added>("retry_download", { id });
}

/**
 * Cancels the Queued or Downloading Download, stopping yt-dlp and removing
 * its Partial; rejects once it is neither.
 */
export function cancelDownload(id: number): Promise<void> {
  return invoke("cancel_download", { id });
}

/**
 * Closes the app after the user confirmed it with a Download unfinished:
 * yt-dlp stops, and the Partials stay, so that the same Video with the same
 * Preset resumes after the next launch.
 */
export function closeApp(): Promise<void> {
  return invoke("close_app");
}

/** Takes the Done, Failed or Cancelled Download out of the Queue; its files stay. */
export function removeDownload(id: number): Promise<void> {
  return invoke("remove_download", { id });
}

/** Takes every Done Download out of the Queue; their files stay. */
export function clearDoneDownloads(): Promise<void> {
  return invoke("clear_done_downloads");
}

/** Opens the Done Download's file in its default program. */
export function openDownloadedFile(id: number): Promise<void> {
  return invoke("open_downloaded_file", { id });
}

/** Shows the Done Download's file selected in its folder. */
export function showDownloadedFile(id: number): Promise<void> {
  return invoke("show_downloaded_file", { id });
}

/** Saves the sign-in browser; the change arrives as a settings event. */
export function setSignInBrowser(browser: Browser | null): Promise<void> {
  return invoke("set_sign_in_browser", { browser });
}

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
