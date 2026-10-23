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
