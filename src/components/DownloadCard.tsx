import { useId, useState, type ComponentProps, type ReactNode } from "react";
import {
  BanIcon,
  CircleAlertIcon,
  CircleCheckIcon,
  ClockIcon,
  CogIcon,
  CopyIcon,
  FileIcon,
  FolderOpenIcon,
  RotateCwIcon,
  XIcon,
} from "lucide-react";
import { cn } from "cn";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import {
  cancelDownload,
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
 * What the user pastes into a GitHub issue about a Failed Download: its
 * URL, what it saves, why it failed and the last lines yt-dlp printed.
 */
function detailsText(download: Extract<Download, { status: "failed" }>) {
  const { details } = strings.download;
  // Longer than any run of backticks yt-dlp printed, so that none ends it.
  const longestRun = Math.max(
    0,
    ...download.output.flatMap((line) =>
      (line.match(/`+/g) ?? []).map((run) => run.length),
    ),
  );
  const fence = "`".repeat(Math.max(3, longestRun + 1));
  return [
    details.url(download.url),
    ...(download.preset === null ? [] : [details.preset(download.preset)]),
    ...(download.transcriptLanguage === null
      ? []
      : [details.transcript(download.transcriptLanguage)]),
    details.message(download.message),
    "",
    details.output,
    fence,
    ...download.output,
    fence,
  ].join("\n");
}

/** A flat action of a Download; the main one is in the accent colour. */
function ActionButton({
  main = false,
  className,
  ...props
}: ComponentProps<typeof Button> & { main?: boolean }) {
  return (
    <Button
      variant="ghost"
      size="sm"
      className={cn(
        "font-normal",
        main && "font-semibold text-primary hover:text-primary",
        className,
      )}
      {...props}
    />
  );
}

/** A line of the Download's status, in its colour, after its icon. */
function StatusLine({ className, ...props }: ComponentProps<"p">) {
  return (
    <p
      className={cn(
        "flex items-start gap-1.5 text-[13px] [&_svg]:mt-0.5 [&_svg]:size-3.5 [&_svg]:shrink-0",
        className,
      )}
      {...props}
    />
  );
}

/**
 * A Download in the `queue`, as a row of the Queue: its Video, what it
 * saves and its status.
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
  /** How copying the details of a Failed Download ended, once it has. */
  const [copying, setCopying] = useState<"copied" | "failed" | null>(null);
  const duplicate = useDuplicateWarning(queue);

  function openOrShowFile(action: (id: number) => Promise<void>) {
    setActionFailed(false);
    action(download.id).catch(() => setActionFailed(true));
  }

  function retry() {
    duplicate.clear();
    setCopying(null);
    retryDownload(download.id)
      .then(duplicate.note)
      // It fails for a Download no longer Failed or Cancelled, which the
      // Queue's next event shows, and for a Cancelled one whose yt-dlp is
      // still exiting, which can be retried in a moment.
      .catch(() => {});
  }

  function copyDetails(failed: Extract<Download, { status: "failed" }>) {
    setCopying(null);
    navigator.clipboard.writeText(detailsText(failed)).then(
      () => setCopying("copied"),
      () => setCopying("failed"),
    );
  }

  function cancel() {
    // It fails only for a Download no longer Queued or Downloading, which
    // the Queue's next event shows.
    cancelDownload(download.id).catch(() => {});
  }

  const actionRow = (buttons: ReactNode) => (
    <div className="-ml-2 flex flex-wrap gap-0.5">{buttons}</div>
  );

  const cancelButton = (
    <ActionButton onClick={cancel}>
      <XIcon />
      {strings.download.cancel}
    </ActionButton>
  );

  const removeButton = (
    <ActionButton onClick={() => void removeDownload(download.id)}>
      <XIcon />
      {strings.download.remove}
    </ActionButton>
  );

  const retryOrRemoveButtons = (more?: ReactNode) => (
    <>
      {actionRow(
        <>
          <ActionButton main onClick={retry}>
            <RotateCwIcon />
            {strings.download.retry}
          </ActionButton>
          {more}
          {removeButton}
        </>,
      )}
      {duplicate.shown && (
        <StatusLine role="alert" className="text-destructive">
          {strings.download.duplicate}
        </StatusLine>
      )}
    </>
  );

  return (
    <div
      role="region"
      aria-labelledby={titleId}
      className="grid grid-cols-[6rem_minmax(0,1fr)] gap-3.5 px-3.5 py-3"
    >
      {download.video.thumbnail ? (
        <img
          src={download.video.thumbnail}
          alt={strings.probe.thumbnail}
          className="aspect-video w-full rounded-sm object-cover"
        />
      ) : (
        <div className="aspect-video rounded-sm bg-muted" />
      )}
      <div className="flex min-w-0 flex-col gap-1">
        <div className="flex items-baseline gap-2">
          <h3 id={titleId} className="min-w-0 truncate font-semibold">
            {download.video.title}
          </h3>
          <span className="shrink-0 text-xs text-muted-foreground">
            {savesText(download)}
          </span>
        </div>
        {download.status === "queued" && (
          <>
            <StatusLine className="text-muted-foreground">
              <ClockIcon />
              {strings.download.queued}
            </StatusLine>
            {actionRow(cancelButton)}
          </>
        )}
        {download.status === "downloading" && (
          <>
            <Progress
              className="my-1"
              value={
                download.percent === null ? null : Math.floor(download.percent)
              }
            />
            <p className="flex justify-between gap-4 text-[13px]">
              <span>{strings.download.downloading[download.receiving]}</span>
              <span className="text-muted-foreground tabular-nums">
                {progressText(download)}
              </span>
            </p>
            {actionRow(cancelButton)}
          </>
        )}
        {download.status === "processing" && (
          <>
            <Progress className="my-1" value={null} />
            <StatusLine>
              <CogIcon />
              {strings.download.processing}
            </StatusLine>
          </>
        )}
        {download.status === "done" && (
          <>
            <StatusLine>
              <CircleCheckIcon className="text-success" />
              <span className="min-w-0 break-words">
                {strings.download.done}
                <span className="text-muted-foreground">
                  {" — "}
                  {fileName(download.file)}
                </span>
              </span>
            </StatusLine>
            {actionRow(
              <>
                <ActionButton
                  main
                  onClick={() => openOrShowFile(openDownloadedFile)}
                >
                  <FileIcon />
                  {strings.download.openFile}
                </ActionButton>
                <ActionButton
                  onClick={() => openOrShowFile(showDownloadedFile)}
                >
                  <FolderOpenIcon />
                  {strings.download.showInFolder}
                </ActionButton>
                {removeButton}
              </>,
            )}
            {actionFailed && (
              <StatusLine role="alert" className="text-destructive">
                {strings.download.actionFailed}
              </StatusLine>
            )}
          </>
        )}
        {download.status === "failed" && (
          <>
            <StatusLine role="alert" className="text-destructive">
              <CircleAlertIcon />
              {download.failure === "extractorBroken" &&
              download.ytdlpUpdate !== null
                ? strings.download.extractorBroken[download.ytdlpUpdate]
                : strings.download.failed[download.failure]}
            </StatusLine>
            {retryOrRemoveButtons(
              <ActionButton onClick={() => copyDetails(download)}>
                <CopyIcon />
                {strings.download.copyDetails}
              </ActionButton>,
            )}
            {copying === "copied" && (
              <StatusLine role="status" className="text-muted-foreground">
                {strings.download.detailsCopied}
              </StatusLine>
            )}
            {copying === "failed" && (
              <StatusLine role="alert" className="text-destructive">
                {strings.download.copyFailed}
              </StatusLine>
            )}
          </>
        )}
        {download.status === "cancelled" && (
          <>
            <StatusLine className="text-muted-foreground">
              <BanIcon />
              {strings.download.cancelled}
            </StatusLine>
            {retryOrRemoveButtons()}
          </>
        )}
      </div>
    </div>
  );
}
