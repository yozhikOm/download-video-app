import { useId, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import {
  openDownloadedFile,
  removeDownload,
  retryDownload,
  showDownloadedFile,
  type Download,
  type Preset,
} from "@/backend";
import { useDuplicateWarning } from "@/lib/duplicate";
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

/**
 * The Presets by name; a Download doesn't tell whether M4A keeps the sound
 * as it is, so it isn't called original.
 */
const presetNames: Record<Preset, string> = {
  ...strings.presets.video,
  ...strings.presets.audio,
  m4a: strings.presets.m4aEncoded,
};

/** What the Download saves: `1080p`, `MP3 + транскрипт` or `Транскрипт`. */
function savesText({ preset, transcriptLanguage }: Download) {
  if (preset === null) {
    return strings.queue.transcriptAlone;
  }
  const name = presetNames[preset];
  return transcriptLanguage === null
    ? name
    : strings.queue.withTranscript(name);
}

/**
 * A Download in the `queue`: its Video, what it saves and its status.
 */
export function DownloadCard({
  download,
  queue,
}: {
  download: Download;
  queue: Download[];
}) {
  const titleId = useId();
  const [actionFailed, setActionFailed] = useState(false);
  const duplicate = useDuplicateWarning(queue);

  function openOrShowFile(action: (id: number) => Promise<void>) {
    setActionFailed(false);
    action(download.id).catch(() => setActionFailed(true));
  }

  function retry() {
    duplicate.clear();
    retryDownload(download.id)
      .then(duplicate.note)
      // It fails only for a Download no longer Failed, which the Queue's
      // next event shows.
      .catch(() => {});
  }

  return (
    <Card size="sm" role="region" aria-labelledby={titleId}>
      <CardContent className="flex gap-3">
        {download.video.thumbnail && (
          <img
            src={download.video.thumbnail}
            alt={strings.probe.thumbnail}
            className="aspect-video w-24 shrink-0 self-start rounded-md object-cover"
          />
        )}
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div>
            <CardTitle>
              <h3 id={titleId} className="truncate">
                {download.video.title}
              </h3>
            </CardTitle>
            <CardDescription>{savesText(download)}</CardDescription>
          </div>
          {download.status === "queued" && (
            <p className="text-muted-foreground">{strings.download.queued}</p>
          )}
          {download.status === "downloading" && (
            <>
              <Progress
                value={
                  download.percent === null
                    ? null
                    : Math.floor(download.percent)
                }
              />
              <p className="flex justify-between gap-4">
                <span>{strings.download.downloading[download.receiving]}</span>
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
              <div className="flex flex-wrap gap-2">
                <Button onClick={() => openOrShowFile(openDownloadedFile)}>
                  {strings.download.openFile}
                </Button>
                <Button
                  variant="outline"
                  onClick={() => openOrShowFile(showDownloadedFile)}
                >
                  {strings.download.showInFolder}
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => void removeDownload(download.id)}
                >
                  {strings.download.remove}
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
                <Button onClick={retry}>{strings.download.retry}</Button>
              </div>
              {duplicate.shown && (
                <p role="alert" className="text-destructive">
                  {strings.download.duplicate}
                </p>
              )}
            </>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
