import type { ReactNode } from "react";
import { CircleAlertIcon } from "lucide-react";

/** An error the user should notice: a red bar with a warning icon. */
export function ErrorBar({ children }: { children: ReactNode }) {
  return (
    <div
      role="alert"
      className="flex items-start gap-2.5 rounded-lg border bg-destructive-surface px-3.5 py-3"
    >
      <CircleAlertIcon className="mt-0.5 size-4 shrink-0 text-destructive" />
      <p>{children}</p>
    </div>
  );
}
