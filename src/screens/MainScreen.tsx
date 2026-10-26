import { useEffect, useState, type SubmitEvent } from "react";
import { DownloadCard } from "@/components/DownloadCard";
import { SignInBrowserPicker } from "@/components/SignInBrowserPicker";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { VideoPreview, type PresetChoice } from "@/components/VideoPreview";
import {
  appInfo as fetchAppInfo,
  download as downloadVideo,
  probe as probeUrl,
  type AppInfo,
  type Browser,
  type Download,
  type Preset,
  type ProbeError,
  type ProbeState,
  type SettingsState,
} from "@/backend";
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

export function MainScreen({ probe, settings, download }: Props) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [failed, setFailed] = useState(false);
  const [url, setUrl] = useState("");
  const [submittedUrl, setSubmittedUrl] = useState<string | null>(null);
  // A new URL in the field puts the previous Video and its Done or Failed
  // Download out of sight; a running Download stays to show its progress.
  // «Скачать» hides the Video too, until the field gets its URL back after
  // an edit or the URL is submitted again.
  const [previewHidden, setPreviewHidden] = useState(false);
  const [endedHidden, setEndedHidden] = useState(false);
  // Each «Скачать» gets a card of its own, so that the previous Download's
  // failure to open its file doesn't carry over to the next one.
  const [downloadsStarted, setDownloadsStarted] = useState(0);
  // The tab and Presets the user chose, for the Probe they were chosen in;
  // each Probe starts on the Video tab with its own defaults, while a card
  // that comes back keeps the choice.
  const [presetChoice, setPresetChoice] = useState<{
    probe: ProbeState;
    choice: PresetChoice;
  } | null>(null);

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
      setEndedHidden(true);
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

  function presetChoiceFor(
    probed: Extract<ProbeState, { status: "probed" }>,
  ): PresetChoice {
    return presetChoice?.probe === probed
      ? presetChoice.choice
      : {
          group: "video",
          video: probed.defaultVideoPreset,
          audio: probed.audioPresets[0] ?? null,
        };
  }

  /** The Preset on the chosen tab. */
  function chosenPreset(
    probed: Extract<ProbeState, { status: "probed" }>,
  ): Preset {
    const choice = presetChoiceFor(probed);
    return choice.group === "audio" && choice.audio
      ? choice.audio
      : choice.video;
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
          videoPresets={probe.videoPresets}
          audioPresets={probe.audioPresets}
          choice={presetChoiceFor(probe)}
          onChoose={(choice) => setPresetChoice({ probe, choice })}
          canDownload={!isRunning(download)}
          onDownload={() => {
            // The Download's card takes over from the Video's.
            setPreviewHidden(true);
            setEndedHidden(false);
            setDownloadsStarted((count) => count + 1);
            // With no Video to download, its card comes back.
            downloadVideo(chosenPreset(probe)).catch(() =>
              setPreviewHidden(false),
            );
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
      {download && (isRunning(download) || !endedHidden) && (
        <DownloadCard key={downloadsStarted} download={download} />
      )}
    </div>
  );
}
