import { Channel, invoke } from "@tauri-apps/api/core";

export type Tool = "ytDlp" | "ffmpeg" | "ffprobe";
export type Package = "ytDlp" | "ffmpeg";

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
      total: number;
      /** The tool that failed to start and is being restored. */
      broken: Tool | null;
    }
  | { status: "failed"; error: SetupError; broken: Tool | null }
  | { status: "ready" };

/** What the backend pushes to the UI; the backend owns the state. */
export type AppEvent = { type: "setup"; data: SetupState };

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

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
