import type { VideoInfo } from "../api";

function formatDuration(totalSeconds: number): string {
  const s = Math.round(totalSeconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

export function VideoCard({ info }: { info: VideoInfo }) {
  return (
    <div className="video-card">
      <div className="thumb">
        {info.thumbnail ? (
          <img src={info.thumbnail} alt="" />
        ) : (
          <svg viewBox="0 0 24 24" width="36" height="36" aria-hidden="true">
            <path d="M8 5v14l11-7z" fill="currentColor" />
          </svg>
        )}
        {info.duration != null && (
          <span className="duration">{formatDuration(info.duration)}</span>
        )}
      </div>
      <div className="meta">
        <h2 title={info.title}>{info.title}</h2>
        {info.uploader && <p className="muted">{info.uploader}</p>}
        <p className="muted url" title={info.url}>
          {info.url}
        </p>
      </div>
    </div>
  );
}
