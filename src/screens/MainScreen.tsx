import { useRef, useState, type SubmitEvent } from "react";
import {
  CircleAlertIcon,
  LinkIcon,
  LoaderCircleIcon,
  SettingsIcon,
  XIcon,
} from "lucide-react";
import { ErrorBar } from "@/components/ErrorBar";
import { QueueList } from "@/components/QueueList";
import { SignInBrowserPicker } from "@/components/SignInBrowserPicker";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { VideoPreview, type CardChoice } from "@/components/VideoPreview";
import { useDuplicateWarning } from "@/lib/duplicate";
import {
  addDownload,
  probe as probeUrl,
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
  queue: Download[];
  /** Out of sight behind the settings, keeping what it shows. */
  hidden: boolean;
  onOpenSettings: () => void;
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

export function MainScreen({
  probe,
  settings,
  queue,
  hidden,
  onOpenSettings,
}: Props) {
  const urlField = useRef<HTMLInputElement>(null);
  const [url, setUrl] = useState("");
  const [submittedUrl, setSubmittedUrl] = useState<string | null>(null);
  // A new URL in the field puts the previous Video out of sight. «Скачать»
  // hides the Video too, until the field gets its URL back after an edit or
  // the URL is submitted again.
  const [previewHidden, setPreviewHidden] = useState(false);
  // The Probe a submitted URL replaces stays out of sight until the backend
  // sends the next one.
  const [supersededProbe, setSupersededProbe] = useState<ProbeState | null>(
    null,
  );
  // The submitted URL's Probe couldn't start, so the backend sends none.
  const [probeFailed, setProbeFailed] = useState(false);
  // «Скачать» added nothing: the same Download is Queued or running. The
  // warning stays while it is, until the choice, the Video or the URL
  // changes.
  const duplicate = useDuplicateWarning(queue);
  // The tab, Presets and Transcript the user chose, for the Probe they were
  // chosen in; each Probe starts on the Video tab with its own defaults and
  // without the Transcript, while a card that comes back keeps the choice.
  const [cardChoice, setCardChoice] = useState<{
    probe: ProbeState;
    choice: CardChoice;
  } | null>(null);

  function changeUrl(value: string) {
    setUrl(value);
    // The same URL back in the field brings its Video back.
    setPreviewHidden(value.trim() !== submittedUrl);
    duplicate.clear();
  }

  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = url.trim();
    if (trimmed) {
      setSubmittedUrl(trimmed);
      setPreviewHidden(false);
      setSupersededProbe(probe);
      setProbeFailed(false);
      duplicate.clear();
      probeUrl(trimmed).catch(() => setProbeFailed(true));
    }
  }

  function cardChoiceFor(
    probed: Extract<ProbeState, { status: "probed" }>,
  ): CardChoice {
    return cardChoice?.probe === probed
      ? cardChoice.choice
      : {
          group: "video",
          video: probed.defaultVideoPreset,
          audio: probed.defaultAudioPreset,
          withTranscript: false,
          transcriptLanguage: probed.defaultTranscriptLanguage,
        };
  }

  /**
   * The Preset on the chosen tab, `null` on the Transcript tab, and the
   * Transcript language when the Transcript is saved.
   */
  function chosenDownload(
    probed: Extract<ProbeState, { status: "probed" }>,
  ): [Preset | null, string | null] {
    const choice = cardChoiceFor(probed);
    const transcript = choice.withTranscript ? choice.transcriptLanguage : null;
    switch (choice.group) {
      case "video":
        return [choice.video, transcript];
      case "audio":
        return [choice.audio, transcript];
      case "transcript":
        return [null, choice.transcriptLanguage];
    }
  }

  // Signing in steps aside for the URL just submitted, unless its Probe
  // failed: the user may need to sign in or change the browser to retry.
  const awaitsSubmitted =
    url.trim() === submittedUrl && probe.status !== "failed";
  const previewShown = !previewHidden && probe !== supersededProbe;

  return (
    <div
      hidden={hidden}
      className="flex w-full max-w-[53.75rem] flex-col gap-3.5 self-start"
    >
      <header className="flex min-h-8.5 items-center gap-2.5">
        <h1 className="font-heading text-xl font-semibold">
          {strings.main.title}
        </h1>
        <Button
          variant="ghost"
          size="icon"
          className="ml-auto"
          aria-label={strings.main.settings}
          title={strings.main.settings}
          onClick={onOpenSettings}
        >
          <SettingsIcon />
        </Button>
      </header>
      <form className="flex gap-2" onSubmit={submit}>
        <label htmlFor="url" className="sr-only">
          {strings.probe.url}
        </label>
        <div className="relative min-w-0 flex-1">
          <LinkIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          {/* A Windows text box: its bottom edge turns accent in focus. */}
          <Input
            ref={urlField}
            id="url"
            className="h-9 border-b-muted-foreground pr-9 pl-8 focus-visible:border-input focus-visible:border-b-primary focus-visible:shadow-[inset_0_-1px_0_var(--color-primary)] focus-visible:ring-0 md:text-sm"
            // Not type="url": the browser's own check would block a URL
            // without "https://", which yt-dlp accepts.
            inputMode="url"
            spellCheck={false}
            value={url}
            onChange={(event) => changeUrl(event.target.value)}
            placeholder={strings.probe.urlPlaceholder}
            autoFocus
          />
          {url && (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              className="absolute top-1/2 right-1.5 -translate-y-1/2 text-muted-foreground"
              aria-label={strings.probe.clearUrl}
              title={strings.probe.clearUrl}
              onClick={() => {
                changeUrl("");
                urlField.current?.focus();
              }}
            >
              <XIcon />
            </Button>
          )}
        </div>
        <Button type="submit" size="lg" className="px-4.5 font-semibold">
          {strings.probe.submit}
        </Button>
      </form>
      {probe.status === "probing" && (
        <Card className="flex-row items-center gap-3 p-4 text-muted-foreground">
          <LoaderCircleIcon className="size-4 animate-spin text-primary motion-reduce:animate-none" />
          {strings.probe.probing}
        </Card>
      )}
      {probe.status === "probed" && previewShown && (
        <VideoPreview
          video={probe.video}
          videoPresets={probe.videoPresets}
          audioPresets={probe.audioPresets}
          m4aKeepsSound={probe.m4aKeepsSound}
          transcriptLanguages={probe.transcriptLanguages}
          choice={cardChoiceFor(probe)}
          onChoose={(choice) => {
            setCardChoice({ probe, choice });
            duplicate.clear();
          }}
          onDownload={() => {
            // The Download in the Queue takes over from the Video's card.
            setPreviewHidden(true);
            duplicate.clear();
            // With nothing added, the card comes back.
            addDownload(...chosenDownload(probe))
              .then((added) => {
                if (added.status === "duplicate") {
                  setPreviewHidden(false);
                }
                duplicate.note(added);
              })
              .catch(() => setPreviewHidden(false));
          }}
        />
      )}
      {duplicate.shown && probe.status === "probed" && previewShown && (
        <p role="alert" className="flex items-start gap-1.5 text-destructive">
          <CircleAlertIcon className="mt-0.5 size-4 shrink-0" />
          {strings.download.duplicate}
        </p>
      )}
      {probeFailed ? (
        <ErrorBar>{strings.probe.other}</ErrorBar>
      ) : (
        probe.status === "failed" && (
          <ErrorBar>{errorText(probe.error, probe.signInBrowser)}</ErrorBar>
        )
      )}
      {/* With a YouTube URL in the field, a chosen browser stays on screen
          so that it can be turned off, except for the URL just submitted. */}
      {!awaitsSubmitted &&
        (settings.signInBrowser || asksToSignIn(probe)) &&
        isYoutube(url) && (
          <SignInBrowserPicker signInBrowser={settings.signInBrowser} />
        )}
      {queue.length > 0 && <QueueList queue={queue} />}
    </div>
  );
}
