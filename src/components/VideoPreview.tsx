import { useId } from "react";
import { DownloadIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type {
  AudioPreset,
  Format,
  Preset,
  TranscriptLanguage,
  VideoCard,
  VideoPreset,
} from "@/backend";
import { formatDuration, formatLanguage } from "@/lib/format";
import { strings } from "@/strings";

/**
 * What the user chose on the Video's card: the tab, the Preset on each, the
 * Format in the Advanced mode, and the Transcript. A Video without sound
 * has no Audio tab, so its `audio` is never shown.
 */
export type CardChoice = {
  group: "video" | "audio" | "transcript" | "formats";
  video: VideoPreset;
  audio: AudioPreset;
  /** The Format's id; `null` until one is chosen. */
  format: string | null;
  /** The Transcript is saved with the media on the media's tabs. */
  withTranscript: boolean;
  /** `null` for a Video without a Transcript. */
  transcriptLanguage: string | null;
  /** The Transcript is saved as `.srt` too, with its timing. */
  srt: boolean;
};

/** The Presets of one group the Video has, to choose one from, in a grid. */
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
  const labelId = useId();
  return (
    <div className="flex flex-col gap-1.5">
      <p id={labelId} className="text-xs text-muted-foreground">
        {label}
      </p>
      <RadioGroup
        aria-labelledby={labelId}
        value={chosen}
        onValueChange={(value) => onChoose(value as P)}
        className="flex flex-wrap gap-x-4.5 gap-y-1.5"
      >
        {presets.map((preset) => (
          <label key={preset} className="flex items-center gap-2">
            <RadioGroupItem value={preset} />
            {names[preset]}
          </label>
        ))}
      </RadioGroup>
    </div>
  );
}

const decimal = new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 1 });

/** A codec as the list shows it: a dash for none or one the site doesn't tell. */
function codecText(codec: string | null) {
  return codec === null || codec === "none" ? strings.formats.unknown : codec;
}

/** What the list tells of a Format, column by column. */
function formatCells(format: Format) {
  const { formats } = strings;
  let resolution = formats.unknown;
  if (format.videoCodec === "none") {
    resolution = formats.audioOnly;
  } else if (format.width !== null || format.height !== null) {
    resolution = formats.resolution(
      String(format.width ?? "?"),
      String(format.height ?? "?"),
    );
    if (format.fps) {
      resolution = formats.fps(resolution, String(format.fps));
    }
  }
  return {
    id: format.id,
    ext: format.ext,
    resolution,
    videoCodec: codecText(format.videoCodec),
    // A picture without the sound downloads with the best sound added.
    audioCodec:
      format.audioCodec === "none" && format.videoCodec !== "none"
        ? formats.bestAudio
        : codecText(format.audioCodec),
    bitrate:
      format.bitrate === null
        ? formats.unknown
        : formats.bitrate(String(Math.round(format.bitrate))),
    size:
      format.size === null
        ? formats.unknown
        : formats.size(decimal.format(format.size / (1024 * 1024))),
    note:
      [
        format.note,
        format.language,
        format.protocol?.startsWith("m3u8") && formats.hls,
      ]
        .filter(Boolean)
        .join(", ") || formats.unknown,
  } satisfies Record<keyof typeof strings.formats.columns, string>;
}

const formatColumns = Object.keys(
  strings.formats.columns,
) as (keyof typeof strings.formats.columns)[];

/**
 * Every Format of the Video in a table, one to choose in each row, for the
 * Advanced mode.
 */
function FormatPicker({
  formats,
  chosen,
  onChoose,
}: {
  formats: Format[];
  chosen: string | null;
  onChoose: (id: string) => void;
}) {
  return (
    <RadioGroup
      aria-label={strings.formats.list}
      value={chosen ?? ""}
      onValueChange={onChoose}
      className="block max-h-72 overflow-auto rounded-md border"
    >
      <table className="w-full text-xs whitespace-nowrap">
        <thead className="sticky top-0 bg-card text-muted-foreground">
          <tr>
            <th className="w-8" />
            {formatColumns.map((column) => (
              <th key={column} className="px-1.5 py-1.5 text-left font-normal">
                {strings.formats.columns[column]}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {formats.map((format) => {
            const cells = formatCells(format);
            return (
              <tr
                key={format.id}
                className="cursor-pointer border-t hover:bg-muted/50"
                onClick={() => onChoose(format.id)}
              >
                <td className="px-2 py-1.5">
                  <RadioGroupItem
                    value={format.id}
                    aria-label={formatColumns
                      .map((column) => cells[column])
                      .join(" ")}
                  />
                </td>
                {formatColumns.map((column) => (
                  <td
                    key={column}
                    className={
                      column === "id"
                        ? "px-1.5 py-1.5 font-mono"
                        : "px-1.5 py-1.5 tabular-nums"
                    }
                  >
                    {cells[column]}
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
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
  formats,
  advanced,
  choice,
  onChoose,
  onDownload,
}: {
  video: VideoCard;
  videoPresets: VideoPreset[];
  audioPresets: AudioPreset[];
  m4aKeepsSound: boolean;
  transcriptLanguages: TranscriptLanguage[];
  formats: Format[];
  /** The Advanced mode: the Formats tab. */
  advanced: boolean;
  choice: CardChoice;
  onChoose: (choice: CardChoice) => void;
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
  const srtCheckbox = (
    <label className="flex items-center gap-2">
      <Checkbox
        checked={choice.srt}
        onCheckedChange={(checked) =>
          onChoose({ ...choice, srt: checked === true })
        }
      />
      {strings.transcript.srt}
    </label>
  );
  return (
    <Card
      className={
        video.thumbnail
          ? "grid grid-cols-[minmax(0,15rem)_minmax(0,1fr)] gap-5 p-4 has-[>img:first-child]:pt-4 *:[img:first-child]:rounded-lg"
          : "p-4"
      }
    >
      {video.thumbnail && (
        <img
          src={video.thumbnail}
          alt={strings.probe.thumbnail}
          className="aspect-video w-full object-cover"
        />
      )}
      <div className="flex min-w-0 flex-col gap-3">
        <header>
          <h2 className="font-heading text-base leading-snug font-semibold">
            {video.title}
          </h2>
          <p className="mt-0.5 flex flex-wrap gap-x-2 text-muted-foreground">
            {video.channel && <span>{video.channel}</span>}
            {video.channel && video.duration !== null && (
              <span aria-hidden="true">·</span>
            )}
            {video.duration !== null && (
              <span className="tabular-nums">
                {formatDuration(video.duration)}
              </span>
            )}
          </p>
        </header>
        <Tabs
          value={choice.group}
          onValueChange={(group) =>
            onChoose({ ...choice, group: group as CardChoice["group"] })
          }
        >
          <TabsList className="h-8.5">
            <TabsTrigger value="video">{strings.presets.videoTab}</TabsTrigger>
            {audioPresets.length > 0 && (
              <TabsTrigger value="audio">
                {strings.presets.audioTab}
              </TabsTrigger>
            )}
            <TabsTrigger value="transcript">
              {strings.transcript.tab}
            </TabsTrigger>
            {advanced && (
              <TabsTrigger value="formats">{strings.formats.tab}</TabsTrigger>
            )}
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
              <div className="flex min-h-8 flex-wrap items-center gap-x-3.5 gap-y-2">
                {languagePicker}
                {srtCheckbox}
              </div>
            ) : (
              <p className="text-muted-foreground">{strings.transcript.none}</p>
            )}
          </TabsContent>
          {advanced && (
            <TabsContent value="formats">
              <FormatPicker
                formats={formats}
                chosen={choice.format}
                onChoose={(format) => onChoose({ ...choice, format })}
              />
            </TabsContent>
          )}
        </Tabs>
        {choice.group !== "transcript" && (
          <div className="flex min-h-8 flex-wrap items-center gap-x-3.5 gap-y-2">
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
            {choice.withTranscript && srtCheckbox}
          </div>
        )}
        <div>
          <Button
            size="lg"
            className="px-4.5 font-semibold"
            disabled={
              (choice.group === "transcript" && !hasTranscript) ||
              (choice.group === "formats" && choice.format === null)
            }
            onClick={onDownload}
          >
            <DownloadIcon />
            {strings.download.start}
          </Button>
        </div>
      </div>
    </Card>
  );
}
