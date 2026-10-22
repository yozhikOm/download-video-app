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
import type { AppEvent, ProbeState, SetupState } from "./backend";

const MB = 1024 * 1024;

/** Mocks the backend; `setup` and `probe` push a state to the UI's channel. */
function mockBackend({ appInfoFails = false } = {}) {
  let channel: Channel<AppEvent> | undefined;
  const commands: string[] = [];
  const probedUrls: string[] = [];
  mockIPC((cmd, args) => {
    commands.push(cmd);
    switch (cmd) {
      case "probe":
        probedUrls.push((args as { url: string }).url);
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
    setup: (state: SetupState) => send({ type: "setup", data: state }),
    probe: (state: ProbeState) => send({ type: "probe", data: state }),
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
async function startReady() {
  const backend = mockBackend();
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
    "Видео требует входа в аккаунт — пока не поддерживается",
  ],
  [
    { kind: "botCheck" },
    "YouTube просит подтвердить, что вы не робот. Так бывает в рабочих сетях и через VPN — попробуйте позже или из другой сети",
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

    await backend.probe({ status: "failed", error });

    expect(screen.getByRole("alert")).toHaveTextContent(text);
    expect(screen.queryAllByRole("heading")).toHaveLength(1);
  },
);
