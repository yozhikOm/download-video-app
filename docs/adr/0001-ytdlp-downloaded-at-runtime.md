# yt-dlp and ffmpeg are downloaded at runtime, not bundled

Sites change often and break yt-dlp extractors, so a copy pinned inside the installer goes stale until the app itself ships a new release. Instead, the app downloads the latest yt-dlp release into its app data folder on first run and updates it in place: in the background at startup (at most once a day), on demand ("Update yt-dlp"), and automatically when a Download fails because an extractor broke. An update never replaces yt-dlp while a Download is running; it waits until that Download ends.

ffmpeg, which yt-dlp needs to merge separate video and audio streams and to convert audio, is fetched the same way on first run, using the build that yt-dlp recommends (yt-dlp/FFmpeg-Builds). It rarely breaks, so it is not updated automatically.

## Considered Options

- **Sidecar** (Tauri `externalBin`): simplest and works offline from the first launch, but users stay stuck on a broken yt-dlp between app releases.
- **User installs yt-dlp on PATH**: no maintenance for us, but too much friction for non-technical users.

## Consequences

- The first launch needs internet access before anything can be downloaded.
- The app downloads and runs executables at runtime, so it must fetch only from the official GitHub releases over HTTPS and verify the published checksums.
