import type { DownloadStatus } from "../api";

export interface DownloadItem {
  id: string;
  title: string;
  /** Например «1080p · MP4» или «Расшифровка · TXT». */
  detail: string;
  percent: number;
  status: DownloadStatus;
  error?: string;
}

const STATUS_LABEL: Record<DownloadStatus, string> = {
  queued: "В очереди",
  downloading: "Загрузка",
  processing: "Обработка",
  done: "Готово",
  error: "Ошибка",
  cancelled: "Отменено",
};

const ACTIVE: DownloadStatus[] = ["queued", "downloading", "processing"];

interface Props {
  items: DownloadItem[];
  onCancel: (id: string) => void;
  onRemove: (id: string) => void;
}

export function DownloadList({ items, onCancel, onRemove }: Props) {
  return (
    <section className="download-list">
      <h3>Загрузки</h3>
      {items.length === 0 ? (
        <p className="muted empty">Здесь появятся загрузки</p>
      ) : (
        <ul>
          {items.map((item) => {
            const active = ACTIVE.includes(item.status);
            return (
              <li key={item.id} className={`download-row status-${item.status}`}>
                <div className="row-head">
                  <span className="title" title={item.title}>
                    {item.title}
                  </span>
                  <span className="muted">{item.detail}</span>
                </div>
                <div className="progress" aria-hidden="true">
                  <div className="bar" style={{ width: `${item.percent}%` }} />
                </div>
                <div className="row-foot">
                  <span className="status">
                    {STATUS_LABEL[item.status]}
                    {item.error && `: ${item.error}`}
                  </span>
                  {active ? (
                    <button className="link" onClick={() => onCancel(item.id)}>
                      Отмена
                    </button>
                  ) : (
                    <button className="link" onClick={() => onRemove(item.id)}>
                      Убрать
                    </button>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
