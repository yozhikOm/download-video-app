// Настройки приложения, хранятся в settings.json через tauri-plugin-store.
import { LazyStore } from "@tauri-apps/plugin-store";
import { downloadDir } from "@tauri-apps/api/path";

export interface Settings {
  outDir: string;
}

const store = new LazyStore("settings.json");

export async function loadSettings(): Promise<Settings> {
  const outDir = (await store.get<string>("outDir")) ?? (await downloadDir());
  return { outDir };
}

export async function saveSettings(patch: Partial<Settings>): Promise<void> {
  for (const [key, value] of Object.entries(patch)) {
    await store.set(key, value);
  }
  await store.save();
}
