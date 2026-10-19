import { useState } from "react";

interface Props {
  loading: boolean;
  onAnalyze: (url: string) => void;
}

export function UrlInput({ loading, onAnalyze }: Props) {
  const [url, setUrl] = useState("");

  return (
    <form
      className="url-input"
      onSubmit={(e) => {
        e.preventDefault();
        onAnalyze(url);
      }}
    >
      <input
        type="url"
        value={url}
        onChange={(e) => setUrl(e.currentTarget.value)}
        placeholder="Вставьте ссылку на видео (YouTube, VK, Rutube…)"
        autoFocus
        spellCheck={false}
      />
      <button type="submit" className="primary" disabled={loading || !url.trim()}>
        {loading ? "Анализ…" : "Анализ"}
      </button>
    </form>
  );
}
