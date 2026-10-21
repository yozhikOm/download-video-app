import { useEffect, useState, type SubmitEvent } from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  appInfo as fetchAppInfo,
  probe as probeUrl,
  type AppInfo,
  type ProbeError,
  type ProbeState,
  type VideoCard as Video,
} from "@/backend";
import { strings } from "@/strings";

type Props = {
  probe: ProbeState;
};

/** `m:ss`, or `h:mm:ss` from an hour on. */
function formatDuration(seconds: number) {
  const total = Math.round(seconds);
  const pad = (n: number) => String(n).padStart(2, "0");
  const [h, m, s] = [
    Math.floor(total / 3600),
    Math.floor(total / 60) % 60,
    total % 60,
  ];
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

function errorText(error: ProbeError) {
  switch (error.kind) {
    case "playlist":
      return strings.probe.playlist;
    case "live":
      return strings.probe.live;
    case "signInRequired":
      return strings.probe.signInRequired;
    case "unsupported":
      return strings.probe.unsupported;
    case "other":
      return strings.probe.other;
  }
}

function VideoCard({ video }: { video: Video }) {
  return (
    <Card className="pt-0">
      {video.thumbnail && (
        <img
          src={video.thumbnail}
          alt={strings.probe.thumbnail}
          className="aspect-video w-full object-cover"
        />
      )}
      <CardHeader>
        <CardTitle>
          <h2>{video.title}</h2>
        </CardTitle>
        <CardDescription className="flex justify-between gap-4">
          <span>{video.channel}</span>
          {video.duration !== null && (
            <span className="tabular-nums">
              {formatDuration(video.duration)}
            </span>
          )}
        </CardDescription>
      </CardHeader>
    </Card>
  );
}

export function MainScreen({ probe }: Props) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [failed, setFailed] = useState(false);
  const [url, setUrl] = useState("");

  useEffect(() => {
    fetchAppInfo()
      .then(setAppInfo)
      .catch(() => setFailed(true));
  }, []);

  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = url.trim();
    if (trimmed) {
      void probeUrl(trimmed);
    }
  }

  return (
    <div className="flex w-full max-w-xl flex-col gap-4 self-start">
      {failed && (
        <p role="alert" className="text-destructive">
          {strings.main.versionFailed}
        </p>
      )}
      {appInfo && (
        <header className="flex items-baseline justify-between">
          <h1 className="font-medium">{appInfo.name}</h1>
          <p className="text-sm text-muted-foreground">
            {strings.main.version(appInfo.version)}
          </p>
        </header>
      )}
      <form className="flex gap-2" onSubmit={submit}>
        <label htmlFor="url" className="sr-only">
          {strings.probe.url}
        </label>
        <Input
          id="url"
          // Not type="url": the browser's own check would block a URL
          // without "https://", which yt-dlp accepts.
          inputMode="url"
          spellCheck={false}
          value={url}
          onChange={(event) => setUrl(event.target.value)}
          placeholder={strings.probe.urlPlaceholder}
          autoFocus
        />
        <Button type="submit">{strings.probe.submit}</Button>
      </form>
      {probe.status === "probing" && (
        <Card>
          <CardContent className="text-muted-foreground">
            {strings.probe.probing}
          </CardContent>
        </Card>
      )}
      {probe.status === "probed" && <VideoCard video={probe.video} />}
      {probe.status === "failed" && (
        <p role="alert" className="text-sm text-destructive">
          {errorText(probe.error)}
        </p>
      )}
    </div>
  );
}
