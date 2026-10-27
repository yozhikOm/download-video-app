import { AlertDialog } from "radix-ui";
import { closeApp } from "@/backend";
import { Button } from "@/components/ui/button";
import { strings } from "@/strings";

/**
 * Asks before closing the app while a Download is unfinished; closing keeps
 * its Partial for the next launch.
 */
export function CloseDialog({
  open,
  onDismiss,
}: {
  open: boolean;
  /** Either button closes the dialog. */
  onDismiss: () => void;
}) {
  return (
    <AlertDialog.Root
      open={open}
      onOpenChange={(open) => {
        if (!open) {
          onDismiss();
        }
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 z-50 bg-black/40" />
        <AlertDialog.Content className="fixed top-1/2 left-1/2 z-50 flex w-full max-w-sm -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-xl bg-card p-4 text-sm text-card-foreground ring-1 ring-foreground/10">
          <AlertDialog.Title className="text-base font-medium">
            {strings.close.title}
          </AlertDialog.Title>
          <AlertDialog.Description className="text-muted-foreground">
            {strings.close.description}
          </AlertDialog.Description>
          <div className="flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <Button variant="outline">{strings.close.decline}</Button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <Button
                variant="destructive"
                onClick={() => {
                  closeApp().catch(() => {});
                }}
              >
                {strings.close.confirm}
              </Button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
