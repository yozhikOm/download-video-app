import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import {
  openDownloadedFile,
  retryDownload,
  showDownloadedFile,
  type Download,
} from "@/backend";
import { formatDuration } from "@/lib/format";
import { strings } from "@/strings";

const decimal = new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 1 });

/** The file's name without its folder. */
function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

/** What is known of the progress: `25% · 1,5 МБ/с · осталось 1:07`. */
function progressText(download: Extract<Download, { status: "downloading" }>) {
  const parts = [];
  if (download.percent !== null) {
    parts.push(strings.download.percent(String(Math.floor(download.percent))));
  }
  if (download.speed !== null) {
    parts.push(
      strings.download.speed(decimal.format(download.speed / (1024 * 1024))),
    );
  }
  if (download.eta !== null) {
    parts.push(strings.download.eta(formatDuration(download.eta)));
  }
  return parts.join(" · ");
}

/** The current Download's status, and its file once it is Done. */
export function DownloadCard({ download }: { download: Download }) {
  const [actionFailed, setActionFailed] = useState(false);

  function openOrShowFile(action: () => Promise<void>) {
    setActionFailed(false);
    action().catch(() => setActionFailed(true));
  }

  return (
    <Card role="region" aria-labelledby="download-title">
      <CardHeader>
        <CardTitle>
          <h2 id="download-title">{download.video.title}</h2>
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2 text-sm">
        {download.status === "downloading" && (
          <>
            <Progress
              value={
                download.percent === null ? null : Math.floor(download.percent)
              }
            />
            <p className="flex justify-between gap-4">
              <span>{strings.download.downloading}</span>
              <span className="text-muted-foreground tabular-nums">
                {progressText(download)}
              </span>
            </p>
          </>
        )}
        {download.status === "processing" && (
          <>
            <Progress value={null} />
            <p>{strings.download.processing}</p>
          </>
        )}
        {download.status === "done" && (
          <>
            <p>
              {strings.download.done}
              <span className="text-muted-foreground">
                {" — "}
                {fileName(download.file)}
              </span>
            </p>
            <div className="flex gap-2">
              <Button onClick={() => openOrShowFile(openDownloadedFile)}>
                {strings.download.openFile}
              </Button>
              <Button
                variant="outline"
                onClick={() => openOrShowFile(showDownloadedFile)}
              >
                {strings.download.showInFolder}
              </Button>
            </div>
            {actionFailed && (
              <p role="alert" className="text-destructive">
                {strings.download.actionFailed}
              </p>
            )}
          </>
        )}
        {download.status === "failed" && (
          <>
            <p role="alert" className="text-destructive">
              {strings.download.failed}
            </p>
            <div>
              <Button onClick={() => void retryDownload()}>
                {strings.download.retry}
              </Button>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
