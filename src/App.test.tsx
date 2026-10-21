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
import type { AppEvent, SetupState } from "./backend";

const MB = 1024 * 1024;

/** Mocks the backend; `setup` pushes a setup state to the UI's channel. */
function mockBackend({ appInfoFails = false } = {}) {
  let channel: Channel<AppEvent> | undefined;
  const commands: string[] = [];
  mockIPC((cmd, args) => {
    commands.push(cmd);
    switch (cmd) {
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

  return {
    commands,
    async setup(state: SetupState) {
      await waitFor(() => expect(channel).toBeDefined());
      act(() => channel!.onmessage({ type: "setup", data: state }));
    },
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
    error: { kind: "checksumMismatch", package: "ffmpeg" },
    broken: null,
  });

  expect(screen.getByRole("alert")).toHaveTextContent(
    "ffmpeg не прошёл проверку контрольной суммы",
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
