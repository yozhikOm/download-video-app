import { useState } from "react";
import type {
  AudioFormat,
  DownloadOptions,
  TranscriptFormat,
  TranscriptOptions,
  VideoContainer,
  VideoInfo,
} from "../api";

type Tab = "video" | "audio" | "transcript";

const TABS: { id: Tab; label: string }[] = [
  { id: "video", label: "Видео" },
  { id: "audio", label: "Аудио" },
  { id: "transcript", label: "Расшифровка" },
];

interface Props {
  info: VideoInfo;
  outDir: string;
  onDownload: (options: DownloadOptions) => void;
  onDownloadTranscript: (options: TranscriptOptions) => void;
}

export function FormatPicker({ info, outDir, onDownload, onDownloadTranscript }: Props) {
  const [tab, setTab] = useState<Tab>("video");
  const [height, setHeight] = useState<number | null>(info.videoQualities[0] ?? null);
  const [container, setContainer] = useState<VideoContainer>("mp4");
  const [audioFormat, setAudioFormat] = useState<AudioFormat>("mp3");
  const [subIndex, setSubIndex] = useState(0);
  const [transcriptFormat, setTranscriptFormat] = useState<TranscriptFormat>("txt");

  function submit() {
    if (tab === "transcript") {
      const track = info.subtitles[subIndex];
      if (!track) return;
      onDownloadTranscript({
        url: info.url,
        lang: track.lang,
        auto: track.auto,
        format: transcriptFormat,
        outDir,
      });
    } else {
      onDownload({ url: info.url, kind: tab, height, container, audioFormat, outDir });
    }
  }

  const noSubtitles = tab === "transcript" && info.subtitles.length === 0;

  return (
    <div className="format-picker">
      <div className="tabs" role="tablist">
        {TABS.map((t) => (
          <button
            key={t.id}
            role="tab"
            aria-selected={tab === t.id}
            className={tab === t.id ? "tab active" : "tab"}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="tab-body">
        {tab === "video" && (
          <>
            <label>
              Качество
              <select
                value={height ?? ""}
                onChange={(e) => setHeight(Number(e.currentTarget.value) || null)}
              >
                {info.videoQualities.map((h) => (
                  <option key={h} value={h}>
                    {h}p
                  </option>
                ))}
                <option value="">Лучшее доступное</option>
              </select>
            </label>
            <label>
              Контейнер
              <select
                value={container}
                onChange={(e) => setContainer(e.currentTarget.value as VideoContainer)}
              >
                <option value="mp4">MP4</option>
                <option value="mkv">MKV</option>
                <option value="webm">WebM</option>
              </select>
            </label>
          </>
        )}

        {tab === "audio" && (
          <label>
            Формат
            <select
              value={audioFormat}
              onChange={(e) => setAudioFormat(e.currentTarget.value as AudioFormat)}
            >
              <option value="mp3">MP3</option>
              <option value="m4a">M4A (AAC)</option>
              <option value="opus">Opus</option>
            </select>
          </label>
        )}

        {tab === "transcript" &&
          (noSubtitles ? (
            <p className="muted">У этого видео нет субтитров.</p>
          ) : (
            <>
              <label>
                Язык
                <select
                  value={subIndex}
                  onChange={(e) => setSubIndex(Number(e.currentTarget.value))}
                >
                  {info.subtitles.map((s, i) => (
                    <option key={`${s.lang}-${s.auto}`} value={i}>
                      {s.name}
                      {s.auto ? " (авто)" : ""}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Формат
                <select
                  value={transcriptFormat}
                  onChange={(e) =>
                    setTranscriptFormat(e.currentTarget.value as TranscriptFormat)
                  }
                >
                  <option value="txt">TXT — сплошной текст</option>
                  <option value="srt">SRT — с таймкодами</option>
                  <option value="vtt">VTT — с таймкодами</option>
                </select>
              </label>
            </>
          ))}
      </div>

      <div className="actions">
        <span className="muted out-dir" title={outDir}>
          Папка: {outDir || "—"}
        </span>
        <button className="primary" onClick={submit} disabled={noSubtitles || !outDir}>
          Скачать
        </button>
      </div>
    </div>
  );
}
