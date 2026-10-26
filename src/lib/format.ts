/** `m:ss`, or `h:mm:ss` from an hour on. */
export function formatDuration(seconds: number) {
  const total = Math.round(seconds);
  const pad = (n: number) => String(n).padStart(2, "0");
  const [h, m, s] = [
    Math.floor(total / 3600),
    Math.floor(total / 60) % 60,
    total % 60,
  ];
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

const languageNames = new Intl.DisplayNames(["ru"], { type: "language" });

function nameOf(code: string) {
  try {
    return languageNames.of(code);
  } catch {
    return undefined;
  }
}

/**
 * The name of a language by its code, as a list shows it: `Английский` for
 * `en`, `Английский (Великобритания)` for `en-GB`.
 */
export function formatLanguage(code: string) {
  // YouTube names a second track of a language like `en-nP7-2PuUl7o`,
  // which isn't a language code.
  const name = nameOf(code) ?? nameOf(code.split(/[-_]/)[0]) ?? code;
  return name.charAt(0).toLocaleUpperCase("ru") + name.slice(1);
}
