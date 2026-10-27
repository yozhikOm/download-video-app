import { useState } from "react";
import { Button } from "@/components/ui/button";
import { updateYtdlp, type SetupError, type YtDlpUpdateState } from "@/backend";
import { strings } from "@/strings";

function errorText(error: SetupError) {
  switch (error.kind) {
    case "offline":
      return strings.ytdlp.offline;
    case "checksumMismatch":
      return strings.ytdlp.checksumMismatch;
    case "other":
      return strings.ytdlp.other;
  }
}

/** Updates yt-dlp on request and tells how the update goes. */
export function YtDlpUpdater({ state }: { state: YtDlpUpdateState }) {
  // The command couldn't be sent, so no update state follows.
  const [requestFailed, setRequestFailed] = useState(false);
  const running = state.status === "updating" || state.status === "waiting";

  function update() {
    setRequestFailed(false);
    updateYtdlp().catch(() => setRequestFailed(true));
  }

  return (
    <footer className="flex flex-col gap-1.5 text-sm">
      <div>
        <Button variant="outline" disabled={running} onClick={update}>
          {strings.ytdlp.update}
        </Button>
      </div>
      {requestFailed ? (
        <p role="alert" className="text-destructive">
          {strings.ytdlp.other}
        </p>
      ) : (
        <>
          {state.status === "updating" && (
            <p role="status" className="text-muted-foreground">
              {strings.ytdlp.updating}
            </p>
          )}
          {state.status === "waiting" && (
            <p role="status" className="text-muted-foreground">
              {strings.ytdlp.waiting}
            </p>
          )}
          {state.status === "updated" && (
            <p role="status" className="text-muted-foreground">
              {strings.ytdlp.updated(state.version)}
            </p>
          )}
          {state.status === "upToDate" && (
            <p role="status" className="text-muted-foreground">
              {strings.ytdlp.upToDate(state.version)}
            </p>
          )}
          {state.status === "failed" && (
            <p role="alert" className="text-destructive">
              {errorText(state.error)}
            </p>
          )}
        </>
      )}
    </footer>
  );
}
