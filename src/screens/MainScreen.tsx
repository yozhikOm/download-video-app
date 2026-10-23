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
  download as downloadVideo,
  probe as probeUrl,
  setSignInBrowser,
  type AppInfo,
  type Browser,
  type Download,
  type ProbeError,
  type ProbeState,
  type SettingsState,
  type VideoCard,
} from "@/backend";
import { formatDuration } from "@/lib/format";
import { DownloadCard } from "@/screens/DownloadCard";
import { strings } from "@/strings";

type Props = {
  probe: ProbeState;
  settings: SettingsState;
  download: Download | null;
};

/** The error's text; it names the sign-in browser the Probe ran with. */
function errorText(error: ProbeError, signInBrowser: Browser | null) {
  const browser = signInBrowser && strings.browsers[signInBrowser];
  switch (error.kind) {
    case "playlist":
      return strings.probe.playlist;
    case "live":
      return strings.probe.live;
    case "signInRequired":
      return browser
        ? strings.probe.signInRequiredSignedIn(browser)
        : strings.probe.signInRequired;
    case "botCheck":
      return browser
        ? strings.probe.botCheckSignedIn(browser)
        : strings.probe.botCheck;
    case "cookiesUnreadable":
      // Cookies are read only with a sign-in browser.
      return browser
        ? strings.probe[error.reason](browser)
        : strings.probe.other;
    case "truncatedUrl":
      return strings.probe.truncatedUrl;
    case "unsupported":
      return strings.probe.unsupported;
    case "other":
      return strings.probe.other;
  }
}

/** The probed Video, for the user to confirm it's the right one and download it. */
function VideoPreview({
  video,
  canDownload,
  onDownload,
}: {
  video: VideoCard;
  canDownload: boolean;
  onDownload: () => void;
}) {
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
      <CardContent>
        <Button disabled={!canDownload} onClick={onDownload}>
          {strings.download.start}
        </Button>
      </CardContent>
    </Card>
  );
}

/** One Download at a time until the Queue arrives. */
function isRunning(download: Download | null) {
  return (
    download?.status === "downloading" || download?.status === "processing"
  );
}

/** Signing in is offered only once YouTube asks for it. */
function asksToSignIn(probe: ProbeState) {
  return (
    probe.status === "failed" &&
    (probe.error.kind === "botCheck" || probe.error.kind === "signInRequired")
  );
}

const youtubeHosts = ["youtube.com", "youtu.be", "youtube-nocookie.com"];

/** The field holds a YouTube URL, the only one signing in is for. */
function isYoutube(url: string) {
  const trimmed = url.trim();
  if (!trimmed) {
    return false;
  }
  let host;
  try {
    // yt-dlp accepts a URL without "https://".
    host = new URL(trimmed.includes("://") ? trimmed : `https://${trimmed}`)
      .hostname;
  } catch {
    return false;
  }
  return youtubeHosts.some(
    (site) => host === site || host.endsWith(`.${site}`),
  );
}

const browsers = Object.keys(strings.browsers) as Browser[];

/** Which browser's YouTube sign-in the app uses, if any. */
function SignInBrowserPicker({
  signInBrowser,
}: {
  signInBrowser: Browser | null;
}) {
  const [saveFailed, setSaveFailed] = useState(false);

  function choose(value: string) {
    setSaveFailed(false);
    setSignInBrowser(value ? (value as Browser) : null).catch(() =>
      setSaveFailed(true),
    );
  }

  return (
    <section className="flex flex-col gap-1.5 text-sm">
      <div className="flex items-center justify-between gap-4">
        <label htmlFor="sign-in-browser">{strings.signIn.browser}</label>
        <select
          id="sign-in-browser"
          value={signInBrowser ?? ""}
          onChange={(event) => choose(event.target.value)}
          className="h-8 rounded-lg border border-input bg-transparent px-2 outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
        >
          <option value="">{strings.signIn.none}</option>
          {browsers.map((browser) => (
            <option key={browser} value={browser}>
              {strings.browsers[browser]}
            </option>
          ))}
        </select>
      </div>
      <p className="text-muted-foreground">
        {signInBrowser
          ? strings.signIn.warning(strings.browsers[signInBrowser])
          : strings.signIn.hint}
      </p>
      {saveFailed && (
        <p role="alert" className="text-destructive">
          {strings.signIn.saveFailed}
        </p>
      )}
    </section>
  );
}

export function MainScreen({ probe, settings, download }: Props) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [failed, setFailed] = useState(false);
  const [url, setUrl] = useState("");
  const [submittedUrl, setSubmittedUrl] = useState<string | null>(null);
  // A new URL in the field puts the previous Video and its finished
  // Download out of sight; a running Download stays to show its progress.
  // «Скачать» hides the Video too, until its URL is submitted again.
  const [previewHidden, setPreviewHidden] = useState(false);
  const [finishedHidden, setFinishedHidden] = useState(false);

  useEffect(() => {
    fetchAppInfo()
      .then(setAppInfo)
      .catch(() => setFailed(true));
  }, []);

  function changeUrl(value: string) {
    setUrl(value);
    const isNew = value.trim() !== submittedUrl;
    // The same URL back in the field brings its Video back.
    setPreviewHidden(isNew);
    if (isNew && download && !isRunning(download)) {
      setFinishedHidden(true);
    }
  }

  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = url.trim();
    if (trimmed) {
      setSubmittedUrl(trimmed);
      setPreviewHidden(false);
      void probeUrl(trimmed);
    }
  }

  // Signing in steps aside for the URL just submitted, unless its Probe
  // failed: the user may need to sign in or change the browser to retry.
  const awaitsSubmitted =
    url.trim() === submittedUrl && probe.status !== "failed";

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
          onChange={(event) => changeUrl(event.target.value)}
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
      {probe.status === "probed" && !previewHidden && (
        <VideoPreview
          video={probe.video}
          canDownload={!isRunning(download)}
          onDownload={() => {
            // The Download's card takes over from the Video's.
            setPreviewHidden(true);
            setFinishedHidden(false);
            void downloadVideo();
          }}
        />
      )}
      {probe.status === "failed" && (
        <p role="alert" className="text-sm text-destructive">
          {errorText(probe.error, probe.signInBrowser)}
        </p>
      )}
      {/* With a YouTube URL in the field, a chosen browser stays on screen
          so that it can be turned off, except for the URL just submitted. */}
      {!awaitsSubmitted &&
        (settings.signInBrowser || asksToSignIn(probe)) &&
        isYoutube(url) && (
          <SignInBrowserPicker signInBrowser={settings.signInBrowser} />
        )}
      {download && (isRunning(download) || !finishedHidden) && (
        <DownloadCard download={download} />
      )}
    </div>
  );
}
