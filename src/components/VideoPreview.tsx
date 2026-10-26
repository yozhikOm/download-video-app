import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { AudioPreset, Preset, VideoCard, VideoPreset } from "@/backend";
import { formatDuration } from "@/lib/format";
import { strings } from "@/strings";

/**
 * What the user chose on the Video's card: the tab, and the Preset on each.
 * A Video without sound has no Audio tab, so its `audio` is never shown.
 */
export type PresetChoice = {
  group: "video" | "audio";
  video: VideoPreset;
  audio: AudioPreset;
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

/** The probed Video, for the user to confirm it's the right one, choose a Preset and download it. */
export function VideoPreview({
  video,
  videoPresets,
  audioPresets,
  m4aKeepsSound,
  choice,
  onChoose,
  canDownload,
  onDownload,
}: {
  video: VideoCard;
  videoPresets: VideoPreset[];
  audioPresets: AudioPreset[];
  m4aKeepsSound: boolean;
  choice: PresetChoice;
  onChoose: (choice: PresetChoice) => void;
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
      <CardContent className="flex flex-col gap-4">
        <Tabs
          value={choice.group}
          onValueChange={(group) =>
            onChoose({ ...choice, group: group as PresetChoice["group"] })
          }
        >
          <TabsList>
            <TabsTrigger value="video">{strings.presets.videoTab}</TabsTrigger>
            {audioPresets.length > 0 && (
              <TabsTrigger value="audio">
                {strings.presets.audioTab}
              </TabsTrigger>
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
        </Tabs>
        <div>
          <Button disabled={!canDownload} onClick={onDownload}>
            {strings.download.start}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
