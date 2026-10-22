import type { Channel } from "@tauri-apps/api/core";
import { mockIPC } from "@tauri-apps/api/mocks";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { expect, test } from "vitest";
import App from "./App";
import type {
  AppEvent,
  Browser,
  ProbeState,
  SettingsState,
  SetupState,
} from "./backend";

const MB = 1024 * 1024;

/**
 * Mocks the backend; `setup`, `probe` and `settings` push a state to the
 * UI's channel.
 */
function mockBackend({ appInfoFails = false, savingFails = false } = {}) {
  let channel: Channel<AppEvent> | undefined;
  const commands: string[] = [];
  const probedUrls: string[] = [];
  const signInBrowsers: (Browser | null)[] = [];
  mockIPC((cmd, args) => {
    commands.push(cmd);
    switch (cmd) {
      case "probe":
        probedUrls.push((args as { url: string }).url);
        return null;
      case "set_sign_in_browser":
        if (savingFails) {
          throw new Error("disk is full");
        }
        signInBrowsers.push((args as { browser: Browser | null }).browser);
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
    setup: (state: SetupState) => send({ type: "setup", data: state }),
    probe: (state: ProbeState) => send({ type: "probe", data: state }),
    settings: (state: SettingsState) => send({ type: "settings", data: state }),
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

  await backend.probe({
    status: "probed",
    video: {
      title: "A Simple Way to Break a Bad Habit | Judson Brewer | TED",
      channel: "TED",
      thumbnail: "https://i.ytimg.com/vi/-moW9jvvMr4/maxresdefault.jpg",
      duration: 565,
    },
  });

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

  await backend.probe({
    status: "probed",
    video: {
      title: "Around the World with Peppa Pig",
      channel: null,
      thumbnail: null,
      duration: 14339,
    },
  });

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

/** YouTube answers the Probe with its bot check; no sign-in browser was used. */
function botCheck(backend: ReturnType<typeof mockBackend>) {
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

test("a chosen sign-in browser stays visible so that it can be turned off", async () => {
  const backend = await startReady();

  await backend.settings({ signInBrowser: "firefox" });

  expect(
    screen.getByRole("combobox", { name: "Вход в YouTube" }),
  ).toHaveDisplayValue("Firefox");
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

const probedVideo = {
  status: "probed",
  video: {
    title: "A Simple Way to Break a Bad Habit | Judson Brewer | TED",
    channel: "TED",
    thumbnail: null,
    duration: 565,
  },
} as const;

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
])("a chosen sign-in browser stays visible for %s", async (url) => {
  const backend = await startReady();
  await backend.settings({ signInBrowser: "firefox" });

  enterUrl(url);

  expect(
    screen.getByRole("combobox", { name: "Вход в YouTube" }),
  ).toBeInTheDocument();
});

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
