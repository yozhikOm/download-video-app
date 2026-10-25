# Backend architecture

The Rust backend in `src-tauri/src/` owns all state. The React UI only shows the events the backend sends and sends commands back. Terms such as Video, Probe, Download, Preset, Partial, Format, Tool, Package, Setup and Sign-in browser are defined in [`GLOSSARY.md`](../GLOSSARY.md).

```
lib.rs (Tauri commands, wiring)
  ├─ Prober ──────┬─ Toolbox ──┬─ Releases   ─ GithubReleases │ FakeReleases
  └─ Downloader ──┤            ├─ Processes  ─ SystemProcesses │ FakeProcesses
                  │            ├─ Clock      ─ SystemClock │ FixedClock
                  │            └─ EventSink
                  ├─ Settings ── EventSink
                  ├─ preset
                  └─ EventSink  ─ Subscribers (lib.rs) │ RecordingSink
```

## Modules

**`toolbox.rs`: `Toolbox`**
The app's own installed Tools in the app data folder (`tools/`), and the only code that knows which Tools exist and which Package provides each one. It runs Setup: it reads the manifest, checks which Tools are in place, downloads the missing Packages through `Releases`, verifies their SHA-256, and installs them. It also runs a Tool through `Processes`, pointing yt-dlp at the app's own ffmpeg and deno, and at the cookies of the Sign-in browser the caller passes (ADR-0002). When a Tool fails to start, `Toolbox` forgets it and shows the setup screen again. `SetupState` is what the setup screen shows, and every change to it is sent as an event.

**`probe.rs`: `Prober`**
Probes a URL by running `yt-dlp -J` through `Toolbox`, with the Sign-in browser from `Settings` when one is chosen and the URL is YouTube's, and keeps what it learned as a `Video` (Formats, Transcript languages, original language, chapters, whether it was a stream) for the Download. Only the latest Probe counts: a late answer to an older one is dropped. `ProbeState` is what the main screen shows: the Video's card with the Video presets it has and the one chosen at first, or a `ProbeError` for a playlist or channel, a stream that is live now, a Video that needs signing in, YouTube's bot check, a Sign-in browser whose cookies yt-dlp can't read (and why), a URL whose Video id was cut off, or an unsupported URL, along with the Sign-in browser the Probe ran with. When yt-dlp fails to start, `Prober` drops the Probe and runs Setup to restore it. Its tests (`probe/tests.rs`) feed real yt-dlp answers from `probe/fixtures/` through `FakeProcesses`.

**`preset.rs`: `VideoPreset`**
The Video presets and which of them a Video has. A Preset counts resolution as yt-dlp's `res` does, by the smaller side of the picture, so that a vertical 1080×1920 is 1080p. A Video has a Preset when one of its video Formats reaches the Preset's resolution but not the next one up (1080p takes 721–1080, so a widescreen 1920×800 is 1080p too; 480p takes 361–480); «Лучшее» it always has. The one chosen at first is 1080p, or the nearest lower one the Video has, or the nearest higher one when it has none lower, or «Лучшее» when it has no other.

**`download.rs`: `Downloader`**
Downloads the Video the latest Probe found with a Video preset as an MP4, through `Toolbox` with the Sign-in browser from `Settings` when the URL is YouTube's, as `Prober` does. The Preset becomes yt-dlp's Format order (`-S res:<resolution>,vcodec:h264,acodec:aac`, plain `res` for «Лучшее»): the highest resolution up to the Preset's, then H.264 and AAC over VP9, AV1 and Opus of the same resolution; above 1080p YouTube has only VP9 and AV1. yt-dlp takes separate video and audio (`-f bv+ba/b`), since preferring AAC would otherwise pick a Format with both, which YouTube sends slowly over HLS; a site with only such Formats falls back to the best of them. ffmpeg merges the video and audio into an MP4. yt-dlp works in the `partials/` folder in the app data folder under a name made of the Video's id and the Preset (`<id>.<preset>.<ext>`, such as `<id>.1080p.mp4`), never the title, so that a later Download of the same Video with the same Preset can resume. yt-dlp reports on stdout, read line by line as it arrives: the separate Formats it is about to receive with their sizes, then a JSON progress line per chunk (`--progress-template`) naming the Format being received, so that one percent and time left cover all of them (a Format that comes in fragments, such as HLS, counts by the share of fragments received, since its size is only an estimate, far too small at first), `[processing]` right before ffmpeg merges the video and audio, and the final file's path as a JSON string. When yt-dlp ends, `Downloader` moves the file to the save folder (the user's Downloads folder; when the system doesn't report one, `~/Downloads`, then `downloads/` in the app data folder) under the Video's title, with the characters Windows forbids replaced by lookalikes and " (1)", " (2)"… on a clash; it never overwrites a file. `Download` is what the main screen shows: the Video's card and its status (Downloading with percent, speed and time left, Processing, Done with the saved file, or Failed). One Download runs at a time; one asked for meanwhile is ignored. A Failed Download keeps its Partial, and retrying it downloads its own Video with its own Preset again, so that yt-dlp resumes from the Partial. When yt-dlp fails to start, `Downloader` drops the Download and runs Setup to restore it. Its tests (`download/tests.rs`) run a fake yt-dlp through `FakeProcesses` that writes its file where it is told.

**`settings.rs`: `Settings`**
The app's settings in `settings.json` in the app data folder; today only the Sign-in browser, which it gives only for a YouTube URL. A missing or unreadable file means the defaults. Every change is saved at once and sent as an event; a change that can't be saved changes nothing. Its tests (`settings/tests.rs`) relaunch on the same folder.

**`releases.rs`: `Releases`**
The only code that talks to the network: it asks for a Package's latest release (`latest_release`) and streams an asset to a writer (`fetch`). It does not know where files go or what a Tool is. `GithubReleases` (`releases/github.rs`) uses the official GitHub releases of yt-dlp, FFmpeg-Builds and deno, and reads each one's checksums file. `FakeReleases` (`releases/fake.rs`) publishes small fake releases, and a test can make it go offline, publish a wrong checksum or point to an unofficial asset.

**`processes.rs`: `Processes`**
The only code that starts Tools: `run` takes a path and arguments and waits for the program to exit; `run_streaming` also hands each stdout line to a callback as soon as it is printed. It does not know which Tool it is running. `SystemProcesses` starts the program without a console window. `FakeProcesses` (`processes/fake.rs`) records what was started and answers with scripted output, or with what a script returns for the arguments, and a test can make a Tool fail to start.

**`clock.rs`: `Clock`**
The current time, so tests can fix it (`FixedClock`).

**`events.rs`: `AppEvent`, `EventSink`**
Events from the backend to the UI. `Subscribers` in `lib.rs` forwards them to the UI channels. In tests, `RecordingSink` collects them.

**`lib.rs`**
Tauri commands (`app_info`, `subscribe`, `retry_setup`, `probe`, `set_sign_in_browser`, `download`, `retry_download`, `open_downloaded_file`, `show_downloaded_file`) and wiring: it creates the `Settings`, the `Toolbox`, the `Prober` and the `Downloader` with the real implementations and starts Setup in the background on launch. The `download` command takes the Video preset and refuses one the probed Video doesn't have. The Done Download's file is opened in its default program and shown in Explorer through the opener plugin; that is the user's file, not a Tool, so it doesn't go through `Processes`. The backend picks the file, so that the UI never names a path to open.

## Seams

`Releases`, `Processes`, `Clock` and `EventSink` are traits so that `Toolbox`, `Prober` and `Downloader` can be tested without the network, real programs or the real time (`toolbox/tests.rs`, `probe/tests.rs`, `download/tests.rs`). Code that needs the network or runs a Tool goes through one of these traits, never around it.
