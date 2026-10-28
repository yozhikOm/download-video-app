import type {
  AudioPreset,
  Browser,
  Failure,
  Package,
  Receiving,
  Tool,
  VideoPreset,
  YtDlpUpdate,
} from "@/backend";

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
    title: "Скачать видео",
    settings: "Настройки",
  },
  settings: {
    title: "Настройки",
    back: "Назад",
    saveDir: "Папка сохранения",
    chooseSaveDir: "Изменить…",
    saveDirFailed: "Не удалось сохранить папку.",
    fileName: "Имя файла",
    fileNameHint:
      "Шаблон, по которому называются видео, аудио и транскрипты. Нажмите на переменную, чтобы вставить её:",
    /** What each variable of the file name template puts in. */
    variables: {
      "{title}": "название видео",
      "{channel}": "канал",
      "{date}": "дата публикации, ГГГГ-ММ-ДД",
    },
    fileNameFailed: "Не удалось сохранить шаблон имени.",
    defaultVideoPreset: "Качество видео по умолчанию",
    defaultVideoPresetHint:
      "Выбирается после поиска видео. Если у видео такого нет — ближайшее меньшее.",
    defaultVideoPresetFailed: "Не удалось сохранить качество.",
    defaultAudioPreset: "Формат аудио по умолчанию",
    defaultAudioPresetHint:
      "Выбирается на вкладке «Аудио» после поиска видео. M4A сохраняет звук без перекодирования, если сайт отдаёт AAC.",
    /** The Audio presets apart from any Video, so M4A isn't called original. */
    audioPresets: {
      m4a: "M4A",
      mp3: "MP3",
    } satisfies Record<AudioPreset, string>,
    defaultAudioPresetFailed: "Не удалось сохранить формат.",
    version: (version: string) => `Версия ${version}`,
    versionFailed: "Не удалось получить версию приложения",
  },
  ytdlp: {
    update: "Обновить yt-dlp",
    updating: "Обновляю yt-dlp…",
    waiting:
      "Новая версия yt-dlp установится, когда закончится текущая загрузка.",
    updated: (version: string) => `yt-dlp обновлён до версии ${version}.`,
    upToDate: (version: string) =>
      `Установлена последняя версия yt-dlp: ${version}.`,
    offline: "Не удалось обновить yt-dlp: нет подключения к интернету.",
    checksumMismatch:
      "Не удалось обновить yt-dlp: скачанный файл не прошёл проверку контрольной суммы.",
    other: "Не удалось обновить yt-dlp. Попробуйте ещё раз.",
  },
  probe: {
    url: "Ссылка на видео",
    urlPlaceholder: "Вставьте сюда ссылку на видео",
    clearUrl: "Очистить",
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
  presets: {
    videoTab: "Видео",
    audioTab: "Аудио",
    quality: "Качество",
    audioFormat: "Формат",
    video: {
      best: "Лучшее",
      "2160p": "2160p",
      "1440p": "1440p",
      "1080p": "1080p",
      "720p": "720p",
      "480p": "480p",
      "360p": "360p",
    } satisfies Record<VideoPreset, string>,
    audio: {
      m4a: "M4A (оригинал)",
      mp3: "MP3",
    } satisfies Record<AudioPreset, string>,
    /** M4A for a Video whose sound it encodes again. */
    m4aEncoded: "M4A",
  },
  transcript: {
    tab: "Транскрипт",
    save: "Сохранить транскрипт",
    language: "Язык транскрипта",
    /** A language whose text comes from the site's captions. */
    automatic: (language: string) => `${language} (автосубтитры)`,
    none: "У этого видео нет транскрипта.",
  },
  download: {
    start: "Скачать",
    downloading: {
      transcript: "Скачивание транскрипта…",
      video: "Скачивание видео…",
      audio: "Скачивание аудио…",
    } satisfies Record<Receiving, string>,
    percent: (percent: string) => `${percent}%`,
    speed: (mbPerSecond: string) => `${mbPerSecond} МБ/с`,
    eta: (time: string) => `осталось ${time}`,
    queued: "В очереди",
    processing: "Обработка…",
    done: "Готово",
    openFile: "Открыть файл",
    showInFolder: "Показать в папке",
    /** Why a Download failed, one text for each kind of failure. */
    failed: {
      offline:
        "Нет подключения к интернету или сайт не отвечает. Проверьте сеть и повторите.",
      unsupported: "Сайт не поддерживается или видео больше недоступно.",
      signInRequired:
        "Видео доступно только после входа в аккаунт. Если это YouTube, выберите в «Вход в YouTube» браузер, в котором вы вошли, и повторите.",
      extractorBroken:
        "yt-dlp не смог разобрать страницу видео — похоже, сайт изменился. Обычно это исправляет новая версия yt-dlp.",
      disk: "Не удалось записать файл — на диске нет места или нет доступа к папке.",
      unknown: "Не удалось скачать видео. Попробуйте ещё раз.",
    } satisfies Record<Failure, string>,
    /** A broken extractor, by what became of the update of yt-dlp it started. */
    extractorBroken: {
      updating:
        "yt-dlp не смог разобрать страницу видео — похоже, сайт изменился. Обновляю yt-dlp…",
      updated:
        "yt-dlp не смог разобрать страницу видео — похоже, сайт изменился. yt-dlp обновлён, попробуйте ещё раз.",
      upToDate:
        "yt-dlp не смог разобрать страницу видео — похоже, сайт изменился. Исправления в новой версии yt-dlp пока нет — попробуйте позже.",
      failed:
        "yt-dlp не смог разобрать страницу видео — похоже, сайт изменился. Обновить yt-dlp не удалось — попробуйте позже.",
    } satisfies Record<YtDlpUpdate, string>,
    copyDetails: "Скопировать подробности",
    detailsCopied: "Подробности скопированы",
    copyFailed: "Не удалось скопировать подробности.",
    /** The details of a Failed Download, to paste into a GitHub issue. */
    details: {
      url: (url: string) => `Ссылка: ${url}`,
      preset: (preset: string) => `Preset: ${preset}`,
      transcript: (language: string) => `Транскрипт: ${language}`,
      message: (message: string) => `Ошибка: ${message}`,
      output: "Последние строки вывода yt-dlp:",
    },
    retry: "Повторить",
    cancel: "Отменить",
    cancelled: "Отменено",
    actionFailed:
      "Не удалось открыть файл — возможно, его переместили или удалили.",
    duplicate:
      "Это видео с теми же настройками уже скачивается или ждёт в очереди.",
    remove: "Убрать",
  },
  queue: {
    title: "Очередь",
    clearDone: "Очистить завершённые",
    /** What a Download saves: its Preset, with or without the Transcript. */
    withTranscript: (preset: string) => `${preset} + транскрипт`,
    transcriptAlone: "Транскрипт",
  },
  close: {
    title: "Есть незавершённые загрузки. Закрыть?",
    description:
      "Скачивание прервётся. Если в течение недели снова добавить то же видео с теми же настройками, оно докачается с того же места.",
    confirm: "Закрыть",
    decline: "Не закрывать",
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
