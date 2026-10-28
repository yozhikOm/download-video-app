# yt-dlp, ffmpeg and deno are downloaded at runtime, not bundled

Sites change often and break yt-dlp extractors, so a copy pinned inside the installer goes stale until the app itself ships a new release. Instead, the app downloads the latest yt-dlp release into its app data folder on first run and updates it in place: in the background at startup (at most once a day), on demand ("Update yt-dlp"), and automatically when a Download fails because an extractor broke. An update never replaces yt-dlp while a Download is running; it waits until that Download ends.

ffmpeg, which yt-dlp needs to merge separate video and audio streams and to convert audio, is fetched the same way on first run, using the build that yt-dlp recommends (yt-dlp/FFmpeg-Builds). It rarely breaks, so it is not updated automatically.

deno is fetched the same way on first run, from the official denoland/deno releases (`deno-x86_64-pc-windows-msvc.zip`). Without a JavaScript runtime yt-dlp can no longer extract YouTube, and deno is the runtime yt-dlp recommends. yt-dlp is always given the app's own copy (`--no-js-runtimes --js-runtimes deno:<path>`), so a deno or node the user has installed is never used. The script that solves YouTube's challenges ships inside `yt-dlp.exe`, so the app does not let yt-dlp download it (`--remote-components`). Like ffmpeg, deno is not updated automatically, only restored when it is missing, damaged or fails to start: YouTube changes are fixed by updating yt-dlp and its bundled script, which runs on any recent deno, and deno is a large download (~43 MB).

## Considered Options

- **Sidecar** (Tauri `externalBin`): simplest and works offline from the first launch, but users stay stuck on a broken yt-dlp between app releases.
- **User installs yt-dlp on PATH**: no maintenance for us, but too much friction for non-technical users.
- **Updating deno along with yt-dlp**: keeps the newest runtime, but downloads ~43 MB more on each update for no gain while yt-dlp's script runs on the installed deno.

## Consequences

- The first launch needs internet access before anything can be downloaded.
- The app downloads and runs executables at runtime, so it must fetch only from the official GitHub releases over HTTPS and verify the published checksums. deno publishes its checksum as PowerShell `Get-FileHash` output rather than a `sha256sum` list, so it needs its own parser.
- If a future yt-dlp needs a newer deno than the installed one, the app has to start updating deno too.
