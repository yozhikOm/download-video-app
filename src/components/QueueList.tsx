import { useId } from "react";
import { DownloadCard } from "@/components/DownloadCard";
import { Button } from "@/components/ui/button";
import { clearDoneDownloads, type Download } from "@/backend";
import { strings } from "@/strings";

/** The Downloads in the Queue, in the order they run. */
export function QueueList({ queue }: { queue: Download[] }) {
  const titleId = useId();
  return (
    <section className="flex flex-col gap-2" aria-labelledby={titleId}>
      <header className="flex items-center justify-between gap-4">
        <h2 id={titleId} className="font-medium">
          {strings.queue.title}
        </h2>
        {queue.some((download) => download.status === "done") && (
          <Button variant="ghost" onClick={() => void clearDoneDownloads()}>
            {strings.queue.clearDone}
          </Button>
        )}
      </header>
      <ul aria-labelledby={titleId} className="flex flex-col gap-2">
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
