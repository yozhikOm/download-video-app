import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type {
  AudioPreset,
  Preset,
  TranscriptLanguage,
  VideoCard,
  VideoPreset,
} from "@/backend";
import { formatDuration, formatLanguage } from "@/lib/format";
import { strings } from "@/strings";

/**
 * What the user chose on the Video's card: the tab, the Preset on each, and
 * the Transcript. A Video without sound has no Audio tab, so its `audio` is
 * never shown.
 */
export type CardChoice = {
  group: "video" | "audio" | "transcript";
  video: VideoPreset;
  audio: AudioPreset;
  /** The Transcript is saved with the media on the Video and Audio tabs. */
  withTranscript: boolean;
  /** `null` for a Video without a Transcript. */
  transcriptLanguage: string | null;
};

/** The Presets of one group the Video has, to choose one from. */
function PresetPicker<P extends Preset>({
  label,
  presets,
  names,
  chosen,
  onChoose,
}: {
  label: string;
  presets: P[];
  names: Record<P, string>;
  chosen: P;
  onChoose: (preset: P) => void;
}) {
  return (
    <RadioGroup
      aria-label={label}
      value={chosen}
      onValueChange={(value) => onChoose(value as P)}
      className="flex flex-wrap gap-x-4"
    >
      {presets.map((preset) => (
        <label key={preset} className="flex items-center gap-2">
          <RadioGroupItem value={preset} />
          {names[preset]}
        </label>
      ))}
    </RadioGroup>
  );
}

/** The name of a Transcript language in the list. */
function languageName({ language, automatic }: TranscriptLanguage) {
  const name = formatLanguage(language);
  return automatic ? strings.transcript.automatic(name) : name;
}

/**
 * The languages the Video has a Transcript in, by name; two of one name,
 * such as two tracks in English, are told apart by their codes.
 */
function LanguagePicker({
  languages,
  chosen,
  onChoose,
}: {
  languages: TranscriptLanguage[];
  chosen: string | null;
  onChoose: (language: string) => void;
}) {
  const names = languages.map(languageName);
  const named = languages
    .map((offered, at) => ({ ...offered, name: names[at] }))
    // Two of one name keep the order they came in.
    .sort((a, b) => a.name.localeCompare(b.name, "ru"))
    .map(({ language, name }) => ({
      language,
      name:
        names.indexOf(name) === names.lastIndexOf(name)
          ? name
          : `${name} (${language})`,
    }));
  return (
    <NativeSelect
      aria-label={strings.transcript.language}
      value={chosen ?? ""}
      onChange={(event) => onChoose(event.target.value)}
    >
      {named.map(({ language, name }) => (
        <NativeSelectOption key={language} value={language}>
          {name}
        </NativeSelectOption>
      ))}
    </NativeSelect>
  );
}

/**
 * The probed Video, for the user to confirm it's the right one, choose a
 * Preset or its Transcript and download it.
 */
export function VideoPreview({
  video,
  videoPresets,
  audioPresets,
  m4aKeepsSound,
  transcriptLanguages,
  choice,
  onChoose,
  canDownload,
  onDownload,
}: {
  video: VideoCard;
  videoPresets: VideoPreset[];
  audioPresets: AudioPreset[];
  m4aKeepsSound: boolean;
  transcriptLanguages: TranscriptLanguage[];
  choice: CardChoice;
  onChoose: (choice: CardChoice) => void;
  canDownload: boolean;
  onDownload: () => void;
}) {
  const hasTranscript = transcriptLanguages.length > 0;
  const languagePicker = (
    <LanguagePicker
      languages={transcriptLanguages}
      chosen={choice.transcriptLanguage}
      onChoose={(language) =>
        onChoose({ ...choice, transcriptLanguage: language })
      }
    />
  );
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
      <CardContent className="flex flex-col gap-4">
        <Tabs
          value={choice.group}
          onValueChange={(group) =>
            onChoose({ ...choice, group: group as CardChoice["group"] })
          }
        >
          <TabsList>
            <TabsTrigger value="video">{strings.presets.videoTab}</TabsTrigger>
            {audioPresets.length > 0 && (
              <TabsTrigger value="audio">
                {strings.presets.audioTab}
              </TabsTrigger>
            )}
            <TabsTrigger value="transcript">
              {strings.transcript.tab}
            </TabsTrigger>
          </TabsList>
          <TabsContent value="video">
            <PresetPicker
              label={strings.presets.quality}
              presets={videoPresets}
              names={strings.presets.video}
              chosen={choice.video}
              onChoose={(preset) => onChoose({ ...choice, video: preset })}
            />
          </TabsContent>
          {audioPresets.length > 0 && (
            <TabsContent value="audio">
              <PresetPicker
                label={strings.presets.audioFormat}
                presets={audioPresets}
                names={
                  m4aKeepsSound
                    ? strings.presets.audio
                    : {
                        ...strings.presets.audio,
                        m4a: strings.presets.m4aEncoded,
                      }
                }
                chosen={choice.audio}
                onChoose={(preset) => onChoose({ ...choice, audio: preset })}
              />
            </TabsContent>
          )}
          <TabsContent value="transcript">
            {hasTranscript ? (
              languagePicker
            ) : (
              <p className="text-muted-foreground">{strings.transcript.none}</p>
            )}
          </TabsContent>
        </Tabs>
        {choice.group !== "transcript" && (
          <div className="flex min-h-8 flex-wrap items-center gap-x-4 gap-y-2 text-sm">
            <label className="flex items-center gap-2">
              <Checkbox
                checked={choice.withTranscript}
                disabled={!hasTranscript}
                onCheckedChange={(checked) =>
                  onChoose({ ...choice, withTranscript: checked === true })
                }
              />
              {strings.transcript.save}
            </label>
            {choice.withTranscript && languagePicker}
          </div>
        )}
        <div>
          <Button
            disabled={
              !canDownload || (choice.group === "transcript" && !hasTranscript)
            }
            onClick={onDownload}
          >
            {strings.download.start}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
