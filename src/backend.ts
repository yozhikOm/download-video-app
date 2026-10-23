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

/** What the user sees of a probed Video to confirm it's the right one. */
export type VideoCard = {
  title: string;
  channel: string | null;
  thumbnail: string | null;
  /** Seconds. */
  duration: number | null;
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
  | { status: "probed"; video: VideoCard }
  | {
      status: "failed";
      error: ProbeError;
      /** The sign-in browser the Probe ran with; the choice may have changed since. */
      signInBrowser: Browser | null;
    };

/** What the backend pushes to the UI; the backend owns the state. */
export type AppEvent =
  | { type: "setup"; data: SetupState }
  | { type: "probe"; data: ProbeState }
  | { type: "settings"; data: SettingsState };

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

/** Asks the backend to Probe a URL; the result arrives as a probe event. */
export function probe(url: string): Promise<void> {
  return invoke("probe", { url });
}

/** Saves the sign-in browser; the change arrives as a settings event. */
export function setSignInBrowser(browser: Browser | null): Promise<void> {
  return invoke("set_sign_in_browser", { browser });
}

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
