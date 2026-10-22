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

/** Why a URL can't be downloaded. */
export type ProbeError =
  | { kind: "playlist" }
  | { kind: "live" }
  | { kind: "signInRequired" }
  /** YouTube asks to prove the user isn't a bot; it's about the network, not the Video. */
  | { kind: "botCheck" }
  | { kind: "truncatedUrl" }
  | { kind: "unsupported" }
  /** `message` is for the log; the UI shows a generic text. */
  | { kind: "other"; message: string };

export type ProbeState =
  | { status: "idle" }
  | { status: "probing" }
  | { status: "probed"; video: VideoCard }
  | { status: "failed"; error: ProbeError };

/** What the backend pushes to the UI; the backend owns the state. */
export type AppEvent =
  { type: "setup"; data: SetupState } | { type: "probe"; data: ProbeState };

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

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
