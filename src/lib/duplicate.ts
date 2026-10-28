import { useState } from "react";
import type { Added, Download } from "@/backend";

/**
 * The warning that «Скачать» or «Повторить» added nothing: it shows while
 * the Download it duplicates is Queued or running, and goes once that one
 * ends, since asking again would add it then.
 */
export function useDuplicateWarning(queue: Download[]) {
  const [of, setOf] = useState<number | null>(null);
  const shown = queue.some(
    (download) =>
      download.id === of &&
      (download.status === "queued" ||
        download.status === "downloading" ||
        download.status === "processing"),
  );
  return {
    shown,
    /** Warns when nothing was added. */
    note: (added: Added) =>
      setOf(added.status === "duplicate" ? added.of : null),
    clear: () => setOf(null),
  };
}
