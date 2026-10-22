import type { Browser, Package, Tool } from "@/backend";

/** Every piece of UI text. */
export const strings = {
  tools: {
    ytDlp: "yt-dlp",
    ffmpeg: "ffmpeg",
    ffprobe: "ffprobe",
    deno: "deno",
  } satisfies Record<Tool | Package, string>,
  browsers: {
    brave: "Brave",
    chrome: "Chrome",
    chromium: "Chromium",
    edge: "Edge",
    firefox: "Firefox",
    opera: "Opera",
    vivaldi: "Vivaldi",
    whale: "Whale",
  } satisfies Record<Browser, string>,
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
    signInRequired:
      "Видео требует входа в аккаунт. Выберите ниже браузер, в котором вы вошли в YouTube.",
    signInRequiredSignedIn: (browser: string) =>
      `Видео требует входа, а в ${browser} не выполнен вход в YouTube или у аккаунта нет доступа к этому видео.`,
    botCheck:
      "YouTube просит подтвердить, что вы не робот. Так бывает в рабочих сетях и через VPN — выберите ниже браузер, в котором вы вошли в YouTube, или попробуйте из другой сети.",
    botCheckSignedIn: (browser: string) =>
      `YouTube всё равно просит подтвердить, что вы не робот. Проверьте, что в ${browser} выполнен вход в YouTube.`,
    browserNotFound: (browser: string) =>
      `Не удалось найти данные ${browser} — браузер не установлен или в нём ещё не открывали сайты. Выберите другой браузер.`,
    browserOpen: (browser: string) =>
      `${browser} не даёт прочитать cookies, пока он открыт. Закройте его полностью и нажмите «Найти» ещё раз.`,
    cantDecrypt: (browser: string) =>
      `${browser} шифрует cookies так, что другие программы не могут их прочитать. Выберите другой браузер, например Firefox.`,
    truncatedUrl:
      "Ссылка неполная — похоже, часть её потерялась при копировании. Скопируйте её ещё раз.",
    unsupported: "Сайт не поддерживается или по ссылке нет видео.",
    other: "Не удалось получить сведения о видео. Попробуйте ещё раз.",
  },
  signIn: {
    browser: "Вход в YouTube",
    none: "Не использовать",
    hint: "Нужен, если YouTube просит подтвердить, что вы не робот, или видео доступно только после входа.",
    warning: (browser: string) =>
      `Приложение будет обращаться к YouTube от имени вашего аккаунта в ${browser}. Если скачивать очень много, YouTube может ограничить этот аккаунт.`,
    saveFailed: "Не удалось сохранить выбор браузера.",
  },
};
