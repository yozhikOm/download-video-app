import { useId } from "react";
import { DownloadCard } from "@/components/DownloadCard";
import { Button } from "@/components/ui/button";
import { clearDoneDownloads, type Download } from "@/backend";
import { strings } from "@/strings";

/** The Downloads in the Queue, in the order they run, in one card. */
export function QueueList({ queue }: { queue: Download[] }) {
  const titleId = useId();
  return (
    <section className="mt-1.5 flex flex-col gap-2" aria-labelledby={titleId}>
      <header className="flex min-h-7 items-center gap-2">
        <h2 id={titleId} className="font-semibold">
          {strings.queue.title}
        </h2>
        <span className="min-w-5 rounded-full border bg-muted px-1.5 text-center text-xs tabular-nums">
          {queue.length}
        </span>
        {queue.some((download) => download.status === "done") && (
          <Button
            variant="ghost"
            size="sm"
            className="ml-auto font-normal text-primary hover:text-primary"
            onClick={() => void clearDoneDownloads()}
          >
            {strings.queue.clearDone}
          </Button>
        )}
      </header>
      <ul
        aria-labelledby={titleId}
        className="divide-y overflow-hidden rounded-xl border bg-card"
      >
        {queue.map((download) => (
          // Each Download keeps its own card, and with it its own errors.
          <li key={download.id}>
            <DownloadCard download={download} queue={queue} />
          </li>
        ))}
      </ul>
    </section>
  );
}
