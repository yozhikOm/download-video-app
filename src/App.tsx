import { useEffect, useState } from "react";
import {
  api,
  errorMessage,
  type DownloadOptions,
  type TranscriptOptions,
  type VideoInfo,
} from "./api";
import { loadSettings, saveSettings } from "./settings";
import { UrlInput } from "./components/UrlInput";
import { VideoCard } from "./components/VideoCard";
import { FormatPicker } from "./components/FormatPicker";
import { DownloadList, type DownloadItem } from "./components/DownloadList";
import { Settings } from "./components/Settings";

function App() {
  const [info, setInfo] = useState<VideoInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outDir, setOutDir] = useState("");
  const [downloads, setDownloads] = useState<DownloadItem[]>([]);
  const [showSettings, setShowSettings] = useState(false);

  useEffect(() => {
    loadSettings().then(
      (s) => setOutDir(s.outDir),
      (e) => setError(errorMessage(e)),
    );
  }, []);

  async function analyze(url: string) {
    setLoading(true);
    setError(null);
    setInfo(null);
    try {
      setInfo(await api.fetchInfo(url));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }

  function changeOutDir(dir: string) {
    setOutDir(dir);
    saveSettings({ outDir: dir }).catch((e) => setError(errorMessage(e)));
  }

  function addItem(detail: string): string {
    const id = crypto.randomUUID();
    const item: DownloadItem = {
      id,
      title: info?.title ?? "",
      detail,
      percent: 0,
      status: "queued",
    };
    setDownloads((items) => [item, ...items]);
    return id;
  }

  function updateItem(id: string, patch: Partial<DownloadItem>) {
    setDownloads((items) => items.map((i) => (i.id === id ? { ...i, ...patch } : i)));
  }

  async function download(options: DownloadOptions) {
    const detail =
      options.kind === "video"
        ? `${options.height ? `${options.height}p` : "Лучшее"} · ${options.container.toUpperCase()}`
        : `Аудио · ${options.audioFormat.toUpperCase()}`;
    const id = addItem(detail);
    try {
      await api.startDownload(options);
      // Дальше статус будут обновлять события "download://progress".
      updateItem(id, { status: "downloading" });
    } catch (e) {
      updateItem(id, { status: "error", error: errorMessage(e) });
    }
  }

  async function downloadTranscript(options: TranscriptOptions) {
    const id = addItem(`Расшифровка ${options.lang} · ${options.format.toUpperCase()}`);
    try {
      await api.downloadTranscript(options);
      updateItem(id, { status: "done", percent: 100 });
    } catch (e) {
      updateItem(id, { status: "error", error: errorMessage(e) });
    }
  }

  function cancel(id: string) {
    api.cancelDownload(id).then(
      () => updateItem(id, { status: "cancelled" }),
      (e) => updateItem(id, { error: errorMessage(e) }),
    );
  }

  function remove(id: string) {
    setDownloads((items) => items.filter((i) => i.id !== id));
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>Download Video</h1>
        <button
          className="icon"
          aria-label="Настройки"
          title="Настройки"
          onClick={() => setShowSettings(true)}
        >
          <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
            <path
              fill="currentColor"
              d="M19.14 12.94a7.07 7.07 0 0 0 0-1.88l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7.03 7.03 0 0 0-1.63-.94l-.36-2.54a.5.5 0 0 0-.5-.42h-3.84a.5.5 0 0 0-.5.42l-.36 2.54c-.59.24-1.13.56-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.65 8.84a.5.5 0 0 0 .12.64l2.03 1.58a7.07 7.07 0 0 0 0 1.88l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32c.13.22.39.3.6.22l2.39-.96c.5.38 1.04.7 1.63.94l.36 2.54c.05.24.25.42.5.42h3.84c.25 0 .45-.18.5-.42l.36-2.54c.59-.24 1.13-.56 1.63-.94l2.39.96c.22.08.47 0 .6-.22l1.92-3.32a.5.5 0 0 0-.12-.64l-2.03-1.58ZM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7Z"
            />
          </svg>
        </button>
      </header>

      <main className="content">
        <UrlInput loading={loading} onAnalyze={analyze} />
        {error && <p className="error">{error}</p>}
        {loading && <div className="video-card skeleton" />}
        {info && (
          <>
            <VideoCard info={info} />
            <FormatPicker
              key={info.url}
              info={info}
              outDir={outDir}
              onDownload={download}
              onDownloadTranscript={downloadTranscript}
            />
          </>
        )}
      </main>

      <DownloadList items={downloads} onCancel={cancel} onRemove={remove} />

      {showSettings && (
        <Settings
          outDir={outDir}
          onOutDirChange={changeOutDir}
          onClose={() => setShowSettings(false)}
        />
      )}
    </div>
  );
}

export default App;
