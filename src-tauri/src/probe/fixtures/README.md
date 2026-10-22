# Probe fixtures

Real answers of `yt-dlp -J --no-playlist --flat-playlist -- <URL>`, captured on 2026-10-21 with yt-dlp 2026.08.19. YouTube was captured with browser cookies and Node as the JS runtime, because YouTube asks this network to sign in to confirm it isn't a bot.

- `*.json`: stdout of a successful Probe. Signed stream URLs (they carry the client's IP), HTTP headers, cookies, thumbnail lists, descriptions and tags are removed; a playlist or channel keeps its first three entries, and each Transcript language its first caption format.
- `*.stderr`: stderr of a failed Probe, unchanged.

| File                            | URL                                                                          |
| ------------------------------- | ---------------------------------------------------------------------------- |
| `youtube-video.json`            | `https://www.youtube.com/watch?v=-moW9jvvMr4`                                |
| `youtube-finished-stream.json`  | `https://www.youtube.com/watch?v=pZMrdjuU-CY`                                |
| `youtube-stream-with-chat.json` | `https://www.youtube.com/watch?v=fp_7X3VWIuA`                                |
| `youtube-live.json`             | `https://www.youtube.com/watch?v=0sfDth4lMLc`                                |
| `youtube-playlist.json`         | `https://www.youtube.com/playlist?list=PLbpi6ZahtOH6Blw3RGYpWkSByi_T7Rygb`   |
| `youtube-channel.json`          | `https://www.youtube.com/@YouTube`                                           |
| `rutube-video.json`             | `https://rutube.ru/video/c6cc4d620b1d4338901770a44b3e82f4/`                  |
| `youtube-members-only.stderr`   | `https://www.youtube.com/watch?v=LvCtTbfm2O8`                                |
| `youtube-age-restricted.stderr` | `https://www.youtube.com/watch?v=Tq92D6wQ1mg`                                |
| `youtube-unavailable.stderr`    | `https://www.youtube.com/watch?v=yZIXLfi8CZQ`                                |
| `youtube-bot-check.stderr`      | `https://www.youtube.com/watch?v=jNQXAC9IVRw`, without cookies               |
| `unsupported-url.stderr`        | `https://example.com/`                                                       |
| `youtube-truncated-id.stderr`   | `https://www.youtube.com/watch?v=-moW9jvvMr`, on 2026-10-22, without cookies |
