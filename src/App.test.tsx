import type { Channel } from "@tauri-apps/api/core";
import { mockIPC } from "@tauri-apps/api/mocks";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { expect, test } from "vitest";
import App from "./App";
import type {
  Added,
  AppEvent,
  AudioPreset,
  Browser,
  Download,
  Preset,
  ProbeState,
  SettingsState,
  SetupState,
  TranscriptLanguage,
  VideoCard,
  VideoPreset,
  YtDlpUpdate,
} from "./backend";

const MB = 1024 * 1024;

/** The settings of an app just installed. */
const defaultSettings: SettingsState = {
  signInBrowser: null,
  saveDir: "C:\\Users\\me\\Downloads",
  fileNameTemplate: "{title}",
  defaultVideoPreset: "1080p",
};

/**
 * A Probe that found `video`, with the Presets of a 1080p Video with sound
 * and its Transcript in English and Russian.
 */
function probed(
  video: VideoCard,
  videoPresets: VideoPreset[] = ["1080p", "720p", "480p", "360p"],
  defaultVideoPreset: VideoPreset = "1080p",
  audioPresets: AudioPreset[] = ["m4a", "mp3"],
  m4aKeepsSound = true,
  transcriptLanguages: TranscriptLanguage[] = [
    { language: "ru", automatic: false },
    { language: "en", automatic: false },
  ],
  defaultTranscriptLanguage: string | null = "en",
): ProbeState {
  return {
    status: "probed",
    video,
    videoPresets,
    defaultVideoPreset,
    audioPresets,
    m4aKeepsSound,
    transcriptLanguages,
    defaultTranscriptLanguage,
  };
}

/** A Download as the backend sends it, without what tells it apart. */
type DownloadState = Download extends infer D
  ? D extends Download
    ? Omit<D, "id" | "url" | "preset" | "transcriptLanguage">
    : never
  : never;

/**
 * A Download in the Queue: of 1080p without a Transcript unless told, of
 * the Video at `youtubeUrl`.
 */
function queued(
  id: number,
  state: DownloadState,
  preset: Preset | null = "1080p",
  transcriptLanguage: string | null = null,
): Download {
  return { id, url: youtubeUrl, preset, transcriptLanguage, ...state };
}

/**
 * Mocks the backend; `setup`, `probe`, `settings` and `queue` push a state
 * to the UI's channel, and `download` pushes a Queue of one Download.
 * Adding or retrying a Download finds a duplicate of the Download with id
 * `duplicateOf` when told.
 */
function mockBackend({
  appInfoFails = false,
  probeFails = false,
  savingFails = false,
  openingFails = false,
  downloadFails = false,
  duplicateOf = null as number | null,
} = {}) {
  const added: Added =
    duplicateOf === null
      ? { status: "queued" }
      : { status: "duplicate", of: duplicateOf };
  let channel: Channel<AppEvent> | undefined;
  const commands: string[] = [];
  const probedUrls: string[] = [];
  const signInBrowsers: (Browser | null)[] = [];
  const fileNameTemplates: string[] = [];
  const defaultVideoPresets: VideoPreset[] = [];
  const downloadedPresets: (Preset | null)[] = [];
  const downloadedTranscripts: (string | null)[] = [];
  /** The ids each command about one Download was sent with. */
  const downloadIds: Record<string, number[]> = {};
  mockIPC((cmd, args) => {
    commands.push(cmd);
    const { id } = (args ?? {}) as { id?: number };
    if (id !== undefined) {
      (downloadIds[cmd] ??= []).push(id);
    }
    switch (cmd) {
      case "probe":
        probedUrls.push((args as { url: string }).url);
        if (probeFails) {
          throw new Error("backend is down");
        }
        return null;
      case "set_sign_in_browser":
        if (savingFails) {
          throw new Error("disk is full");
        }
        signInBrowsers.push((args as { browser: Browser | null }).browser);
        return null;
      case "choose_save_dir":
        if (savingFails) {
          throw new Error("disk is full");
        }
        return null;
      case "set_file_name_template":
        if (savingFails) {
          throw new Error("disk is full");
        }
        fileNameTemplates.push((args as { template: string }).template);
        return null;
      case "set_default_video_preset":
        if (savingFails) {
          throw new Error("disk is full");
        }
        defaultVideoPresets.push((args as { preset: VideoPreset }).preset);
        return null;
      case "add_download":
        if (downloadFails) {
          throw new Error("no probed Video");
        }
        downloadedPresets.push((args as { preset: Preset | null }).preset);
        downloadedTranscripts.push(
          (args as { transcript: string | null }).transcript,
        );
        return added;
      case "retry_download":
        return added;
      case "cancel_download":
        return null;
      case "open_downloaded_file":
        if (openingFails) {
          throw new Error("file is gone");
        }
        return null;
      case "subscribe":
        channel = (args as { events: Channel<AppEvent> }).events;
        return null;
      case "app_info":
        if (appInfoFails) {
          throw new Error("backend is down");
        }
        return { name: "download-video-app", version: "1.2.3" };
    }
  });

  async function send(event: AppEvent) {
    await waitFor(() => expect(channel).toBeDefined());
    act(() => channel!.onmessage(event));
  }

  return {
    commands,
    probedUrls,
    signInBrowsers,
    fileNameTemplates,
    defaultVideoPresets,
    downloadedPresets,
    downloadedTranscripts,
    downloadIds,
    setup: (state: SetupState) => send({ type: "setup", data: state }),
    probe: (state: ProbeState) => send({ type: "probe", data: state }),
    /** Pushes the settings, the defaults where not told. */
    settings: (state: Partial<SettingsState>) =>
      send({ type: "settings", data: { ...defaultSettings, ...state } }),
    queue: (...downloads: Download[]) =>
      send({ type: "queue", data: downloads }),
    download: (state: DownloadState) =>
      send({ type: "queue", data: [queued(1, state)] }),
    closeRequested: () => send({ type: "closeRequested" }),
  };
}

test("first run shows the setup screen with download progress", async () => {
  const backend = mockBackend();
  render(<App />);

  await backend.setup({
    status: "installing",
    downloaded: 50 * MB,
    total: 200 * MB,
    broken: null,
  });

  expect(
    screen.getByRole("heading", {
      name: "Идёт загрузка необходимых для работы файлов…",
    }),
  ).toBeInTheDocument();
  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "25",
  );
  expect(screen.getByText("50 из 200 МБ")).toBeInTheDocument();
});

test("setup shows the download screen before the download size is known", async () => {
  const backend = mockBackend();
  render(<App />);

  await backend.setup({
    status: "installing",
    downloaded: 0,
    total: 0,
    broken: null,
  });

  expect(
    screen.getByRole("heading", {
      name: "Идёт загрузка необходимых для работы файлов…",
    }),
  ).toBeInTheDocument();
  expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
  expect(screen.queryByText(/МБ/)).not.toBeInTheDocument();
});

test("the main screen with name and version follows a finished setup", async () => {
  const backend = mockBackend();
  render(<App />);
  await backend.setup({
    status: "installing",
    downloaded: 0,
    total: 200 * MB,
    broken: null,
  });

  await backend.setup({ status: "ready" });

  expect(
    await screen.findByRole("heading", { name: "download-video-app" }),
  ).toBeInTheDocument();
  expect(screen.getByText("Версия 1.2.3")).toBeInTheDocument();
  expect(
    screen.queryByRole("heading", {
      name: "Идёт загрузка необходимых для работы файлов…",
    }),
  ).not.toBeInTheDocument();
});

test("setup without internet shows an error and retries on request", async () => {
  const backend = mockBackend();
  render(<App />);

  await backend.setup({
    status: "failed",
    error: { kind: "offline" },
    broken: null,
  });

  expect(
    screen.getByRole("heading", {
      name: "Не удалось загрузить необходимые для работы файлы",
    }),
  ).toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Нет подключения к интернету",
  );
  fireEvent.click(screen.getByRole("button", { name: "Повторить" }));
  await waitFor(() => expect(backend.commands).toContain("retry_setup"));
});

test("a checksum mismatch names the tool that failed verification", async () => {
  const backend = mockBackend();
  render(<App />);

  await backend.setup({
    status: "failed",
    error: { kind: "checksumMismatch", package: "deno" },
    broken: null,
  });

  expect(screen.getByRole("alert")).toHaveTextContent(
    "deno не прошёл проверку контрольной суммы",
  );
});

test("a tool that failed to start is reported as broken while it is restored", async () => {
  const backend = mockBackend();
  render(<App />);

  await backend.setup({
    status: "installing",
    downloaded: 0,
    total: 0,
    broken: "ytDlp",
  });

  expect(
    screen.getByText("yt-dlp повреждён — восстанавливаю"),
  ).toBeInTheDocument();
});

test("main screen reports when the app version fails to load", async () => {
  const backend = mockBackend({ appInfoFails: true });
  render(<App />);

  await backend.setup({ status: "ready" });

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Не удалось получить версию приложения",
  );
});

/** Renders the main screen after a finished setup. */
async function startReady(options?: Parameters<typeof mockBackend>[0]) {
  const backend = mockBackend(options);
  render(<App />);
  await backend.setup({ status: "ready" });
  return backend;
}

test("a pasted URL is probed on Enter while the screen says so", async () => {
  const backend = await startReady();

  const input = await screen.findByRole("textbox", { name: "Ссылка на видео" });
  fireEvent.change(input, {
    target: { value: "  https://www.youtube.com/watch?v=-moW9jvvMr4  " },
  });
  fireEvent.submit(input);
  await waitFor(() =>
    expect(backend.probedUrls).toEqual([
      "https://www.youtube.com/watch?v=-moW9jvvMr4",
    ]),
  );
  await backend.probe({ status: "probing" });

  expect(screen.getByText("Получаю сведения о видео…")).toBeInTheDocument();
});

test("an empty URL isn't probed", async () => {
  const backend = await startReady();

  fireEvent.submit(
    await screen.findByRole("textbox", { name: "Ссылка на видео" }),
  );

  expect(backend.probedUrls).toEqual([]);
});

test("a probed Video shows its thumbnail, title, channel and duration", async () => {
  const backend = await startReady();

  await backend.probe(
    probed({
      title: "A Simple Way to Break a Bad Habit | Judson Brewer | TED",
      channel: "TED",
      thumbnail: "https://i.ytimg.com/vi/-moW9jvvMr4/maxresdefault.jpg",
      duration: 565,
    }),
  );

  expect(
    screen.getByRole("heading", {
      name: "A Simple Way to Break a Bad Habit | Judson Brewer | TED",
    }),
  ).toBeInTheDocument();
  expect(screen.getByText("TED")).toBeInTheDocument();
  expect(screen.getByText("9:25")).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "Превью" })).toHaveAttribute(
    "src",
    "https://i.ytimg.com/vi/-moW9jvvMr4/maxresdefault.jpg",
  );
});

test("a duration over an hour shows hours", async () => {
  const backend = await startReady();

  await backend.probe(
    probed({
      title: "Around the World with Peppa Pig",
      channel: null,
      thumbnail: null,
      duration: 14339,
    }),
  );

  expect(screen.getByText("3:58:59")).toBeInTheDocument();
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
});

test.each([
  [{ kind: "playlist" }, "Плейлисты пока не поддерживаются"],
  [
    { kind: "live" },
    "Трансляция ещё идёт — скачать можно будет после окончания",
  ],
  [
    { kind: "signInRequired" },
    "Видео требует входа в аккаунт. Выберите ниже браузер, в котором вы вошли в YouTube",
  ],
  [
    { kind: "botCheck" },
    "YouTube просит подтвердить, что вы не робот. Так бывает в рабочих сетях и через VPN — выберите ниже браузер, в котором вы вошли в YouTube, или попробуйте из другой сети",
  ],
  [
    { kind: "truncatedUrl" },
    "Ссылка неполная — похоже, часть её потерялась при копировании. Скопируйте её ещё раз",
  ],
  [{ kind: "unsupported" }, "Сайт не поддерживается или по ссылке нет видео"],
  [
    { kind: "other", message: "ERROR: something broke" },
    "Не удалось получить сведения о видео",
  ],
] as const)(
  "a URL that can't be downloaded (%o) shows why instead of a card",
  async (error, text) => {
    const backend = await startReady();

    await backend.probe({ status: "failed", error, signInBrowser: null });

    expect(screen.getByRole("alert")).toHaveTextContent(text);
    expect(screen.queryAllByRole("heading")).toHaveLength(1);
  },
);

const youtubeUrl = "https://www.youtube.com/watch?v=jNQXAC9IVRw";

/**
 * The user submits a YouTube URL, and YouTube answers its Probe with the bot
 * check; no sign-in browser was used.
 */
function botCheck(backend: ReturnType<typeof mockBackend>) {
  submitUrl(youtubeUrl);
  return backend.probe({
    status: "failed",
    error: { kind: "botCheck" },
    signInBrowser: null,
  });
}

test("signing in stays hidden until YouTube asks for it", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: null });
  await backend.probe({
    status: "failed",
    error: { kind: "unsupported" },
    signInBrowser: null,
  });

  expect(
    screen.queryByRole("combobox", { name: "Вход в YouTube" }),
  ).not.toBeInTheDocument();
});

test.each([{ kind: "botCheck" }, { kind: "signInRequired" }] as const)(
  "%o offers signing in, with no browser chosen by default",
  async (error) => {
    const backend = await startReady();
    await backend.settings({ signInBrowser: null });
    submitUrl(youtubeUrl);

    await backend.probe({ status: "failed", error, signInBrowser: null });

    expect(
      screen.getByRole("combobox", { name: "Вход в YouTube" }),
    ).toHaveDisplayValue("Не использовать");
    expect(
      screen.queryByText(/от имени вашего аккаунта/),
    ).not.toBeInTheDocument();
  },
);

test("signing in is hidden again when the next Probe needs no sign-in", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: null });
  await botCheck(backend);

  await backend.probe({ status: "probing" });

  expect(
    screen.queryByRole("combobox", { name: "Вход в YouTube" }),
  ).not.toBeInTheDocument();
});

test("a chosen sign-in browser stays hidden while the URL field is empty", async () => {
  const backend = await startReady();

  await backend.settings({ signInBrowser: "firefox" });

  expect(
    screen.queryByRole("combobox", { name: "Вход в YouTube" }),
  ).not.toBeInTheDocument();
});

/** Types a URL into the field without submitting it. */
function enterUrl(url: string) {
  fireEvent.change(screen.getByRole("textbox", { name: "Ссылка на видео" }), {
    target: { value: url },
  });
}

/** Types a URL into the field and presses «Найти». */
function submitUrl(url: string) {
  enterUrl(url);
  fireEvent.submit(screen.getByRole("textbox", { name: "Ссылка на видео" }));
}

const probedVideo = probed({
  title: "A Simple Way to Break a Bad Habit | Judson Brewer | TED",
  channel: "TED",
  thumbnail: null,
  duration: 565,
});

test("submitting a URL hides signing in, even with a chosen browser", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: "firefox" });

  submitUrl("https://www.youtube.com/watch?v=-moW9jvvMr4");

  expect(
    screen.queryByRole("combobox", { name: "Вход в YouTube" }),
  ).not.toBeInTheDocument();
});

test("a probed Video keeps signing in hidden", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: "firefox" });
  submitUrl("https://www.youtube.com/watch?v=-moW9jvvMr4");

  await backend.probe({ status: "probing" });
  await backend.probe(probedVideo);

  expect(
    screen.queryByRole("combobox", { name: "Вход в YouTube" }),
  ).not.toBeInTheDocument();
});

test("another YouTube URL after a probed Video shows signing in again", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: "firefox" });
  submitUrl("https://www.youtube.com/watch?v=-moW9jvvMr4");
  await backend.probe(probedVideo);

  enterUrl("https://www.youtube.com/watch?v=dQw4w9WgXcQ");

  expect(
    screen.getByRole("combobox", { name: "Вход в YouTube" }),
  ).toHaveDisplayValue("Firefox");
});

test.each([
  { kind: "botCheck" },
  { kind: "cookiesUnreadable", reason: "browserOpen" },
] as const)(
  "a submitted URL failing with %o shows signing in again",
  async (error) => {
    const backend = await startReady();
    await backend.settings({ signInBrowser: "firefox" });
    submitUrl("https://www.youtube.com/watch?v=-moW9jvvMr4");

    await backend.probe({ status: "failed", error, signInBrowser: "firefox" });

    expect(
      screen.getByRole("combobox", { name: "Вход в YouTube" }),
    ).toHaveDisplayValue("Firefox");
  },
);

test("signing in is hidden for a URL of another site", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: null });
  await botCheck(backend);

  enterUrl("https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/");

  expect(
    screen.queryByRole("combobox", { name: "Вход в YouTube" }),
  ).not.toBeInTheDocument();
});

test("a chosen sign-in browser is hidden for a URL of another site", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: "firefox" });

  enterUrl("https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/");

  expect(
    screen.queryByRole("combobox", { name: "Вход в YouTube" }),
  ).not.toBeInTheDocument();
});

test.each([
  "https://www.youtube.com/watch?v=-moW9jvvMr4",
  "youtube.com/watch?v=-moW9jvvMr4",
  "https://m.youtube.com/watch?v=-moW9jvvMr4",
  "https://youtu.be/-moW9jvvMr4",
  "https://www.youtube-nocookie.com/embed/-moW9jvvMr4",
])(
  "a chosen sign-in browser stays visible for %s so that it can be turned off",
  async (url) => {
    const backend = await startReady();
    await backend.settings({ signInBrowser: "firefox" });

    enterUrl(url);

    expect(
      screen.getByRole("combobox", { name: "Вход в YouTube" }),
    ).toHaveDisplayValue("Firefox");
  },
);

test("choosing a sign-in browser saves it and warns about the account", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: null });
  await botCheck(backend);

  fireEvent.change(
    await screen.findByRole("combobox", { name: "Вход в YouTube" }),
    { target: { value: "firefox" } },
  );
  await waitFor(() => expect(backend.signInBrowsers).toEqual(["firefox"]));
  await backend.settings({ signInBrowser: "firefox" });

  expect(
    screen.getByRole("combobox", { name: "Вход в YouTube" }),
  ).toHaveDisplayValue("Firefox");
  expect(
    screen.getByText(
      "Приложение будет обращаться к YouTube от имени вашего аккаунта в Firefox. Если скачивать очень много, YouTube может ограничить этот аккаунт.",
    ),
  ).toBeInTheDocument();
});

test("choosing not to sign in saves no browser", async () => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: "edge" });
  enterUrl(youtubeUrl);

  fireEvent.change(
    await screen.findByRole("combobox", { name: "Вход в YouTube" }),
    { target: { value: "" } },
  );

  await waitFor(() => expect(backend.signInBrowsers).toEqual([null]));
});

test("a sign-in browser that can't be saved says so", async () => {
  const backend = await startReady({ savingFails: true });
  await botCheck(backend);

  fireEvent.change(screen.getByRole("combobox", { name: "Вход в YouTube" }), {
    target: { value: "firefox" },
  });

  expect(
    await screen.findByText("Не удалось сохранить выбор браузера."),
  ).toHaveRole("alert");
});

test.each([
  [
    "firefox",
    { kind: "signInRequired" },
    "Видео требует входа, а в Firefox не выполнен вход в YouTube или у аккаунта нет доступа к этому видео",
  ],
  [
    "firefox",
    { kind: "botCheck" },
    "YouTube всё равно просит подтвердить, что вы не робот. Проверьте, что в Firefox выполнен вход в YouTube",
  ],
  [
    "vivaldi",
    { kind: "cookiesUnreadable", reason: "browserNotFound" },
    "Не удалось найти данные Vivaldi — браузер не установлен или в нём ещё не открывали сайты. Выберите другой браузер",
  ],
  [
    "edge",
    { kind: "cookiesUnreadable", reason: "browserOpen" },
    "Edge не даёт прочитать cookies, пока он открыт. Закройте его полностью и нажмите «Найти» ещё раз",
  ],
  [
    "chrome",
    { kind: "cookiesUnreadable", reason: "cantDecrypt" },
    "Chrome шифрует cookies так, что другие программы не могут их прочитать. Выберите другой браузер, например Firefox",
  ],
] as const)(
  "a Probe that ran with %s as the sign-in browser names it in %o",
  async (browser, error, text) => {
    const backend = await startReady();

    await backend.probe({ status: "failed", error, signInBrowser: browser });

    expect(screen.getByRole("alert")).toHaveTextContent(text);
  },
);

test("a failed Probe names the browser it ran with, not the one chosen since", async () => {
  const backend = await startReady();
  await backend.probe({
    status: "failed",
    error: { kind: "botCheck" },
    signInBrowser: null,
  });

  await backend.settings({ signInBrowser: "firefox" });

  expect(screen.getByRole("alert")).toHaveTextContent(
    "выберите ниже браузер, в котором вы вошли в YouTube",
  );
});

/** How a Download that timed out fails. */
const readTimedOut: Pick<
  Extract<Download, { status: "failed" }>,
  "failure" | "message" | "output" | "ytdlpUpdate"
> = {
  failure: "offline",
  message: "ERROR: unable to download video data: Read timed out.",
  output: [
    "WARNING: [youtube] Retrying (1/3)...",
    "ERROR: unable to download video data: Read timed out.",
  ],
  ytdlpUpdate: null,
};

const zoo = {
  title: "Me at the zoo",
  channel: "jawed",
  thumbnail: null,
  duration: 19,
};

test("a probed Video is downloaded on request with the chosen Video preset", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));

  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["1080p"]));
});

test("a probed Video offers only its Video presets on the Video tab", async () => {
  const backend = await startReady();

  await backend.probe(probed(zoo, ["720p", "480p", "360p"], "720p"));

  expect(screen.getByRole("tab", { name: "Видео" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  const presets = screen.getByRole("radiogroup", { name: "Качество" });
  const radios = within(presets).getAllByRole("radio");
  expect(radios).toHaveLength(3);
  ["720p", "480p", "360p"].forEach((name, at) =>
    expect(radios[at]).toHaveAccessibleName(name),
  );
  expect(screen.getByRole("radio", { name: "720p" })).toBeChecked();
});

test("another Video preset can be chosen before downloading", async () => {
  const backend = await startReady();
  // «Лучшее» is offered above 2160p.
  await backend.probe(probed(zoo, ["best", "2160p", "1080p", "720p"]));

  fireEvent.click(screen.getByRole("radio", { name: "Лучшее" }));
  expect(screen.getByRole("radio", { name: "Лучшее" })).toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["best"]));
});

test("a new Probe chooses its own default Video preset again", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  fireEvent.click(screen.getByRole("radio", { name: "720p" }));

  await backend.probe({ status: "probing" });
  await backend.probe(probed(zoo));

  expect(screen.getByRole("radio", { name: "1080p" })).toBeChecked();
});

/** Opens a tab of the Video's card; Radix switches tabs on mouse down. */
function openTab(name: string) {
  fireEvent.mouseDown(screen.getByRole("tab", { name }));
}

test("a Video with sound offers M4A and MP3 on the Audio tab", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));

  openTab("Аудио");

  expect(screen.getByRole("tab", { name: "Аудио" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  const presets = screen.getByRole("radiogroup", { name: "Формат" });
  const radios = within(presets).getAllByRole("radio");
  expect(radios).toHaveLength(2);
  expect(radios[0]).toHaveAccessibleName("M4A (оригинал)");
  expect(radios[1]).toHaveAccessibleName("MP3");
  expect(radios[0]).toBeChecked();
});

test("M4A isn't called original for a Video whose sound it encodes again", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo, ["1080p"], "1080p", ["m4a", "mp3"], false));

  openTab("Аудио");

  const presets = screen.getByRole("radiogroup", { name: "Формат" });
  const radios = within(presets).getAllByRole("radio");
  expect(radios[0]).toHaveAccessibleName("M4A");
  expect(radios[1]).toHaveAccessibleName("MP3");
});

test("the Audio tab downloads M4A unless MP3 is chosen", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  openTab("Аудио");

  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["m4a"]));
});

test("an Audio preset can be chosen before downloading", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  openTab("Аудио");

  fireEvent.click(screen.getByRole("radio", { name: "MP3" }));
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["mp3"]));
});

test("back on the Video tab the chosen Video preset is downloaded", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  fireEvent.click(screen.getByRole("radio", { name: "720p" }));
  openTab("Аудио");
  fireEvent.click(screen.getByRole("radio", { name: "MP3" }));

  openTab("Видео");
  expect(screen.getByRole("radio", { name: "720p" })).toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["720p"]));
});

test("a Video without sound has no Audio tab", async () => {
  const backend = await startReady();

  await backend.probe(probed(zoo, ["1080p"], "1080p", []));

  expect(screen.getByRole("tab", { name: "Видео" })).toBeInTheDocument();
  expect(screen.queryByRole("tab", { name: "Аудио" })).not.toBeInTheDocument();
});

test("a Download without the transcript checkbox saves no Transcript", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));

  expect(
    screen.getByRole("checkbox", { name: "Сохранить транскрипт" }),
  ).not.toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["1080p"]));
  expect(backend.downloadedTranscripts).toEqual([null]);
});

test("the transcript checkbox saves the Transcript in the Video's own language with the media", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));

  fireEvent.click(
    screen.getByRole("checkbox", { name: "Сохранить транскрипт" }),
  );
  expect(
    screen.getByRole("combobox", { name: "Язык транскрипта" }),
  ).toHaveDisplayValue("Английский");
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["1080p"]));
  expect(backend.downloadedTranscripts).toEqual(["en"]);
});

test("the transcript checkbox saves the Transcript with the sound on the Audio tab", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  openTab("Аудио");

  fireEvent.click(
    screen.getByRole("checkbox", { name: "Сохранить транскрипт" }),
  );
  fireEvent.change(screen.getByRole("combobox", { name: "Язык транскрипта" }), {
    target: { value: "ru" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["m4a"]));
  expect(backend.downloadedTranscripts).toEqual(["ru"]);
});

test("the Transcript tab saves the Transcript alone in the Video's own language", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));

  openTab("Транскрипт");
  expect(
    screen.getByRole("combobox", { name: "Язык транскрипта" }),
  ).toHaveDisplayValue("Английский");
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual([null]));
  expect(backend.downloadedTranscripts).toEqual(["en"]);
});

test("the Transcript tab lists the Video's languages by name, captions marked", async () => {
  const backend = await startReady();
  await backend.probe(
    probed(zoo, ["1080p"], "1080p", ["m4a", "mp3"], true, [
      { language: "ru", automatic: false },
      { language: "de", automatic: false },
      { language: "en", automatic: true },
    ]),
  );
  openTab("Транскрипт");

  const languages = screen.getByRole("combobox", { name: "Язык транскрипта" });
  expect(
    within(languages)
      .getAllByRole("option")
      .map((option) => option.textContent),
  ).toEqual(["Английский (автосубтитры)", "Немецкий", "Русский"]);
  fireEvent.change(languages, { target: { value: "de" } });
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedTranscripts).toEqual(["de"]));
});

test("two Transcripts of one language are told apart by their codes", async () => {
  const backend = await startReady();
  await backend.probe(
    probed(zoo, ["1080p"], "1080p", ["m4a", "mp3"], true, [
      { language: "en", automatic: false },
      { language: "en-nP7-2PuUl7o", automatic: false },
      { language: "ru", automatic: false },
    ]),
  );
  openTab("Транскрипт");

  const languages = screen.getByRole("combobox", { name: "Язык транскрипта" });
  expect(
    within(languages)
      .getAllByRole("option")
      .map((option) => option.textContent),
  ).toEqual(["Английский (en)", "Английский (en-nP7-2PuUl7o)", "Русский"]);
});

test("a Video without a Transcript says so on its tab and can't save one", async () => {
  const backend = await startReady();
  await backend.probe(
    probed(zoo, ["1080p"], "1080p", ["m4a", "mp3"], true, [], null),
  );

  expect(
    screen.getByRole("checkbox", { name: "Сохранить транскрипт" }),
  ).toBeDisabled();
  openTab("Транскрипт");

  expect(
    screen.getByText("У этого видео нет транскрипта."),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Скачать" })).toBeDisabled();
});

test("a new Probe takes its own Transcript language and unchecks the checkbox", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  fireEvent.click(
    screen.getByRole("checkbox", { name: "Сохранить транскрипт" }),
  );
  fireEvent.change(screen.getByRole("combobox", { name: "Язык транскрипта" }), {
    target: { value: "ru" },
  });

  await backend.probe({ status: "probing" });
  await backend.probe(probed(zoo));

  expect(
    screen.getByRole("checkbox", { name: "Сохранить транскрипт" }),
  ).not.toBeChecked();
  openTab("Транскрипт");
  expect(
    screen.getByRole("combobox", { name: "Язык транскрипта" }),
  ).toHaveDisplayValue("Английский");
});

test("a Download shows its percent, speed and time left while downloading", async () => {
  const backend = await startReady();

  await backend.download({
    video: zoo,
    status: "downloading",
    receiving: "video",
    percent: 25.4,
    speed: 1.5 * MB,
    eta: 67,
  });

  const status = screen.getByRole("region", { name: "Me at the zoo" });
  expect(status).toHaveTextContent("Скачивание видео…");
  expect(status).toHaveTextContent("25%");
  expect(status).toHaveTextContent("1,5 МБ/с");
  expect(status).toHaveTextContent("осталось 1:07");
  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "25",
  );
});

test("a Download shows that it is starting before yt-dlp tells its progress", async () => {
  const backend = await startReady();

  await backend.download({
    video: zoo,
    status: "downloading",
    receiving: "video",
    percent: null,
    speed: null,
    eta: null,
  });

  const status = screen.getByRole("region", { name: "Me at the zoo" });
  expect(status).toHaveTextContent("Скачивание видео…");
  expect(status).not.toHaveTextContent("%");
  expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
});

test("a Download shows whether it receives the Transcript, the video or the audio", async () => {
  const backend = await startReady();
  const downloading = {
    video: zoo,
    status: "downloading",
    percent: null,
    speed: null,
    eta: null,
  } as const;
  const status = () => screen.getByRole("region", { name: "Me at the zoo" });

  await backend.download({ ...downloading, receiving: "transcript" });
  expect(status()).toHaveTextContent("Скачивание транскрипта…");

  await backend.download({ ...downloading, receiving: "video" });
  expect(status()).toHaveTextContent("Скачивание видео…");

  await backend.download({ ...downloading, receiving: "audio" });
  expect(status()).toHaveTextContent("Скачивание аудио…");
});

test("a Download shows processing while ffmpeg merges the video and audio", async () => {
  const backend = await startReady();

  await backend.download({ video: zoo, status: "processing" });

  expect(
    screen.getByRole("region", { name: "Me at the zoo" }),
  ).toHaveTextContent("Обработка");
});

test("another Video is added to the Queue while a Download runs", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  await backend.download({ video: zoo, status: "processing" });

  openTab("Аудио");
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.downloadedPresets).toEqual(["m4a"]));
});

const habit: VideoCard = {
  title: "A Simple Way to Break a Bad Habit",
  channel: "TED",
  thumbnail: "https://i.ytimg.com/vi/-moW9jvvMr4/maxresdefault.jpg",
  duration: 579,
};

const lecture: VideoCard = { ...habit, title: "A lecture" };

test("the Queue shows each Download's thumbnail, title, Preset and status in order", async () => {
  const backend = await startReady();

  await backend.queue(
    queued(3, {
      video: zoo,
      status: "downloading",
      receiving: "video",
      percent: 10,
      speed: null,
      eta: null,
    }),
    queued(4, { video: habit, status: "queued" }, "mp3", "en"),
    queued(5, { video: lecture, status: "queued" }, null, "ru"),
  );

  const items = within(
    screen.getByRole("list", { name: "Очередь" }),
  ).getAllByRole("listitem");
  expect(
    items.map((item) => within(item).getByRole("heading").textContent),
  ).toEqual([
    "Me at the zoo",
    "A Simple Way to Break a Bad Habit",
    "A lecture",
  ]);
  expect(items[0]).toHaveTextContent("1080p");
  expect(items[0]).toHaveTextContent("Скачивание видео…");
  expect(items[1]).toHaveTextContent("MP3 + транскрипт");
  expect(items[1]).toHaveTextContent("В очереди");
  expect(within(items[1]).getByRole("img", { name: "Превью" })).toHaveAttribute(
    "src",
    habit.thumbnail,
  );
  expect(items[2]).toHaveTextContent("Транскрипт");
  expect(items[2]).toHaveTextContent("В очереди");
});

test("the Queue's header counts its Downloads", async () => {
  const backend = await startReady();

  await backend.queue(
    queued(1, { video: zoo, status: "queued" }),
    queued(2, { video: habit, status: "queued" }),
    queued(3, { video: lecture, status: "cancelled" }),
  );

  expect(
    within(screen.getByRole("region", { name: "Очередь" })).getByText("3"),
  ).toBeInTheDocument();
});

test("a duplicate of a Queued or running Download gets a warning and the Video's card stays", async () => {
  const backend = await startReady({ duplicateOf: 1 });
  await backend.download({ video: zoo, status: "queued" });
  await backend.probe(probed(zoo));

  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "уже скачивается или ждёт в очереди",
  );
  expect(screen.getByRole("button", { name: "Скачать" })).toBeInTheDocument();
});

test("the duplicate warning goes once another Preset is chosen", async () => {
  const backend = await startReady({ duplicateOf: 1 });
  await backend.download({ video: zoo, status: "queued" });
  await backend.probe(probed(zoo));
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));
  await screen.findByRole("alert");

  fireEvent.click(screen.getByRole("radio", { name: "720p" }));

  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("the duplicate warning goes once the Download it duplicates is Done", async () => {
  const backend = await startReady({ duplicateOf: 1 });
  await backend.download({ video: zoo, status: "processing" });
  await backend.probe(probed(zoo));
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));
  await screen.findByRole("alert");

  await backend.download({
    video: zoo,
    status: "done",
    file: "C:\\Downloads\\Me at the zoo.mp4",
  });

  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("a Done Download is removed from the Queue by its own «Убрать»", async () => {
  const backend = await startReady();
  await backend.queue(
    queued(1, { video: zoo, status: "done", file: "C:\\Downloads\\a.mp4" }),
    queued(2, { video: habit, status: "done", file: "C:\\Downloads\\b.mp4" }),
  );

  fireEvent.click(
    within(screen.getByRole("region", { name: habit.title })).getByRole(
      "button",
      { name: "Убрать" },
    ),
  );

  await waitFor(() =>
    expect(backend.downloadIds["remove_download"]).toEqual([2]),
  );
});

test.each([
  [{ status: "failed", ...readTimedOut }],
  [{ status: "cancelled" }],
] as const)(
  "a Failed or Cancelled Download is removed by its own «Убрать» (%o)",
  async (status) => {
    const backend = await startReady();
    await backend.queue(queued(1, { video: zoo, ...status }));

    fireEvent.click(screen.getByRole("button", { name: "Убрать" }));

    await waitFor(() =>
      expect(backend.downloadIds["remove_download"]).toEqual([1]),
    );
    expect(
      screen.queryByRole("button", { name: "Очистить завершённые" }),
    ).not.toBeInTheDocument();
  },
);

test.each([[{ status: "queued" }], [{ status: "processing" }]] as const)(
  "a Download that hasn't ended can't be removed (%o)",
  async (status) => {
    const backend = await startReady();

    await backend.download({ video: zoo, ...status });

    expect(
      screen.queryByRole("button", { name: "Убрать" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Очистить завершённые" }),
    ).not.toBeInTheDocument();
  },
);

test("«Очистить завершённые» clears the Done Downloads", async () => {
  const backend = await startReady();
  await backend.queue(
    queued(1, { video: zoo, status: "done", file: "C:\\Downloads\\a.mp4" }),
    queued(2, { video: habit, status: "processing" }),
  );

  fireEvent.click(screen.getByRole("button", { name: "Очистить завершённые" }));

  await waitFor(() =>
    expect(backend.commands).toContain("clear_done_downloads"),
  );
});

test("a Done Download opens its file or shows it in its folder", async () => {
  const backend = await startReady();

  await backend.download({
    video: zoo,
    status: "done",
    file: "C:\\Users\\me\\Downloads\\Me at the zoo (1).mp4",
  });

  const status = screen.getByRole("region", { name: "Me at the zoo" });
  expect(status).toHaveTextContent("Готово");
  expect(status).toHaveTextContent("Me at the zoo (1).mp4");
  expect(status).not.toHaveTextContent("Downloads");
  fireEvent.click(screen.getByRole("button", { name: "Открыть файл" }));
  fireEvent.click(screen.getByRole("button", { name: "Показать в папке" }));
  await waitFor(() =>
    expect(backend.downloadIds).toEqual({
      open_downloaded_file: [1],
      show_downloaded_file: [1],
    }),
  );
});

test.each([
  ["offline", "Нет подключения к интернету или сайт не отвечает"],
  ["unsupported", "Сайт не поддерживается или видео больше недоступно"],
  ["signInRequired", "Видео доступно только после входа в аккаунт"],
  ["extractorBroken", "yt-dlp не смог разобрать страницу видео"],
  ["disk", "Не удалось записать файл"],
  ["unknown", "Не удалось скачать видео"],
] as const)("a Download Failed as %s says why", async (failure, text) => {
  const backend = await startReady();

  await backend.download({
    video: zoo,
    status: "failed",
    ...readTimedOut,
    failure,
  });

  expect(screen.getByRole("alert")).toHaveTextContent(text);
});

/** Makes the clipboard take the texts written to it, or fail when told. */
function mockClipboard({ fails = false } = {}) {
  const written: string[] = [];
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: {
      writeText(text: string) {
        if (fails) {
          return Promise.reject(new Error("not allowed"));
        }
        written.push(text);
        return Promise.resolve();
      },
    },
  });
  return written;
}

test("a Failed Download's details copy its URL, Preset and what yt-dlp printed last", async () => {
  const clipboard = mockClipboard();
  const backend = await startReady();
  await backend.queue(
    queued(1, { video: zoo, status: "failed", ...readTimedOut }, "mp3", "en"),
  );

  fireEvent.click(
    screen.getByRole("button", { name: "Скопировать подробности" }),
  );

  await waitFor(() =>
    expect(clipboard).toEqual([
      [
        "Ссылка: https://www.youtube.com/watch?v=jNQXAC9IVRw",
        "Preset: mp3",
        "Транскрипт: en",
        "Ошибка: ERROR: unable to download video data: Read timed out.",
        "",
        "Последние строки вывода yt-dlp:",
        "```",
        "WARNING: [youtube] Retrying (1/3)...",
        "ERROR: unable to download video data: Read timed out.",
        "```",
      ].join("\n"),
    ]),
  );
  expect(
    await screen.findByText("Подробности скопированы"),
  ).toBeInTheDocument();
});

test("the details fence what yt-dlp printed beyond the backticks it printed", async () => {
  const clipboard = mockClipboard();
  const backend = await startReady();
  await backend.download({
    video: zoo,
    status: "failed",
    ...readTimedOut,
    output: ["WARNING: ```", "ERROR: ````"],
  });

  fireEvent.click(
    screen.getByRole("button", { name: "Скопировать подробности" }),
  );

  await waitFor(() => expect(clipboard).toHaveLength(1));
  expect(clipboard[0]).toContain(
    ["`````", "WARNING: ```", "ERROR: ````", "`````"].join("\n"),
  );
});

test("the details of a Failed Transcript alone name no Preset", async () => {
  const clipboard = mockClipboard();
  const backend = await startReady();
  await backend.queue(
    queued(1, { video: zoo, status: "failed", ...readTimedOut }, null, "en"),
  );

  fireEvent.click(
    screen.getByRole("button", { name: "Скопировать подробности" }),
  );

  await waitFor(() => expect(clipboard).toHaveLength(1));
  expect(clipboard[0]).not.toContain("Preset");
  expect(clipboard[0]).toContain("Транскрипт: en");
});

test("details that can't be copied say so", async () => {
  mockClipboard({ fails: true });
  const backend = await startReady();
  await backend.download({ video: zoo, status: "failed", ...readTimedOut });

  fireEvent.click(
    screen.getByRole("button", { name: "Скопировать подробности" }),
  );

  expect(
    await screen.findByText("Не удалось скопировать подробности."),
  ).toBeInTheDocument();
});

test("a Failed Download is retried from its card", async () => {
  const backend = await startReady();
  await backend.download({
    video: zoo,
    status: "failed",
    ...readTimedOut,
  });

  fireEvent.click(screen.getByRole("button", { name: "Повторить" }));

  await waitFor(() =>
    expect(backend.downloadIds["retry_download"]).toEqual([1]),
  );
});

const failedZoo = queued(1, {
  video: zoo,
  status: "failed",
  ...readTimedOut,
});

test("a Failed Download retried no longer says its details were copied", async () => {
  mockClipboard();
  const backend = await startReady();
  await backend.queue(failedZoo);
  fireEvent.click(
    screen.getByRole("button", { name: "Скопировать подробности" }),
  );
  await screen.findByText("Подробности скопированы");

  fireEvent.click(screen.getByRole("button", { name: "Повторить" }));
  await backend.queue(failedZoo);

  expect(screen.queryByText("Подробности скопированы")).not.toBeInTheDocument();
});

test("a Failed Download whose duplicate is in the Queue says so on retry", async () => {
  const backend = await startReady({ duplicateOf: 2 });
  await backend.queue(failedZoo, queued(2, { video: zoo, status: "queued" }));

  fireEvent.click(screen.getByRole("button", { name: "Повторить" }));

  await waitFor(() =>
    expect(
      screen.getAllByRole("region", { name: "Me at the zoo" })[0],
    ).toHaveTextContent("уже скачивается или ждёт в очереди"),
  );
});

test("a Failed Download's duplicate warning goes once its duplicate is Done", async () => {
  const backend = await startReady({ duplicateOf: 2 });
  await backend.queue(failedZoo, queued(2, { video: zoo, status: "queued" }));
  fireEvent.click(screen.getByRole("button", { name: "Повторить" }));
  const failed = screen.getAllByRole("region", { name: "Me at the zoo" })[0];
  await waitFor(() =>
    expect(failed).toHaveTextContent("уже скачивается или ждёт в очереди"),
  );

  await backend.queue(
    failedZoo,
    queued(2, { video: zoo, status: "done", file: "C:\\Downloads\\a.mp4" }),
  );

  expect(failed).not.toHaveTextContent("уже скачивается или ждёт в очереди");
});

test("a Queued or Downloading Download is cancelled from its card", async () => {
  const backend = await startReady();
  await backend.queue(
    queued(1, {
      video: zoo,
      status: "downloading",
      receiving: "video",
      percent: 25,
      speed: null,
      eta: null,
    }),
    queued(2, { video: zoo, status: "queued" }),
  );

  for (const card of screen.getAllByRole("region", { name: "Me at the zoo" })) {
    fireEvent.click(within(card).getByRole("button", { name: "Отменить" }));
  }

  await waitFor(() =>
    expect(backend.downloadIds["cancel_download"]).toEqual([1, 2]),
  );
});

test("a Download being processed or Done can't be cancelled", async () => {
  const backend = await startReady();

  await backend.queue(
    queued(1, { video: zoo, status: "processing" }),
    queued(2, { video: zoo, status: "done", file: "C:\\Downloads\\a.mp4" }),
  );

  expect(screen.queryByRole("button", { name: "Отменить" })).toBeNull();
});

test("a Cancelled Download says so and is retried from its card", async () => {
  const backend = await startReady();
  await backend.download({ video: zoo, status: "cancelled" });

  expect(
    screen.getByRole("region", { name: "Me at the zoo" }),
  ).toHaveTextContent("Отменено");
  fireEvent.click(screen.getByRole("button", { name: "Повторить" }));

  await waitFor(() =>
    expect(backend.downloadIds["retry_download"]).toEqual([1]),
  );
});

const nextUrl = "https://www.youtube.com/watch?v=-moW9jvvMr4";

test("a new URL hides the previous Video's card", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));

  enterUrl(nextUrl);

  expect(
    screen.queryByRole("heading", { name: "Me at the zoo" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Скачать" }),
  ).not.toBeInTheDocument();
});

test("a URL whose Probe can't start says so instead of the previous Video", async () => {
  const backend = await startReady({ probeFails: true });
  await backend.probe(probed(zoo));

  submitUrl(nextUrl);

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Не удалось получить сведения о видео",
  );
  expect(
    screen.queryByRole("heading", { name: "Me at the zoo" }),
  ).not.toBeInTheDocument();
});

test.each([
  [{ status: "done", file: "C:\\Downloads\\Me at the zoo.mp4" }],
  [{ status: "failed", ...readTimedOut }],
  [{ status: "processing" }],
] as const)(
  "the Queue stays on screen when a new URL is entered (%o)",
  async (status) => {
    const backend = await startReady();
    await backend.download({ video: zoo, ...status });

    enterUrl(nextUrl);

    expect(
      screen.getByRole("region", { name: "Me at the zoo" }),
    ).toBeInTheDocument();
  },
);

test("a file that can't be opened is told on its own Download only", async () => {
  const backend = await startReady({ openingFails: true });
  await backend.queue(
    queued(1, { video: zoo, status: "done", file: "C:\\Downloads\\a.mp4" }),
    queued(2, { video: habit, status: "done", file: "C:\\Downloads\\b.mp4" }),
  );

  fireEvent.click(
    within(screen.getByRole("region", { name: "Me at the zoo" })).getByRole(
      "button",
      { name: "Открыть файл" },
    ),
  );

  expect(
    await within(
      screen.getByRole("region", { name: "Me at the zoo" }),
    ).findByRole("alert"),
  ).toHaveTextContent("Не удалось открыть файл");
  expect(
    within(screen.getByRole("region", { name: habit.title })).queryByRole(
      "alert",
    ),
  ).not.toBeInTheDocument();
});

test("pressing «Скачать» hides the Video's card", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));

  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.commands).toContain("add_download"));
  expect(
    screen.queryByRole("heading", { name: "Me at the zoo" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Скачать" }),
  ).not.toBeInTheDocument();
});

test("a Video's card comes back when the backend has nothing to download", async () => {
  const backend = await startReady({ downloadFails: true });
  await backend.probe(probed(zoo));

  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  await waitFor(() => expect(backend.commands).toContain("add_download"));
  expect(
    await screen.findByRole("heading", { name: "Me at the zoo" }),
  ).toBeInTheDocument();
});

test("a Video's card that comes back keeps the chosen Video preset", async () => {
  const backend = await startReady({ downloadFails: true });
  await backend.probe(probed(zoo));
  fireEvent.click(screen.getByRole("radio", { name: "720p" }));

  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  expect(await screen.findByRole("radio", { name: "720p" })).toBeChecked();
});

test("a Video's card that comes back keeps the Audio tab and its preset", async () => {
  const backend = await startReady({ downloadFails: true });
  await backend.probe(probed(zoo));
  openTab("Аудио");
  fireEvent.click(screen.getByRole("radio", { name: "MP3" }));

  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  expect(await screen.findByRole("radio", { name: "MP3" })).toBeChecked();
});

test("a new Probe opens on the Video tab again", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  openTab("Аудио");

  await backend.probe(probed(zoo));

  expect(screen.getByRole("tab", { name: "Видео" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("a Video probed again without probing in between gets its default Video preset", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));
  fireEvent.click(screen.getByRole("radio", { name: "720p" }));

  await backend.probe(probed(zoo));

  expect(screen.getByRole("radio", { name: "1080p" })).toBeChecked();
});

test("a Video's card hidden by «Скачать» returns when its URL is submitted again", async () => {
  const backend = await startReady();
  submitUrl(nextUrl);
  await backend.probe(probed(zoo));
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));

  submitUrl(nextUrl);
  await backend.probe(probed(zoo));

  expect(
    screen.getByRole("heading", { name: "Me at the zoo" }),
  ).toBeInTheDocument();
});

test("a Video's card hidden by «Скачать» stays hidden until its URL submitted again is probed", async () => {
  const backend = await startReady();
  submitUrl(nextUrl);
  await backend.probe(probed(zoo));
  fireEvent.click(screen.getByRole("button", { name: "Скачать" }));
  await waitFor(() => expect(backend.commands).toContain("add_download"));

  submitUrl(nextUrl);

  expect(
    screen.queryByRole("heading", { name: "Me at the zoo" }),
  ).not.toBeInTheDocument();
});

test("submitting another URL doesn't bring the previous Video's card back", async () => {
  const backend = await startReady();
  await backend.probe(probed(zoo));

  submitUrl(nextUrl);

  expect(
    screen.queryByRole("heading", { name: "Me at the zoo" }),
  ).not.toBeInTheDocument();
});

test("closing with unfinished Downloads asks first and closes once confirmed", async () => {
  const backend = mockBackend();
  render(<App />);
  await backend.setup({ status: "ready" });

  await backend.closeRequested();

  const dialog = screen.getByRole("alertdialog", {
    name: "Есть незавершённые загрузки. Закрыть?",
  });
  expect(backend.commands).not.toContain("close_app");
  fireEvent.click(within(dialog).getByRole("button", { name: "Закрыть" }));
  await waitFor(() => expect(backend.commands).toContain("close_app"));
});

test("declining to close keeps the app open and asks again next time", async () => {
  const backend = mockBackend();
  render(<App />);
  await backend.setup({ status: "ready" });
  await backend.closeRequested();

  fireEvent.click(screen.getByRole("button", { name: "Не закрывать" }));

  await waitFor(() =>
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(),
  );
  expect(backend.commands).not.toContain("close_app");
  await backend.closeRequested();
  expect(screen.getByRole("alertdialog")).toBeInTheDocument();
});

test("the main screen offers no update of yt-dlp, which updates on its own", async () => {
  await startReady();

  await screen.findByRole("heading", { name: "download-video-app" });

  expect(
    screen.queryByRole("button", { name: "Обновить yt-dlp" }),
  ).not.toBeInTheDocument();
});

test.each([
  ["updating", "Обновляю yt-dlp…"],
  ["updated", "yt-dlp обновлён, попробуйте ещё раз."],
  ["upToDate", "Исправления в новой версии yt-dlp пока нет"],
  ["failed", "Обновить yt-dlp не удалось"],
] satisfies [YtDlpUpdate, string][])(
  "a Download Failed with a broken extractor tells when yt-dlp is %s",
  async (ytdlpUpdate, text) => {
    const backend = await startReady();

    await backend.download({
      video: zoo,
      status: "failed",
      ...readTimedOut,
      failure: "extractorBroken",
      ytdlpUpdate,
    });

    const card = screen.getByRole("region", { name: "Me at the zoo" });
    expect(within(card).getByRole("alert")).toHaveTextContent(text);
    // The user retries it.
    expect(backend.commands).not.toContain("retry_download");
    expect(
      within(card).getByRole("button", { name: "Повторить" }),
    ).toBeEnabled();
  },
);

/** Opens the settings from the main screen. */
async function openSettings() {
  fireEvent.click(await screen.findByRole("button", { name: "Настройки" }));
}

test("the settings open from the main screen and go back to it as it was", async () => {
  await startReady();
  enterUrl(youtubeUrl);

  await openSettings();

  expect(
    screen.getByRole("heading", { name: "Настройки" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("textbox", { name: "Ссылка на видео" }),
  ).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Назад" }));

  expect(screen.getByRole("textbox", { name: "Ссылка на видео" })).toHaveValue(
    youtubeUrl,
  );
  expect(
    screen.queryByRole("heading", { name: "Настройки" }),
  ).not.toBeInTheDocument();
});

test("the settings show the save folder and choose another on request", async () => {
  const backend = await startReady();
  await backend.settings({ saveDir: "D:\\Видео" });
  await openSettings();

  expect(screen.getByText("D:\\Видео")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Изменить…" }));

  await waitFor(() => expect(backend.commands).toContain("choose_save_dir"));
});

test("a save folder that can't be saved says so", async () => {
  await startReady({ savingFails: true });
  await openSettings();

  fireEvent.click(screen.getByRole("button", { name: "Изменить…" }));

  expect(await screen.findByText("Не удалось сохранить папку.")).toHaveRole(
    "alert",
  );
});

test("the file name template is saved once the field is left", async () => {
  const backend = await startReady();
  await backend.settings({});
  await openSettings();
  const field = screen.getByRole("textbox", { name: "Имя файла" });
  expect(field).toHaveValue("{title}");

  fireEvent.change(field, { target: { value: "{date} {title}" } });
  expect(backend.fileNameTemplates).toEqual([]);
  fireEvent.blur(field);

  await waitFor(() =>
    expect(backend.fileNameTemplates).toEqual(["{date} {title}"]),
  );
  await backend.settings({ fileNameTemplate: "{date} {title}" });
  expect(field).toHaveValue("{date} {title}");
});

test("the file name template is saved on Enter", async () => {
  const backend = await startReady();
  await openSettings();
  const field = screen.getByRole("textbox", { name: "Имя файла" });

  fireEvent.change(field, { target: { value: "{channel} - {title}" } });
  fireEvent.keyDown(field, { key: "Enter" });

  await waitFor(() =>
    expect(backend.fileNameTemplates).toEqual(["{channel} - {title}"]),
  );
});

test("a file name template saved on Enter isn't saved again once the field is left", async () => {
  const backend = await startReady();
  await openSettings();
  const field = screen.getByRole("textbox", { name: "Имя файла" });

  fireEvent.change(field, { target: { value: "{channel} - {title}" } });
  fireEvent.keyDown(field, { key: "Enter" });
  fireEvent.blur(field);

  await waitFor(() =>
    expect(backend.fileNameTemplates).toEqual(["{channel} - {title}"]),
  );
});

test("typing on after Enter keeps what was typed", async () => {
  await startReady();
  await openSettings();
  const field = screen.getByRole("textbox", { name: "Имя файла" });

  fireEvent.change(field, { target: { value: "{date}" } });
  fireEvent.keyDown(field, { key: "Enter" });
  fireEvent.change(field, { target: { value: "{date} {title}" } });

  // The save of "{date}" has answered by now.
  await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
  expect(field).toHaveValue("{date} {title}");
});

test("an unchanged file name template isn't saved again", async () => {
  const backend = await startReady();
  await openSettings();
  const field = screen.getByRole("textbox", { name: "Имя файла" });

  fireEvent.focus(field);
  fireEvent.blur(field);

  expect(backend.fileNameTemplates).toEqual([]);
});

test("the file name template's hints tell its variables and put one in on a click", async () => {
  const backend = await startReady();
  await openSettings();
  const field = screen.getByRole("textbox", { name: "Имя файла" });
  expect(screen.getByText("название видео")).toBeInTheDocument();
  expect(screen.getByText("канал")).toBeInTheDocument();
  expect(screen.getByText("дата публикации, ГГГГ-ММ-ДД")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "{channel}" }));

  expect(field).toHaveValue("{title}{channel}");
  await waitFor(() =>
    expect(backend.fileNameTemplates).toEqual(["{title}{channel}"]),
  );
});

test("a file name template that can't be saved says so and keeps what was typed", async () => {
  await startReady({ savingFails: true });
  await openSettings();
  const field = screen.getByRole("textbox", { name: "Имя файла" });

  fireEvent.change(field, { target: { value: "{date} {title}" } });
  fireEvent.blur(field);

  expect(
    await screen.findByText("Не удалось сохранить шаблон имени."),
  ).toHaveRole("alert");
  expect(field).toHaveValue("{date} {title}");
});

test("the default Video preset shows the saved one and saves another chosen", async () => {
  const backend = await startReady();
  await backend.settings({ defaultVideoPreset: "720p" });
  await openSettings();
  const select = screen.getByRole("combobox", {
    name: "Качество по умолчанию",
  });
  expect(select).toHaveDisplayValue("720p");

  fireEvent.change(select, { target: { value: "best" } });

  await waitFor(() => expect(backend.defaultVideoPresets).toEqual(["best"]));
});

test("a default Video preset that can't be saved says so", async () => {
  await startReady({ savingFails: true });
  await openSettings();

  fireEvent.change(
    screen.getByRole("combobox", { name: "Качество по умолчанию" }),
    { target: { value: "480p" } },
  );

  expect(await screen.findByText("Не удалось сохранить качество.")).toHaveRole(
    "alert",
  );
});
