import { useRef, useState, type ReactNode } from "react";
import { ArrowLeftIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import {
  chooseSaveDir,
  setDefaultAudioPreset,
  setDefaultVideoPreset,
  setFileNameTemplate,
  type AudioPreset,
  type SettingsState,
  type VideoPreset,
} from "@/backend";
import { strings } from "@/strings";

/** Every Video preset, the Video's best first, then highest first. */
const videoPresets = Object.keys(strings.presets.video) as VideoPreset[];

/** Every Audio preset, in the order the Video's card offers them. */
const audioPresets = Object.keys(
  strings.settings.audioPresets,
) as AudioPreset[];

const variables = Object.keys(
  strings.settings.variables,
) as (keyof typeof strings.settings.variables)[];

/**
 * A row of Windows Settings: what it sets and its hints on the left, the
 * control on the right, and a failure to save it below.
 */
function SettingRow({
  label,
  labelFor,
  hint,
  failed,
  children,
}: {
  label: string;
  labelFor?: string;
  hint: ReactNode;
  failed: string | null;
  children: ReactNode;
}) {
  return (
    <Card className="flex-row items-center justify-between gap-4 px-4 py-3">
      <div className="flex min-w-0 flex-col gap-0.5">
        {labelFor ? (
          <label htmlFor={labelFor} className="font-semibold">
            {label}
          </label>
        ) : (
          <p className="font-semibold">{label}</p>
        )}
        <div className="text-xs text-muted-foreground">{hint}</div>
        {failed && (
          <p role="alert" className="text-xs text-destructive">
            {failed}
          </p>
        )}
      </div>
      {children}
    </Card>
  );
}

/** The template field, saved once it is left or on Enter. */
function FileNameTemplate({ saved }: { saved: string }) {
  const field = useRef<HTMLInputElement>(null);
  // What the user typed and the backend hasn't saved yet.
  const [draft, setDraft] = useState<string | null>(null);
  // The template being saved, so that Enter and then leaving the field save
  // it once.
  const saving = useRef<string | null>(null);
  const [saveFailed, setSaveFailed] = useState(false);
  const template = draft ?? saved;

  function save(value: string) {
    if (value === saved || value === saving.current) {
      return;
    }
    saving.current = value;
    setSaveFailed(false);
    setFileNameTemplate(value)
      // What the user typed on since stays.
      .then(() => setDraft((typed) => (typed === value ? null : typed)))
      .catch(() => setSaveFailed(true))
      .finally(() => {
        if (saving.current === value) {
          saving.current = null;
        }
      });
  }

  /** Puts `variable` in where the cursor is, and saves the template. */
  function insert(variable: string) {
    const start = field.current?.selectionStart ?? template.length;
    const end = field.current?.selectionEnd ?? start;
    const value = template.slice(0, start) + variable + template.slice(end);
    setDraft(value);
    save(value);
  }

  return (
    <SettingRow
      label={strings.settings.fileName}
      labelFor="file-name-template"
      hint={
        <>
          <p>{strings.settings.fileNameHint}</p>
          <ul className="mt-1 flex flex-col gap-0.5">
            {variables.map((variable) => (
              <li key={variable} className="flex items-center gap-1.5">
                <Button
                  type="button"
                  variant="outline"
                  size="xs"
                  className="font-mono"
                  // The field keeps the focus and the cursor, and isn't
                  // saved on its own before the variable is in.
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => insert(variable)}
                >
                  {variable}
                </Button>
                <span>{strings.settings.variables[variable]}</span>
              </li>
            ))}
          </ul>
        </>
      }
      failed={saveFailed ? strings.settings.fileNameFailed : null}
    >
      <Input
        ref={field}
        id="file-name-template"
        className="w-64 shrink-0 font-mono"
        spellCheck={false}
        value={template}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => save(template)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            save(template);
          }
        }}
      />
    </SettingRow>
  );
}

/**
 * The app's settings: where files are saved, how they are named, and the
 * Video and Audio presets chosen at first.
 */
export function SettingsScreen({
  settings,
  onBack,
}: {
  settings: SettingsState;
  onBack: () => void;
}) {
  const [saveDirFailed, setSaveDirFailed] = useState(false);
  const [presetFailed, setPresetFailed] = useState(false);
  const [audioPresetFailed, setAudioPresetFailed] = useState(false);

  function chooseFolder() {
    setSaveDirFailed(false);
    chooseSaveDir().catch(() => setSaveDirFailed(true));
  }

  function choosePreset(preset: VideoPreset) {
    setPresetFailed(false);
    setDefaultVideoPreset(preset).catch(() => setPresetFailed(true));
  }

  function chooseAudioPreset(preset: AudioPreset) {
    setAudioPresetFailed(false);
    setDefaultAudioPreset(preset).catch(() => setAudioPresetFailed(true));
  }

  return (
    <div className="flex w-full max-w-[53.75rem] flex-col gap-3.5 self-start">
      <header className="flex min-h-8.5 items-center gap-2">
        <Button
          variant="ghost"
          size="icon"
          aria-label={strings.settings.back}
          onClick={onBack}
        >
          <ArrowLeftIcon />
        </Button>
        <h1 className="font-heading text-xl font-semibold">
          {strings.settings.title}
        </h1>
      </header>
      <SettingRow
        label={strings.settings.saveDir}
        hint={<p className="break-all">{settings.saveDir}</p>}
        failed={saveDirFailed ? strings.settings.saveDirFailed : null}
      >
        <Button variant="outline" onClick={chooseFolder}>
          {strings.settings.chooseSaveDir}
        </Button>
      </SettingRow>
      <FileNameTemplate saved={settings.fileNameTemplate} />
      <SettingRow
        label={strings.settings.defaultVideoPreset}
        labelFor="default-video-preset"
        hint={<p>{strings.settings.defaultVideoPresetHint}</p>}
        failed={presetFailed ? strings.settings.defaultVideoPresetFailed : null}
      >
        <NativeSelect
          id="default-video-preset"
          className="shrink-0"
          value={settings.defaultVideoPreset}
          onChange={(event) => choosePreset(event.target.value as VideoPreset)}
        >
          {videoPresets.map((preset) => (
            <NativeSelectOption key={preset} value={preset}>
              {strings.presets.video[preset]}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </SettingRow>
      <SettingRow
        label={strings.settings.defaultAudioPreset}
        labelFor="default-audio-preset"
        hint={<p>{strings.settings.defaultAudioPresetHint}</p>}
        failed={
          audioPresetFailed ? strings.settings.defaultAudioPresetFailed : null
        }
      >
        <NativeSelect
          id="default-audio-preset"
          className="shrink-0"
          value={settings.defaultAudioPreset}
          onChange={(event) =>
            chooseAudioPreset(event.target.value as AudioPreset)
          }
        >
          {audioPresets.map((preset) => (
            <NativeSelectOption key={preset} value={preset}>
              {strings.settings.audioPresets[preset]}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </SettingRow>
    </div>
  );
}
