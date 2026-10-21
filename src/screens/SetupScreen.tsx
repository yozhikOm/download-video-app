import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { retrySetup, type SetupError, type SetupState } from "@/backend";
import { strings } from "@/strings";

type Props = {
  state: Extract<SetupState, { status: "installing" | "failed" }>;
};

const megabytes = new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 1 });

function mb(bytes: number) {
  return megabytes.format(bytes / (1024 * 1024));
}

function errorText(error: SetupError) {
  switch (error.kind) {
    case "offline":
      return strings.setup.offline;
    case "checksumMismatch":
      return strings.setup.checksumMismatch(strings.tools[error.package]);
    case "other":
      return strings.setup.other;
  }
}

export function SetupScreen({ state }: Props) {
  const brokenNote =
    state.broken && strings.setup.broken(strings.tools[state.broken]);

  return (
    <Card className="w-full max-w-md">
      <CardHeader>
        <CardTitle>
          <h1>
            {state.status === "failed"
              ? strings.setup.failedTitle
              : strings.setup.title}
          </h1>
        </CardTitle>
        {brokenNote && <CardDescription>{brokenNote}</CardDescription>}
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {state.status === "installing" ? (
          <>
            <Progress
              value={
                state.total > 0
                  ? Math.round((state.downloaded / state.total) * 100)
                  : 0
              }
            />
            {state.total > 0 && (
              <p className="text-sm text-muted-foreground">
                {strings.setup.progress(mb(state.downloaded), mb(state.total))}
              </p>
            )}
          </>
        ) : (
          <>
            <p role="alert" className="text-sm text-destructive">
              {errorText(state.error)}
            </p>
            <Button className="self-start" onClick={() => void retrySetup()}>
              {strings.setup.retry}
            </Button>
          </>
        )}
      </CardContent>
    </Card>
  );
}
