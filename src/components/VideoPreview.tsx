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
import type { VideoCard, VideoPreset } from "@/backend";
import { formatDuration } from "@/lib/format";
import { strings } from "@/strings";

/** The Video presets the Video has, to choose the quality from. */
function VideoPresetPicker({
  presets,
  chosen,
  onChoose,
}: {
  presets: VideoPreset[];
  chosen: VideoPreset;
  onChoose: (preset: VideoPreset) => void;
}) {
  return (
    <RadioGroup
      aria-label={strings.presets.quality}
      value={chosen}
      onValueChange={(value) => onChoose(value as VideoPreset)}
      className="flex flex-wrap gap-x-4"
    >
      {presets.map((preset) => (
        <label key={preset} className="flex items-center gap-2">
          <RadioGroupItem value={preset} />
          {strings.presets.video[preset]}
        </label>
      ))}
    </RadioGroup>
  );
}

/** The probed Video, for the user to confirm it's the right one, choose a Preset and download it. */
export function VideoPreview({
  video,
  videoPresets,
  videoPreset,
  onChooseVideoPreset,
  canDownload,
  onDownload,
}: {
  video: VideoCard;
  videoPresets: VideoPreset[];
  videoPreset: VideoPreset;
  onChooseVideoPreset: (preset: VideoPreset) => void;
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
        <Tabs defaultValue="video">
          <TabsList>
            <TabsTrigger value="video">{strings.presets.videoTab}</TabsTrigger>
          </TabsList>
          <TabsContent value="video">
            <VideoPresetPicker
              presets={videoPresets}
              chosen={videoPreset}
              onChoose={onChooseVideoPreset}
            />
          </TabsContent>
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
