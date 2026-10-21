import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import App from "./App";

test("main screen shows app name and version from the backend", async () => {
  mockIPC((cmd) => {
    if (cmd === "app_info") {
      return { name: "download-video-app", version: "1.2.3" };
    }
  });

  render(<App />);

  expect(
    await screen.findByRole("heading", { name: "download-video-app" }),
  ).toBeInTheDocument();
  expect(screen.getByText("Версия 1.2.3")).toBeInTheDocument();
});

test("main screen reports when the backend fails to answer", async () => {
  mockIPC(() => {
    throw new Error("backend is down");
  });

  render(<App />);

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Не удалось получить версию приложения",
  );
});
