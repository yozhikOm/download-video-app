import type { Package, Tool } from "@/backend";

/** Every piece of UI text. */
export const strings = {
  tools: {
    ytDlp: "yt-dlp",
    ffmpeg: "ffmpeg",
    ffprobe: "ffprobe",
  } satisfies Record<Tool | Package, string>,
  setup: {
    title: "Скачиваю yt-dlp и ffmpeg…",
    progress: (downloadedMb: string, totalMb: string) =>
      `${downloadedMb} из ${totalMb} МБ`,
    broken: (tool: string) => `${tool} повреждён — восстанавливаю`,
    failedTitle: "Не удалось подготовить yt-dlp и ffmpeg",
    offline: "Нет подключения к интернету. Проверьте сеть и повторите.",
    checksumMismatch: (tool: string) =>
      `Скачанный ${tool} не прошёл проверку контрольной суммы и удалён.`,
    other: "Что-то пошло не так при скачивании. Попробуйте ещё раз.",
    retry: "Повторить",
  },
  main: {
    version: (version: string) => `Версия ${version}`,
    versionFailed: "Не удалось получить версию приложения",
  },
};
