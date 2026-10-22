import type { Package, Tool } from "@/backend";

/** Every piece of UI text. */
export const strings = {
  tools: {
    ytDlp: "yt-dlp",
    ffmpeg: "ffmpeg",
    ffprobe: "ffprobe",
  } satisfies Record<Tool | Package, string>,
  setup: {
    title: "Идёт загрузка необходимых для работы файлов…",
    progress: (downloadedMb: string, totalMb: string) =>
      `${downloadedMb} из ${totalMb} МБ`,
    broken: (tool: string) => `${tool} повреждён — восстанавливаю`,
    failedTitle: "Не удалось загрузить необходимые для работы файлы",
    offline: "Нет подключения к интернету. Проверьте сеть и повторите.",
    checksumMismatch: (packageName: string) =>
      `Скачанный ${packageName} не прошёл проверку контрольной суммы и удалён.`,
    other: "Что-то пошло не так при скачивании. Попробуйте ещё раз.",
    retry: "Повторить",
  },
  main: {
    version: (version: string) => `Версия ${version}`,
    versionFailed: "Не удалось получить версию приложения",
  },
  probe: {
    url: "Ссылка на видео",
    urlPlaceholder: "https://www.youtube.com/watch?v=…",
    submit: "Найти",
    probing: "Получаю сведения о видео…",
    thumbnail: "Превью",
    playlist: "Плейлисты пока не поддерживаются.",
    live: "Трансляция ещё идёт — скачать можно будет после окончания.",
    signInRequired: "Видео требует входа в аккаунт — пока не поддерживается.",
    unsupported: "Сайт не поддерживается или по ссылке нет видео.",
    other: "Не удалось получить сведения о видео. Попробуйте ещё раз.",
  },
};
