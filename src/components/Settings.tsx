import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorMessage } from "../api";

interface Props {
  outDir: string;
  onOutDirChange: (dir: string) => void;
  onClose: () => void;
}

export function Settings({ outDir, onOutDirChange, onClose }: Props) {
  const [ytdlp, setYtdlp] = useState("…");

  useEffect(() => {
    api.ytdlpVersion().then(setYtdlp, (e) => setYtdlp(errorMessage(e)));
  }, []);

  async function pickFolder() {
    const dir = await open({ directory: true, defaultPath: outDir || undefined });
    if (typeof dir === "string") onOutDirChange(dir);
  }

  return (
    <div className="overlay" onClick={onClose}>
      <div
        className="dialog"
        role="dialog"
        aria-label="Настройки"
        onClick={(e) => e.stopPropagation()}
      >
        <h3>Настройки</h3>

        <div className="field">
          <span className="label">Папка для загрузок</span>
          <div className="folder">
            <span className="path" title={outDir}>
              {outDir || "—"}
            </span>
            <button onClick={pickFolder}>Выбрать…</button>
          </div>
        </div>

        <div className="field">
          <span className="label">yt-dlp</span>
          <span className="muted">{ytdlp}</span>
        </div>

        <div className="dialog-actions">
          <button className="primary" onClick={onClose}>
            Готово
          </button>
        </div>
      </div>
    </div>
  );
}
